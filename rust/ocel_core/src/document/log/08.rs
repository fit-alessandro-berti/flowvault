impl CompactOcelLog {

    fn object_search(&self, request: &ObjectSearchRequest) -> ObjectSearchResult {
        let query = request
            .query
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let object_type_symbol = request
            .object_type
            .as_deref()
            .and_then(|object_type| self.symbol_for_value(object_type));
        let limit = request.limit.unwrap_or(30).clamp(1, 200);
        let mut matches = Vec::new();

        for object in &self.objects {
            if object_type_symbol.is_some_and(|symbol| object.type_name != symbol) {
                continue;
            }
            let object_id = self.pool.resolve(object.id);
            if !query.is_empty() && !object_id.to_ascii_lowercase().contains(&query) {
                continue;
            }
            matches.push(ObjectSearchHit {
                object_id: object_id.to_owned(),
                object_type: self.pool.resolve(object.type_name).to_owned(),
                event_count: object.lifecycle.len(),
            });
            if matches.len() >= limit {
                break;
            }
        }

        ObjectSearchResult { objects: matches }
    }

    fn object_lifecycle_detail(&self, object_id: &str) -> OcelResult<ObjectLifecycleDetail> {
        let lookup = self
            .symbol_for_value(object_id)
            .and_then(|symbol| self.object_index.get(&symbol).copied());
        let object_index = lookup.ok_or_else(|| {
            OcelError::new(format!(
                "object id '{object_id}' was not found in the active log"
            ))
        })?;
        let object = &self.objects[object_index];
        let state_attribute = self.symbol_for_value("state");
        let mut events = Vec::new();
        let mut stock_points = Vec::new();
        let mut related = BTreeMap::<(Symbol, String), LifecycleRelatedObjectAccumulator>::new();

        for event_index in &object.lifecycle {
            let event = &self.events[*event_index];
            let state = state_attribute.and_then(|symbol| self.event_state(event, symbol));
            let mut related_objects = Vec::new();
            for relationship in &event.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                let Some(related_index) = self.object_index.get(&relationship.object_id).copied()
                else {
                    continue;
                };
                let related_object = &self.objects[related_index];
                let qualifier = self.pool.resolve(relationship.qualifier).to_owned();
                related_objects.push(LifecycleRelatedObject {
                    object_id: self.pool.resolve(relationship.object_id).to_owned(),
                    object_type: self.pool.resolve(related_object.type_name).to_owned(),
                    qualifier: qualifier.clone(),
                });
                let entry = related
                    .entry((relationship.object_id, qualifier.clone()))
                    .or_insert_with(|| LifecycleRelatedObjectAccumulator {
                        object_type: self.pool.resolve(related_object.type_name).to_owned(),
                        qualifier,
                        event_count: 0,
                    });
                entry.event_count += 1;
            }

            for attribute in &event.attributes {
                let name = self.pool.resolve(attribute.name);
                if !name.to_ascii_lowercase().contains("stock") {
                    continue;
                }
                if let Some(value) = numeric_attr_value(&attribute.value) {
                    stock_points.push(LifecycleStockPoint {
                        name: name.to_owned(),
                        time_ms: event.time_ms,
                        value,
                        event_id: self.pool.resolve(event.id).to_owned(),
                    });
                }
            }

            events.push(LifecycleEventDetail {
                event_id: self.pool.resolve(event.id).to_owned(),
                event_type: self.pool.resolve(event.type_name).to_owned(),
                time_ms: event.time_ms,
                state: state.map(str::to_owned),
                attributes: event
                    .attributes
                    .iter()
                    .map(|attribute| {
                        Ok(LifecycleAttribute {
                            name: self.pool.resolve(attribute.name).to_owned(),
                            value: self.attr_value_to_json(&attribute.value)?,
                        })
                    })
                    .collect::<OcelResult<Vec<_>>>()?,
                related_objects,
            });
        }

        let state_bands = lifecycle_state_bands(&events);
        let related_objects = related
            .into_iter()
            .map(
                |((object_symbol, _), accumulator)| LifecycleRelatedObjectSummary {
                    object_id: self.pool.resolve(object_symbol).to_owned(),
                    object_type: accumulator.object_type,
                    qualifier: accumulator.qualifier,
                    event_count: accumulator.event_count,
                },
            )
            .collect::<Vec<_>>();

        Ok(ObjectLifecycleDetail {
            object_id: self.pool.resolve(object.id).to_owned(),
            object_type: self.pool.resolve(object.type_name).to_owned(),
            event_count: events.len(),
            event_min_ms: events.first().map(|event| event.time_ms),
            event_max_ms: events.last().map(|event| event.time_ms),
            events,
            state_bands,
            stock_points,
            related_objects,
        })
    }

    fn stateful_lifecycle(&self, object: &Object, state_attribute: Symbol) -> Vec<(usize, String)> {
        object
            .lifecycle
            .iter()
            .filter_map(|event_index| {
                let event = &self.events[*event_index];
                self.event_state(event, state_attribute)
                    .map(|state| (*event_index, state.to_owned()))
            })
            .collect()
    }
}

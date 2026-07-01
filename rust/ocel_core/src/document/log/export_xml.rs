impl CompactOcelLog {

    fn export_xml(&self) -> OcelResult<String> {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<log>\n");

        output.push_str("  <event-types>\n");
        for event_type in &self.event_types {
            self.write_type_xml(&mut output, "event-type", event_type, 2)?;
        }
        output.push_str("  </event-types>\n");

        output.push_str("  <object-types>\n");
        for object_type in &self.object_types {
            self.write_type_xml(&mut output, "object-type", object_type, 2)?;
        }
        output.push_str("  </object-types>\n");

        output.push_str("  <events>\n");
        for event in &self.events {
            writeln!(
                output,
                "    <event id=\"{}\" type=\"{}\" time=\"{}\">",
                escape_xml_attr(self.pool.resolve(event.id)),
                escape_xml_attr(self.pool.resolve(event.type_name)),
                format_timestamp_ms(event.time_ms)?
            )
            .expect("writing to String cannot fail");
            self.write_attributes_xml(&mut output, &event.attributes, 6)?;
            self.write_relationships_xml(&mut output, &event.relationships, 6);
            output.push_str("    </event>\n");
        }
        output.push_str("  </events>\n");

        output.push_str("  <objects>\n");
        for object in &self.objects {
            writeln!(
                output,
                "    <object id=\"{}\" type=\"{}\">",
                escape_xml_attr(self.pool.resolve(object.id)),
                escape_xml_attr(self.pool.resolve(object.type_name))
            )
            .expect("writing to String cannot fail");
            self.write_timed_attributes_xml(&mut output, &object.attributes, 6)?;
            self.write_relationships_xml(&mut output, &object.relationships, 6);
            output.push_str("    </object>\n");
        }
        output.push_str("  </objects>\n</log>\n");
        Ok(output)
    }

    fn write_type_xml(
        &self,
        output: &mut String,
        tag: &str,
        type_def: &TypeDef,
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        writeln!(
            output,
            "{pad}<{tag} name=\"{}\">",
            escape_xml_attr(self.pool.resolve(type_def.name))
        )
        .expect("writing to String cannot fail");
        if type_def.attributes.is_empty() {
            writeln!(output, "{pad}  <attributes/>").expect("writing to String cannot fail");
        } else {
            writeln!(output, "{pad}  <attributes>").expect("writing to String cannot fail");
            for attribute in &type_def.attributes {
                writeln!(
                    output,
                    "{pad}    <attribute name=\"{}\" type=\"{}\"/>",
                    escape_xml_attr(self.pool.resolve(attribute.name)),
                    attribute.attr_type.as_str()
                )
                .expect("writing to String cannot fail");
            }
            writeln!(output, "{pad}  </attributes>").expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</{tag}>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[Attribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_timed_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[TimedAttribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\" time=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                format_timestamp_ms(attribute.time_ms)?,
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_relationships_xml(
        &self,
        output: &mut String,
        relationships: &[Relationship],
        indent: usize,
    ) {
        if relationships.is_empty() {
            return;
        }

        let pad = " ".repeat(indent);
        writeln!(output, "{pad}<objects>").expect("writing to String cannot fail");
        for relationship in relationships {
            writeln!(
                output,
                "{pad}  <relationship object-id=\"{}\" qualifier=\"{}\"/>",
                escape_xml_attr(self.pool.resolve(relationship.object_id)),
                escape_xml_attr(self.pool.resolve(relationship.qualifier))
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</objects>").expect("writing to String cannot fail");
    }

    fn attr_value_to_xml_text(&self, value: &AttrValue) -> OcelResult<String> {
        match value {
            AttrValue::String(symbol) => Ok(self.pool.resolve(*symbol).to_owned()),
            AttrValue::Time(ms) => format_timestamp_ms(*ms),
            AttrValue::Integer(number) => Ok(number.to_string()),
            AttrValue::Float(number) => {
                if !number.is_finite() {
                    return Err(OcelError::new("cannot export non-finite float value"));
                }
                Ok(number.to_string())
            }
            AttrValue::Boolean(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
        }
    }
}

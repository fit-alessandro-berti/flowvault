
fn feature_table_to_csv(table: &FeatureTableData) -> String {
    let mut output = String::new();
    output.push_str("object_id");
    for column in &table.columns {
        output.push(',');
        output.push_str(&csv_escape(column));
    }
    output.push('\n');

    for row in &table.rows {
        output.push_str(&csv_escape(&row.object_id));
        for value in &row.values {
            output.push(',');
            output.push_str(&format_numeric_feature(*value));
        }
        output.push('\n');
    }

    output
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CausalNodeRole {
    Observable,
    Latent,
    Outcome,
}

fn validate_causal_nodes<'a>(
    nodes: &'a [CausalModelNodeRequest],
    features: &[String],
) -> OcelResult<HashMap<&'a str, &'a CausalModelNodeRequest>> {
    let feature_set = features.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut node_by_id = HashMap::new();
    for node in nodes {
        if node.id.trim().is_empty() {
            return Err(OcelError::new("causal model nodes must have non-empty ids"));
        }
        if node_by_id.insert(node.id.as_str(), node).is_some() {
            return Err(OcelError::new(format!(
                "duplicate causal model node id '{}'",
                node.id
            )));
        }

        match causal_role(&node.role)? {
            CausalNodeRole::Observable | CausalNodeRole::Outcome => {
                let feature = node.feature.as_deref().ok_or_else(|| {
                    OcelError::new(format!("node '{}' must reference a feature", node.label))
                })?;
                if !feature_set.contains(feature) {
                    return Err(OcelError::new(format!(
                        "node '{}' references unknown feature '{feature}'",
                        node.label
                    )));
                }
                validate_causal_operation(&node.operation)?;
            }
            CausalNodeRole::Latent => {
                if node.feature.is_some() {
                    return Err(OcelError::new(format!(
                        "latent node '{}' cannot reference an observed feature",
                        node.label
                    )));
                }
            }
        }
    }
    Ok(node_by_id)
}

fn validate_causal_edges(
    edges: &[CausalModelEdgeRequest],
    node_by_id: &HashMap<&str, &CausalModelNodeRequest>,
) -> OcelResult<()> {
    let mut seen = HashSet::new();
    for edge in edges {
        if edge.source == edge.target {
            return Err(OcelError::new("causal model edges cannot be self-loops"));
        }
        if !seen.insert((edge.source.as_str(), edge.target.as_str())) {
            return Err(OcelError::new(format!(
                "duplicate causal edge '{} -> {}'",
                edge.source, edge.target
            )));
        }
        let source = node_by_id.get(edge.source.as_str()).ok_or_else(|| {
            OcelError::new(format!("unknown causal edge source '{}'", edge.source))
        })?;
        let target = node_by_id.get(edge.target.as_str()).ok_or_else(|| {
            OcelError::new(format!("unknown causal edge target '{}'", edge.target))
        })?;
        let source_role = causal_role(&source.role)?;
        let target_role = causal_role(&target.role)?;
        let valid = matches!(
            (source_role, target_role),
            (CausalNodeRole::Observable, CausalNodeRole::Latent)
                | (CausalNodeRole::Latent, CausalNodeRole::Latent)
                | (CausalNodeRole::Latent, CausalNodeRole::Outcome)
        );
        if !valid {
            return Err(OcelError::new(format!(
                "invalid causal edge '{} -> {}': use observable -> latent, latent -> latent, or latent -> outcome",
                source.label, target.label
            )));
        }
    }
    Ok(())
}

fn causal_topological_order(
    nodes: &[CausalModelNodeRequest],
    edges: &[CausalModelEdgeRequest],
) -> OcelResult<Vec<String>> {
    let mut indegree = nodes
        .iter()
        .map(|node| (node.id.as_str(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = nodes
        .iter()
        .map(|node| (node.id.as_str(), Vec::<&str>::new()))
        .collect::<HashMap<_, _>>();
    for edge in edges {
        *indegree
            .get_mut(edge.target.as_str())
            .expect("edge target was validated") += 1;
        outgoing
            .get_mut(edge.source.as_str())
            .expect("edge source was validated")
            .push(edge.target.as_str());
    }

    let mut ready = nodes
        .iter()
        .filter_map(|node| (indegree[node.id.as_str()] == 0).then_some(node.id.as_str()))
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node_id) = ready.pop_front() {
        order.push(node_id.to_owned());
        for target in outgoing.get(node_id).into_iter().flatten() {
            let degree = indegree
                .get_mut(target)
                .expect("topological target was validated");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(target);
            }
        }
    }

    if order.len() != nodes.len() {
        return Err(OcelError::new(
            "causal model must be a DAG; remove latent-to-latent cycles before fitting",
        ));
    }
    Ok(order)
}

fn causal_role(role: &str) -> OcelResult<CausalNodeRole> {
    match role.trim().to_ascii_lowercase().as_str() {
        "observable" => Ok(CausalNodeRole::Observable),
        "latent" => Ok(CausalNodeRole::Latent),
        "outcome" => Ok(CausalNodeRole::Outcome),
        other => Err(OcelError::new(format!(
            "unknown causal node role '{other}'; expected observable, latent, or outcome"
        ))),
    }
}

fn validate_causal_operation(operation: &str) -> OcelResult<()> {
    match normalized_causal_operation(operation).as_str() {
        "identity" | "log10" | "log_e" | "sqrt" => Ok(()),
        other => Err(OcelError::new(format!(
            "unknown causal transform '{other}'; expected identity, log10, log_e, or sqrt"
        ))),
    }
}

fn normalized_causal_operation(operation: &str) -> String {
    match operation.trim().to_ascii_lowercase().as_str() {
        "" | "none" | "raw" | "identity" => "identity".to_owned(),
        "log_10" | "log10" => "log10".to_owned(),
        "ln" | "loge" | "log_e" => "log_e".to_owned(),
        "sqrt" | "square_root" => "sqrt".to_owned(),
        other => other.to_owned(),
    }
}

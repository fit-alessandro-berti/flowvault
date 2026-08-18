
fn compact_relationships(
    relationships: &[SourceRelationship],
    pool: &mut StringPool,
) -> Vec<Relationship> {
    relationships
        .iter()
        .map(|relationship| Relationship {
            object_id: pool.intern(&relationship.object_id),
            qualifier: pool.intern(&relationship.qualifier),
        })
        .collect()
}

fn compact_value(
    source_value: &SourceValue,
    attr_type: Option<AttrType>,
    pool: &mut StringPool,
) -> OcelResult<AttrValue> {
    let attr_type = attr_type.unwrap_or_else(|| infer_attr_type(source_value));
    match attr_type {
        AttrType::String => Ok(AttrValue::String(
            pool.intern(&source_value_as_string(source_value)),
        )),
        AttrType::Time => parse_time_value(source_value).map(AttrValue::Time),
        AttrType::Integer => parse_integer_value(source_value).map(AttrValue::Integer),
        AttrType::Float => parse_float_value(source_value).map(AttrValue::Float),
        AttrType::Boolean => parse_boolean_value(source_value).map(AttrValue::Boolean),
    }
}

fn infer_attr_type(source_value: &SourceValue) -> AttrType {
    match source_value {
        SourceValue::String(_) => AttrType::String,
        SourceValue::Integer(_) => AttrType::Integer,
        SourceValue::Float(_) => AttrType::Float,
        SourceValue::Boolean(_) => AttrType::Boolean,
    }
}

fn source_value_as_string(source_value: &SourceValue) -> String {
    match source_value {
        SourceValue::String(value) => value.clone(),
        SourceValue::Integer(value) => value.to_string(),
        SourceValue::Float(value) => value.to_string(),
        SourceValue::Boolean(value) => value.to_string(),
    }
}

fn parse_time_value(source_value: &SourceValue) -> OcelResult<i64> {
    match source_value {
        SourceValue::String(value) => parse_timestamp_micros(value),
        _ => Err(OcelError::new("time attributes must be ISO 8601 strings")),
    }
}

fn parse_integer_value(source_value: &SourceValue) -> OcelResult<i64> {
    match source_value {
        SourceValue::Integer(value) => Ok(*value),
        SourceValue::Float(value) if value.fract() == 0.0 => Ok(*value as i64),
        SourceValue::String(value) => value
            .trim()
            .parse::<i64>()
            .map_err(|err| OcelError::new(format!("invalid integer attribute '{value}': {err}"))),
        SourceValue::Float(_) | SourceValue::Boolean(_) => {
            Err(OcelError::new("integer attributes must be integer values"))
        }
    }
}

fn parse_float_value(source_value: &SourceValue) -> OcelResult<f64> {
    let value = match source_value {
        SourceValue::Integer(value) => *value as f64,
        SourceValue::Float(value) => *value,
        SourceValue::String(value) => value
            .trim()
            .parse::<f64>()
            .map_err(|err| OcelError::new(format!("invalid float attribute '{value}': {err}")))?,
        SourceValue::Boolean(_) => return Err(OcelError::new("float attributes must be numeric")),
    };

    if value.is_finite() {
        Ok(value)
    } else {
        Err(OcelError::new("float attributes must be finite"))
    }
}

fn parse_boolean_value(source_value: &SourceValue) -> OcelResult<bool> {
    match source_value {
        SourceValue::Boolean(value) => Ok(*value),
        SourceValue::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(OcelError::new(format!(
                "invalid boolean attribute '{value}'"
            ))),
        },
        SourceValue::Integer(1) => Ok(true),
        SourceValue::Integer(0) => Ok(false),
        SourceValue::Integer(_) | SourceValue::Float(_) => Err(OcelError::new(
            "boolean attributes must be true/false or 1/0",
        )),
    }
}

fn decode_ocel_bytes(input: &[u8]) -> OcelResult<Vec<u8>> {
    if input.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzDecoder::new(input);
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .map_err(|err| OcelError::new(format!("could not decompress gzip OCEL file: {err}")))?;
        Ok(decoded)
    } else {
        Ok(input.to_vec())
    }
}

fn detect_format(input: &str, hint: Option<&str>) -> OcelResult<OcelFormat> {
    if let Some(format) = detect_format_hint(hint) {
        return Ok(format);
    }

    let first = input
        .trim_start()
        .chars()
        .next()
        .ok_or_else(|| OcelError::new("cannot import an empty OCEL file"))?;
    match first {
        '{' => Ok(OcelFormat::Json),
        '<' => Ok(OcelFormat::Xml),
        _ if looks_like_ocel_csv(input) => Ok(OcelFormat::Csv),
        _ => Err(OcelError::new(
            "could not detect OCEL format; expected JSON, XML, CSV, SQLite, or bundled input",
        )),
    }
}

fn detect_format_hint(hint: Option<&str>) -> Option<OcelFormat> {
    let hint = hint?.trim().to_ascii_lowercase();
    let hint = hint.strip_suffix(".gz").unwrap_or(&hint);
    if hint.ends_with(".json") || hint.ends_with(".jsonocel") || hint == "json" {
        Some(OcelFormat::Json)
    } else if hint.ends_with(".xml") || hint.ends_with(".xmlocel") || hint == "xml" {
        Some(OcelFormat::Xml)
    } else if hint.ends_with(".ocel.csv") || hint == "csv" {
        Some(OcelFormat::Csv)
    } else if hint.ends_with(".sqlite") || hint.ends_with(".sqlite3") || hint == "sqlite" {
        Some(OcelFormat::Sqlite)
    } else if hint.ends_with(".ocel.zip") || hint == "bundle" || hint == "parquet" {
        Some(OcelFormat::Bundle)
    } else {
        None
    }
}

fn looks_like_ocel_csv(input: &str) -> bool {
    let header = input.lines().next().unwrap_or_default().trim_end_matches('\r');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(header.as_bytes());
    reader.records().next().is_some_and(|record| {
        record.is_ok_and(|record| {
            let fields = record.iter().collect::<HashSet<_>>();
            fields.contains("id") && fields.contains("activity") && fields.contains("timestamp")
        })
    })
}

fn is_zip_bytes(input: &[u8]) -> bool {
    input.starts_with(b"PK\x03\x04")
        || input.starts_with(b"PK\x05\x06")
        || input.starts_with(b"PK\x07\x08")
}

fn parse_timestamp_ms(input: &str) -> OcelResult<i64> {
    Ok(parse_timestamp_micros(input)?.div_euclid(1_000))
}

fn parse_timestamp_micros(input: &str) -> OcelResult<i64> {
    let value = input.trim();
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Ok(timestamp.timestamp_micros());
    }

    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(timestamp) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(
                DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc).timestamp_micros()
            );
        }
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| OcelError::new(format!("invalid date '{value}'")))?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc).timestamp_micros());
    }

    Err(OcelError::new(format!(
        "invalid ISO 8601 timestamp '{input}'"
    )))
}

fn format_timestamp_micros(timestamp_micros: i64) -> OcelResult<String> {
    let timestamp = DateTime::<Utc>::from_timestamp_micros(timestamp_micros).ok_or_else(|| {
        OcelError::new(format!("timestamp {timestamp_micros} is outside chrono range"))
    })?;
    let precision = if timestamp_micros % 1_000_000 == 0 {
        SecondsFormat::Secs
    } else if timestamp_micros % 1_000 == 0 {
        SecondsFormat::Millis
    } else {
        SecondsFormat::Micros
    };
    Ok(timestamp.to_rfc3339_opts(precision, true))
}

fn escape_xml_attr(value: &str) -> String {
    escape_xml(value, true)
}

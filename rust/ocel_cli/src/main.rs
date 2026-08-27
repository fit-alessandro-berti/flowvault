use ocel_core::OcelDocumentCore;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const USAGE: &str = r#"Usage:
  ocel_cli <command> --input <path> [options]

Commands:
  summary
  apply-state-query
  state-transition-kpis
  ocdfg
  sa-ocdfg
  state-patterns
  state-detection
  state-detection-assignments
  apply-state-detection
  evaluation-bundle
  export

Options:
  --query <path>          Apply a state query before running the command.
  --request <json|path>   JSON request text or path to a JSON request file.
  --object-type <name>    Leading object type for the flat OCDFG command.
  --output <path>         Write command output to a file instead of stdout.
  --metrics <path>        Write timing and peak-RSS metadata separately.
  --bundle-dir <path>     Write evaluation-bundle components to this directory.
  --export-format <name>  json, xml, csv, sqlite, or bundle (default: json).
"#;

fn parse_arguments() -> Result<(String, BTreeMap<String, String>), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().ok_or(USAGE)?;
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        print!("{USAGE}");
        std::process::exit(0);
    }
    let mut options = BTreeMap::new();
    while let Some(flag) = arguments.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected positional argument '{flag}'\n{USAGE}").into());
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        options.insert(flag.trim_start_matches("--").to_owned(), value);
    }
    Ok((command, options))
}

fn required<'a>(
    options: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, Box<dyn Error>> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing required option '--{name}'").into())
}

fn text_or_file(value: &str) -> Result<String, Box<dyn Error>> {
    let path = Path::new(value);
    if path.is_file() {
        Ok(fs::read_to_string(path)?)
    } else {
        Ok(value.to_owned())
    }
}

fn request(options: &BTreeMap<String, String>) -> Result<String, Box<dyn Error>> {
    text_or_file(required(options, "request")?)
}

fn peak_rss_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kibibytes = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(kibibytes * 1024)
}

fn write_output(options: &BTreeMap<String, String>, output: &[u8]) -> io::Result<()> {
    if let Some(path) = options.get("output") {
        fs::write(path, output)
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write_all(output)?;
        if !output.ends_with(b"\n") {
            stdout.write_all(b"\n")?;
        }
        Ok(())
    }
}

fn write_bundle_part(directory: &Path, name: &str, value: String) -> Result<u64, Box<dyn Error>> {
    let path = directory.join(name);
    fs::write(path, value.as_bytes())?;
    Ok(value.len() as u64)
}

fn run() -> Result<(), Box<dyn Error>> {
    let (command, options) = parse_arguments()?;
    let input_path = PathBuf::from(required(&options, "input")?);
    let input = fs::read(&input_path)?;
    let input_bytes = input.len();
    let start = Instant::now();
    let mut document = OcelDocumentCore::from_bytes(
        &input,
        input_path.file_name().and_then(|name| name.to_str()),
    )?;
    drop(input);
    let applied_query = if let Some(query) = options.get("query") {
        Some(document.apply_state_query(&text_or_file(query)?)?)
    } else {
        None
    };

    let mut bundled_output_bytes = None;
    let output = match command.as_str() {
        "summary" => document.summary_json().into_bytes(),
        "apply-state-query" => applied_query
            .ok_or("apply-state-query requires --query")?
            .into_bytes(),
        "state-transition-kpis" => document
            .state_transition_kpis_json(&request(&options)?)?
            .into_bytes(),
        "ocdfg" => match options.get("object-type") {
            Some(object_type) => document.directly_follows_graph_json(object_type)?,
            None => document.object_centric_directly_follows_graph_json()?,
        }
        .into_bytes(),
        "sa-ocdfg" => document.state_aware_ocdfg_json()?.into_bytes(),
        "state-patterns" => match options.get("request") {
            Some(value) => document.state_patterns_with_request_json(&text_or_file(value)?)?,
            None => document.state_patterns_json()?,
        }
        .into_bytes(),
        "state-detection" => document
            .state_detection_json(&request(&options)?)?
            .into_bytes(),
        "state-detection-assignments" => document
            .state_detection_assignments_json(&request(&options)?)?
            .into_bytes(),
        "apply-state-detection" => document
            .apply_state_detection(&request(&options)?)?
            .into_bytes(),
        "evaluation-bundle" => {
            if applied_query.is_none() {
                return Err("evaluation-bundle requires --query".into());
            }
            let directory = PathBuf::from(required(&options, "bundle-dir")?);
            fs::create_dir_all(&directory)?;
            let bundle_request: Value = serde_json::from_str(&request(&options)?)?;
            let object_type = bundle_request["object_type"]
                .as_str()
                .ok_or("evaluation-bundle request requires object_type")?;
            let transition_request = serde_json::to_string(
                bundle_request
                    .get("transition_kpis")
                    .ok_or("evaluation-bundle request requires transition_kpis")?,
            )?;
            let detection_request = serde_json::to_string(
                bundle_request
                    .get("state_detection")
                    .ok_or("evaluation-bundle request requires state_detection")?,
            )?;
            let files = [
                ("summary", "summary.json"),
                ("enriched", "enriched.json"),
                ("transition_kpis", "transition_kpis.json"),
                ("ocdfg", "ocdfg.json"),
                ("sa_ocdfg", "sa_ocdfg.json"),
                (
                    "state_detection_assignments",
                    "state_detection_assignments.json",
                ),
            ];
            let mut total_bytes = 0;
            total_bytes += write_bundle_part(&directory, files[0].1, document.summary_json())?;
            total_bytes += write_bundle_part(&directory, files[1].1, document.export_json()?)?;
            total_bytes += write_bundle_part(
                &directory,
                files[2].1,
                document.state_transition_kpis_json(&transition_request)?,
            )?;
            total_bytes += write_bundle_part(
                &directory,
                files[3].1,
                document.directly_follows_graph_json(object_type)?,
            )?;
            total_bytes +=
                write_bundle_part(&directory, files[4].1, document.state_aware_ocdfg_json()?)?;
            total_bytes += write_bundle_part(
                &directory,
                files[5].1,
                document.state_detection_assignments_json(&detection_request)?,
            )?;
            bundled_output_bytes = Some(total_bytes);
            serde_json::to_vec(&json!({"files": files}))?
        }
        "export" => match options
            .get("export-format")
            .map(String::as_str)
            .unwrap_or("json")
        {
            "json" => document.export_json()?.into_bytes(),
            "xml" => document.export_xml()?.into_bytes(),
            "csv" => document.export_csv()?.into_bytes(),
            "sqlite" => document.export_sqlite()?,
            "bundle" => document.export_bundle()?,
            format => return Err(format!("unsupported export format '{format}'").into()),
        },
        _ => return Err(format!("unknown command '{command}'\n{USAGE}").into()),
    };
    write_output(&options, &output)?;
    if let Some(path) = options.get("metrics") {
        let metrics = json!({
            "command": command,
            "input_bytes": input_bytes,
            "output_bytes": bundled_output_bytes.unwrap_or(output.len() as u64),
            "wall_time_ms": start.elapsed().as_secs_f64() * 1000.0,
            "peak_rss_bytes": peak_rss_bytes(),
        });
        fs::write(path, format!("{}\n", serde_json::to_string(&metrics)?))?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ocel_cli: {error}");
        std::process::exit(2);
    }
}

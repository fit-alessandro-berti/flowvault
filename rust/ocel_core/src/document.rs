//! Document import, export, filtering, and analysis implementation.

use crate::{OcelError, OcelResult};
use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use flate2::read::GzDecoder;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write;
use std::io::Read;

include!("document/types.rs");
include!("document/log.rs");
include!("document/analysis.rs");
include!("document/api.rs");
include!("document/state_query.rs");
include!("document/source.rs");

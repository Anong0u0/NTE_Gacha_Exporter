use std::fs::File;
use std::io::{BufRead, BufReader};
#[cfg(windows)]
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
#[cfg(any(windows, test))]
use serde::{Deserialize, Serialize};

use crate::protocol::{ParseWarning, ParsedRow, ProtocolAssembler, parse_payload_blocks};

include!("raw/types.rs");
include!("raw/writer.rs");
include!("raw/reader.rs");
include!("raw/network.rs");
include!("raw/tests.rs");

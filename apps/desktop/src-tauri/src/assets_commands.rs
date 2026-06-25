use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use nte_core::AssetsPackManifest;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{ApiError, api_error};
use crate::state::{AppState, with_store};

include!("assets_commands/types.rs");
include!("assets_commands/commands.rs");
include!("assets_commands/status.rs");
include!("assets_commands/paths.rs");
include!("assets_commands/io.rs");

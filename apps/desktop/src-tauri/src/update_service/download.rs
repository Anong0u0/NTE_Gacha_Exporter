use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nte_core::UpdatePackage;
use serde::Deserialize;

use super::types::{MAX_UPDATE_JSON_BYTES, READ_BUFFER_BYTES, USER_AGENT};
use crate::error::{ApiError, api_error, api_error_message};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, ApiError> {
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let text = read_text_limited(
        response.into_reader(),
        MAX_UPDATE_JSON_BYTES,
        "update_response_too_large",
        "update JSON response",
    )?;
    serde_json::from_str(&text).map_err(api_error)
}

pub(super) fn download_update_archive(
    root: &Path,
    package: &UpdatePackage,
) -> Result<PathBuf, ApiError> {
    let downloads = root.join("update").join("downloads").join(&package.version);
    fs::create_dir_all(&downloads).map_err(api_error)?;
    let path = downloads.join(&package.asset_name);
    let tmp_path = downloads.join(format!("{}.tmp", package.asset_name));
    let response = http_agent()
        .get(&package.download_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    if let Some(content_length) = response.header("Content-Length") {
        let size = content_length.trim().parse::<u64>().map_err(api_error)?;
        if size != package.size {
            return Err(api_error_message(
                "update_archive_size_mismatch",
                format!(
                    "update archive content length mismatch: expected {}, got {size}",
                    package.size
                ),
            ));
        }
    }
    let mut reader = response.into_reader();
    let mut file = fs::File::create(&tmp_path).map_err(api_error)?;
    let result = copy_limited(&mut reader, &mut file, package.size);
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = file.flush() {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    Ok(path)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout_write(HTTP_WRITE_TIMEOUT)
        .build()
}

pub(super) fn read_text_limited(
    mut reader: impl Read,
    max_bytes: u64,
    error_code: &str,
    label: &str,
) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        let next_len = bytes.len() as u64 + read as u64;
        if next_len > max_bytes {
            return Err(api_error_message(
                error_code,
                format!("{label} exceeds {max_bytes} bytes"),
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(bytes).map_err(api_error)
}

pub(super) fn copy_limited(
    reader: &mut impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
) -> Result<(), ApiError> {
    let mut written = 0_u64;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > max_bytes {
            return Err(api_error_message(
                "update_archive_too_large",
                format!("update archive exceeds {max_bytes} bytes"),
            ));
        }
        writer.write_all(&buffer[..read]).map_err(api_error)?;
    }
    Ok(())
}

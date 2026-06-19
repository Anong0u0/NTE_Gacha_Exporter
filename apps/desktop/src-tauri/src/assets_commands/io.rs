fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout_write(HTTP_WRITE_TIMEOUT)
        .build()
}

fn read_text_limited(
    mut reader: impl Read,
    max_bytes: u64,
    _label: &str,
) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    copy_limited(&mut reader, &mut bytes, max_bytes)?;
    String::from_utf8(bytes).map_err(api_error)
}

fn copy_limited(
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
                "assets_pack_too_large",
                format!("assets pack exceeds {max_bytes} bytes"),
            ));
        }
        writer.write_all(&buffer[..read]).map_err(api_error)?;
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, GuiError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn short_hash(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

fn response(status: u16, content_type: &str, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header(tauri::http::header::CONTENT_TYPE, content_type)
        .header(
            tauri::http::header::CACHE_CONTROL,
            "public, max-age=31536000",
        )
        .body(body)
        .expect("asset protocol response must build")
}

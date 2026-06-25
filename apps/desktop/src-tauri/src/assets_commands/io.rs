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

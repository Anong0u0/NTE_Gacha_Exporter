fn capture_error(error: impl std::fmt::Display) -> CliError {
    let text = error.to_string();
    if text.contains("Windows") || text.contains("administrator") || text.contains("pktmon") {
        CliError::new(3, text)
    } else {
        CliError::new(2, text)
    }
}

struct DefaultPaths {
    json: PathBuf,
    csv: PathBuf,
    raw: PathBuf,
}

impl DefaultPaths {
    fn new() -> Self {
        let stamp = chrono::Local::now().format("%y%m%d-%H%M%S").to_string();
        let output = PathBuf::from("output");
        Self {
            json: output.join(format!("history-{stamp}.json")),
            csv: output.join(format!("history-{stamp}.csv")),
            raw: output.join(format!("raw-{stamp}.jsonl")),
        }
    }
}

type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
struct CliError {
    code: i32,
    message: String,
}

impl CliError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_error(error: impl std::fmt::Display) -> Self {
        Self::new(2, error.to_string())
    }
}

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

const DEFAULT_LINE_THRESHOLD: usize = 600;
const CODE_EXTENSIONS: &[&str] = &[
    "rs", "py", "sh", "ps1", "ts", "tsx", "js", "jsx", "vue", "css", "html",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("check-long-code") => {
            let threshold = parse_check_long_code_args(args)?;
            check_long_code(threshold)
        }
        Some("ci") => parse_ci_args(args),
        Some("-h" | "--help") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown xtask command: {command}\n\n{}",
            help_text()
        )),
    }
}

fn parse_check_long_code_args(args: impl Iterator<Item = String>) -> Result<usize, String> {
    let mut threshold = DEFAULT_LINE_THRESHOLD;
    let mut pending_threshold = false;
    for arg in args {
        if pending_threshold {
            threshold = parse_threshold(&arg)?;
            pending_threshold = false;
            continue;
        }
        match arg.as_str() {
            "--threshold" | "-t" => pending_threshold = true,
            "-h" | "--help" => {
                println!(
                    "Usage: cargo xtask check-long-code [--threshold <LINES>]\n\nDefault threshold: {DEFAULT_LINE_THRESHOLD}"
                );
                return Ok(threshold);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown check-long-code option: {value}"));
            }
            value => threshold = parse_threshold(value)?,
        }
    }
    if pending_threshold {
        return Err("--threshold requires a numeric value".to_string());
    }
    Ok(threshold)
}

fn parse_ci_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        None => run_ci(),
        Some("-h" | "--help") => {
            println!("Usage: cargo xtask ci");
            Ok(())
        }
        Some(value) => Err(format!("unknown ci option: {value}")),
    }
}

fn parse_threshold(value: &str) -> Result<usize, String> {
    let threshold = value
        .parse::<usize>()
        .map_err(|_| format!("invalid line threshold: {value}"))?;
    if threshold == 0 {
        Err("line threshold must be greater than 0".to_string())
    } else {
        Ok(threshold)
    }
}

fn check_long_code(threshold: usize) -> Result<(), String> {
    let root = repo_root()?;
    let paths = git_code_paths(&root)?;
    let entries = long_code_entries(&root, &paths, threshold)?;
    if entries.is_empty() {
        println!("OK: no code files over {threshold} lines");
        return Ok(());
    }

    for entry in entries {
        println!("{}\t{}", entry.lines, entry.path);
    }
    Err(format!("code files exceed {threshold} lines"))
}

fn run_ci() -> Result<(), String> {
    let root = repo_root()?;
    run_command(&root, "cargo", &["fmt", "--all", "--check"])?;
    run_command(
        &root,
        "cargo",
        &[
            "fmt",
            "--manifest-path",
            "tools/agent-smoke/Cargo.toml",
            "--check",
        ],
    )?;
    check_long_code(DEFAULT_LINE_THRESHOLD)?;
    run_command(&root, "cargo", &["test", "--workspace"])?;
    run_command(
        &root,
        "cargo",
        &["test", "--manifest-path", "tools/agent-smoke/Cargo.toml"],
    )?;
    run_command(
        &root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run_command(
        &root,
        "cargo",
        &[
            "clippy",
            "--manifest-path",
            "tools/agent-smoke/Cargo.toml",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;

    let desktop = root.join("apps").join("desktop");
    run_command(&desktop, "bun", &["install", "--frozen-lockfile"])?;
    run_command(&desktop, "bun", &["run", "typecheck"])?;
    run_command(&desktop, "bun", &["run", "build"])
}

fn repo_root() -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to resolve repo root: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

fn git_code_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to list git files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to list git files: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).to_string()))
        .filter(|path| is_code_path(path))
        .collect())
}

fn long_code_entries(
    root: &Path,
    paths: &[PathBuf],
    threshold: usize,
) -> Result<Vec<LongCodeEntry>, String> {
    let mut entries = Vec::new();
    for path in paths {
        let full_path = root.join(path);
        if !full_path.is_file() {
            continue;
        }
        let lines = count_file_lines(&full_path)?;
        if lines > threshold {
            entries.push(LongCodeEntry {
                lines,
                path: normalize_path(path),
            });
        }
    }
    entries.sort_by(|left, right| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(entries)
}

fn is_code_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| CODE_EXTENSIONS.contains(&extension))
}

fn count_file_lines(path: &Path) -> Result<usize, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(count_lines(&bytes))
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newline_count = bytes.iter().filter(|byte| **byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        newline_count
    } else {
        newline_count + 1
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn run_command(cwd: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    eprintln!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    "Usage: cargo xtask <COMMAND>\n\nCommands:\n  check-long-code  Fail when non-ignored code files exceed the line threshold\n  ci               Run the local CI gate\n"
}

struct LongCodeEntry {
    lines: usize,
    path: String,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn count_lines_handles_trailing_newline() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"a"), 1);
        assert_eq!(count_lines(b"a\n"), 1);
        assert_eq!(count_lines(b"a\nb"), 2);
        assert_eq!(count_lines(b"a\nb\n"), 2);
    }

    #[test]
    fn code_path_filter_uses_configured_extensions() {
        assert!(is_code_path(Path::new("src/main.rs")));
        assert!(is_code_path(Path::new("app/App.vue")));
        assert!(is_code_path(Path::new("script.ps1")));
        assert!(!is_code_path(Path::new("Cargo.toml")));
        assert!(!is_code_path(Path::new("fixtures/sample.raw.jsonl")));
    }

    #[test]
    fn long_code_entries_skip_missing_files_and_sort_by_line_count() {
        let temp = temp_dir("entries");
        write_lines(&temp.join("a.rs"), 3);
        write_lines(&temp.join("b.ts"), 5);
        write_lines(&temp.join("notes.txt"), 10);

        let entries = long_code_entries(
            &temp,
            &[
                PathBuf::from("a.rs"),
                PathBuf::from("b.ts"),
                PathBuf::from("missing.rs"),
            ],
            2,
        )
        .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "b.ts");
        assert_eq!(entries[0].lines, 5);
        assert_eq!(entries[1].path, "a.rs");
        assert_eq!(entries[1].lines, 3);

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn git_code_paths_include_untracked_and_respect_gitignore() {
        let temp = temp_dir("git");
        run_git(&temp, &["init"]);
        fs::write(temp.join(".gitignore"), "ignored/\n*.tmp\n").unwrap();
        write_lines(&temp.join("visible.rs"), 1);
        write_lines(&temp.join("visible.ts"), 1);
        fs::create_dir_all(temp.join("ignored")).unwrap();
        write_lines(&temp.join("ignored").join("hidden.rs"), 1);
        write_lines(&temp.join("scratch.tmp"), 1);

        let mut paths = git_code_paths(&temp)
            .unwrap()
            .into_iter()
            .map(|path| normalize_path(&path))
            .collect::<Vec<_>>();
        paths.sort();

        assert_eq!(paths, vec!["visible.rs", "visible.ts"]);

        let _ = fs::remove_dir_all(temp);
    }

    fn write_lines(path: &Path, lines: usize) {
        let text = (0..lines)
            .map(|index| format!("line {index}\n"))
            .collect::<String>();
        fs::write(path, text).unwrap();
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("nte-xtask-{name}-{}-{nonce}", std::process::id()));
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        fs::create_dir_all(&path).unwrap();
        path
    }
}

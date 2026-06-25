fn relaunch_capture_as_admin() -> CliResult<bool> {
    if !cfg!(windows) || is_admin() {
        return Ok(false);
    }
    println!("Requesting administrator permission for capture.");
    relaunch_current_process_as_admin()?;
    Ok(true)
}

#[cfg(windows)]
fn relaunch_current_process_as_admin() -> CliResult<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(CliError::from_error)?;
    let parameters = build_relaunch_parameters(std::env::args_os().skip(1));
    let directory = std::env::current_dir().map_err(CliError::from_error)?;
    let verb = wide_null(OsStr::new("runas"));
    let file = wide_null(exe.as_os_str());
    let params = wide_null(OsStr::new(&parameters));
    let dir = wide_null(directory.as_os_str());
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            dir.as_ptr(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result <= 32 {
        return Err(CliError::new(
            3,
            format!("administrator relaunch failed: ShellExecuteW={result}"),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn relaunch_current_process_as_admin() -> CliResult<()> {
    Err(CliError::new(3, "administrator relaunch requires Windows"))
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn build_relaunch_parameters(args: impl IntoIterator<Item = OsString>) -> String {
    args.into_iter()
        .map(|arg| windows_quote_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn windows_quote_arg(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if !text.is_empty() && !text.chars().any(|char| char.is_whitespace() || char == '"') {
        return text.into_owned();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for char in text.chars() {
        match char {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(char);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}


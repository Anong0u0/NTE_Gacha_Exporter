use std::thread;
use std::time::Duration;

use crate::error::AutomationResult;
use crate::matcher::ImageTemplateMatcher;
use crate::profile::load_profile;
use crate::screenshot::WindowCaptureClient;
use crate::window;

const RESTORE_HOME_ATTEMPTS: usize = 10;
const RESTORE_HOME_INTERVAL: Duration = Duration::from_millis(400);

pub fn restore_game_home(pid: u32) -> AutomationResult<()> {
    let base_profile = load_profile()?;
    let mut game_window = window::resolve_game_window(pid, &base_profile.window.class_name)?;
    let profile = base_profile.scaled(game_window.client_size())?;
    let capture = WindowCaptureClient::new(game_window.hwnd);
    let matcher = ImageTemplateMatcher::new(profile);

    for _ in 0..RESTORE_HOME_ATTEMPTS {
        game_window = window::refresh_window(&game_window)?;
        window::force_foreground(&game_window)?;
        let image = capture.capture_client(game_window.client_size())?;
        if matcher.verify("homeBoardFileIcon", &image).is_ok()
            && matcher.verify("homeForkEntryIcon", &image).is_ok()
        {
            return Ok(());
        }
        window::foreground_escape(&game_window)?;
        thread::sleep(RESTORE_HOME_INTERVAL);
    }

    game_window = window::refresh_window(&game_window)?;
    window::force_foreground(&game_window)?;
    let image = capture.capture_client(game_window.client_size())?;
    matcher.verify("homeBoardFileIcon", &image)?;
    matcher.verify("homeForkEntryIcon", &image)?;
    Ok(())
}

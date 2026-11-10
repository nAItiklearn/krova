//! Windows shell glue for wave 4: opening the releases page in the browser.

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::pointer::is_safe_https_url;
use crate::PlatformError;

/// Open `url` with the default browser. Krova itself makes no network request:
/// the browser does, and only because the user asked for it.
pub(crate) fn open_url(url: &str) -> Result<(), PlatformError> {
    if !is_safe_https_url(url) {
        return Err(PlatformError::Refused("not a plain https address"));
    }
    let file = HSTRING::from(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute returns a value greater than 32 on success.
    if result.0 as usize > 32 {
        Ok(())
    } else {
        Err(PlatformError::Os(format!(
            "the browser could not be opened (code {})",
            result.0 as usize
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_addresses_never_reach_the_shell() {
        assert!(matches!(
            open_url("file:///C:/Windows/System32/calc.exe"),
            Err(PlatformError::Refused(_))
        ));
        assert!(matches!(
            open_url("http://example.com"),
            Err(PlatformError::Refused(_))
        ));
    }

    /// Opens the default browser on the Krova releases page.
    #[test]
    #[ignore = "opens a real browser window"]
    fn opens_the_releases_page_in_the_browser() {
        open_url("https://github.com/nAItiklearn/krova/releases").unwrap();
    }
}

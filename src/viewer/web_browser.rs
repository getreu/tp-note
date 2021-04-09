//! Launch the user's favourite web browser.

use webbrowser::{open_browser, Browser};

#[inline]
/// Launches a web browser and displays the note's HTML rendition.
pub fn launch_web_browser(url: &str) -> Result<(), anyhow::Error> {
    open_browser(Browser::Default, url)?;
    Ok(())
}

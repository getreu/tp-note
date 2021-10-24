//! Launch the user's favourite web browser.

use crate::config::CFG;
use crate::error::FileError;
use crate::process_ext::ChildExt;
use crate::viewer::error::ViewerError;
use std::process::Command;
use std::process::Stdio;
use webbrowser::{open_browser, Browser};

#[inline]
/// Launches a web browser and displays the note's HTML rendition.
/// When not in _fall back mode: this function blocks until the user
/// closes the browser window.
pub fn launch_web_browser(url: &str) -> Result<(), ViewerError> {
    if let Err(e) = launch_listed_browser(url) {
        log::warn!("{}", e);
        log::warn!("As fall back workaround, trying to launch the system's default web browser.");
        // This might not block in all circumstances.
        open_browser(Browser::Default, url)?;
    };
    Ok(())
}

/// Launches one be one, all browsers from the list `CFG.app_args.browser` until
/// it finds an installed one. This blocks until the browser is closed by the
/// user.
pub fn launch_listed_browser(url: &str) -> Result<(), ViewerError> {
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // Choose the right parameter list.
    let browser_args = &CFG.app_args.browser;

    // Prepare launch of browser/viewer.

    for app in &*browser_args {
        executable_list.push(&app[0]);
        let mut args: Vec<&str> = Vec::new();
        for s in app[1..].iter() {
            args.push(s);
        }
        args.push(url);
        args_list.push(args);
    }

    // Move and make immutable.
    let args_list = args_list;
    let executable_list = executable_list;

    // Launch web browser.
    let mut executable_found = false;
    for i in 0..executable_list.len() {
        log::info!(
            "Trying to launch the web browser:\n'{}' {}",
            executable_list[i],
            args_list[i]
                .iter()
                .map(|p| {
                    let mut s = "'".to_string();
                    s.push_str(p);
                    s.push_str("' ");
                    s
                })
                .collect::<String>()
        );

        // Check if this is a `flatpak run <app>` command.
        #[cfg(target_family = "unix")]
        if executable_list[i].starts_with("flatpak")
            && args_list[i].len() >= 3
            && args_list[i][0] == "run"
        {
            // Check if the flatpak is installed on this system with `flatpak info <app>`.
            if let Ok(ecode) = Command::new(executable_list[i])
                .args(&["info", args_list[i][1]])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status()
            {
                if !ecode.success() {
                    // This is a flatpak command, but the application is not installed on this system.
                    // Silently ignore this flatpak command.
                    log::info!("Flatpak executable \"{}\" not found.", args_list[i][1]);
                    continue;
                };
            };
        };

        let (config_stdin, config_stdout) = (Stdio::null(), Stdio::null());

        let mut command = Command::new(&executable_list[i]);

        command
            .args(&args_list[i])
            .stdin(config_stdin)
            .stdout(config_stdout)
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                let mut child = ChildExt::from(child);
                let ecode = child.wait()?;

                if ecode.success() {
                    executable_found = true;
                    break;
                } else {
                    return Err(FileError::ApplicationReturn {
                        code: ecode,
                        var_name: "browser_agrs".to_string(),
                        args: (*browser_args[i]).to_vec(),
                    }
                    .into());
                }
            }
            Err(e) => {
                log::info!("Web browser \"{}\" not found: {}", executable_list[i], e);
            }
        }
    }

    if !executable_found {
        return Err(FileError::NoApplicationFound {
            app_list: executable_list
                .into_iter()
                .map(|s| s.to_owned())
                .collect::<Vec<String>>(),
            var_name: "[app_args] browser".to_string(),
        }
        .into());
    };

    Ok(())
}

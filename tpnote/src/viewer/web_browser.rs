//! Launch the user's favourite web browser.
use crate::config::CFG;
use crate::error::ConfigFileError;
use crate::process_ext::ChildExt;
use crate::settings::ENV_VAR_TPNOTE_BROWSER;
use crate::viewer::error::ViewerError;
use percent_encoding::percent_decode_str;
use std::env;
use std::process::Command;
use std::process::Stdio;
use webbrowser::{open_browser, Browser};

#[inline]
/// Launches a web browser and displays the note's HTML rendition.
/// When not in _fall back mode: this function blocks until the user
/// closes the browser window.
pub fn launch_web_browser(url: &str) -> Result<(), ViewerError> {
    if let Err(e) = launch_listed_browser(url) {
        log::warn!(
            "{}\n\
            As fall back workaround, trying to launch the\n\
            system's default web browser.",
            e
        );
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
    #[allow(unused_assignments)]
    let mut var_name = String::new();

    // Choose the right parameter list.
    let vv: Vec<Vec<String>>;

    #[cfg(target_os = "linux")]
    let app_args = &CFG.app_args.linux;
    #[cfg(target_os = "windows")]
    let app_args = &CFG.app_args.windows;
    #[cfg(target_os = "macos")]
    let app_args = &CFG.app_args.macos;
    let browser_args = if let Ok(s) = env::var(ENV_VAR_TPNOTE_BROWSER) {
        if s.is_empty() {
            var_name = "app_args.browser".to_string();
            &app_args.browser
        } else {
            var_name = ENV_VAR_TPNOTE_BROWSER.to_string();
            vv = vec![s
                .split_ascii_whitespace()
                .map(|s| percent_decode_str(s).decode_utf8_lossy().to_string())
                .collect::<Vec<String>>()];
            &vv
        }
    } else {
        var_name = "app_args.browser".to_string();
        &app_args.browser
    };

    // Prepare launch of browser/viewer.
    for app in browser_args {
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
        log::debug!(
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
                .args(["info", args_list[i][1]])
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

        let mut command = Command::new(executable_list[i]);

        command
            .args(&args_list[i])
            .stdin(config_stdin)
            .stdout(config_stdout)
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                let ecode = child.wait_subprocess()?;

                if ecode.success() {
                    executable_found = true;
                    break;
                } else {
                    return Err(ConfigFileError::ApplicationReturn {
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
        let mut app_list = String::new();
        for l in browser_args.iter() {
            app_list.push_str("\n\t");
            for a in l {
                app_list.push_str(a);
                app_list.push(' ');
            }
            app_list.truncate(app_list.len() - " ".len());
        }

        return Err(ConfigFileError::NoApplicationFound { app_list, var_name }.into());
    };

    Ok(())
}

//! Launch the user's favourite web browser.

use crate::config::CFG;
use crate::process_ext::ChildExt;
use anyhow::anyhow;
use std::process::Command;
use std::process::Stdio;
use webbrowser::{open_browser, Browser};

#[inline]
/// Launches a web browser and displays the note's HTML rendition.
pub fn launch_web_browser(url: &str) -> Result<(), anyhow::Error> {
    if launch_listed_broswer(url).is_err() {
        log::warn!(
            "The `browser_args` configuration file variable \
             is not configured properly. Trying to launch the system's \
             default web browser.",
        );
        open_browser(Browser::Default, url)?;
    };
    Ok(())
}

pub fn launch_listed_broswer(url: &str) -> Result<(), anyhow::Error> {
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // Choose the right parameter list.
    let browser_args = &CFG.browser_args;

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
            "Trying to launch the web browser: \"{}\" with {:?}",
            executable_list[i],
            args_list[i]
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
                    return Err(anyhow!(
                        "The web browser did not terminate gracefully:\n\
                     \t{}\n\
                     \n\
                     Edit the variable `browser_args` in Tp-Note's configuration file\n\
                     and correct the following:\n\
                     \t{:?}",
                        ecode.to_string(),
                        &*browser_args[i],
                    ));
                }
            }
            Err(e) => {
                log::info!("Web browser \"{}\" not found: {}", executable_list[i], e);
            }
        }
    }

    if !executable_found {
        return Err(anyhow!(
            "None of the following external web browser\n\
             applications can be found on your system:\n\
             \t{:?}\n\
             \n\
             Register some already installed web browser in the variable\n\
             `browser_args` in Tp-Note's configuration file  or \n\
             install one of the above listed applications.",
            &executable_list,
        ));
    };

    Ok(())
}

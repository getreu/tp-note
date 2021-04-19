//! Launch the user's favourite file editor.

use crate::config::CFG;
use crate::config::RUNS_ON_CONSOLE;
use crate::process_ext::ChildExt;
use anyhow::anyhow;
#[cfg(not(target_family = "windows"))]
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[inline]
/// Launch some external text editor. The editor can be chosen through
/// `tp-note`'s configuration file. This function searches the lists
/// `CFG.editor_console_args` or `CFG.editor_args` until it finds un installed
/// text editor. Once the editor is launched, the function blocks until the user
/// closes the editor window.
pub fn launch_editor(path: &Path) -> Result<(), anyhow::Error> {
    // Both lists have always the same number of items.
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // Choose the right parameter list.
    let editor_args = match *RUNS_ON_CONSOLE {
        true => &CFG.editor_console_args,
        false => &CFG.editor_args,
    };

    // Prepare launch of editor/viewer.

    for app in &*editor_args {
        executable_list.push(&app[0]);
        let mut args: Vec<&str> = Vec::new();
        for s in app[1..].iter() {
            args.push(s);
        }
        args.push(
            path.to_str()
                .ok_or_else(|| anyhow!("Failed to convert the argument: {:?}", path))?,
        );
        args_list.push(args);
    }

    // Move and make immutable.
    let args_list = args_list;
    let executable_list = executable_list;

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        log::info!(
            "Trying to launch the file editor: \"{}\" with {:?}",
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

        // Connect `stdin` of child process to `/dev/tty`.
        #[cfg(not(target_family = "windows"))]
        let (config_stdin, config_stdout) = if *RUNS_ON_CONSOLE {
            if let Ok(file) = File::open("/dev/tty") {
                (Stdio::from(file), Stdio::inherit())
            } else {
                (Stdio::null(), Stdio::null())
            }
        } else {
            (Stdio::null(), Stdio::null())
        };
        #[cfg(target_family = "windows")]
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

                if !ecode.success() {
                    return Err(anyhow!(
                        "The external file editor did not terminate gracefully: {}\n\
                        \n\
                        Edit the variable `{}` in Tp-Note's configuration file\n\
                        and correct the following:\n\
                        \t{:?}",
                        ecode.to_string(),
                        if *RUNS_ON_CONSOLE {
                            "editor_console_args"
                        } else {
                            "editor_args"
                        },
                        &*editor_args[i],
                    ));
                };

                executable_found = true;
                break;
            }
            Err(e) => {
                log::info!("File editor \"{}\" not found: {}", executable_list[i], e);
            }
        }
    }

    if !executable_found {
        return Err(anyhow!(
            "None of the following external file editor\n\
             applications can be found on your system:\n\
             \t{:?}\n\
             \n\
             Register some already installed file editor in the variable\n\
             `{}` in Tp-Note's configuration file  or \n\
             install one of the above listed applications.",
            &executable_list,
            // Choose the right parameter list.
            match *RUNS_ON_CONSOLE {
                true => "editor_console_args",
                false => "editor_args",
            }
        ));
    };

    Ok(())
}

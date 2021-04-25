//! Launch the user's favourite file editor.

use crate::config::CFG;
use crate::config::RUNS_ON_CONSOLE;
use crate::error::FileError;
use crate::process_ext::ChildExt;
#[cfg(not(target_family = "windows"))]
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[inline]
/// Launch some external text editor. The editor can be chosen through
/// _Tp-Note_'s configuration file. This function searches the lists
/// `CFG.editor_console_args` or `CFG.editor_args` until it finds un installed
/// text editor. Once the editor is launched, the function blocks until the user
/// closes the editor window.
pub fn launch_editor(path: &Path) -> Result<(), FileError> {
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
        args.push(path.to_str().ok_or(FileError::PathNotUtf8 {
            path: path.to_path_buf(),
        })?);
        args_list.push(args);
    }

    // Move and make immutable.
    let args_list = args_list;
    let executable_list = executable_list;

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        log::info!(
            "Trying to launch the file editor:\n\'{}\' {}",
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
                    return Err(FileError::ApplicationReturn {
                        code: ecode,
                        var_name: if *RUNS_ON_CONSOLE {
                            "editor_console_args".to_string()
                        } else {
                            "editor_args".to_string()
                        },
                        args: (*editor_args[i]).to_vec(),
                    });
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
        return Err(FileError::NoApplicationFound {
            app_list: executable_list
                .into_iter()
                .map(|s| s.to_owned())
                .collect::<Vec<String>>(),
            // Choose the right parameter list.
            var_name: match *RUNS_ON_CONSOLE {
                true => "editor_console_args".to_string(),
                false => "editor_args".to_string(),
            },
        });
    };

    Ok(())
}

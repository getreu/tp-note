//! Launch the user's favourite file editor.

use crate::config::CFG;
use crate::error::ConfigFileError;
use crate::process_ext::ChildExt;
use crate::settings::ENV_VAR_TPNOTE_EDITOR;
use crate::settings::RUNS_ON_CONSOLE;
use percent_encoding::percent_decode_str;
use std::env;
#[cfg(not(target_family = "windows"))]
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[inline]
/// Launch some external text editor. The editor can be chosen through
/// _Tp-Note_'s configuration file. This function searches the lists
/// `CFG.app_args.editor_console` or `CFG.app_args.editor` until it finds an installed
/// text editor. Once the editor is launched, the function blocks until the user
/// closes the editor window.
pub fn launch_editor(path: &Path) -> Result<(), ConfigFileError> {
    // Both lists have always the same number of items.
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // Choose the right parameter list.
    let env_var = env::var(ENV_VAR_TPNOTE_EDITOR).ok();
    let vv: Vec<Vec<String>>;
    let editor_args = match (&env_var, *RUNS_ON_CONSOLE) {
        // If the environment variable is defined, it has precedence.
        (Some(s), false) => {
            if s.is_empty() {
                &CFG.app_args.editor
            } else {
                vv = vec![s
                    .split_ascii_whitespace()
                    .map(|s| percent_decode_str(s).decode_utf8_lossy().to_string())
                    .collect::<Vec<String>>()];
                &vv
            }
        }
        (None, false) => &CFG.app_args.editor,
        (_, true) => &CFG.app_args.editor_console,
    };

    // Prepare launch of editor/viewer.

    for app in editor_args {
        executable_list.push(&app[0]);
        let mut args: Vec<&str> = Vec::new();
        for s in app[1..].iter() {
            args.push(s);
        }
        args.push(path.to_str().ok_or(ConfigFileError::PathNotUtf8 {
            path: path.to_path_buf(),
        })?);
        args_list.push(args);
    }

    // Move and make immutable.
    let args_list = args_list;
    let executable_list = executable_list;

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        log::debug!(
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
        if executable_list[i].ends_with("flatpak")
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

        let mut command = Command::new(executable_list[i]);

        command
            .args(&args_list[i])
            .stdin(config_stdin)
            .stdout(config_stdout)
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                let ecode = child.wait_subprocess()?;

                if !ecode.success() {
                    // Check if this is a console command running in a terminal
                    // emulator.
                    #[cfg(target_family = "unix")]
                    if executable_list[i].ends_with("alacritty")
                        && args_list[i].len() >= 3
                        && (args_list[i][args_list[i].len() - 2] == "-e"
                            || args_list[i][args_list[i].len() - 2] == "--command")
                    {
                        // This is a teminal emulator command, but the
                        // application is not installed on this system.
                        // Silently ignore this flatpak command.
                        log::info!(
                            "Console file editor executable \"{}\" not found.",
                            args_list[i][args_list[i].len() - 2]
                        );
                        continue;
                    };

                    return Err(ConfigFileError::ApplicationReturn {
                        code: ecode,
                        var_name: if *RUNS_ON_CONSOLE {
                            "app_args.editor_console".to_string()
                        } else {
                            "app_args.editor".to_string()
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
    } // All executables in the list are launched, without success.

    if !executable_found {
        let mut app_list = String::new();
        for l in editor_args.iter() {
            app_list.push_str("\n\t");
            for a in l {
                app_list.push_str(a);
                app_list.push(' ');
            }
            app_list.truncate(app_list.len() - " ".len());
        }

        return Err(ConfigFileError::NoApplicationFound {
            app_list,
            // Choose the right parameter list.
            var_name: match (&env_var, *RUNS_ON_CONSOLE) {
                (Some(_), false) => ENV_VAR_TPNOTE_EDITOR.to_string(),
                (_, true) => "app_args.editor_console".to_string(),
                (None, false) => "app_args.editor".to_string(),
            },
        });
    };

    Ok(())
}

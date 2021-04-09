//! Launch the user's favourite file editor.

extern crate semver;
use crate::config::ARGS;
use crate::config::CFG;
use crate::config::RUNS_ON_CONSOLE;
use anyhow::{anyhow, Context};
#[cfg(not(target_family = "windows"))]
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[inline]
/// Launch some external editor. The editor can be chosen through
/// `tp-note`'s configuration file.
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

    // Launch editor/viewer.
    if ARGS.debug {
        eprintln!("*** Debug: Opening file {:?}", path);
    };

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        if ARGS.debug {
            eprint!(
                "*** Debug: Trying to launch the executable: {}",
                executable_list[i]
            );
            for j in &args_list[i] {
                eprint!(" \"{}\"", j);
            }
            eprintln!()
        };

        // Check if this is a `flatpak run <app>` command.
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
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: Flatpak executable \"{}\" not found.",
                            args_list[i][1]
                        );
                    }
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

        let child = Command::new(&executable_list[i])
            .args(&args_list[i])
            .stdin(config_stdin)
            .stdout(config_stdout)
            .stderr(Stdio::null())
            .spawn();

        if let Ok(mut child) = child {
            let ecode = child.wait().context("Failed to wait on editor to close.")?;

            if !ecode.success() {
                return Err(anyhow!(
                    "The external file editor did not terminate gracefully:\n\
                     \t{}\n\
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
        } else if ARGS.debug {
            eprintln!(
                "*** Debug: Executable \"{}\" not found.",
                executable_list[i]
            );
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
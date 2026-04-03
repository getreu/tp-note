#!/usr/bin/env nu

# Orchestration script for the Tp-Note build pipeline (Nushell version)
def main [] {
    # 1. Change to the script's directory to ensure relative paths work
    let script_dir = ($env.CURRENT_FILE | path dirname)
    cd $script_dir

    print $"(ansi cyan_bold)=== Starting Tp-Note Nu-Pipeline ===(ansi reset)"

    # List of .nu scripts to execute in order
    let steps = [
        { file: "10-clear-targets.nu",            msg: "Clearing targets" }
        { file: "11-test.nu",                     msg: "Running tests" }
        { file: "13-make-docs.nu",                msg: "Building documentation" }
        { file: "14-make-targets.nu",             msg: "Building targets" }
        { file: "18-make-win-msi-package.nu",     msg: "Creating Windows MSI package" }
        { file: "25-generate-winget-package.nu",  msg: "Generating winget manifests" }
        { file: "19-symlink-installer.nu",        msg: "Creating installer symlinks" }
        { file: "30-clear-targets-keep-binaries.nu", msg: "Performing final cleanup" }
    ]

    # Execute each step
    for step in $steps {
        print $"(ansi yellow)Step: ($step.msg)...(ansi reset)"

        # Verify the script exists before calling it
        if not ($step.file | path exists) {
            error make {msg: $"Script file not found: ($step.file)"}
        }

        # Call the script using the 'nu' interpreter
        # This ensures they run in the same environment context
        nu $step.file

        # Check if the last command succeeded
        if $env.LAST_EXIT_CODE != 0 {
            print $"(ansi red_bold)FAILURE: ($step.msg) exited with code ($env.LAST_EXIT_CODE)(ansi reset)"
            exit 1
        }

        print $"(ansi green)Done.(ansi reset)\n"
    }

    print $"(ansi green_bold)🎉 All build steps completed successfully!(ansi reset)"
}

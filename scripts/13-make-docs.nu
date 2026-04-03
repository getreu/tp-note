#!/usr/bin/env nu

# 1. Get the project root (up two levels from this script's directory)
let project_dir = ($env.FILE_PWD | path dirname)

# 2. Navigate to the documentation directory
cd ($project_dir | path join "docs")

print $"(ansi cyan)Attempting to build documentation using nix flake...(ansi reset)"

# 3. Execute the build command
# Using ^ ensures we call the external script/binary explicitly
^./make--all

# 4. Handle exit status
let exit_code = $env.LAST_EXIT_CODE

if $exit_code == 0 {
    print $"(ansi green_bold)Documentation built successfully with nix develop(ansi reset)"
    print $"Generated documentation is available in: (ansi u)docs/build/(ansi rst_u)"
} else {
    print $"(ansi red_bold)Documentation build failed with exit code: ($exit_code)(ansi reset)"
    exit 1
}

print "Documentation build completed!"

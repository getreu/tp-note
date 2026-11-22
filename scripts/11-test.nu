#!/usr/bin/env nu

let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

# Unit testing - use nix develop here for proper environment
print "Running unit tests..."
cargo test --all

let exit_code = $env.LAST_EXIT_CODE
if $exit_code == 0 {
    print "Cargo test succeeded."
} else {
    print "Cargo test failed."
    exit 1
}

# Compile matrix - every feature on its own, plus none and all of them.
# This replaced tests 900 to 906 of the shell suite that used to live in
# tests/functional, which checked seven of these combinations by hand;
# `--each-feature` covers those seven and additionally `mermaid`, `latex` and
# `default`.
print "Checking the compile matrix..."
cargo hack check --each-feature --workspace

let exit_code = $env.LAST_EXIT_CODE
if $exit_code == 0 {
    print "Compile matrix succeeded."
} else {
    print "Compile matrix failed."
    exit 1
}

print "All tests completed successfully!"

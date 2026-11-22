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
# This replaces the former tests 900 to 906 of tests/functional/run-tests,
# which checked seven of these combinations by hand; `--each-feature` covers
# those seven and additionally `mermaid`, `latex` and `default`.
print "Checking the compile matrix..."
cargo hack check --each-feature --workspace

let exit_code = $env.LAST_EXIT_CODE
if $exit_code == 0 {
    print "Compile matrix succeeded."
} else {
    print "Compile matrix failed."
    exit 1
}

# Functional tests - run functional tests in the Nix development environment
print "Running functional tests..."
print "This may take several minutes..."

# Run functional tests in the Nix development environment
# This is the only place where we actually use nix develop
./tests/functional/run-tests

let exit_code = $env.LAST_EXIT_CODE
if $exit_code == 0 {
  print "Functional tests completed successfully!"
} else {
  print "Functional tests failed!"
  exit 1
}

print "All tests completed successfully!"

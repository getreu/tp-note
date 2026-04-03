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

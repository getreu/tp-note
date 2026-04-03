#!/usr/bin/env nu

# Change to this script directory
let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

# Clean up target directories while preserving important files
print "Cleaning target directories..."

# Remove top-level build directories that are not needed
if ("target" | path exists) {
    rm -rf target
}

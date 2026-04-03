#!/usr/bin/env nu

let project_dir = ($env.CURRENT_FILE | path dirname | path dirname)
cd $project_dir

# Remove build directories safely
if ("target" | path exists ) {
    rm -rf target
}
if ("doc/build" | path exists ) {
    rm -rf doc/build
}
if ("build" | path exists ) {
    rm -rf build/*
}

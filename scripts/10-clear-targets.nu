#!/usr/bin/env nu

let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

# Remove build directories safely
if ("target" | path exists ) {
    rm -rf target
}
if ("doc/build" | path exists ) {
    rm -rf doc/build
}
if ("build" | path exists ) {
    # Preserve existing MSI packages across cleans.
    let tmp = (mktemp -d)
    let msi_files = (glob "build/package/x86_64-pc-windows-gnu/*.msi")
    if ($msi_files | is-not-empty) {
        for $f in $msi_files { mv $f $tmp }
    }
    rm -rf build/*
    if ($msi_files | is-not-empty) {
        mkdir build/package/x86_64-pc-windows-gnu
        for $f in (ls $tmp | where type == file | get name) {
            mv $f build/package/x86_64-pc-windows-gnu/
        }
    }
    rm -rf $tmp
}

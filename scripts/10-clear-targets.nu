#!/usr/bin/env nu

let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

# Remove build directories safely
if ("target" | path exists ) {
    rm -rf target
}
if ("docs/build" | path exists ) {
    rm -rf docs/build
}
if ("build" | path exists ) {
    # Preserve existing MSI packages across cleans (the Wine-based MSI build is
    # slow). build/ is flat, so the installers live directly in build/*.msi.
    let tmp = (mktemp -d)
    let msi_files = (glob "build/*.msi")
    if ($msi_files | is-not-empty) {
        for $f in $msi_files { mv $f $tmp }
    }
    rm -rf build/*
    if ($msi_files | is-not-empty) {
        mkdir build
        for $f in (ls $tmp | where type == file | get name) {
            mv $f build/
        }
    }
    rm -rf $tmp
}

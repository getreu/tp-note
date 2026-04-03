#!/usr/bin/env nu

# 1. Change to the script's directory, then up one level
let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir
### Prepare build directory
let build_dir = ($project_dir | path join "target" "wix")
# Extract version from Cargo.toml using structured data (rather than sed)
let bin_version = (open Cargo.toml | get workspace.package.version)
let package_name = "tpnote"
let bin_name = "tpnote"
let exe_name = $"($bin_name).exe"
print $"Building Windows MSI package for Tp-Note version ($bin_version)"

# 2. Reset the build directory
if ($build_dir | path exists) { rm -rf $build_dir }
mkdir $build_dir

# 3. Copy necessary files
cp $"($project_dir)/tpnote/tpnote.ico" $build_dir
cp $"($project_dir)/build/bin/x86_64-pc-windows-gnu/($exe_name)" $build_dir
cp $"($project_dir)/wix/tpnote.wxs" ($build_dir | path join "tpnote.wxs")
### Build Windows installer package
cd $build_dir

# 4. Update version in .wxs file using string replacement
let wxs_content = (open tpnote.wxs | str replace 'Version="1.0.0"' $"Version=\"($bin_version)\"")
$wxs_content | save -f "tpnote-tmp.wxs"

# 5. Execute the build via Nix
# We use ^ to ensure we are calling external commands
let output_msi = $"($package_name)-($bin_version)-x86_64.msi"
^nix develop $"($project_dir)/wix" --command wine64 wix.exe build tpnote-tmp.wxs -b . -o $output_msi

# 6. Copy artifact to target structure
let target_pkg_dir = ($project_dir | path join "build" "package" "x86_64-pc-windows-gnu")
mkdir $target_pkg_dir
cp $output_msi $target_pkg_dir

# 7. Clean up
cd $project_dir
rm -rf $build_dir
print $"Windows MSI package created and copied to build/package/x86_64-pc-windows-gnu/"
print $"Package name: ($output_msi)"

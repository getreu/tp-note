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
# Obtain the Windows binary straight from the Nix build. build/ is flat now
# (no build/bin/<target>/ tree), so we resolve the store path on demand.
let win_store = (^nix build $"($project_dir)#tpnote-x86_64-pc-windows-gnu" --no-link --print-out-paths | lines | first | str trim)
cp $"($win_store)/bin/($exe_name)" $build_dir
cp $"($project_dir)/wix/tpnote.wxs" ($build_dir | path join "tpnote.wxs")
### Build Windows installer package
cd $build_dir

# 4. Update version in .wxs file using string replacement
let wxs_content = (open tpnote.wxs | str replace 'Version="1.0.0"' $"Version=\"($bin_version)\"")
$wxs_content | save -f "tpnote-tmp.wxs"

# 5. Execute the build via Nix
# We use ^ to ensure we are calling external commands
let output_msi = $"($package_name)-($bin_version)-x64.msi"
^nix develop $"($project_dir)/wix" --command wine64 wix.exe build tpnote-tmp.wxs -b . -o $output_msi

# 6. Move artifact to target structure.
# Use `mv` (atomic rename on the same filesystem), not `cp`: nushell's `cp`
# opens the destination O_TRUNC and only then tries to preserve ownership, so a
# failure (e.g. chown-ing a pre-existing file owned by another user) leaves the
# previous good MSI truncated to 0 bytes. `mv` replaces the target in one step
# and never clobbers a good artifact on failure. The source in $build_dir is
# deleted by the cleanup step below anyway.
let target_pkg_dir = ($project_dir | path join "build")
mkdir $target_pkg_dir
mv -f $output_msi ($target_pkg_dir | path join $output_msi)

# 7. Clean up
cd $project_dir
rm -rf $build_dir
print $"Windows MSI package created and copied to build/"
print $"Package name: ($output_msi)"

#!/usr/bin/env nu

# Change to the script's directory, then up one level
let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir
print "Updating 'latest' symlinks..."

# 1. Windows MSI Symlink
let win_pkg_dir = ($project_dir | path join "build" "package" "x86_64-pc-windows-gnu")
if ($win_pkg_dir | path exists) {
    # Use a more robust method to find MSI files
    let msi_files = (ls $win_pkg_dir | where type == file | where name =~ "\\.msi$")

    if ($msi_files | is-not-empty) {
        # Sort by modification time (newest first)
        let sorted_msi = ($msi_files | sort-by modified -r)
        let latest_msi = ($sorted_msi | first).name
        let link_name = ($win_pkg_dir | path join "tpnote-latest-x86_64.msi")

        # Create symlink
        ln -sf ($latest_msi | path basename) $link_name
        print $"[LINK] tpnote-latest-x86_64.msi -> ($latest_msi | path basename)"
    }
}

# 2. Debian Package Symlink
let deb_dest_root = ($project_dir | path join "build" "package" "x86_64-unknown-linux-gnu")
if ($deb_dest_root | path exists) {
    # Use a more robust method to find DEB files
    let deb_files = (ls $deb_dest_root | where type == file | where name =~ "\\.deb$")

    if ($deb_files | is-not-empty) {
        # Sort by modification time (newest first)
        let sorted_deb = ($deb_files | sort-by modified -r)
        let latest_deb = ($sorted_deb | first).name
        let link_name = ($deb_dest_root | path join "tpnote_latest_amd64.deb")

        # Create symlink
        ln -sf ($latest_deb | path basename) $link_name
        print $"[LINK] tpnote_latest_amd64.deb -> ($latest_deb | path basename)"
    }
}

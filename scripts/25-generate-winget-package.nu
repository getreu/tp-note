#!/usr/bin/env nu

# Nushell script to generate WinGet manifests for Tp-Note without external dependencies
def main [] {
    print "=== Tp-Note Winget Packaging (Native Nu Version) ==="

    # Change to the script's directory, then up one level
    let project_dir = ($env.FILE_PWD | path dirname)
    cd $project_dir

    # 1. Extract version from Cargo.toml
    let cargo_toml = open Cargo.toml
    let version = $cargo_toml.workspace.package.version
    if ($version | is-empty) {
        error make {msg: "Could not determine version from Cargo.toml"}
    }
    print $"Using version: ($version)"

    # 2. Define paths and metadata
    let package_id = "getreu.tpnote"
    let base_dir = ["build" "package" "winget-manifests" "getreu" "tpnote"] | path join
    let final_dir = $base_dir | path join $version
    let msi_name = $"tpnote-($version)-x86_64.msi"
    let msi_path = ["build" "package" "x86_64-pc-windows-gnu" $msi_name] | path join
    let download_url = $"https://blog.getreu.net/projects/tp-note/_downloads/package/x86_64-pc-windows-gnu/($msi_name)"

    # 3. MSI Check & Hash calculation
    if not ($msi_path | path exists) {
        print $"(ansi red)Error: MSI file not found at ($msi_path)(ansi reset)"
        print "Please run the Windows build scripts first."
        exit 1
    }

    print "Calculating SHA256 hash..."
    # 'open --raw' reads the file as a binary blob for the hash command
    let msi_hash = (open --raw $msi_path | hash sha256)
    print $"MSI Hash: ($msi_hash)"

    # 4. Ensure target directory exists
    mkdir $final_dir

    # 5. Generate Manifests

    # --- version.yaml ---
    print "Generating version manifest..."
    {
        PackageIdentifier: $package_id
        PackageVersion: $version
        DefaultLocale: "en-US"
        ManifestType: "version"
        ManifestVersion: "1.5.0"
    } | to yaml | save -f ($final_dir | path join $"($package_id).yaml")

    # --- installer.yaml ---
    print "Generating installer manifest..."
    {
        PackageIdentifier: $package_id
        PackageVersion: $version
        MinimumOSVersion: "10.0.0.0"
        InstallerType: "msi"
        Installers: [
            {
                Architecture: "x64"
                InstallerUrl: $download_url
                InstallerSha256: $msi_hash
            }
        ]
        ManifestType: "installer"
        ManifestVersion: "1.5.0"
    } | to yaml | save -f ($final_dir | path join $"($package_id).installer.yaml")

    # --- defaultLocale.yaml ---
    print "Generating locale manifest..."
    {
        PackageIdentifier: $package_id
        PackageVersion: $version
        PackageLocale: "en-US"
        Publisher: "Jens Getreu"
        PackageName: "Tp-Note"
        ShortDescription: "Fast note-taking with templates and filename analysis"
        Description: "Tp-Note is a note-taking tool and a template system that facilitates personal knowledge management."
        License: "MIT"
        PackageUrl: "https://blog.getreu.net/projects/tp-note/"
        ManifestType: "defaultLocale"
        ManifestVersion: "1.5.0"
    } | to yaml | save -f ($final_dir | path join $"($package_id).locale.en-US.yaml")

    print $"(ansi green)Success! Manifests created in: ($final_dir)(ansi reset)"
}

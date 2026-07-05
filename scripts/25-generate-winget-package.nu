#!/usr/bin/env nu

# Tp-Note Windows Package Manager Distribution Guide
#
# This document explains how to distribute Tp-Note through the Windows Package
# Manager (winget) by using the output of this script.
#
# ## Overview
#
# Tp-Note can be distributed through Microsoft's Windows Package Manager
# (winget) to make it easily discoverable and installable for Windows users.
# This guide covers the setup and distribution process.
#
# ## Prerequisites
#
# - Windows 10 or Windows 11 with Windows Package Manager installed
# - PowerShell 5.1 or later
# - Git for version control (if contributing to the official repository)
#
# ## Installing Windows Package Manager
#
# If you don't have winget installed:
#
# 1. Open Microsoft Store
# 2. Search for "Windows Package Manager"
# 3. Install "App Installer" (which includes winget)
#
# Alternatively, download it from:
#
# - Microsoft Store: https://apps.microsoft.com/store/detail/app-installer/9NBLGGH4NNS1
# - GitHub Releases: https://github.com/microsoft/winget-cli/releases
#
# ## Installing Tp-Note via Winget
#
# Once winget is installed, you can install Tp-Note:
#
#     winget install getreu.tpnote
#
# ## How Tp-Note is Distributed via Winget
#
# ### Manifest Structure
#
# Tp-Note uses the following manifest structure in the winget repository:
#
#     manifests/
#     └── g/
#         └── getreu/
#             └── tpnote/
#                 └── <version>/
#                     ├── getreu.tpnote.yaml
#                     ├── getreu.tpnote.installer.yaml
#                     └── getreu.tpnote.locale.en-US.yaml
#
# ### Manifest Components
#
# 1. getreu.tpnote.yaml: Version manifest (package identifier, version,
#    default locale)
# 2. getreu.tpnote.installer.yaml: Installer details including URL and SHA256
#    hash
# 3. getreu.tpnote.locale.en-US.yaml: Default-locale metadata (publisher,
#    description, license)
#
# ## Local Development and Testing
#
# ### Prerequisites
#
# - Nushell (https://www.nushell.sh/) (the script is written in Nu; no other
#   external dependencies are required — SHA256 hashing is done natively)
# - A freshly built Windows MSI at
#   `build/package/x86_64-pc-windows-gnu/tpnote-<version>-x86_64.msi`
#
# ### Generating Manifests
#
# Run this script to generate winget manifests:
#
#     ./scripts/25-generate-winget-package.nu
#
# This will:
#
# 1. Parse the version from `Cargo.toml` (workspace package version)
# 2. Create the directory structure under
#    `build/package/winget-manifests/getreu/tpnote/<version>/`
# 3. Generate the three manifest files with appropriate metadata
# 4. Calculate the SHA256 hash of the MSI installer
#
# The script aborts with an error if the expected MSI file is not present, so
# build the Windows package first.
#
# ### Testing Locally
#
# On a Windows machine, validate and (optionally) install from the generated
# manifests:
#
#     winget validate --manifest build\package\winget-manifests\getreu\tpnote\<version>
#     winget install --manifest build\package\winget-manifests\getreu\tpnote\<version>
#
# ## Contributing to Official Winget Repository
#
# To contribute Tp-Note to the official winget-pkgs repository:
#
# 1. Fork the official winget-pkgs repository:
#    https://github.com/microsoft/winget-pkgs
# 2. Create the appropriate directory structure:
#        manifests/g/getreu/tpnote/<version>/
# 3. Add your generated manifest files
# 4. Create a pull request to the official repository
#
# ### Required Information for Submission
#
# When submitting to winget-pkgs, ensure your manifests contain:
#
# 1. Accurate package identifier (`getreu.tpnote`)
# 2. Current version number
# 3. Correct SHA256 hash of the MSI file
# 4. Proper installer URL (must be publicly accessible)
# 5. Complete package metadata including:
#    - Publisher
#    - License information
#    - Description
#    - Tags
#    - Supported architectures
#
# ## Troubleshooting
#
# ### Common Issues
#
# 1. "Package not found":
#    - Ensure you're using the correct identifier: `getreu.tpnote`
#    - Verify your winget client is up to date: `winget upgrade`
#
# 2. Installation fails:
#    - Check internet connectivity
#    - Verify the MSI URL is accessible
#    - Confirm the SHA256 hash matches the downloaded file
#
# 3. Manifest validation errors:
#    - Check YAML formatting
#    - Ensure all required fields are present
#    - Validate with `winget validate`
#
# ### Manual Installation
#
# If automatic installation fails, you can:
#
# 1. Download the MSI directly from:
#        https://blog.getreu.net/projects/tp-note/_downloads/package/x86_64-pc-windows-gnu/tpnote-latest-x86_64.msi
# 2. Install manually by double-clicking the MSI file
#
# ## Future Enhancements
#
# The current winget integration is minimal but functional. Future
# improvements could include:
#
# 1. Automated submission to winget-pkgs repository
# 2. Better error handling in build scripts
# 3. Integration with CI/CD pipelines for automatic manifest updates
# 4. Support for multiple architecture versions beyond x64
#
# ## License
#
# Tp-Note is distributed under the MIT/Apache-2.0 license. See LICENSE-MIT and
# LICENSE-APACHE for details.

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
        ManifestVersion: "1.12.0"
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
        ManifestVersion: "1.12.0"
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
        ManifestVersion: "1.12.0"
    } | to yaml | save -f ($final_dir | path join $"($package_id).locale.en-US.yaml")

    print $"(ansi green)Success! Manifests created in: ($final_dir)(ansi reset)"
}

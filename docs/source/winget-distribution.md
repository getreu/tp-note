# Tp-Note Windows Package Manager Distribution Guide

This document explains how to distribute Tp-Note through the Windows Package Manager (winget).

## Overview

Tp-Note can be distributed through Microsoft's Windows Package Manager (winget) to make it easily discoverable and installable for Windows users. This guide covers the setup and distribution process.

## Prerequisites

- Windows 10 or Windows 11 with Windows Package Manager installed
- PowerShell 5.1 or later
- Git for version control (if contributing to the official repository)

## Installing Windows Package Manager

If you don't have winget installed:

1. Open Microsoft Store
2. Search for "Windows Package Manager"
3. Install "App Installer" (which includes winget)

Alternatively, download it from:

- [Microsoft Store](https://apps.microsoft.com/store/detail/app-installer/9NBLGGH4NNS1)
- [GitHub Releases](https://github.com/microsoft/winget-cli/releases)

## Installing Tp-Note via Winget

Once winget is installed, you can install Tp-Note:

```powershell
winget install getreu.tpnote
```

## How Tp-Note is Distributed via Winget

### Manifest Structure

Tp-Note uses the following manifest structure in the winget repository:

```
manifests/
└── g/
    └── getreu/
        └── tpnote/
            └── <version>/
                ├── manifest.yaml
                ├── Version.yaml
                └── Installer.yaml
```

### Manifest Components

1. **manifest.yaml**: Main package metadata
2. **Version.yaml**: Version-specific information
3. **Installer.yaml**: Installer details including URL and hash

## Local Development and Testing

### Prerequisites

- Bash shell (Git Bash, WSL, or similar)
- OpenSSL or sha256sum/shasum utilities
- Internet connectivity for downloading dependencies

### Generating Manifests

Run the following script to generate winget manifests:

```bash
./scripts/22-generate-winget-manifests
```

This will:

1. Parse the version from Cargo.toml
2. Create the proper directory structure
3. Generate manifest files with appropriate metadata
4. Calculate SHA256 hash for the MSI installer

### Testing Locally

Test your generated manifests:

```bash
./scripts/23-test-winget-manifests
```

This script validates:

1. That manifests exist in the expected location
2. That required manifest files are present
3. Provides guidance for manual testing

## Contributing to Official Winget Repository

To contribute Tp-Note to the official winget-pkgs repository:

1. Fork the [official winget-pkgs repository](https://github.com/microsoft/winget-pkgs)
2. Create the appropriate directory structure:
   ```
   manifests/g/getreu/tpnote/<version>/
   ```
3. Add your generated manifest files
4. Create a pull request to the official repository

### Required Information for Submission

When submitting to winget-pkgs, ensure your manifests contain:

1. Accurate package identifier (`getreu.tpnote`)
2. Current version number
3. Correct SHA256 hash of the MSI file
4. Proper installer URL (must be publicly accessible)
5. Complete package metadata including:
   - Publisher
   - License information
   - Description
   - Tags
   - Supported architectures

## Troubleshooting

### Common Issues

1. **"Package not found"**:
   - Ensure you're using the correct identifier: `getreu.tpnote`
   - Verify your winget client is up to date: `winget upgrade`

2. **Installation fails**:
   - Check internet connectivity
   - Verify the MSI URL is accessible
   - Confirm the SHA256 hash matches the downloaded file

3. **Manifest validation errors**:
   - Check YAML formatting
   - Ensure all required fields are present
   - Validate with `winget validate`

### Manual Installation

If automatic installation fails, you can:

1. Download the MSI directly from:

   ```
   https://blog.getreu.net/projects/tp-note/_downloads/package/x86_64-pc-windows-gnu/tpnote-latest-x86_64.msi
   ```

2. Install manually by double-clicking the MSI file

## Future Enhancements

The current winget integration is minimal but functional. Future improvements could include:

1. Automated submission to winget-pkgs repository
2. Better error handling in build scripts
3. Integration with CI/CD pipelines for automatic manifest updates
4. Support for multiple architecture versions beyond x64

## License

Tp-Note is distributed under the MIT/Apache-2.0 license. See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE) for details.

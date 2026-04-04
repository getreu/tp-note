#!/usr/bin/env nu

# Build directory structure

# build/bin/
# ├── x86_64-unknown-linux-gnu/          # Native Linux binary (x86_64)
# ├── musl-unknown-linux-gnu/            # Musl-compatible Linux binary
# ├── x86_64-pc-windows-gnu/             # Windows binary (x86_64)
# ├── aarch64-unknown-linux-gnu/         # Raspberry Pi 64-bit binary (AArch64)
# ├── armv7-unknown-linux-gnueabihf/     # Raspberry Pi 32-bit binary (ARMv7)
# ├── x86_64-apple-darwin/               # macOS Intel binary (x86_64)
# └── aarch64-apple-darwin/              # macOS Apple Silicon binary (ARM64)

# build/package/
# ├── x86_64-unknown-linux-gnu/          # Debian packages for x86_64 Linux
# ├── musl-unknown-linux-gnu/            # Debian packages for musl Linux
# ├── x86_64-pc-windows-gnu/             # Windows MSI packages
# ├── aarch64-unknown-linux-gnu/         # Debian packages for AArch64 Linux
# ├── armv7-unknown-linux-gnueabihf/     # Debian packages for ARMv7 Linux
# ├── x86_64-apple-darwin/               # macOS packages (Intel)
# └── aarch64-apple-darwin/              # macOS packages (Apple Silicon)

# Define target configurations
let target_map = [
    [target, bin_dir, file_name, desc];
    ["tpnote-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "tpnote", "Native Linux binary"]
    ["tpnote-x86_64-unknown-linux-musl", "musl-unknown-linux-gnu", "tpnote", "musl Linux binary"]
    ["tpnote-x86_64-pc-windows-gnu", "x86_64-pc-windows-gnu", "tpnote.exe", "Windows binary"]
    ["tpnote-armv7-unknown-linux-gnueabihf", "armv7-unknown-linux-gnueabihf", "tpnote", "RPi 32-bit binary"]
    ["tpnote-aarch64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "tpnote", "RPi 64-bit binary"]
    ["tpnote-x86_64-apple-darwin", "x86_64-apple-darwin", "tpnote", "macOS Intel binary"]
    ["tpnote-aarch64-apple-darwin", "aarch64-apple-darwin", "tpnote", "macOS ARM binary"]
]

# --- Environment Setup ---

let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

print "Initializing directory structure..."
let archs = $target_map.bin_dir
mkdir ...($archs | each { |it| [$"build/bin/($it)", $"build/package/($it)"] } | flatten)
#chmod -R u+w build/

# --- Helper Functions ---

def get_store_path [target: string] {
    # Interpolate the target string to prevent Nix from seeing literal '$target'
    let info = (do { nix path-info $"#($target)" } | complete)
    if $info.exit_code == 0 {
        $info.stdout | lines | first | str trim
    } else {
        ""
    }
}

def copy_artifact [target_info: record, store_path: path] {
    let dest_dir = $"build/bin/($target_info.bin_dir)"

    # Check both standard bin paths and root paths in the nix store
    let potential_sources = [
        ($store_path | path join "bin" $target_info.file_name),
        ($store_path | path join $target_info.file_name)
    ]

    let source = ($potential_sources | where { |p| $p | path exists } | first)

    if ($source | is-not-empty) {
        cp -f $source $dest_dir
        print $"[OK] Copied ($target_info.desc) to ($dest_dir)"
        return true
    }
    print $"[ERROR] Could not find binary in store path: ($store_path)"
    false
}

# --- Main Build Loop ---

print $"(ansi cyan_bold)Starting build process... (ansi reset)"

let results = ($target_map | each { |cfg|
    let dest_path = $"build/bin/($cfg.bin_dir)/($cfg.file_name)"

    if ($dest_path | path exists) {
        print $"[SKIP] ($cfg.target) already present."
        return { name: $cfg.target, status: "skipped" }
    }

    print $"[BUILD] Building ($cfg.target)..."

    # Use string interpolation to pass the correct attribute name to Nix
    let build_result = (do { nix build $"#($cfg.target)" --no-link } | complete)

    if $build_result.exit_code == 0 {
        let store_path = (get_store_path $cfg.target)
        if ($store_path != "") and (copy_artifact $cfg $store_path) {
            return { name: $cfg.target, status: "success" }
        }
    }

    print $"(ansi red_bold)[FAIL] ($cfg.target) failed to build.(ansi reset)"
    return { name: $cfg.target, status: "failed" }
})

# --- Debian Packaging Logic ---

let deb_target = "tpnote-deb"
let deb_dest_root = "build/package/x86_64-unknown-linux-gnu/"

# Use path join to safely construct the glob pattern and avoid double slashes
let deb_glob = ($deb_dest_root | path join "*.deb")

if (glob $deb_glob | is-empty) {
    print "[BUILD] Creating Debian package..."
    let deb_build = (do { nix build $"#($deb_target)" --no-link } | complete)

    if $deb_build.exit_code == 0 {
        let store = (get_store_path $deb_target)
        # Use path join for the store glob as well
        let store_glob = ($store | path join "**" "*.deb")
        let deb_files = (glob $store_glob)

        if ($deb_files | is-not-empty) {
            cp ($deb_files | first) $deb_dest_root
            print "[OK] Debian package staged."
        }
    }
}

# --- Final Report ---

let success_count = ($results | where status == "success" | length)
let failed_list = ($results | where status == "failed")

print "\n(ansi green_underline)Build Summary:(ansi reset)"
print $"* Success: ($success_count)"
print $"* Skipped: ($results | where status == "skipped" | length)"

if ($failed_list | is-not-empty) {
    print $"(ansi red)* Failed: ($failed_list | length) targets -> ($failed_list.name | str join ', ')(ansi reset)"
}

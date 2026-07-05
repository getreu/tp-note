#!/usr/bin/env nu

# Build all Nix-buildable release targets and stage them, flat, into build/.
#
# `nix build .#release` cross-compiles every target Nix-on-Linux can produce
# (x86_64 / musl / aarch64 / armv7 Linux + windows-gnu), wraps each in a
# standard-named archive and builds the Debian package -- all in one
# derivation:
#
#   build/tpnote-<version>-x86_64-unknown-linux-gnu.tar.gz
#   build/tpnote-<version>-x86_64-unknown-linux-musl.tar.gz
#   build/tpnote-<version>-aarch64-unknown-linux-gnu.tar.gz
#   build/tpnote-<version>-armv7-unknown-linux-gnueabihf.tar.gz
#   build/tpnote-<version>-x86_64-pc-windows-gnu.zip
#   build/tpnote_<version>_amd64.deb
#
# The layout is flat so it mirrors the GitHub release 1:1. macOS archives
# (built natively on GitHub's macOS runners) and the Windows .msi (scripts/18)
# are added into the same flat build/ afterwards.

let project_dir = ($env.FILE_PWD | path dirname)
cd $project_dir

mkdir build

print $"(ansi cyan_bold)Building release archives + Debian package via nix ...(ansi reset)"
print $"(ansi cyan)Cross-compiles every target; this can take many minutes. Streaming live build logs:(ansi reset)"

# Build with -L (--print-build-logs) so nix streams full per-derivation logs to
# the terminal as it works, instead of a silent progress bar. We deliberately do
# NOT wrap this in `do { ... } | complete`, which buffers all output until the
# build finishes (that is what made the step appear frozen for 15+ minutes).
try {
    ^nix build ".#release" --no-link --print-build-logs
    if $env.LAST_EXIT_CODE != 0 {
        error make { msg: "nix build .#release exited non-zero" }
    }
} catch {
    print $"(ansi red_bold)[FAIL] nix build .#release failed.(ansi reset)"
    exit 1
}

# Resolve the (now cached, so instant) output path in a second invocation.
let store = (^nix build ".#release" --no-link --print-out-paths | lines | last | str trim)
if ($store | is-empty) {
    print $"(ansi red_bold)[FAIL] nix build .#release produced no output path.(ansi reset)"
    exit 1
}

# Copy the flat artifacts into build/. The copies stay read-only (they come
# from the read-only Nix store) -- that is fine: later steps only add new files
# alongside them, and 10-clear-targets' `rm -rf build/*` removes read-only
# files without trouble (build/ itself is group-writable). Remove any existing
# same-named copy first: nushell's `cp` opens the destination O_TRUNC, which
# would fail on a pre-existing read-only file (e.g. a re-run without a clean).
for $f in (ls $store | where type == file | get name) {
    let dest = ("build" | path join ($f | path basename))
    rm -f $dest
    cp -f $f build/
}

print $"(ansi green_underline)Staged release artifacts into build/:(ansi reset)"
ls build | where type == file | select name size | print

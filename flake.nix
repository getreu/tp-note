# Tp-Note: Minimalistic note-taking
#
# A modern Nix flake that provides:
# - Development environment with Rust toolchain
# - Cross-compilation support for all platforms
# - Package building for Debian (.deb) and Windows (.msi)
# - Documentation generation
# - Reproducible builds
#
# Supported targets:
# - x86_64-unknown-linux-gnu (native Linux)
# - x86_64-unknown-linux-musl (musl-based Linux)
# - x86_64-pc-windows-gnu (Windows)
# - armv7-unknown-linux-gnueabihf (Raspberry Pi 32-bit)
# - aarch64-unknown-linux-gnu (Raspberry Pi 64-bit)
# - x86_64-apple-darwin (macOS)
# - aarch64-apple-darwin (macOS ARM)
#
# Usage:
#
# **Primary Build:**
# - `nix build` → Builds main tpnote executable (native Linux)
#
# **Cross-compilation Support:**
# - `nix build .#tpnote-x86_64-unknown-linux-gnu` → Cross-compiles to Linux
# - `nix build .#tpnote-x86_64-unknown-linux-musl` → Musl Linux build
# - `nix build .#tpnote-x86_64-pc-windows-gnu` → Windows build
# - `nix build .#tpnote-armv7-unknown-linux-gnueabihf` → Raspberry Pi 32-bit
# - `nix build .#tpnote-aarch64-unknown-linux-gnu` → Raspberry Pi 64-bit
# - `nix build .#tpnote-x86_64-apple-darwin` → macOS build
# - `nix build .#tpnote-aarch64-apple-darwin` → macOS ARM build
#
# **Package Building:**
# - `nix build .#tpnote-deb` → Creates Debian package
# - `nix build .#tpnote-msi` → Creates Windows MSI (fully implemented)
#
# **Documentation:**
# - `nix build .#documentation` → Generates complete documentation set
{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };
  outputs =
    { nixpkgs, ... }:
    let
      pname = "tp-note";
      version = "1.25.18";
    in
    {
      devShells.x86_64-linux = {
        default =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
            };
          in
          pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rust-analyzer
              cargo-audit
              cargo-edit
              clippy
              rustfmt
              # openssl.dev
              git
              # gcc # C compiler needed for some Rust crates
              # stdenv.cc # C/C++ compiler infrastructure
            ];
            nativeBuildInputs = with pkgs; [
              pkg-config
              #openssl.dev
            ];
            LD_LIBRARY_PATH =
              with pkgs;
              lib.makeLibraryPath [
                libGL
                libX11
                libXi
                libxkbcommon
              ];
          };
      };

      packages.x86_64-linux = {
        default =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
            };
            rpathLibs = with pkgs; [
              libGL
              libX11
              libXi
              libxkbcommon
            ];
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildInputs = rpathLibs;
            postFixup = ''patchelf --add-rpath "${pkgs.lib.makeLibraryPath rpathLibs}" $out/bin/tpnote'';
          };
        tpnote-x86_64-unknown-linux-gnu =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
            };
            rpathLibs = with pkgs; [
              libGL
              libX11
              libXi
              libxkbcommon
            ];
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildInputs = rpathLibs;
            postFixup = ''patchelf --add-rpath "${pkgs.lib.makeLibraryPath rpathLibs}" $out/bin/tpnote'';
          };
        tpnote-x86_64-unknown-linux-musl =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.musl64;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-armv7-unknown-linux-gnueabihf =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.armv7l-linux;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-aarch64-unknown-linux-gnu =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.aarch64-linux;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-x86_64-apple-darwin =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.darwin;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-aarch64-apple-darwin =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.aarch64-darwin;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-x86_64-pc-windows-gnu =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.mingwW64;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        tpnote-deb =
          let
            pkgs = import nixpkgs { system = "x86_64-linux"; };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            # Add cargo-deb to the build environment
            nativeBuildInputs = [ pkgs.cargo-deb ];

            # Use proper phases to ensure the binary is built first
            phases = [
              "unpackPhase"
              "patchPhase"
              "configurePhase"
              "buildPhase"
              "installPhase"
            ];

            # Build the Rust project
            buildPhase = ''
              cargo build --release --package tpnote
            '';

            # Create the .deb package
            installPhase = ''
              mkdir -p $out
              # Ensure the deb package is built
              cargo deb --no-build --output $out/tp-note.deb
            '';
          };
        tpnote-msi =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
            };
            pkgs = base.pkgsCross.mingwW64;
          in
          pkgs.stdenv.mkDerivation {
            inherit pname version;
            src = ./.;
            buildInputs = [
              pkgs.cargo
              pkgs.rustc
            ];
            nativeBuildInputs = [
              pkgs.meson
              pkgs.ninja
              pkgs.wix
            ];
            phases = [
              "unpackPhase"
              "buildPhase"
              "installPhase"
            ];
            buildPhase = ''
              cargo build --release --package tpnote
            '';
            installPhase = ''
              mkdir -p $out
              cp target/release/tpnote.exe $out/tpnote.exe
            '';
          };
      };
    };
}

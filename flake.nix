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
{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };
  outputs =
    { nixpkgs, ... }:
    let
      pname = "tpnote";
      version = "1.25.19";
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
              cargo-binutils
              clippy
              rustfmt
              komac
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
            cargoBuildFlags = [ "--locked" ];
            buildInputs = rpathLibs;
            dontStrip = false;
            doCheck = false;
            nativeBuildInputs = [
              pkgs.cargo-binutils
              pkgs.stdenv.cc.bintools
            ];
            postInstall = ''
              strip $out/bin/tpnote
            '';
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
            cargoBuildFlags = [ "--locked" ];
            buildInputs = rpathLibs;
            dontStrip = false;
            doCheck = false;
            nativeBuildInputs = [
              pkgs.cargo-binutils
              pkgs.stdenv.cc.bintools
            ];
            postInstall = ''
              strip $out/bin/tpnote
            '';
          };
        tpnote-x86_64-unknown-linux-musl =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "x86_64-unknown-linux-musl";
                isStatic = true;
              };
            };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--locked" ];
            dontStrip = false;
            doCheck = false;
            nativeBuildInputs = [
              pkgs.cargo-binutils
            ];
          };
        tpnote-armv7-unknown-linux-gnueabihf =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "armv7l-unknown-linux-gnueabihf";
              };
            };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            dontStrip = false;
            doCheck = false;
            cargoBuildFlags = [ "--locked" ];
            nativeBuildInputs = [
              pkgs.cargo-binutils
            ];
          };
        tpnote-aarch64-unknown-linux-gnu =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "aarch64-unknown-linux-gnu";
              };
            };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            dontStrip = false;
            cargoBuildFlags = [ "--locked" ];
            nativeBuildInputs = [
              pkgs.cargo-binutils
            ];
          };
        tpnote-x86_64-apple-darwin =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "x86_64-apple-darwin";
              };
            };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            dontStrip = false;
            doCheck = false;
            cargoBuildFlags = [ "--locked" ];
            nativeBuildInputs = [
              pkgs.cargo-binutils
            ];
          };
        tpnote-aarch64-apple-darwin =
          let
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "aarch64-apple-darwin";
              };
            };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            dontStrip = false;
            doCheck = false;
            cargoBuildFlags = [ "--locked" ];
            nativeBuildInputs = [
              pkgs.cargo-binutils
            ];
          };
        tpnote-x86_64-pc-windows-gnu =
          let
            base = import nixpkgs {
              system = "x86_64-linux";
              crossSystem = {
                config = "x86_64-pc-windows-gnu";
              };
            };
            pkgs = base.pkgsCross.mingwW64;
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            dontStrip = false;
            doCheck = false;
            cargoBuildFlags = [ "--locked" ];
            nativeBuildInputs = [ pkgs.cargo-binutils ];
          };
        tpnote-deb =
          let
            pkgs = import nixpkgs { system = "x86_64-linux"; };
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--locked" ];
            # Add cargo-deb to the build environment
            nativeBuildInputs = [
              pkgs.cargo-deb
              pkgs.cargo-binutils
            ];
            dontStrip = false;
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
              cargo deb --no-build --output $out/${pname}-${version}-x86_64.deb
            '';
          };
      };
    };
}

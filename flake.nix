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
# - x86_64-unknown-linux-musl (musl-based Linux, static linking)
# - x86_64-pc-windows-gnu (Windows)
# - armv7-unknown-linux-gnueabihf (Raspberry Pi 32-bit, Debian/Ubuntu compatible)
# - aarch64-unknown-linux-gnu (Raspberry Pi 64-bit, Debian/Ubuntu compatible)
# - x86_64-apple-darwin (macOS Intel)
# - aarch64-apple-darwin (macOS ARM)
#
# Usage:
#
# **Primary Build:**
# - `nix build` → Builds main tpnote executable (native Linux)
#
# **Cross-compilation Support:**
# - `nix build .#tpnote-x86_64-unknown-linux-gnu` → Linux binary
# - `nix build .#tpnote-x86_64-unknown-linux-musl` → Static musl Linux build
# - `nix build .#tpnote-x86_64-pc-windows-gnu` → Windows build
# - `nix build .#tpnote-armv7-unknown-linux-gnueabihf` → Raspberry Pi 32-bit (Debian/Ubuntu)
# - `nix build .#tpnote-aarch64-unknown-linux-gnu` → Raspberry Pi 64-bit (Debian/Ubuntu)
# - `nix build .#tpnote-x86_64-apple-darwin` → macOS Intel build
# - `nix build .#tpnote-aarch64-apple-darwin` → macOS ARM build
#
# **Package Building:**
# - `nix build .#tpnote-deb` → Creates Debian package (x86_64 only)
#
# **Debian/Ubuntu Compatibility Notes:**
# The ARM cross-compiled binaries (armv7 and aarch64) are built using Nixpkgs'
# cross-compilation infrastructure which produces binaries compatible with:
# - Debian 11 (Bullseye) and newer
# - Ubuntu 20.04 (Focal) and newer
# - Raspberry Pi OS (Debian-based)
#
# These binaries link against glibc and use standard Debian/Ubuntu library paths.
# To verify compatibility, run:
#   readelf -d <binary> | grep NEEDED  # Check dynamic dependencies
#   readelf -d <binary> | grep interpreter  # Check dynamic linker
{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };
  outputs =
    { nixpkgs, ... }:
    let
      pname = "tpnote";
      version = "1.26.0";

      # Helper function for building Rust packages with cross-compilation
      # Ensures proper linker configuration for Debian/Ubuntu compatibility
      buildRustTarget =
        {
          system,
          crossSystemConfig,
          extraBuildInputs ? [ ],
          extraNativeBuildInputs ? [ ],
          extraRustFlags ? "",
        }:
        let
          pkgs = import nixpkgs {
            inherit system;
            crossSystem = {
              config = crossSystemConfig;
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
            pkgs.stdenv.cc.bintools
          ]
          ++ extraNativeBuildInputs;
          buildInputs = extraBuildInputs;
          # Pass linker flags for proper dynamic linker configuration
          RUSTFLAGS = extraRustFlags;
          postInstall = ''
            ${pkgs.stdenv.cc}/bin/${crossSystemConfig}-strip $out/bin/tpnote
          '';
        };
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
              git
            ];
            nativeBuildInputs = with pkgs; [
              pkg-config
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
        # ARMv7 (32-bit) cross-compilation for Debian/Ubuntu
        # Produces binaries compatible with:
        # - Debian 11 (Bullseye) and newer
        # - Ubuntu 20.04 (Focal) and newer
        # - Raspberry Pi OS (Debian-based)
        # - Raspberry Pi 2/3/4/5 (32-bit mode)
        tpnote-armv7-unknown-linux-gnueabihf = buildRustTarget {
          system = "x86_64-linux";
          crossSystemConfig = "armv7l-unknown-linux-gnueabihf";
          extraRustFlags = ''
            -C link-arg=-Wl,--dynamic-linker=/lib/ld-linux-armhf.so.3
          '';
        };
        # ARM64 (64-bit) cross-compilation for Debian/Ubuntu
        # Produces binaries compatible with:
        # - Debian 11 (Bullseye) and newer
        # - Ubuntu 20.04 (Focal) and newer
        # - Raspberry Pi OS (Debian-based)
        # - Raspberry Pi 4/5 (64-bit mode)
        # - ARM servers
        tpnote-aarch64-unknown-linux-gnu = buildRustTarget {
          system = "x86_64-linux";
          crossSystemConfig = "aarch64-unknown-linux-gnu";
          extraRustFlags = ''
            -C link-arg=-Wl,--dynamic-linker=/lib/ld-linux-aarch64.so.1
          '';
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

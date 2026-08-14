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
#   Named per Debian policy: `tpnote_<version>_amd64.deb`.
#
# **Release Archives (flat, standard-named; one file per target):**
# - `nix build .#release` → stages all build-ready release assets in a single
#   flat directory: one `.tar.gz`/`.zip` per target (Rust target-triple names)
#   plus the `.deb`. macOS archives and the Windows `.msi` come from other
#   build hosts/tools and are merged in downstream.
# - `nix build .#release-<target>` → a single archive for one target, e.g.
#   `tpnote-<version>-x86_64-unknown-linux-gnu.tar.gz`.
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
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      pname = "tpnote";
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

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
            pkgsNative.patchelf
          ]
          ++ extraNativeBuildInputs;
          buildInputs = extraBuildInputs;
          # Pass linker flags for proper dynamic linker configuration
          RUSTFLAGS = extraRustFlags;
          postInstall = ''
            ${pkgs.stdenv.cc}/bin/${crossSystemConfig}-strip $out/bin/tpnote
            patchelf --remove-rpath $out/bin/tpnote
          '';
        };

      lib = nixpkgs.lib;
      pkgsNative = import nixpkgs { system = "x86_64-linux"; };

      # Wrap a built target derivation (holding $out/bin/tpnote[.exe]) into a
      # single release archive named with the standard Rust convention:
      #   <pname>-<version>-<target-triple>.tar.gz   (Linux / macOS)
      #   <pname>-<version>-<target-triple>.zip       (Windows)
      # The archive holds the bare binary at its root, matching the CI
      # native-matrix job so the whole release uses one consistent scheme.
      # Archives are built reproducibly (fixed mtime/owner, no gzip timestamp).
      mkArchive =
        {
          target,
          drv,
          exe ? false,
        }:
        pkgsNative.runCommand "${pname}-${version}-${target}"
          { nativeBuildInputs = lib.optional exe pkgsNative.zip; }
          (
            if exe then
              ''
                mkdir -p $out
                cd ${drv}/bin
                zip -X -q "$out/${pname}-${version}-${target}.zip" tpnote.exe
              ''
            else
              ''
                mkdir -p $out
                tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
                  -C ${drv}/bin -cf - tpnote \
                  | gzip -n > "$out/${pname}-${version}-${target}.tar.gz"
              ''
          );
    in
    {
      devShells.x86_64-linux = {
        default =
          let
            toolchain = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain;
            pkgs = import nixpkgs {
              system = "x86_64-linux";
              overlays = [ rust-overlay.overlays.default ];
            };
            rustPkg = pkgs.rust-bin.stable.${toolchain.channel}.default.override {
              extensions = toolchain.components;
              targets = toolchain.targets;
            };
          in
          pkgs.mkShell {
            packages = with pkgs; [
              rustPkg
              cargo-audit
              cargo-edit
              cargo-binutils
              komac
              git
              gh
              glab
              # Pipeline orchestration (scripts/*.nu) expects nushell on PATH so
              # `nix develop --command nu scripts/01-make-all.nu` runs (CI and
              # local). The documentation toolchain (pandoc + weasyprint) is NOT
              # here: scripts/13-make-docs.nu enters docs/flake.nix's devShell
              # for that, which is the single source of truth for docs deps.
              nushell
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
      packages.x86_64-linux = rec {
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
              pkgs.patchelf
            ];
            postInstall = ''
              strip $out/bin/tpnote
              patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 $out/bin/tpnote
              patchelf --remove-rpath $out/bin/tpnote
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
        # macOS targets (x86_64-apple-darwin, aarch64-apple-darwin) are intentionally
        # absent from packages.x86_64-linux: cross-compiling to macOS requires Apple's
        # cctools and SDK, which are only available on macOS hosts and cannot be
        # evaluated by nixpkgs on Linux.
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
              pkgs.patchelf
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
              patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 target/release/tpnote
              patchelf --remove-rpath target/release/tpnote
              # Let cargo-deb name the file per Debian policy:
              #   <package>_<version>_<debian-arch>.deb  (e.g. tpnote_1.26.4_amd64.deb)
              cargo deb --no-build
              cp target/debian/*.deb $out/
            '';
          };

        # --- Release archives (flat, standard-named; one file per target) ---
        # `nix build .#release` collects build-ready assets into one flat
        # directory. macOS archives (native-matrix CI) and the Windows .msi
        # (scripts/18 + wix) are produced elsewhere and merged in downstream.
        "release-x86_64-unknown-linux-gnu" = mkArchive {
          target = "x86_64-unknown-linux-gnu";
          drv = tpnote-x86_64-unknown-linux-gnu;
        };
        "release-x86_64-unknown-linux-musl" = mkArchive {
          target = "x86_64-unknown-linux-musl";
          drv = tpnote-x86_64-unknown-linux-musl;
        };
        "release-armv7-unknown-linux-gnueabihf" = mkArchive {
          target = "armv7-unknown-linux-gnueabihf";
          drv = tpnote-armv7-unknown-linux-gnueabihf;
        };
        "release-aarch64-unknown-linux-gnu" = mkArchive {
          target = "aarch64-unknown-linux-gnu";
          drv = tpnote-aarch64-unknown-linux-gnu;
        };
        "release-x86_64-pc-windows-gnu" = mkArchive {
          target = "x86_64-pc-windows-gnu";
          drv = tpnote-x86_64-pc-windows-gnu;
          exe = true;
        };
        release = pkgsNative.runCommand "${pname}-release-${version}" { } ''
          mkdir -p $out
          cp ${release-x86_64-unknown-linux-gnu}/* $out/
          cp ${release-x86_64-unknown-linux-musl}/* $out/
          cp ${release-armv7-unknown-linux-gnueabihf}/* $out/
          cp ${release-aarch64-unknown-linux-gnu}/* $out/
          cp ${release-x86_64-pc-windows-gnu}/* $out/
          cp ${tpnote-deb}/*.deb $out/
        '';
      };
    };
}

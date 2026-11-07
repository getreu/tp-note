{
  description = "Run WiX Toolset 6 under Wine on Linux (via MSI installer)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };

      # --- WiX version to install ---
      # Check the latest release on GitHub:
      #   https://github.com/wixtoolset/wix/releases
      # The asset we want is: wix-cli-x64.msi
      wixVersion = "6.0.2";

      # --- MSI download + checksum ---
      # To update:
      #   1. Replace the version in the URL below.
      #   2. Run: nix-prefetch-url https://github.com/wixtoolset/wix/releases/download/v<version>/wix-cli-x64.msi
      #   3. Copy the printed sha256 hash into this file.
      wixMsi = pkgs.fetchurl {
        url = "https://github.com/wixtoolset/wix/releases/download/v${wixVersion}/wix-cli-x64.msi";
        sha256 = "a8a5cc7443353cef3ab900c60cd7a3a5ee601746319d104ac7b12ad0ced2345c";
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = [
          # wine64Packages (64-bit only, no 32-bit multilib) rather than
          # wineWowPackages: the Wow (32+64-bit) combination has no cached
          # binary for this revision on either this machine or GitHub's
          # runners, forcing a from-source Wine build that -- compounded by
          # the crash-dialog loop below -- made CI run for 2.5+ hours before
          # being cancelled. WiX 6's CLI is .NET-only and never needs 32-bit
          # support, and wine64Packages.stableFull substitutes instantly from
          # cache.nixos.org (confirmed on this exact revision).
          #
          # The *Full* variant bundles the wine-mono and wine-gecko MSIs in
          # the store, so Wine installs the .NET Framework runtime (WiX 6 CLI
          # targets .NETFramework v4.7.2) SILENTLY from its local copy at
          # prefix creation instead of popping an interactive "download
          # Mono?" dialog. This is what makes a cold prefix reproducible in
          # CI (fully cached, no network).
          pkgs.wine64Packages.stableFull
          pkgs.cabextract
          pkgs.unzip
          pkgs.msitools
        ];

        shellHook = ''
          set -e

          export RUST_BACKTRACE=1
          export WINEPREFIX="$HOME/.wine-tpnote"
          export WINEARCH=win64
          export WINEDEBUG=-all
          # Non-interactive: never let Wine pop a GUI installer dialog. Mono is
          # provided locally by the *Full* wine build (installed silently below);
          # gecko/mshtml is not needed by WiX, so disable it outright.
          export WINEDLLOVERRIDES="mshtml=d"
          # Headless CI has no X display; keep Wine from trying to use one.
          unset DISPLAY

          # First-time installation of WiX
          if [ ! -f "$WINEPREFIX/.wix-installed" ]; then
            echo ">>> Initializing Wine prefix (silently installs bundled Mono)..."
            mkdir -p "$WINEPREFIX"
            # The very first wineboot on a cold prefix can transiently fail while
            # core DLLs are staged (STATUS_DLL_NOT_FOUND); retry once, then block
            # until the wineserver has finished creating the prefix.
            wineboot -u || wineboot -u
            wineserver -w

            # Headless CI has no window driver, so any crash (e.g. a Windows
            # process failing to create a window) must terminate the
            # faulting process instead of popping winedbg's "Unhandled
            # exception" dialog -- that dialog has no display to render on
            # either, and without a display each attempt re-faults, spawning
            # a new winedbg --auto per failure with nothing to stop the next
            # one: an unbounded process fork loop (confirmed: 70+ processes
            # spawned in under a minute before being killed by hand).
            wine reg add "HKCU\\Software\\Wine\\WineDbg" /v ShowCrashDialog /t REG_DWORD /d 0 /f
            wineserver -w

            echo ">>> Installing WiX ${wixVersion} (MSI)..."
            wine msiexec /i ${wixMsi} /qn
            wineserver -w

            touch "$WINEPREFIX/.wix-installed"
            echo ">>> WiX installed."
          fi

          # Add wix.exe to PATH (check both Program Files dirs)
          if [ -d "$WINEPREFIX/drive_c/Program Files/WiX Toolset v6" ]; then
            export PATH="$WINEPREFIX/drive_c/Program Files/WiX Toolset v6:$PATH"
          elif [ -d "$WINEPREFIX/drive_c/Program Files (x86)/WiX Toolset v6" ]; then
            export PATH="$WINEPREFIX/drive_c/Program Files (x86)/WiX Toolset v6:$PATH"
          fi

          # Default WiX preprocessor variables
          export Version="1.0.0"
          export Platform="x64"

          echo
          echo ">>> WiX development shell ready."
          echo ">>> Run: wine wix.exe --version"
          echo ">>> Build with: wine wix.exe build tpnote.wxs -o tpnote.msi"
          echo
        '';
      };
    };
}

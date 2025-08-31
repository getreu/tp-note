{
  description = "Run WiX Toolset 6 under Wine on Linux (via MSI installer)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
  };

  outputs = { self, nixpkgs }: let
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
  in {
    devShells.${system}.default = pkgs.mkShell {
      nativeBuildInputs = [
        pkgs.wineWowPackages.stable
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

        # First-time installation of WiX
        if [ ! -f "$WINEPREFIX/.wix-installed" ]; then
          echo ">>> Initializing Wine prefix..."
          wineboot -i

          echo ">>> Installing WiX ${wixVersion} (MSI)..."
          wine64 msiexec /i ${wixMsi} /qn

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
        echo ">>> Run: wine64 wix.exe --version"
        echo ">>> Build with: wine64 wix.exe build tpnote.wxs -o tpnote.msi"
        echo
      '';
    };
  };
}

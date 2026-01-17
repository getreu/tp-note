{
  description = "Pandoc + WeasyPrint dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux"; # change if needed
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          pandoc
          python311
          python311Packages.weasyprint

          # WeasyPrint runtime dependencies
          cairo
          pango
          gdk-pixbuf
          libffi
          glib
        ];
      };
    };
}

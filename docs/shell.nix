{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pandoc
    python311Packages.weasyprint
  ];
  
}


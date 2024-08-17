# { pkgs ? import <nixpkgs> {} }:
# pkgs.mkShell {
#   nativeBuildInputs = with pkgs; [ 
#     cargo
#     cargo-edit
#     rustc 
#     rustfmt 
#     clippy 
#     rust-analyzer
#   ];
# 
#   # Certain Rust tools won't work without this
#   # This can also be fixed by using oxalica/rust-overlay and specifying the rust-src extension
#   # See https://discourse.nixos.org/t/rust-src-not-found-and-other-misadventures-of-developing-rust-on-nixos/11570/3?u=samuela. for more details.
#   RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
# }

/*
based on
https://discourse.nixos.org/t/how-can-i-set-up-my-rust-programming-environment/4501/9
*/
let
  rust_overlay = import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz");
  pkgs = import <nixpkgs> { overlays = [ rust_overlay ]; };
  # rustVersion = "latest";
  rustVersion = "1.80.1";
  rust = pkgs.rust-bin.stable.${rustVersion}.default.override {
    extensions = [
      "rust-src" # for rust-analyzer
      "rust-analyzer"
    ];
  };
in
pkgs.mkShell {
  buildInputs = [
    rust
  ] ++ (with pkgs; [
    pkg-config
    # other dependencies
    #gtk3
    #wrapGAppsHook
  ]);
  RUST_BACKTRACE = 1;
}

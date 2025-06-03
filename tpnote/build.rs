extern crate winresource;

use std::env;
use std::error::Error;
use winresource::WindowsResource;

/// Cross compile with icons is a new feature in `winres 0.1.12`:
/// 
/// * [Adding an icon issues when building from Linux for Windows · Issue #33 · mxre/winres · GitHub](https://github.com/mxre/winres/issues/33)
/// * [Enable cross compiling from unix/macos by moshensky · Pull Request #24 · mxre/winres · GitHub](https://github.com/mxre/winres/pull/24)
/// * [Rust 1.61 in Linux does no add resources to the EXE · Issue #40 · mxre/winres · GitHub](https://github.com/mxre/winres/issues/40#issuecomment-1321141396)
///
fn add_icon_to_bin_when_building_for_win(icon_path: &str) -> Result<(), Box<dyn Error>> {
    if env::var("CARGO_CFG_TARGET_FAMILY")? == "windows" {
        let mut res = WindowsResource::new();
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV")?;
        match target_env.as_str() {
            "gnu" => res
                .set_ar_path("x86_64-w64-mingw32-ar")
                .set_windres_path("x86_64-w64-mingw32-windres")
                .set_toolkit_path(".")
                .set_icon(icon_path),
            "msvc" => res.set_icon(icon_path),
            _ => panic!("Unsupported env: {}", target_env),
        };
        res.compile()?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    add_icon_to_bin_when_building_for_win("tpnote.ico")
}

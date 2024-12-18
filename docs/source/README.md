# Tp-Note: Markup enhanced granular note-taking

**Save and edit your clipboard content as a note file**


[![Cargo](https://img.shields.io/crates/v/tpnote.svg)](
https://crates.io/crates/tp-note)
[![Documentation](https://docs.rs/tpnote-lib/badge.svg)](
https://docs.rs/tpnote-lib)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://gitlab.com/getreu/tp-note)

_Tp-Note_ is a note-taking-tool and a template system - freely available for
Windows, MacOS and Linux - that consistently synchronizes the note’s meta-data
with its filename. _Tp-Note_'s main design goal is to convert some input text -
usually provided by the system's clipboard - into a Markdown note file with
a descriptive YAML header and a meaningful filename.
_Tp-Note_ collects various information about its environment
and the clipboard and stores them in variables. New notes are created by
filling these variables in predefined and customizable _Tera_-templates.
_TP-Note's_ default templates are written in Markdown and can be easily adapted
to any other markup language if needed. After creating a new note, _TP-Note_
launches the system's text editor and connects the default web browser to _Tp-
Note_'s internal Markdown/RestructuredText renderer and web server. The viewer
detects note file changes and updates the rendition accordingly.

![Screenshot](./assets/tpnote-screenshot.png)

_On Tue, 2023-12-19 at 12:58 +1100, Dev Rain wrote:_

> _Found Tp-Note awhile back and it has become part of my daily workflow,
> and indeed part of my daily note-taking life. I wanted to extend my
> thanks; so thank you. 
> dev.rain_

Read more in [Tp-Note’s user manual], [Download Tp-Note] or visit the project 
page: [Tp-Note - Minimalistic note-taking].


---



# Documentation

User documentation:

* Introductory video

  [Tp-Note - Most common use cases - YouTube]

* Project page:

  [Tp-Note's project page], which
  you are reading right now, lists where you can download _Tp-Note_ and gives
  an overview of _Tp-Note_'s resources and documentation.

* User manual:

  The user manual showcases how to best use use _Tp-Note_ and how to integrate it
  with your file manager.

  [Tp-Note user manual - html]

  [Tp-Note user manual - pdf]

* Unix man-page:

  The Unix man-page is _Tp-Note_'s technical reference. Here you learn how to customize
  _Tp-Note_'s templates and how to change its default settings.

  [Tp-Note manual page - html]

  [Tp-Note manual page - pdf]

* [Blogposts about Tp-Note]

Developer documentation:

* API documentation

  _Tp-Note_'s program code documentation targets mainly software developers.
  The code is split into a library [tpnote-lib] and the command line
  application [tpnote].
  The advanced user may consult the [Tp-Note's config module documentation]
  which explains the default templates and setting. Many of them can be
  customized through _Tp-Note_'s configuration file.

  [Constants in API documentation]



# Source code

Repository:

* [Tp-Note on Gitlab]

* [Tp-Note on Github (mirror)]



# Distribution

## Download installer packages and binaries

### Tp-Note Microsoft Windows installer package

* Installer package for Windows:

  [tpnote-latest-x86_64.msi]

  As this early version of the Windows installer is not signed yet, Windows
  will show the error message “Windows protected your PC”. As a work-around,
  when you click on the link “More info”, a ”Run anyway” button will appear
  allowing you to continue the installation process. In general, regardless
  of where a program comes from, I recommend checking every installable
  file with [VirusTotal].

### Tp-Note Debian/Ubuntu installer package

* Package compiled for Debian/Ubuntu:

  [x86_64-unknown-linux-gnu/debian/tpnote_latest_amd64.deb]

### Various binaries for Windows, MacOS and Linux

* Binaries for Ubuntu-Linux 18.04, Windows, MacOS:

    1. Open: [Releases - getreu/tp-note]

    2. Open the latest release.

    3. Open *assets*.

    4. Download the packed executable for your operating system.

    5. Installation: see below.

* Executable for Windows:

    * [x86_64-pc-windows-gnu/release/tpnote.exe]

* Linux binary (compiled with Debian):

    * [x86_64-unknown-linux-gnu/release/tpnote]

    * The following "musl" version also works on a headless system.

      [x86_64-unknown-linux-musl/release/tpnote]

* Binaries for Raspberry Pi (32 bit):

    * [armv7-unknown-linux-gnueabihf/release/tpnote]

* Binaries for Raspberry Pi (64 bit):

    * [aarch64-unknown-linux-gnu/release/tpnote]



## Tp-Note in official package repositories

### Tp-Note on NetBSD

* An official package is available on NetBSD and other "pkgsrc" supported 
  platforms.

  To install Tp-Note on NetBSD, simply use the native package manager:

  ```sh
  pkgin install tpnote
  ```

### Tp-Note on NixOS

* An official package is available on NixOS:

  ```nix
   environment.systemPackages = [ pkgs.tpnote ]; 
  ```

### Get Tp-Note with the Nix package manager

* First install the [Nix package manager](https://nixos.org/download)
  available for Linux, MacOS and Windows (WSL2). 
  Alternatively, for Linux there are also [prebuilt Deb/Pacman/Rpm 
  Nix installers](https://nix-community.github.io/nix-installers/)
  available.
  
  Once you have the Nix package manager installed on your system, 
  try out Tp-Note with:  

  ```nix
  nix-shell -p tpnote
  ```

  or follow installation instructions here:
  [NixOS packages - tpnote](https://search.nixos.org/packages?channel=unstable&show=tpnote&from=0&size=50&sort=relevance&type=packages&query=tpnote)
  

## Other resources

* Copy the Unix man-page to `/usr/local/share/man/man1`:

  - [tpnote.1.gz]

* Copy Tp-Note's icon to `/usr/local/share/icons/`:

  - [tpnote.svg]



# Installation

Depending on the availability of installer packages for your operating
system, the installation process is more or less automated. For Windows
users the fully automated installation package [tpnote-latest-x86_64.msi]
is available. For more information, please consult the [Distribution
section](#distribution) above and the [Installation section] in
_Tp-Note_'s manual. 



## Upgrading

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure, e.g.:

```
*** ERROR:
Can not load or parse the (merged) configuration file(s):
---
invalid length 3, expected fewer elements in array in `viewer.served_mime_types`


Note: this error may occur after upgrading Tp-Note due to some incompatible
configuration file changes.

Tp-Note renames and thus disables the last sourced configuration file.

Additional technical details:
*    Command line parameters:
tpnote -b 
*    Sourced configuration files:
/home/joe/.config/tpnote/tpnote.toml
```

Mote, the configuration file backup is stored in the same directory as the last
sourced configuration file, e.g. `/home/joe/.config/tpnote/`.
If Tp-Note sources more than one configuration file, consider the possibility
of syntax errors in any of these files (cf. [Customization section] of
Tp-Note's man-page).



# Building

If the above precompiled binaries do not suite you, you can
compile _Tp-Note_ yourself.


1. [Install Rust]

2. Download, compile and install _Tp-Note_:

   **Building on Linux**

   ```sh
   cargo install tpnote
   sudo cp ~/.cargo/bin/tpnote /usr/local/bin
   # Download icon
   cd /usr/local/share/icons
   sudo wget https://blog.getreu.net/projects/tp-note/assets/tpnote.svg
   ```

   On Linux, Tp-Note displays errors and debug messages as notifications.
   This requires a Linux/BSD based desktop environment that follows the XDG
   specification, e.g. KDE, Gnome, XFCE, LXDC, Mate (and most
   others).[^no-message-box]

   [^no-message-box]: In case an XDG desktop environment is not available, you
       can opt out notifications and message boxes by omitting the `message-box`
       feature by adding 
      `--no-default-features --features lang-detection,read-clipboard,renderer,viewer`
      to `cargo install tpnote`. 
      Now, all error messages are dumped on the console from
      where you started _Tp-Note_ into `stderr`.


   **Recommended Linux console and server version**

   The full-featured version of _Tp-Note_ depends on GUI libraries like Xlib
   which might not be available on a headless system. Either download the Musl
   version [x86_64-unknown-linux-musl/release/tpnote] or compile _Tp-Note_
   yourself without default features:

   ```sh
   cargo install --no-default-features \
     --features lang-detection,renderer tpnote
   sudo cp ~/.cargo/bin/tpnote /usr/local/bin
   ```

   If Tp-Note's binary size if of concern, omit the `lang-detection` feature
   in the `cargo` invocation above. The `lang-detection` feature causes 95% 
   of the final binary size because of its extensive language models.


   **Building on Windows and macOS**

   Build the full-featured version[^win] with:

       cargo install tpnote

   [^win]: When building for Windows or macOS, it does not make sense to exclude
   the `message-box` feature, because - under Windows and macOS - it does not
   rely on the notification library. Instead, it uses direct OS-API calls for
   popping up alert boxes. As these calls have no footprint in binary size or
   speed, always keep the `message-box` feature compiled in.


## Cross compilation

Debian makes it easy to cross-compile for foreign architectures. Here are
some examples:

* Target Musl:

  ```sh
  rustup target add x86_64-unknown-linux-musl
  sudo apt install musl-tools

  cargo build --target x86_64-unknown-linux-musl --release
  ```

* Target Raspberry Pi (32 bit):

  ```sh
  rustup target add armv7-unknown-linux-gnueabihf
  sudo apt install crossbuild-essential-armhf
  
  CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc \
    cargo build --target armv7-unknown-linux-gnueabihf --release
  ```

* Target Raspberry Pi (arm64, 64 bit):

  ```sh
  rustup target add aarch64-unknown-linux-gnu
  sudo apt install crossbuild-essential-arm64
  
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/aarch64-linux-gnu-gcc \
   cargo build  --target aarch64-unknown-linux-gnu --release
  ```

* Target Windows:
  
  ```sh
  rustup target add x86_64-pc-windows-gnu  
  sudo apt install binutils-mingw-w64 mingw-w64
  cargo build --target x86_64-pc-windows-gnu --release 
  ```  
  

This project follows [Semantic Versioning].



# About

Author:

* Jens Getreu

Copyright:

* Apache 2 license or MIT license.

[Tp-Note’s user manual]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Download Tp-Note]: https://blog.getreu.net/projects/tp-note/index.html#distribution
[Tp-Note - Minimalistic note-taking]: https://blog.getreu.net/projects/tp-note/
[Tp-Note - Most common use cases - YouTube]: https://www.youtube.com/watch?v=ODhPytPFtYY
[Tp-Note's project page]: https://blog.getreu.net/projects/tp-note/
[Tp-Note user manual - html]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Tp-Note user manual - pdf]: https://blog.getreu.net/_downloads/tpnote--manual.pdf
[Tp-Note manual page - html]: https://blog.getreu.net/projects/tp-note/tpnote--manpage.html
[Tp-Note manual page - pdf]: https://blog.getreu.net/_downloads/tpnote--manpage.pdf
[Blogposts about Tp-Note]: https://blog.getreu.net/tags/tp-note/
[Tp-Note's config module documentation]: https://docs.rs/tpnote-lib/latest/tpnote_lib/config/
[tpnote]: https://crates.io/crates/tp-note
[tpnote-lib]: https://crates.io/crates/tpnote-lib
[Constants in API documentation]: https://docs.rs/tpnote-lib/latest/tpnote_lib/config/index.html#constants
[Tp-Note on Gitlab]: https://gitlab.com/getreu/tp-note
[Tp-Note on Github (mirror)]: https://github.com/getreu/tp-note
[tpnote-latest-x86_64.msi]: https://blog.getreu.net/projects/tp-note/_downloads/wix/tpnote-latest-x86_64.msi
[VirusTotal]: https://www.virustotal.com/gui/home/upload
[x86_64-unknown-linux-gnu/debian/tpnote_latest_amd64.deb]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tpnote_latest_amd64.deb
[Releases - getreu/tp-note]: https://github.com/getreu/tp-note/releases
[x86_64-pc-windows-gnu/release/tpnote.exe]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tpnote.exe
[x86_64-unknown-linux-gnu/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tpnote
[x86_64-unknown-linux-musl/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tpnote
[armv7-unknown-linux-gnueabihf/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/armv7-unknown-linux-gnueabihf/release/tpnote
[aarch64-unknown-linux-gnu/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/aarch64-unknown-linux-gnu/release/tpnote
[tpnote.1.gz]: https://blog.getreu.net/projects/tp-note/_downloads/tpnote.1.gz
[tpnote.svg]: https://blog.getreu.net/projects/tp-note/assets/tpnote.svg
[tpnote-latest-x86_64.msi]: https://blog.getreu.net/projects/tp-note/_downloads/wix/tpnote-latest-x86_64.msi
[Installation section]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html#installation
[Customization section]: https://blog.getreu.net/projects/tp-note/tpnote--manpage.html#customization
[Install Rust]: https://www.rust-lang.org/tools/install
[Semantic Versioning]: https://semver.org/


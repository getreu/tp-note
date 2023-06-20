---
title: "Tp-Note - Minimalistic note taking: save and edit your clipboard content as a note file"
author: Jens Getreu
filename_sync: false
---

[![Cargo](https://img.shields.io/crates/v/tp-note.svg)](
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
launches the system file editor (or any other of your choice, e.g. _MarkText_
or _Typora_) and connects the default web browser to _Tp-Note_'s
internal Markdown/RestructuredText renderer and web server. The viewer detects
note file changes and updates the rendition accordingly.

* Read more in [Tp-Note’s user manual]

* [Download Tp-Note]

* Project page: [Tp-Note - Minimalistic note taking: save and edit your clipboard content as a note file]


---



## Documentation

User documentation:

* Introductory video

  [Tp-Note - Most common use cases - YouTube]

* Project page:

  [Tp-Note's project page], which
  you are reading right now, lists where you can download _Tp-Note_ and gives
  an overview of _Tp-Note_'s resources and documentation.

* User manual:

  The user manual showcases how to best use use _Tp-Note_ and how to integrate it
  with you file manager.

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



## Source code

Repository:

* [Tp-Note on Gitlab]

* [Tp-Note on Github (mirror)]



## Distribution


### Tp-Note Microsoft Windows installer package

* Installer package for Windows:

  [tpnote-latest-x86_64.msi]

  As this early version of the Windows installer is not signed yet, Windows
  will show the error message “Windows protected your PC”. As a work-around,
  when you click on the link “More info”, a ”Run anyway” button will appear
  allowing you to continue the installation process. In general, regardless
  of where a program comes from, I recommend checking every installable
  file with [VirusTotal]


### Tp-Note Debian/Ubuntu installer package

* Package compiled for Debian:

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

    * The following "musl" version also works on headless system.

      [x86_64-unknown-linux-musl/release/tpnote]

* Binaries for Raspberry Pi:

    * [armv7-unknown-linux-gnueabihf/release/tpnote]


### Tp-Note NetBSD

* An official package is available on NetBSD and other "pkgsrc" supported platforms.

  To install Tp-Note on NetBSD, simply use the native package manager:

  ```sh
  pkgin install tp-note
  ```


### Other ressources

* Copy the Unix man-page to `/usr/local/share/man/man1`:

  - [tpnote.1.gz]

* Copy Tp-Note's icon to `/usr/local/share/icons/`:

  - [tpnote.svg]



## Installation

Depending on the availability of installer packages for your operating
system, the installation process is more or less automated. For Windows
users the fully automated installation package [tpnote-latest-x86_64.msi]
is available. For more information, please consult the [Distribution
section](#distribution) above and the [Installation section] in
_Tp-Note_'s manual.


## Upgrading

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure. In order not to lose the changes you made in
this file, the installer does not replace it automatically with a new version.
Instead, _Tp-Note_ renames the old configuration file and prompts:


    NOTE: unable to load, parse or write the configuration file
    ---
    Reason:
            Bad TOML data: missing field `extension_default` at line 1 column 1!

    Note: this error may occur after upgrading Tp-Note due
    to some incompatible configuration file changes.

    Tp-Note backs up the existing configuration
    file and creates a new one with default values.

The configuration file backup is stored in the same directory as the newly
created configuration file (cf. [Customization section] of _Tp-Note_'s
man-page).


## Building

If the above precompiled binaries do not suite you, you can
compile _Tp-Note_ yourself.


1. [Install Rust], e.g.

   ```sh
   curl https://sh.rustup.rs -sSf | sh

   sudo apt install build-essential    
   ```

   A modern Linux desktop usually ships the required shared
   libraries. Here is a list extracted form a Debian binary:

   ```sh
   ldd target/x86_64-unknown-linux-gnu/release/tpnote 
	    linux-vdso.so.1
	    libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1
	    librt.so.1 => /lib/x86_64-linux-gnu/librt.so.1
	    libpthread.so.0 => /lib/x86_64-linux-gnu/libpthread.so.0
	    libm.so.6 => /lib/x86_64-linux-gnu/libm.so.6
	    libdl.so.2 => /lib/x86_64-linux-gnu/libdl.so.2
	    libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6
	    /lib64/ld-linux-x86-64.so.2
   ```

2. Download, compile and install _Tp-Note_:

   **Building on Linux**

   ```sh
   cargo install tp-note
   sudo cp ~/.cargo/bin/tpnote /usr/local/bin
   # Copy icon
   sudo cp assets/tpnote.svg /usr/local/share/icons
   ```

   Unlike previous Linux versions (<= 1.19.13), Tp-Note displays errors
   and debug messages as notifications. This requires a Linux/BSD based
   desktop environment that follows the XDG specification, e.g. KDE,
   Gnome, XFCE, LXDC, Mate (and probably also most others).

   The use of notifications also removes former GTK dependencies. Anyway,
   if you prefer to see error messages on the console only, you can opt
   out notifications and message boxes. In this case all error messages
   are dumped on the console from where you started _Tp-Note_ into
   `stderr`:

   ```sh
   cargo install --no-default-features \
     --features read-clipboard,viewer,renderer,lang-detection tp-note
   sudo cp ~/.cargo/bin/tpnote /usr/local/bin
   ```

   **Recommended Linux console and server version**

   The full-featured version of _Tp-Note_ depends on GUI libraries like Xlib
   that might not be available on a headless system. Either download the Musl
   version [x86_64-unknown-linux-musl/release/tpnote] or compile _Tp-Note_
   yourself without default features:

   ```sh
   cargo install --no-default-features --features renderer,lang-detection tp-note
   sudo cp ~/.cargo/bin/tpnote /usr/local/bin
   ```

   If Tp-Note's binary size if of concern, omit the `lang-detection` feature
   in the `cargo` invocation above. The `lang-detection` feature causes 95% 
   of the final binary size because of its extensive language models.


   **Building on Windows and macOS**

   Build the full-featured version with:

       cargo install tp-note

   When building for Windows or macOS, it does not make sense to exclude the
   `message-box` feature, because - under Windows and macOS - it does not rely
   on the notification library. Instead, it uses direct OS-API calls for
   popping up alert boxes. As these calls have no footprint in binary size or
   speed, always keep the `message-box` feature compiled in.

   See also the user manual for a more detailed installation description.



## Cross compilation

Debian makes it easy to cross-compile for foreign architectures. Here
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

* Target Windows:
  
  ```sh
  rustup target add x86_64-pc-windows-gnu  
  sudo apt install binutils-mingw-w64 mingw-w64
  cargo build --target x86_64-pc-windows-gnu --release 
  ```  
  

This project follows [Semantic Versioning].



## About

Author:

* Jens Getreu

Copyright:

* Apache 2 license or MIT license.

[Tp-Note’s user manual]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Download Tp-Note]: https://blog.getreu.net/projects/tp-note/index.html#distribution
[Tp-Note - Minimalistic note taking: save and edit your clipboard content as a note file]: https://blog.getreu.net/projects/tp-note/
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
[tpnote.1.gz]: https://blog.getreu.net/projects/tp-note/_downloads/tpnote.1.gz
[tpnote.svg]: https://blog.getreu.net/projects/tp-note/images/tpnote.svg
[tpnote-latest-x86_64.msi]: https://blog.getreu.net/projects/tp-note/_downloads/wix/tpnote-latest-x86_64.msi
[Installation section]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html#installation
[Customization section]: https://blog.getreu.net/projects/tp-note/tpnote--manpage.html#customization
[Install Rust]: https://www.rust-lang.org/tools/install
[Semantic Versioning]: https://semver.org/

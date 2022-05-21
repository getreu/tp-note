---
title: "Tp-Note - Minimalistic note taking: save and edit your clipboard content as a note file"
author: Jens Getreu
filename_sync: false
---

[![Cargo](https://img.shields.io/crates/v/tp-note.svg)](
https://crates.io/crates/tp-note)
[![Documentation](https://docs.rs/tp-note/badge.svg)](
https://docs.rs/tp-note)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://gitlab.com/getreu/tp-note)

_Tp-Note_ is a note-taking-tool and a template system - freely available for
Windows, MacOS and Linux - that consistently synchronizes the note’s meta-data
with its filename. If you like to keep your notes next to your files and you
care about expressive filenames, then _Tp-Note_ might be the tool of your choice.
_Tp-Note_ collects various information about its environment
and the clipboard and stores them in variables. New notes are created by
filling these variables in predefined and customizable _Tera_-templates.
_TP-Note's_ default templates are written in Markdown and can be easily adapted
to any other markup language if needed. After creating a new note, _TP-Note_
launches the system file editor (or any other of your choice, e.g. _MarkText_
or _Typora_) and connects the default web browser to _Tp-Note_'s
internal Markdown/RestructuredText renderer and web server.

* Read more in [Tp-Note’s user manual]

* [Download Tp-Note]

* Project page: [Tp-Note - fast note-taking with templates and filename synchronization]


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

* Package compiled for Debian 10+ (Buster):

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

* Universal Linux binary (compiled with Debian 10 Buster):

    * [x86_64-unknown-linux-gnu/release/tpnote]

    * The following "musl" version is well suited for headless systems, as it
      does not require _GTK_ libraries to be installed.

      [x86_64-unknown-linux-musl/release/tpnote]

### Tp-Note NetBSD

* An official package is available on NetBSD and other "pkgsrc" supported platforms:

  To install Tp-Note on NetBSD, simply use the native package manager.

  ```sh
  pkgin install tp-note
  ```

### Other ressources

* Installable Unix man-page:

  - [tpnote.1.gz]

* Zipfile with all binaries and documentation:

  - [Tp-Note all]


## Installation

Depending on the availability of installer packages for your operating system,
the installation process is more or less automated. For Windows users the fully
automated installation package
[tpnote-latest-x86_64.msi]
is available. For more information, please consult the [Distribution section](#distribution)
above and the [Installation section]
in _Tp-Note_'s manual.


## Upgrading

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure. In order not to loose the changes you made in
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

       curl https://sh.rustup.rs -sSf | sh

2. Download, compile and install _Tp-Note_:

   **Building for Linux**

       sudo apt-get install -y xorg-dev libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev
       cargo install tp-note
       sudo cp ~/.cargo/bin/tpnote /usr/local/bin

   In case you experience compilation errors in dependent crates, replace
   `cargo install tp-note` with the following:

       cargo install --locked tp-note

   If - under Linux - you need to reduce the binary size and you can do without
   error message popup boxes (for example on a headless system), no GTK is
   required. In this case all error messages are dumped on the console from
   where you started _Tp-Note_ into `stderr`.

       cargo install --no-default-features --features read-clipboard,viewer,renderer tp-note
       sudo cp ~/.cargo/bin/tpnote /usr/local/bin

   **Recommended Linux console and server version**

   The full featured version of _Tp-Note_ depends on GUI libraries like GTK
   that might not be installable on a headless system. Either download the Musl
   version [x86_64-unknown-linux-musl/release/tpnote] or compile _Tp-Note_
   without default features:

       cargo install --no-default-features --features renderer tp-note
       sudo cp ~/.cargo/bin/tpnote /usr/local/bin

   **Building for Windows**

   Build the full featured version with:

       cargo install tp-note

   When building for Windows, it does not make sense to exclude the
   `message-box` feature, because - under Windows - it does not rely on the
   GTK library. Instead it uses direct Windows-API calls for popping up alert
   boxes. As these calls have no footprint in binary size or speed, always
   keep the `message-box` feature compiled in.

   See also the user manual for a more detailed installation description.



This project follows [Semantic Versioning].



## About

Author:

* Jens Getreu

Copyright:

* Apache 2 licence or MIT licence


[Tp-Note’s user manual]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Download Tp-Note]: https://blog.getreu.net/projects/tp-note/index.html#distribution
[Tp-Note - fast note-taking with templates and filename synchronization]: https://blog.getreu.net/projects/tp-note/
[Tp-Note - Most common use cases - YouTube]: https://www.youtube.com/watch?v=ODhPytPFtYY
[Tp-Note's project page]: https://blog.getreu.net/projects/tp-note/
[Tp-Note user manual - html]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Tp-Note user manual - pdf]: https://blog.getreu.net/_downloads/tpnote--manual.pdf
[Tp-Note manual page - html]: https://blog.getreu.net/projects/tp-note/tpnote--manpage.html
[Tp-Note manual page - pdf]: https://blog.getreu.net/_downloads/tpnote--manpage.pdf
[Blogposts about Tp-Note]: https://blog.getreu.net/tags/tp-note/
[Tp-Note's config module documentation]: https://blog.getreu.net/projects/tp-note/_downloads/doc/tpnote/config/
[Constants in API documentation]: https://blog.getreu.net/projects/tp-note/_downloads/doc/tpnote/config/index.html#constants
[Tp-Note on Gitlab]: https://gitlab.com/getreu/tp-note
[Tp-Note on Github (mirror)]: https://github.com/getreu/tp-note
[tpnote-latest-x86_64.msi]: https://blog.getreu.net/projects/tp-note/_downloads/wix/tpnote-latest-x86_64.msi
[VirusTotal]: https://www.virustotal.com/gui/home/upload
[x86_64-unknown-linux-gnu/debian/tpnote_latest_amd64.deb]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tpnote_latest_amd64.deb
[Releases - getreu/tp-note]: https://github.com/getreu/tp-note/releases
[x86_64-pc-windows-gnu/release/tpnote.exe]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tpnote.exe
[x86_64-unknown-linux-gnu/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tpnote
[x86_64-unknown-linux-musl/release/tpnote]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tpnote
[tpnote.1.gz]: https://blog.getreu.net/projects/tp-note/_downloads/tpnote.1.gz
[Tp-Note all]: https://blog.getreu.net/_downloads/tpnote.zip
[tpnote-latest-x86_64.msi]: https://blog.getreu.net/projects/tp-note/_downloads/wix/tpnote-latest-x86_64.msi
[Installation section]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html#installation
[Customization section]: https://blog.getreu.net/projects/tp-note/tpnote--manpage.html#customization
[Install Rust]: https://www.rust-lang.org/tools/install
[Semantic Versioning]: https://semver.org/

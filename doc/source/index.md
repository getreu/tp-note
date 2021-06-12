---
title: Tp-Note - Minimalist note taking: save and edit your clipboard content as a note file
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

  [tp-note-latest-x86_64.msi]

  As this early version of the Windows installer is not signed yet, Windows
  will show the error message “Windows protected your PC”. As a work-around,
  when you click on the link “More info”, a ”Run anyway” button will appear
  allowing you to continue the installation process. In general, regardless
  of where a program comes from, I recommend checking every installable
  file with [VirusTotal]

### Tp-Note Debian/Ubuntu installer package

* Package compiled for Debian 10+ (Buster):

  [x86_64-unknown-linux-gnu/debian/tp-note_latest_amd64.deb]

### Various binaries for Windows, MacOS and Linux

* Binaries for Ubuntu-Linux 18.04, Windows, MacOS:

    1. Open: [Releases - getreu/tp-note]

    2. Open the latest release.

    3. Open *assets*.

    4. Download the packed executable for your operating system.

    5. Installation: see below.

* Executable for Windows:

    * [x86_64-pc-windows-gnu/release/tp-note.exe]

* Universal Linux binary (compiled with Debian 10 Buster):

    * [x86_64-unknown-linux-gnu/release/tp-note]

    * The following "musl" version is well suited for headless systems, as it
      does not require _GTK_ libraries to be installed.

      [x86_64-unknown-linux-musl/release/tp-note]



* Installable Unix man-page:

  - [tp-note.1.gz]

* Zipfile with all binaries and documentation:

  - [tp-note all]


## Installation

Depending on the availability of installer packages for your operating system,
the installation process is more or less automated. For Windows users the fully
automated installation package
[tp-note-latest-x86_64.msi]
is available. For more information, please consult the [Distribution section](#distribution)
above and the [Installation section]
in _Tp-Note_'s manual.


## Upgrading

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure. In order not to loose the changes you made in
this file, the installer does not replace it automatically with a new version.
Instead, _Tp-Note_ renames the old configuration file and prompts:

    NOTE: configuration file version mismatch:
    ---
    Configuration file version: '1.7.2'
    Tp-Note version: '1.7.4'
    Minimum required configuration file version: '1.7.4'

    For now, Tp-Note backs up the existing configuration
    file and next time it starts, it will create a new one
    with default values.

or

    NOTE: unable to load, parse or write the configuration file
    ---
    Reason:
            Bad TOML data: missing field `extension_default` at line 1 column 1!

    Note: this error may occur after upgrading Tp-Note due
    to some incompatible configuration file changes.

    For now, Tp-Note backs up the existing configuration
    file and next time it starts, it will create a new one
    with default values.

As the above error messages suggests, all you need to do is
to restart _Tp-Note_ in order to create a new updated configuration file.


## Building

If the above precompiled binaries do not suite you, you can
compile _Tp-Note_ yourself.


1. [Install Rust], e.g.

       curl https://sh.rustup.rs -sSf | sh

2. Download, compile and install _Tp-Note_:

   **Building for Linux**

       sudo apt-get install -y xorg-dev libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev
       cargo install tp-note
       sudo cp ~/.cargo/bin/tp-note /usr/local/bin

   In case you experience compilation errors in dependent crates, replace
   `cargo install tp-note` with the following:

       cargo install --locked tp-note

   If - under Linux - you need to reduce the binary size and you can do without
   error message popup boxes (for example on a headless system), no GTK is
   required. In this case all error messages are dumped on the console from
   where you started _Tp-Note_ into `stderr`.

       cargo install --no-default-features --features read-clipboard,viewer,renderer tp-note
       sudo cp ~/.cargo/bin/tp-note /usr/local/bin

   **Minimal Linux console version without dependencies on GUI libraries**

   The full featured version of _Tp-Note_ depends on GUI libraries like GTK
   that might not be installed on a headless system. Either download the Musl
   version [x86_64-unknown-linux-musl/release/tp-note] or compile _Tp-Note_
   without default features:

       cargo install --no-default-features tp-note
       sudo cp ~/.cargo/bin/tp-note /usr/local/bin

   Note, that even though this no-renderer version is deprived of it's 
   markup renderer, limited rendered HTML export is still available (see command
   line option `--export`). This way you can comfortably follow hyperlinks in
   your note files with any text based web browser, e.g. `lynx`.

   **Recommended Linux console and server version**

   The same as the above console version (without GUI libraries), but with
   additional Markdown, ReStructuredText etc. renderer compiled in.

       cargo install --no-default-features --features renderer tp-note
       sudo cp ~/.cargo/bin/tp-note /usr/local/bin

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


[Tp-Note’s user manual]: /projects/tp-note/tp-note--manual.html
[Download Tp-Note]: /projects/tp-note/index.html#distribution
[Tp-Note - fast note-taking with templates and filename synchronization]: /projects/tp-note/
[Tp-Note - Most common use cases - YouTube]: https://www.youtube.com/watch?v=ODhPytPFtYY
[Tp-Note's project page]: /projects/tp-note/
[Tp-Note user manual - html]: /projects/tp-note/tp-note--manual.html
[Tp-Note user manual - pdf]: /_downloads/tp-note--manual.pdf
[Tp-Note manual page - html]: /projects/tp-note/tp-note--manpage.html
[Tp-Note manual page - pdf]: /_downloads/tp-note--manpage.pdf
[Blogposts about Tp-Note]: /tags/tp-note/
[Tp-Note's config module documentation]: /projects/tp-note/_downloads/doc/tp_note/config/
[Constants in API documentation]: /projects/tp-note/_downloads/doc/tp_note/config/index.html#constants
[Tp-Note on Gitlab]: https://gitlab.com/getreu/tp-note
[Tp-Note on Github (mirror)]: https://github.com/getreu/tp-note
[tp-note-latest-x86_64.msi]: /projects/tp-note/_downloads/wix/tp-note-latest-x86_64.msi
[VirusTotal]: https://www.virustotal.com/gui/home/upload
[x86_64-unknown-linux-gnu/debian/tp-note_latest_amd64.deb]: /projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tp-note_latest_amd64.deb
[Releases - getreu/tp-note]: https://github.com/getreu/tp-note/releases
[x86_64-pc-windows-gnu/release/tp-note.exe]: /projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tp-note.exe
[x86_64-unknown-linux-gnu/release/tp-note]: /projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note
[x86_64-unknown-linux-musl/release/tp-note]: /projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tp-note
[tp-note.1.gz]: /projects/tp-note/_downloads/tp-note.1.gz
[tp-note all]: /_downloads/tp-note.zip
[tp-note-latest-x86_64.msi]: /projects/tp-note/_downloads/wix/tp-note-latest-x86_64.msi
[Installation section]: /projects/tp-note/tp-note--manual.html#installation
[Install Rust]: https://www.rust-lang.org/tools/install
[Semantic Versioning]: https://semver.org/

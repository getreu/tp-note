---
title: Tp-Note - fast note-taking with templates and filename synchronization
---

[![Cargo](https://img.shields.io/crates/v/tp-note.svg)](
https://crates.io/crates/tp-note)
[![Documentation](https://docs.rs/tp-note/badge.svg)](
https://docs.rs/tp-note)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/getreu/tp-note)

_Tp-Note_ is a note-taking-tool and a template system - freely available for
Windows, MacOS and Linux - that consistently synchronizes the note’s meta-data
with its filename. If you like to keep your notes next to your files and you
care about expressive filenames, then _Tp-Note_ might be the tool of your choice.
_Tp-Note_ collects various information about its environment
and the clipboard and stores them in variables. New notes are created by
filling these variables in predefined and customizable _Tera_-templates.
_TP-Note's_ default templates are written in Markdown and can be easily adapted
to any other markup language if needed. By default, _TP-Note_ launches the
system file-editor (or any other of your choice, e.g. _MarkText_ or _Typora_)
after creating a new note.

* Read more in [Tp-Note’s user manual](https://blog.getreu.net/projects/tp-note/tp-note--manual.html)

* [Download Tp-Note](https://blog.getreu.net/projects/tp-note/index.html#distribution)

* Project page: [Tp-Note - fast note-taking with templates and filename synchronization](https://blog.getreu.net/projects/tp-note/)


---


## Documentation

User documentation:

* Project page:

  [Tp-Note's project page](https://blog.getreu.net/projects/tp-note/), which
  you are reading right now, lists where you can download _Tp-Note_ and gives
  an overview of _Tp-Note_'s resources and documentation.

* User manual:

  The user manual showcases how to best use use _Tp-Note_ and how to integrate it
  with you file manager.

  [Tp-Note user manual - html](https://blog.getreu.net/projects/tp-note/tp-note--manual.html)

  [Tp-Note user manual - pdf](https://blog.getreu.net/_downloads/tp-note--manual.pdf)

* Unix man-page:

  The Unix man-page is _Tp-Note_'s technical reference. Here you learn how to customize
  _Tp-Note_'s templates and how to change its default settings.

  [Tp-Note manual page - html](https://blog.getreu.net/projects/tp-note/tp-note--manpage.html)

  [Tp-Note manual page - pdf](https://blog.getreu.net/_downloads/tp-note--manpage.pdf)

* [Blogposts about Tp-Note](https://blog.getreu.net/tags/tp-note/)

Developer documentation:

* API documentation

  _Tp-Note_'s program code documentation targets mainly software developers.
  The advanced user may consult the [Tp-Note's config module documentation](https://blog.getreu.net/projects/tp-note/_downloads/doc/tp_note/config/)
  which explains the default templates and setting. Many of them can be
  customized through _Tp-Note_'s configuration file.

  [API documentation](https://blog.getreu.net/projects/tp-note/_downloads/doc/tp_note/)



## Source code

Repository:

* [Tp-Note on Github](https://github.com/getreu/tp-note)


## Distribution

### Tp-Note Microsoft Windows installer package

* Installer package for Windows:

  [tp-note-1.7.6-x86_64.msi](https://blog.getreu.net/projects/tp-note/_downloads/wix/tp-note-1.7.6-x86_64.msi)

  As this early version of the Windows installer is not signed yet, Windows
  will show the error message “Windows protected your PC”. As a work-around,
  when you click on the link “More info”, a ”Run anyway” button will appear
  allowing you to continue the installation process. In general, regardless
  of where a program comes from, I recommend checking every installable
  file with [VirusTotal](https://www.virustotal.com/gui/home/upload).

### Tp-Note Debian/Ubuntu installer package

* Package compiled for Debian 10+ (Buster):

  [x86_64-unknown-linux-gnu/debian/tp-note_1.7.6_amd64.deb](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tp-note_1.7.6_amd64.deb)

### Various binaries for Windows, MacOS and Linux

* Binaries for Ubuntu-Linux 18.04, Windows, MacOS:

    1. Open: [Releases - getreu/tp-note](https://github.com/getreu/tp-note/releases)

    2. Open the latest release.

    3. Open *assets*.

    4. Download the packed executable for your operating system.

    5. Installation: see below.

* Executable for Windows:

    * [x86_64-pc-windows-gnu/release/tp-note.exe](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tp-note.exe)

* Universal Linux binary (compiled with Debian 10 Buster):

    * [x86_64-unknown-linux-gnu/release/tp-note](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note)

    * The following "musl" version is well suited for headless systems, as it
      does not require _GTK_ libraries to be installed.

      [x86_64-unknown-linux-musl/release/tp-note](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tp-note)



* Installable Unix man-page:

  - [tp-note.1.gz](https://blog.getreu.net/projects/tp-note/_downloads/tp-note.1.gz)

* Zipfile with all binaries and documentation:

  - [tp-note all](https://blog.getreu.net/_downloads/tp-note.zip)


## Installation

Depending on the availability of installer packages for your operating system,
the installation process is more or less automated. For Windows users the fully
automated installation package
[tp-note-1.7.6-x86_64.msi](https://blog.getreu.net/projects/tp-note/_downloads/wix/tp-note-1.7.6-x86_64.msi)
is available. For more information, please consult the [Distribution section](#distribution)
above and the [Installation
section](https://blog.getreu.net/projects/tp-note/tp-note--manual.html#installation)
in _Tp-Note_'s manual.


## Upgrading

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure. In order not to loose the changes you made in
this file, the installer does not replace it automatically with a new version.
Instead, _Tp-Note_ renames the erroneous configuration file and prompts:

    NOTE: configuration file version mismatch:
    ---
    Configuration file version: '1.7.2'
    Tp-Note version: '1.7.4'
    Minimum required configuration file version: '1.7.4'

    For now, Tp-Note backs up the existing configuration
    file and next time it starts, it will create a new one
    with default values.
    ---
    Additional technical details:
    *    Command line parameters:
    tp-note
    *    Configuration file path:
    /home/getreu/.config/tp-note/tp-note.tomll

or

    NOTE: unable to load, parse or write the configuration file
    ---
    Note: this error may occur after upgrading Tp-Note due
    to some incompatible configuration file changes.

    For now, Tp-Note backs up the existing configuration
    file and next time it starts, it will create a new one
    with default values.
    ---
    Additional technical details:
    *    Command line parameters:
    tp-note
    *    Configuration file path:
    /home/getreu/.config/tp-note/tp-note.toml

As the above error messages suggests, all you need to do is
to restart _Tp-Note_ in order to create a new updated configuration file.


## Building

If the above precompiled binaries do not suite you, you can
compile _Tp-Note_ yourself.


1. [Install Rust](https://www.rust-lang.org/tools/install), e.g.

       curl https://sh.rustup.rs -sSf | sh

2. Download, compile and install _Tp-Note_:

       sudo apt-get install -y xorg-dev libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev
       cargo install tp-note
       sudo cp ~/.cargo/bin/tp-note /usr/local/bin

   See also the user manual for a detailed installation description.

This project follows [Semantic Versioning](https://semver.org/).



## About

Author:

* Jens Getreu

Copyright:

* Apache 2 licence or MIT licence

<!--
Build status:

* ![status](https://travis-ci.org/getreu/tp-note.svg?branch=master)
-->

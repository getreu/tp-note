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
with its filename. _Tp-Note_ collects various information about its environment
and the clipboard and stores them in variables. New notes are created by
filling these variables in predefined and customizable _Tera_-templates.  _TP-Note's_
default templates are written in Markdown and can be easily adapted to any
other markup language if needed. By default _TP-Note_ launches the system
file-editor (or any other of your choice, e.g. Typora) after creating a new note.

* Read more in [Tp-Note’s user manual](/projects/tp-note/tp-note--manual.html)

* [Download Tp-Note](/projects/tp-note/index.html#distribution)

* Project page: [Tp-Note - fast note-taking with templates and filename synchronization](/projects/tp-note/)


---


## Documentation

User documentation:

* User manual:

  [Tp-Note user manual - html](/projects/tp-note/tp-note--manual.html)

  [Tp-Note user manual - pdf](/_downloads/tp-note--manual.pdf)

* Unix man-page:

  [Tp-Note manual page - html](/projects/tp-note/tp-note--manpage.html)

  [Tp-Note manual page - pdf](/_downloads/tp-note--manpage.pdf)

* [Blogposts about Tp-Note](/tags/tp-note/)

Developer documentation:

* [API documentation](/projects/tp-note/_downloads/doc/tp_note/)



## Source code

Repository:

* [Tp-Note on Github](https://github.com/getreu/tp-note)



## Distribution

### Tp-Note Microsoft Windows installer package

* Installer package for Windows:

  [tp-note-1.5.2-x86_64.msi](/projects/tp-note/_downloads/wix/tp-note-1.5.2-x86_64.msi)

### Tp-Note Debian/Ubuntu installer package

* Package compiled for Debian 10+ (Buster):

  [x86_64-unknown-linux-gnu/debian/tp-note_1.5.2_amd64.deb](/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tp-note_1.5.2_amd64.deb)

### Various binaries for Windows, MacOS and Linux

* Binaries for Ubuntu-Linux 18.04, Windows, MacOS:
  Debian binaries)

    1. Open: [Releases - getreu/tp-note](https://github.com/getreu/tp-note/releases)

    2. Open the latest release.

    3. Open *assets*.

    4. Download the packed executable for your operating system.

    5. Installation: see below.

* Executable for Windows:

    [x86_64-pc-windows-gnu/release/tp-note.exe](/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tp-note.exe)

* Binary for Debian 10 Buster:

    [x86_64-unknown-linux-gnu/release/tp-note](/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note)
    <!--
    [x86_64-unknown-linux-musl/release/tp-note](/projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tp-note)
    -->

* Installable Unix man-page:

  - [tp-note.1.gz](/projects/tp-note/_downloads/tp-note.1.gz)

* Zipfile with all binaries and documentation:

  - [tp-note all](/_downloads/tp-note.zip)


## Installation

Depending on the availability of installer packages for your operating system,
the installation process is more or less automated. For Windows users the fully
automated installation package
[tp-note-1.5.2-x86_64.msi](/projects/tp-note/_downloads/wix/tp-note-1.5.2-x86_64.msi)
is available. Others may consult the
[Installation](/projects/tp-note/tp-note--manual.html#installation)
section in _Tp-Note_'s manual for more information.


## Upgrading

When you install a new version of _Tp-Note_, please delete the old configuration
file, that is automatically written in

* Linux: `~/.config/tp-note/tp-note.toml`

* Windows: `C:\Users\<LOGIN>\AppData\Roaming\tp-note\config\tp-note.toml`

The reason is, that the structure of the configuration file might change from
version to version.  For example, new configuration variables might be added:
When _Tp-Note_ starts, it reads the old configuration and will complain about a
malformed structure. I recommend deleting the old configuration file, even when
there is no error message: new template-variables might activate new features,
that will only be available, when _Tp-Note_ starts with a fresh default
template.

There is no need to say, that in case you modified the configuration file,
you should backup it before deleting.


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

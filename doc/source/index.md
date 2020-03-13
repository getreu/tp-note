---
title: tp-note - fast note-taking with templates and filename synchronization
---

_tp-note_ is a note-taking-tool and a template system, that consistently
synchronizes the note's meta-data with its filename. _tp-note_ collects
various information about its environment and the clipboard and stores them
in variables. New notes are created by filling these variables in predefined
and customizable _Tera_-templates.


## Documentation

User documentation

* User manual

  [tp-note user manual - html](/projects/tp-note/tp-note--manual.html)\
  [tp-note user manual - pdf](/_downloads/tp-note--manual.pdf)

* Unix man-page

  [tp-note manual page - html](/projects/tp-note/tp-note--manpage.html)\
  [tp-note manual page - pdf](/_downloads/tp-note--manpage.pdf)

* [Blogposts about Tp-Note](/tags/tp-note/)

Developer documentation

* [API documentation](/projects/tp-note/_downloads/doc/tp_note/)


## Source code

Repository

* [tp-note on Github](https://github.com/getreu/tp-note)

* [tp-note on Gitlab](https://gitlab.com/getreu/tp-note)


## Distribution

* Binaries and packages

  - Executable for Windows

    [x86_64-pc-windows-gnu/release/tp-note.exe](/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tp-note.exe)

  - Binary for Linux

    [x86_64-unknown-linux-gnu/release/tp-note](/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note) \
    [x86_64-unknown-linux-musl/release/tp-note](/projects/tp-note/_downloads/x86_64-unknown-linux-musl/release/tp-note)

  - Package for Debian and Ubuntu

    [x86_64-unknown-linux-gnu/debian/tp-note_0.9.5_amd64.deb](/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/tp-note_0.9.5_amd64.deb)

* Installable Unix man-page

  - [tp-note.1.gz](/projects/tp-note/_downloads/tp-note.1.gz)

* Zipfile with all binaries and documentation

  - [tp-note all](/_downloads/tp-note.zip)


## Building and installing

1. Install *Rust* with [rustup](https://www.rustup.rs/):

       curl https://sh.rustup.rs -sSf | sh

   The fastest way

       cargo install tp-note

   It it works for you you are done. Otherwise continue the next step.

2. Download [tp-note](#tp-note):

       git clone git@github.com:getreu/tp-note.git

3. Build

   Enter the *tp-note* directory where the file `Cargo.toml`
   resides:

       cd tp-note


   Then execute:

       cargo build --release
       ./doc/make--all

4. Install

   a.  Linux:

           # install binary
           sudo cp target/release/tp-note /usr/local/bin/

           # install man-page
           sudo cp man/tp-note.1.gz /usr/local/man/man1/
           sudo dpkg-reconfigure man-db   # e.g. Debian, Ubuntu

   b.  Windows

       Copy the binary `target/release/tp-note.exe` on your desktop.

   See the user manual for a detailed installation description.






## About

Author

* Jens Getreu

Copyright

* Apache 2 licence or MIT licence

Build status

* ![status](https://travis-ci.org/getreu/tp-note.svg?branch=master)  

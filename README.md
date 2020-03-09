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

  [tp-note user manual - html](https://blog.getreu.net/projects/tp-note/tp-note--manual.html)\
  [tp-note user manual - pdf](https://blog.getreu.net/_downloads/tp-note--manual.pdf)

* Unix man-page

  [tp-note manual page - html](https://blog.getreu.net/projects/tp-note/tp-note--manpage.html)\
  [tp-note manual page - pdf](https://blog.getreu.net/_downloads/tp-note--manpage.pdf)

* [Blogposts about Tp-Note](https://blog.getreu.net/tags/tp-note/)

Developer documentation

* [API documentation](https://blog.getreu.net/projects/tp-note/_downloads/doc/tp_note/)


## Source code

Repository

* [tp-note on Github](https://github.com/getreu/tp-note)

* [tp-note on Gitlab](https://gitlab.com/getreu/tp-note)


## Distribution

Executables are available for Linux and Windows (iOS in progress).
There you can also find packages for Debian and Unbuntu.

Binaries and packages

* [Download](https://blog.getreu.net/projects/tp-note/_downloads/)

Inallable Unix man-page

* [tp-note.1.gz](https://blog.getreu.net/projects/tp-note/_downloads/tp-note.1.gz)

Zifile with all builds and documentation bundled together

* [tp-note all](https://blog.getreu.net/_downloads/tp-note.zip)


## Building and installing

1. Install *Rust* with [rustup](https://www.rustup.rs/):

       curl https://sh.rustup.rs -sSf | sh

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

% TP-NOTE(1) Version 0.9 | Tp-Note documentation

<!--
Date: 2020-03-08
Version: 0.9
-->

# NAME

_tp-note_ - fast note taking with templates and filename synchronization.


# SYNOPSIS

    tp-note [-V] [-b] [-d] [-v] [-c <config-file>] [<path>]



# DESCRIPTION

_tp-note_ is a note-taking-tool and a template system, that consistently
synchronizes the note's meta-data with its filename. _tp-note_ collects
various information about its environment and the clipboard and stores them
in variables. New notes are created by filling these variables in predefined
and customizable _Tera_-templates. In case '`<path>`' points to an existing
'_tp-note_'-file, the note's meta-data is analysed and, if necessary, its
filename is modified. For all other file types, _tp-note_ creates a new note
that annotates the file '`<path>`' points to. If '`<path>`' is a directory (or,
when omitted the current working directory), a new note is created in that
directory. After creation, _tp-note_ launches an external editor of your
choice. Although the note's structure follows '`pandoc`'-conventions, it is not
tied to any specific markup language.

After the user finished editing, _tp-note_ analyses eventual changes in the
notes meta-data and renames, if necessary, the file, so that its meta-data
and filename are in sync again.



# OPERATION MODES

_tp-note_ operates in 4 different modes, depending on its
commend-line-arguments and the clipboard state. Each mode is usually
associated with one content-template and one filename-template.


## New note based on clipboard data

When '`<path>`' is a directory and the clipboard is not empty, the clipboard's
content is stored in the variable '`{{ clipboard }}`'. In addition, if the
content is a hyperlink in markdown format, the hyperlink's name is stored in
'`{{ clipboard_linkname }}`', and its url in '`{{ clipboard_linkurl }}`'. The
new note is then created with the '`tmpl_clipboard_content`' and the
'`tmpl_clipboard_filename`' templates.  Finally, the newly created file is
opened with an external editor. When the editor closes, _tp-note_ synchronizes
with the template '`tmpl_sync_filename`' the note's meta-data and its filename.

> Note: this operation mode also empties the clipboard (configurable feature).

### The clipboard contains a string

Example: While launching _tp-note_ the clipboard contains the string: "`Who
Moved My Cheese?`" and `<path>` is a directory.

``` sh
> tp-note "./03-Favorite Readings/"
```

or

``` sh
> cd "./03-Favorite Readings/"
> tp-note
```

This creates the document:

    "./03-Favorite Readings/20200306-Who Moved My Cheese--Note.md"

with the content:

```yaml
---
title:      "Who Moved My Cheese?"
subtitle:   "Note"
author:     "getreu"
date:       "March  6, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.0"
---
```

We see from the above example, that the default template created a document
with some meta-data, but without content. However, if desired or necessary it
is possible to adapt the template in _tp-note_'s configuration file. Please
note, that the filename is a simplified and sanitized concatenation of: date,
title and subtitle.

### The clipboard contains a markdown link

Example: `<path>` is a directory, the clipboard is not empty and it contains
the string: "`[The Rust Book](https://doc.rust-lang.org/book/)`".

```sh
> tp-note './doc/Lecture 1'
```

This creates the following document:


    "./doc/Lecture 1/20200307-The Rust Book--Notes.md


```yaml
---
title:    "The Rust Book"
subtitle: "Notes"
author:   "getreu"
date:     "March  7, 2020"
lang:     "en_GB.UTF-8"
revision: "1.0"
---

[The Rust Book](https://doc.rust-lang.org/book/)
```

## New note with empty clipboard

In case the clipboard is empty while starting, another set of templates is
used to create the new note: '`tmpl_new_content`' and '`tmpl_new_filename`'.
By default, the new note's title is the parent's directory name. The newly
created file is then opened with an external editor, allowing to change the
proposed title and to add other content. When the editor closes, _tp-note_
synchronizes the note's meta-data and its filename. This operation is
performed with the '`tmpl_sync_filename`' template.


Example: the clipboard is empty and `<path>` is a directory (or empty):

``` sh
> tp-note "./03-Favorite Readings/"
```

or

``` sh
> cd "./03-Favorite Readings"
> tp-note
```

creates the document:

    "./03-Favorite Readings/20200306-Favorite Readings--Note.md"

with the content:

``` yaml
---
title:      "Favorite Readings"
subtitle:   "Note"
author:     "getreu"
date:       "March  6, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.0"
---
```

## New note based on a non-tp-note-file

When '`<path>`' points to a file whose extension is other than '`.md`', a new
note is created with a similar filename and a reference to the original file
copied into the note. The logic of this is implemented in the templates:
'`tmpl_annotate_content`' and '`tmpl_annotate_filename`'. Once the file is
created, it is opened with an external editor. After editing the file, it
will be - if necessary - renamed to be in sync with the note's meta-data.

Example:

``` sh
> tp-note "Classic Shell Scripting.pdf"
```

creates the note:

    "Classic Shell Scripting--Note.md"

with the content:

``` yaml
---
title:      "Classic Shell Scripting"
subtitle:   "Note"
author:     "getreu"
date:       "March  6, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.1"
---

[Classic Shell Scripting.pdf](Classic Shell Scripting.pdf)
```


## Editing notes

If not invoked with '`--batch`', _tp-note_ launches an external editor after
creating a new note. This also happens when '`<path>`' points to an existing
'`.md`'-file.

Example: edit the note from the previous example:

``` bash
> cd "./03-Favorite Readings"
> tp-note 20200306-Favorite Readings--Note.md
```


## Automatic filename synchronization before and after editing

Before launching the editor and after closing it, _tp-note_ synchronizes the
filename with the note's metadata. When the user changes the metadata of a
note, _tp-note_ will replicate that change in the note's filename. As a
result, *all your note's filenames always correspond to their metadata*,
which allows you to find your notes back quickly.

Example:

``` sh
> tp-note "20200306-Favorite Readings--Note.md"
```

_tp-note_ opens the file in an editor. Now the note-taker decides to update
the title in the note's YAML metadata section from
'`title: "Favorite Readings"`' to '`title: "Introduction to bookkeeping"`'.
After closing the editor the filename is automatically updated to:

    "20200306-Introduction to bookkeeping--Note.md"


Note: the sort-tag '`20200306`' has not changed. The filename synchronization
mechanism never does. (See below for more details about filename synchronization).



# OPTIONS

**-b**, **\--batch**

;   Create a new file or rename the file to stay synchronized
    with its meta-data, but does not launch the external editor.

**-c** *CF*, **\--config**=*CF*

:   Load the alternative config file *CF* instead of the default one.

**-d**, **\--debug**

: Print additional log-messages on console. On Windows, the output must be
  redirected into a file to see it, e.g. open the command-prompt and type:

    tp-note.exe -d >debug.txt 2>&1

**-v**, **\--view**

:   Launch the external editor in read-only-mode if possible.

**-V**, **\--version**

:   Print _tp-note_'s version and exit.



# THE NOTE'S DOCUMENT STRUCTURE

A _tp-note_ file starts with a BOM (byte order mark) and is Utf-8 encoded. As
newline, either the Unix standard '`\n`' or the Windows standard '`\r\n`' is
accepted. _tp-note_ writes out newlines according the operating system it runs
on.

_tp-note_ is designed to be compatible with '`Pandoc`'s and '`Rmarkdown`s
document structure as shown in the figure below.

``` yaml
---
<YAML-front matter>
---
<document-body>
```

The YAML front matter starts at the beginning of the document with '`---`'
and ends with '`...`' or '`---`'. Note that according to the YAML standard,
string-literals are always encoded as JSON strings.

There is no restriction about the markup language used in the note's text body.
However, the default templates assume that markdown and the file extension
'`.md`' is used. Both can be changed easily by adapting _tp-note_'s
configuration file.



# META-DATA FILENAME SYNCHRONIZATION

Consider the following _tp-note_-file:

    20151208-Make this world a better place--Suggestions.md

The filename has 4 parts:

    {{ sort-tag }}-{{ title }}--{{ subtitle }}.{{ extension }}

A so called _sort-tag_ is a numerical prefix at the beginning of the
filename. It is used to order files and notes in the filesystem. Besides
numerical digits, a _sort-tag_ can be any combination of
`0123456789-_`[^sort-tag] and is usually used as

* *chronological sort-tag*

        20140211-Reminder.doc
        20151208-Manual.pdf

* or as a *sequence number sort-tag*.

        02-Invoices
        08-Tax documents
        09_02-Notes

When _tp-note_ creates a new note, it prepends automatically a *chronological
sort-tag* of today. The '`{{ title }}`' part is usually derived from the
parent directory name omitting its own *sort-tag*.

[^sort-tag]: The characters '`_`' and '`-`' are not considered to be
part of the *sort-tag* when they appear in first or last position.

A note's filename is in sync with its meta-data, when the following is true
(slightly simplified, see the configuration file for the complete definition):

> filename on disk without *sort-tag* == '`-{{ title }}--{{ subtitle }}.md`'
  ^[The variables '`{{ title }}`' and '`{{ subtitle }}`' reflect the values in
  the note's metadata.]

Consider the following document with the filename:

    20200306-My file.md

and the content:

``` yaml
---
title:      "1. The Beginning"
subtitle:   "Note"
author:     "getreu"
date:       "March  6, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.1"
...
```

As "`-My file.md`" is not equal to "`-'1. The Beginning--Note.md`",
_tp-note_ will rename the file to "`20200306-'1. The Beginning--Note.md`".

If the filename had been "`05_02-My file.md`", it would rename it to
"`05_02-'1. The Beginning--Note.md`".

Note: _tp-note_ never changes a sort-tag, but might changes the rest of the filename!



# CUSTOMIZATION

_tp-note_'s configuration file resides typically in
'`~/.config/tp-note/tp-note.toml`' on Unix or in
'`C:\Users\<LOGIN>\AppData\Roaming\tp-note\config\tp-note.toml>`' on Windows.
When _tp-note_ starts, it tries to find its configuration file. If it fails,
it writes a default configuration file. _tp-note_ is best customized by
starting it once, and then modifying its default configuration.

The configuration file is encoded according to the TOML-standard. Variables
starting with '`tmpl_*`' are _Tera-Template_-strings (see:
<https://tera.netlify.com/docs/#templates>).

_tp-note_ captures and stores its environment in _Tera-variables_. For example,
the variable '`{{ dirname }}`' is initialized with the document's parent
directory. The variable '`{{ clipboard }}`' contains the content of the
clipboard. To learn more about variables, launch _tp-note_ with the '`--debug`'
option and observe what information it captures from its environment.

_tp-note_ distinguishes two template types: content-templates '`tmpl_*_content`'
used to create the note's content (front-matter and body) and filename-templates
'`tmpl_*_filename`' used to calculate the note's filename.

## Content-template conventions

Strings in content-templates are JSON encoded. Therefor all variable used in
this template must pass an additional '`json_encode()`'-filter. For
example, the variable '`{{ dirname }}`' must be written as
'`{{ dirname | json_encode() }}`' instead.

## Filename-template convention

The same applies to filename-template-variables: in this context we must
guarantee, that the variable contains only filesystem friendly characters.
For this purpose _tp-note_ provides all variables in 3 different flavours:

* The original variable '`<var>`', e.g. '`title`'. No filter is applied.

* A filesystem friendly version '`<var>__path`', e.g. '`title__path`'.
  (Note the double underscore '`_`'). In this variant forbidden characters like
  '`?`' are omitted or replaced by '`_`' or space.

* Another filesystem friendly version '`<var>__alphapath`' similar to the above,
  with one exception: when a string starts with a number character '`0`-`9`' the
  string is prepended with `'`.
  For example: "`1. The Show Begins`" becomes "`'1. The Show Begins`".

In filename-templates only variables, whose name end with '`*__path`' or
'`*__alphapath`' should be used.

## Choose your own external editor

The Tera-template variables '`editor_args`' and '`viewer_args`' define a list of
external editors to be launched for editing. '`viewer_args`' is used
when _tp-note_ is invoked with '`--view`' in viewer mode.

The list contains well-known editor names and its command-line arguments.
_tp-note_ tries to launch every editor from the beginning of the list until
it finds an installed editor.

To use your own editor, just place it at the top of the list. To work
properly make sure, that your editor does not fork! Many editors provide an
option not to fork: for example the '`code`'-editor can be launched with the
'`--wait`' option.

## Change the markup language

_tp-note_ is markup language agnostic, however the default templates define
_Markdown_ as default markup language. To change this, just edit the following
2 templates:

1. Change to variable '`note_extension='md'`' to e.g.
   '`note_extension='rst'`'

2. The last line in template '`tmpl_clipboard_content`' defines a hyperlink in
   Markdown format. Change the link format according to your markup language
   convention.



# RESOURCES

_tp-note_ it hosted on:

* Github: <https://github.com/getreu/tp-note> and on

* Gitlab (mirror): <https://gitlab.com/getreu/tp-note>.



# COPYING

Copyright (C) 2016-2020 Jens Getreu

Licensed under either of

* Apache Licence, Version 2.0 (\[LICENSE-APACHE\](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT licence (\[LICENSE-MIT\](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
licence, shall be dual licensed as above, without any additional terms
or conditions. Licensed under the Apache Licence, Version 2.0 (the
\"Licence\"); you may not use this file except in compliance with the
Licence.


# AUTHORS

Jens Getreu <getreu@web.de>

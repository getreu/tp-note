% TP-NOTE(1) Version 1.11.4 | Tp-Note documentation


# NAME

_Tp-Note_ - save and edit your clipboard content as a note file.


# SYNOPSIS

    tp-note [-b] [-c <FILE>] [-d] [-e] [-p <NUM>]
            [-n] [-v] [-V] [-x <DIR>|''|'-'] [<DIR>|<FILE>]



# DESCRIPTION

_Tp-Note_ is a note-taking-tool and a template system, that consistently
synchronizes the note's meta-data with its filename. _Tp-Note_ collects
various information about its environment and the clipboard and stores them
in variables. New notes are created by filling these variables in predefined
and customizable _Tera_-templates. In case '`<path>`' points to an existing
'_Tp-Note_'-file, the note's meta-data is analysed and, if necessary, its
filename is modified. For all other file types, _Tp-Note_ creates a new note
that annotates the file '`<path>`' points to. If '`<path>`' is a directory (or,
when omitted the current working directory), a new note is created in that
directory. After creation, _Tp-Note_ launches an external text editor of your
choice. At the same time the system's default web browser is started and
connected to _Tp-Note_'s internal web server. This server watches and
renders the edited note file and generates a live preview.

After the user finished editing, _Tp-Note_ analyses eventual changes in the
notes meta-data and renames, if necessary, the file, so that its meta-data and
filename are in sync again. Finally, the resulting path is printed to
'`stdout`', log and error messages are dumped to '`stderr`'.

This document is Tp-Note's technical reference. More information
can be found in [Tp-Note's user manual] and at [Tp-Note's project page].

[Tp-Note's user manual]: https://blog.getreu.net/projects/tp-note/tp-note--manual.html
[Tp-Note's project page]: https://blog.getreu.net/projects/tp-note/



# OPERATION MODES

_Tp-Note_ operates in 4 different modes, depending on its
commend-line-arguments and the clipboard state. Each mode is usually
associated with one content-template and one filename-template.


## New note without clipboard

In case the clipboard is empty while starting, the new note is created
with the templates: '`tmpl_new_content`' and '`tmpl_new_filename`'.  By
default, the new note's title is the parent's directory name. The newly created
file is then opened with an external text editor, allowing to change the
proposed title and to add other content. When the text editor closes, _Tp-Note_
synchronizes the note's meta-data and its filename. This operation is performed
with the '`tmpl_sync_filename`' template.

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
---
```


## New note based on clipboard data

When '`<path>`' is a directory and the clipboard is not empty, the clipboard's
content is stored in the variable '`{{ clipboard }}`'. In addition, if the
content contains an hyperlink in Markdown format, the hyperlink's name can be
accessed with '`{{ clipboard | linkname }}`', its URL with
'`{{ clipboard | linktarget }}`' and its title with
'`{{ clipboard | linktitle }}`'. The new note is then created with the
'`tmpl_clipboard_content`' and the '`tmpl_clipboard_filename`' templates.
Finally, the newly created note file is opened again with some external text
editor. When the user closes the text editor, _Tp-Note_ synchronizes the
note's meta-data and its filename with the template '`tmpl_sync_filename`'.

> Note: this operation mode also empties the clipboard (configurable feature).

**Clipboard simulation**

When no mouse and clipboard is available, the clipboard feature can be
simulated by feeding the clipboard data into `stdin`:

```sh
> echo "[The Rust Book](https://doc.rust-lang.org/book/)" | tp-note
```

_Tp-Note_ behaves here as if the clipboard contained the string:
"`[The Rust Book](https://doc.rust-lang.org/book/)`".



### The clipboard contains a string

Example: While launching _Tp-Note_ the clipboard contains the string:
"`Who Moved My Cheese?\n\nChapter 2`" and `<path>` is a directory.

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
title:      "Who Moved My Cheese"
subtitle:   "Note"
author:     "getreu"
date:       "2020-09-11"
lang:       "en_GB.UTF-8"
---

Who Moved My Cheese?

Chapter 2
```

We see from the above example, how the '`tmpl_clipboard_content`' content
template extracts the first line of the clipboards content and inserts it into
the header's '`title:`' field. Then, it copies the entire clipboard content into
the body of the document.  However, if desired or necessary, it is possible to
modify all templates in _Tp-Note_'s configuration file. Note, that not only the
note's content is created with a template, but also its filename: The
'`tmpl_clipboard_filename`' filename template concatenates the current date,
the note's title and subtitle.


### The clipboard contains a hyperlink

Example: `<path>` is a directory, the clipboard is not empty and it contains
the string: "`I recommend:\n[The Rust Book](https://doc.rust-lang.org/book/)`".

```sh
> tp-note './doc/Lecture 1'
```

This creates the following document:

    ./doc/Lecture 1/20200911-The Rust Book--Notes.md

```yaml
---
title:      "The Rust Book"
subtitle:   "URL"
author:     "getreu"
date:       "2020-09-11"
lang:       "en_GB.UTF-8"
---

I recommend:
[The Rust Book](https://doc.rust-lang.org/book/))
```

When analyzing the clipboard's content, _Tp-Note_ searches for hyperlinks in
Markdown, ReStructuredText, Asciidoc and HTML format. When successful, the
content template uses the link text of the first hyperlink found as document
title.


### The clipboard contains a string with a YAML header

Example: `<path>` is a directory, the clipboard is not empty and it contains
the string: "`---\ntitle: Todo\nfile_ext: mdtxt\n---\n\nnothing`".

```sh
> tp-note
```

This creates the note: '`20200911-Todo.mdtxt`' with the following
content:

```yaml
---
title:      "Todo"
subtitle:   ""
author:     "getreu"
date:       "2020-09-11"
lang:       "en_GB.UTF-8"
file_ext:   "mdtxt"
---

nothing
```

Technically, the creation of the new note is performed
using the YAML header variables: '`{{ fm_title }}`',
'`{{ fm_subtitle }}`', '`{{ fm_author }}`', '`{{ fm_date }}`',
'`{{ fm_lang }}`', '`{{ fm_sort_tag }}`' and
'`{{ fm_file_ext }}`' which are evaluated with the
'`tmpl_copy_content`' and the '`tmpl_copy_filename`' templates.

Note, that the same result can also be achieved without any clipboard by tying
in a terminal:

```sh
> echo -e "---\ntitle: Todo\nfile_ext: mdtxt\n---\n\nnothing" | tp-note
```

Furthermore, this operation mode is very handy with pipes in general, as shows the
following example: it downloads some webpage, converts it to Markdown and copies
the result into a _Tp-Note_ file. The procedure preserves the webpage's title in the
note's title:

```sh
curl 'https://blog.getreu.net' | pandoc --standalone -f html -t markdown_strict+yaml_metadata_block | tp-note
```

creates the note file '`20200910-Jens Getreu's blog.md`' with the webpage's
content converted to Markdown:

```yaml
---
title:      "Jens Getreu's blog"
subtitle:   ""
author:     "getreu"
date:       "2020-09-11"
lang:       "en"
---


<a href="/" class="logo">Jens Getreu's blog</a>

-   [Home](https://blog.getreu.net)
-   [Categories](https://blog.getreu.net/categories)
```


### Use Tp-Note in shell scripts

To save some typing while using the above pattern, you can create a script with:

```
> sudo nano /usr/local/bin/download
```

Insert the following content:

```sh
#!/bin/sh
curl "$1" | pandoc --standalone -f html -t markdown_strict+yaml_metadata_block | tp-note
```

and make it executable:

```
> sudo chmod a+x /usr/local/bin/download
```

To execute the script type:

```shell
> download 'https://blog.getreu.net'
```



## New note annotating some non-Tp-Note file

When '`<path>`' points to an existing file, whose file-extension is other than
'`.md`', a new note is created with a similar filename and a reference to the
original file is copied into the new note's body. If the clipboard contains
some text, it is appended there also. The logic of this is implemented in the
templates: '`tmpl_annotate_content`' and '`tmpl_annotate_filename`'. Once the
file is created, it is opened with an external text editor. After editing the
file, it will be - if necessary - renamed to be in sync with the note's
meta-data.

Example:

``` sh
> tp-note "Classic Shell Scripting.pdf"
```

creates the note:

    "Classic Shell Scripting.pdf--Note.md"

with the content:

``` yaml
---
title:      "Classic Shell Scripting.pdf"
subtitle:   "Note"
author:     "getreu"
date:       "March  6, 2020"
lang:       "en_GB.UTF-8"
---

[Classic Shell Scripting.pdf](Classic Shell Scripting.pdf)
```

The configuration file variable '`note_file_extensions`' lists all file
extensions that _Tp-Note_ recognizes and opens as own file types. Others are
treated as described above.

This so called _annotation_ mode can also be used with the clipboard: when it
is not empty, its data is appended to the note's body.


## Editing notes

If not invoked with '`--batch`', _Tp-Note_ launches an external text editor
after creating a new note. This also happens when '`<path>`' points to an
existing '`.md`'-file.

Example: edit the note from the previous example:

``` bash
> cd "./03-Favorite Readings"
> tp-note 20200306-Favorite Readings--Note.md
```


## Automatic filename synchronization before and after editing

Before launching the text editor and after closing it, _Tp-Note_ synchronizes
the filename with the note's metadata. When the user changes the metadata of a
note, _Tp-Note_ will replicate that change in the note's filename. As a result,
*all your note's filenames always correspond to their metadata*, which allows
you to find your notes back quickly.

Example:

``` sh
> tp-note "20200306-Favorite Readings--Note.md"
```

The way how _Tp-Note_ synchronizes the note's metadata and filename is defined
in the template '`tmpl_sync_filename`'.

Once _Tp-Note_ opens the file in an text editor, the note-taker may decide updating
the title in the note's YAML metadata section from '`title: "Favorite
Readings"`' to '`title: "Introduction to bookkeeping"`'.  After closing the text
editor the filename is automatically updated too and looks like:

    "20200306-Introduction to bookkeeping--Note.md"

Note: the sort-tag '`20200306`' has not changed. The filename synchronization
mechanism by default never does. (See below for more details about filename
synchronization).



# OPTIONS

**-b**, **\--batch**

:   Do not launch the external text editor or viewer. All other operations
    are available and are executed in the same way. In batch mode, error
    messages are dumped on the console only and no alert window pops up.

:   _Tp-Note_ ignores the clipboard when run in batch mode with '`--batch`'.
    Instead, if available, it reads the `stdin` stream as if the data came
    from the clipboard.

**-c** *CF*, **\--config**=*CF*

:   Load the alternative config file *CF* instead of the default one.

**-d**, **\--debug**

:   Print additional log-messages on console. It shows the available template
    variables, the templates used and the rendered result of the substitution.
    This option particularly useful for debugging new templates. On Windows,
    the output must be redirected into a file to see it. To do so open the
    command-prompt and type:

        tp-note.exe -d >debug.txt 2>&1

**-e**, **\--edit**

:   Edit only mode: opens the external file editor, but not the file
    viewer. This disables _Tp-Note_'s internal file watcher and web server,
    unless '`-v`' is given. Another way to permanently disable the web server
    is to set the configuration variable '`viewer_enable=false`'.

**-p**, **\--port**=*PORT*

:   Set server port the web browser connects to, to the specified value.
    If not given, a random free port is chosen automatically.

**-n**, **\--no-sync**

:   Whenever _Tp-Note_ opens a note file, it synchronizes its YAML-metadata
    with its filename. '`--no-sync`' disables the synchronization. This is
    mainly useful in scripts for testing '`.md`'-files. When
    '`tp-note -n -b <FILE>`' returns the code '`0`', the note file has a valid
    YAML header with at least one '`title:`' field. In addition, when
    '`tp-note -n -b -x - <FILE>`' returns the code '`0`', the note's body was
    rendered without error.

**-v**, **\--view**

:   View only mode: do not open the external text editor. This flag instructs
    _Tp-Note_ to start an internal file watcher and web server and connect
    the system's default web browser to view the note file and to observe live
    file modifications. This flag has precedence over the configuration
    variable '`viewer_enable=false`'.

**-V**, **\--version**

:   Print _Tp-Note_'s version and exit. When combined with '`--debug`',
    additional technical details are printed.

**-x** *DIRECTORY*, **\--export**=*DIRECTORY*

:   Prints the note as HTML-rendition into _DIRECTORY_. '`-x -`' prints to
    _stdout_. The empty string, e.g. '`--export= `' or '`-x ""`', defaults to
    the directory where the note file resides. No external file editor or
    viewer is launched. Can be combined with '`--batch`' to avoid pop-up
    error alert windows.



# THE NOTE'S DOCUMENT STRUCTURE

A _Tp-Note_-note file is always UTF-8 encoded. As newline, either the Unix
standard '`\n`' or the Windows standard '`\r\n`' is accepted. _Tp-Note_ writes
out newlines according the operating system it runs on.

_Tp-Note_ is designed to be compatible with '`Pandoc`'s and '`RMarkdown`s
document structure as shown in the figure below.

``` yaml
---
<YAML-front matter>
---
<document-body>
```

The YAML front matter starts at the beginning of the document with '`---`'
and ends with '`...`' or '`---`'. Note that according to the YAML standard,
string-literals are always encoded as JSON strings. By convention, a valid
note-file has at least one YAML field named '`title:`' (the name of this
compulsory field is defined by the '`tmpl_compulsory_field_content`'
variable in the configuration file and can be changed there).

There is no restriction about the markup language used in the note's text body.
However, the default templates assume that Markdown and the file extension
'`.md`' is used. Both can be changed easily by adapting _Tp-Note_'s
configuration file.



# META-DATA FILENAME SYNCHRONIZATION

Consider the following _Tp-Note_-file:

    20151208-Make this world a better place--Suggestions.md

The filename has 4 parts:

    {{ fm_sort_tag }}-{{ fm_title }}--{{ fm_subtitle }}.{{ fm_file_ext }}

A so called _sort-tag_ is a numerical prefix at the beginning of the
filename. It is used to order files and notes in the file system. Besides
numerical digits, a _sort-tag_ can be any combination of
`0123456789-_`[^sort-tag] and is usually used as

* *chronological sort-tag*

        20140211-Reminder.doc
        20151208-Manual.pdf

* or as a *sequence number sort-tag*.

        02-Invoices
        08-Tax documents
        09_02-Notes

When _Tp-Note_ creates a new note, it prepends automatically a *chronological
sort-tag* of today. The '`{{ fm_title }}`' part is usually derived from the
parent directory name omitting its own *sort-tag*.

[^sort-tag]: The characters '`_`' and '`-`' are considered to be
part of the *sort-tag* even when they appear in last position.

A note's filename is in sync with its meta-data, when the following is true
(slightly simplified, see the configuration file for the complete definition):

> filename on disk without *sort-tag* == '`-{{ fm_title }}--{{ fm_subtitle }}.md`'
  ^[The variables '`{{ fm_title }}`' and '`{{ fm_subtitle }}`' reflect the values in
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
---
```

As "`-My file.md`" is not equal to "`-'1. The Beginning--Note.md`",
_Tp-Note_ will rename the file to "`20200306-'1. The Beginning--Note.md`".
If the filename had been "`05_02-My file.md`", it would rename it to
"`05_02-'1. The Beginning--Note.md`".

Note: When the YAML front matter does not contain the optional '`sort_tag`'
variable, _Tp-Note_ will never change a sort-tag. Nevertheless, it might
change the rest of the filename!

The reason why by default _Tp-Note_ does not change sort-tags is, that they
define their order in the file listing. In general this order is independent of
the notes content. The simplest way to organize the sort-tags of your files is
by renaming them directly in your file-system. Nevertheless, in some cases you
might want to have full control over the whole filename through the note's YAML
front matter. For example, if — for some reason — you have changed the
document's date in the front matter and you want to change the chronological
sort tag in one go. In order to overwrite the note's sort-tag on disk, you can
add a '`sort_tag`' variable to its front matter:

``` yaml
---
title:      "1. The Beginning"
date:       "March  7, 2020"
sort_tag:   "20200307-"
---
```

When _Tp-Note_ synchronizes the note's metadata with its filename, it will also
change the sort-tag from '`20200306-`' to '`20200307-`'. The resulting filename
becomes "`20200307-'1. The Beginning--Note.md`".

The '`sort_tag`' variable also becomes handy, when you want to create one single
note without any sort-tag:

``` yaml
---
title:      "1. The Beginning"
sort_tag:   ""
---
```

In the same way, how it is possible to pin the sort-tag of the note from within
the note's meta-data, you can also change the file extension by adding the
optional '`file_ext`' variable into the note's front matter:

``` yaml
---
title:      "1. The Beginning"
file_ext:   "rst"
---
```

This will change the file extension from '`.md`' to '`.rst`. The resulting
filename becomes "`20200307-'1. The Beginning--Note.rst`".

Important: '`rst`' must be one of the registered file extensions
listed in the '`note_file_extensions_*`' variables in Tp-Note's configuration
file. If needed you can add more extensions there. If the new filename extension
is not listed in one of theses variables, _Tp-Note_ will not be able to
recognize the note file as such and will not open it in the external file editor
and viewer.

Note: When a '`sort_tag`' variable is defined in the note's YAML header, you
should not change the sort-tag string in the note's file name manually by
renaming the file, as your change will be overwritten next time you open the
note with _Tp-Note_.  However, you can switch back to _Tp-Note_'s default
behaviour any time by deleting the '`sort_tag`' line in the note's metadata.
The same applies to the '`file_ext`' variable.



# CUSTOMIZATION

_Tp-Note_'s configuration file resides typically in
'`~/.config/tp-note/tp-note.toml`' on Unix or in
'`C:\Users\<LOGIN>\AppData\Roaming\tp-note\config\tp-note.toml>`' on Windows.
When _Tp-Note_ starts, it tries to find its configuration file. If it fails,
it writes a default configuration file. _Tp-Note_ is best customized by
starting it once, and then modifying its default configuration.

The configuration file is encoded according to the TOML-standard. Variables
starting with '`tmpl_*`' are _Tera-Template_-strings (see:
<https://tera.netlify.com/docs/#templates>).

_Tp-Note_ captures and stores its environment in _Tera-variables_. For example,
the variable '`{{ path }}`' is initialized with the note's target
directory. The variable '`{{ clipboard }}`' contains the content of the
clipboard. To learn more about variables, launch _Tp-Note_ with the '`--debug`'
option and observe what information it captures from its environment.

## Template variables

All [Tera template variables and functions](https://tera.netlify.com/docs/#templates)
can be used within _Tp-Note_. For example '`{{ get_env(name='LANG') }}'`
gives you access to the '`LANG`' environment variable.

In addition _Tp-Note_ defines the following variables:

* '`{{ file }}`' is the canonicalized fully qualified file name corresponding
  to _Tp-Note_'s positional parameter '`<path>`'. If '`<path>`' points to a
  directory the content of this variable is identical to '`{{ path }}`'.

* '`{{ path }}`' is same as above but without filename and extension.

* '`{{ clipboard }}`' is the complete clipboard text.  In case the clipboard's
  content starts with a YAML header, the latter does not appear in this
  variable.

* '`{{ clipboard_header }}`' is the YAML section of the clipboard data, if
  one exists. Otherwise: empty string.

* '`{{ stdin }}`' is the complete text content originating form the input
  stream '`stdin`'. This stream can replace the clipboard when it is not
  available.  In case the input stream's content starts with a YAML header,
  the latter does not appear in this variable.

* '`{{ stdin_header }}`' is the YAML section of the input stream, if
  one exists. Otherwise: empty string.

* '`{{ extension_default }}`' is the default extension for new notes
  (can be changed in the configuration file),

* '`{{ username }}`' is the content of the first non-empty environment
  variable: `LOGNAME`, `USER` or `USERNAME`.

The following '`{{ fm_* }}`' variables are typically generated, _after_ a
content template was filled in with data: For example a field named '`title:`'
in the content template '`tmpl_new_content`' will generate the variable
'`fm_title`' which can then be used in the corresponding '`tmpl_new_filename`'
filename template. '`{{ fm_* }}`' variables are generated dynamically. This
means, a YAmL front matter variable '`foo:`' in a note will generate a
'`{{ fm_foo }}`' template variable. On the other hand, a missing '`foo:`'
will cause '`{{ fm_foo }}`' to be undefined.

Please note that '`{{ fm_* }}`' variables are available in all filename
templates and in the '`tmpl_copy_content`' content template only.

* '`{{ fm_title }}`' is the '`title:`' as indicated in the YAML front matter of
  the note.

* '`{{ fm_subtitle }}`' is the '`subtitle:`' as indicated in the YAML front
  matter of the note.

* '`{{ fm_author }}`' is the '`author:`' as indicated in the YAML front matter
  of the note.

* '`{{ fm_lang }}`' is the '`lang:`' as indicated in the YAML front matter of
  the note.

* '`{{ fm_file_ext }}`' holds the value of the optional YAML header variable
  '`file_ext:`' (e.g. '`file_ext: "rst"`').

* '`{{ fm_sort_tag }}`': The sort variable as defined in the YAML front matter of
  this note (e.g. '`sort_tag: "20200312-"`').

* '`{{ fm_all }}`': is a collection (map) of all defined '`{{ fm_* }}`'
  variables.  It is used in the '`tmpl_copy_content`' template, typically in a
  loop like:

  ```yaml
  {% for key, value in fm_all %}{{ key }}: {{ value | json_encode }}
  {% endfor %}
  ```

Important: there is no guarantee, that any of the above '`{{ fm_* }}`'
variables is defined! Depending on the last content template result, certain
variables might be undefined. Please take into consideration, that a defined
variable might contain the empty string '`""`'.


## Template filters

In addition to _Tera_'s [built-in
filters](https://tera.netlify.app/docs/#built-in-filters), _Tp-Note_ comes with
some additional filters, e.g.: '`tag`', '`trim_tag`', '`stem`', '`cut`', '`heading`',
'`linkname`', '`linktarget`', '`linktitle`' and '`ext`'.

A filter is always used together with a variable. Here some examples:

* '`{{ file | tag }}`' is the sort-tag (numerical filename prefix) of the
  current note on disk, e.g. '`01-23_9-`' or '`20191022-`'. Useful in content
  templates, for example to create new notes based on a path with a filename
  (e.g.  '`tmpl_annotate_content`').

* '`{{ file | stem }}`' is the note's filename without sort-tag, copy-counter
   and extension.

* '`{{ file | ext }}`' is the note's filename extension without
  dot (period), e.g. '`md`' od '`mdtxt`'.

* '`{{ file | ext | prepend_dot }}`' is the note's filename extension with
  dot (period), e.g. '`.md`' od '`.mdtxt`'.

* '`{{ path | trim_tag }}`' the last element of `path`, which is the parent
   directory's name of the note on disk. If present, the sort-tag is skipped
   and only the following characters are retained.

* '`{{ clipboard | cut }}`' is the first 200 bytes from the clipboard.

* '`{{ clipboard | heading }}`' is the clipboard's content until end of the first
  sentence ending, or the first newline.

* '`{{ clipboard | linkname }}`' is the name of the first Markdown or
  reStructuredText formatted link in the clipboard.

* '`{{ clipboard | linktarget }}`' is the URL of the first Markdown or
  reStruncturedText formatted link in the clipboard.

* '`{{ clipboard | linktitle }}`' is the title of the first Markdown or
  reStruncturedText formatted link in the clipboard.

* '`{{ file | ext }}`' is the filename extension of the current note on disk.

* '`{{ username | json_encode }}`' is the username Json encoded. All YAML
  front matter must be Json encoded, so this filter should be the last in all
  lines of the front matter section.

* '`{{ subtitle | sanit }}`' the note's subtitle as defined in its front-matter,
  sanitized in a filesystem friendly form. Special characters are omitted or
  replaced by '`-`' and '`_`'.

* '`{{ title | sanit(alpha=true) }}`' the note's title as defined in its
  front-matter.  Same as above, but strings starting with a number are
  prepended by an apostrophe.

* '`{{ fm_all | remove(var='fm_title') }}`' represents a collection (map) of
  all '`fm_*`' variables, exclusive of the variable '`fm_title`'.


## Content-template conventions

_Tp-Note_ distinguishes two template types: content-templates '`tmpl_*_content`'
used to create the note's content (front-matter and body) and filename-templates
'`tmpl_*_filename`' used to calculate the note's filename.

Strings in the YAML front matter of content-templates are JSON encoded.
Therefore, all variables used in the front matter must pass an additional
'`json_encode()`'-filter. For example, the variable '`{{ path | stem }}`'
becomes '`{{ path | stem | json_encode() }}`' or just
'`{{ path | stem | json_encode }}`'.


## Filename-template convention

The same applies to filename-template-variables: in this context we must
guarantee, that the variable contains only file system friendly characters.
For this purpose _Tp-Note_ provides the additional Tera filters '`sanit`' and
'`sanit(alpha=true)`'.

* The '`sanit()`' filter transforms a string in a file system friendly from. This
  is done by replacing forbidden characters like '`?`' and '`\\`' with '`_`'
  or space. This filter can be used with any variables, but is most useful with
  filename-templates. For example, in the '`tmpl_sync_filename`'
  template, we find the expression '`{{ subtitle | sanit }}`'.

* '`sanit(alpha=true)`' is similar to the above, with one exception: when a string
  starts with a digit '`0`-`9`', the whole string is prepended with `'`.
  For example: "`1 The Show Begins`" becomes "`'1 The Show Begins`".
  This filter should always be applied to the first variable assembling the new
  filename, e.g. '`{{ title | sanit(alpha=true )}`'. This way, it is always
  possible to distinguish the sort-tag from the actual filename.

In filename-templates most variables must pass either the '`sanit`' or the
'`sanit(alpha=true)`' filter. Exception to this rule are the sort-tag variables
'`{{ file | tag }}`' and '`{{ path | tag }}`'. As these are guaranteed to contain only
the filesystem-friendly characters: '`0..9-_`', no additional filtering is
required. In addition, a '`sanit()`'-filter would needlessly restrict the value
range of '`{{ file | tag }}`' and '`{{ path | tag }}`': a sort tag usually ends with a
'`-`', a character that the '`sanit`'-filter screens out when it appears in
leading or trailing position. For this reason no '`sanit`'-filter is allowed
with '`{{ file | tag }}`' and '`{{ path | tag }}`'.


## Register your own external text editor

The configuration file variables '`editor_args`' and '`viewer_args`' define a
list of external text editors to be launched for editing. '`viewer_args`' is
used when _Tp-Note_ is invoked with '`--view`' in viewer mode.  The list
contains well-known text editor names and its command-line arguments.
_Tp-Note_ tries to launch every text editor from the beginning of the list
until it finds an installed text editor. When _Tp-Note_ is started on a Linux
console, an alternative file editor list used: '`editor_console_args`' and
'`viewer_console_args`'. Here you can register file editors that do not
require a graphical environment, e.g. '`vim`' or '`nano`'.

In order to use your own text editor, just place it at the top of the list. To
make this work properly, make sure, that your text editor does not fork! You
can check this when you launch the text editor from the command-line: if the
prompt returns immediately, then it forks the process. In contrast, it is Ok
when the prompt only comes back at the moment when the text editor is closed.
Many text editors provide an option not to fork: for example the
_VScode_-editor can be launched with the '`--wait`' option and `vim` with
`vim --nofork`. However, _Tp-Note_ also works with forking text editors. Then,
the only drawback is, that _Tp-Note_ can not synchronize the filename with the
note's metadata when the user has finished editing. It will still happen, but
only when the user opens the note again with _Tp-Note_.

Remark for the advanced console user: It is also possible to launch a different
editor without changing the configuration file:

```shell
> FILE=$(tp-note --batch); vi "$FILE"; tp-note --batch "$FILE"
```

Whereby '`FILE=$(tp-note --batch)`' creates the note file, '`vi "$FILE"`' opens the
'`vi`'-file editor and '`tp-note --batch "$FILE"`' synchronizes the filename.


## Register a Flatpak Markdown editor

[Flathub for Linux] is a cross-platform application repository that works well
with _Tp-Note_.  To showcase an example, we will add a _Tp-Note_ launcher for
the _Mark Text_ Markdown file editor available as [Flatpak package]. Before
installing, make sure that you have [setup Flatpack] correctly. Then install
the application with:

[Flathub for Linux]: https://www.flathub.org/home
[Flatpak package]: https://www.flathub.org/apps/details/com.github.marktext.marktext
[setup Flatpack]: https://flatpak.org/setup/

    > sudo flatpak install flathub com.github.marktext.marktext

To test, run _Mark Text_ from the command-line:

    > flatpak run com.github.marktext.marktext

Then open _Tp-Note_'s configuration file `tp-note.toml` and search for the
'`editor_args`' variable, quoted shortened below:

```toml
editor_args = [
    ['typora'],
    [
    'code',
    '-w',
    '-n',
],
#...
]
```

The structure of this variable is a list of lists. Every item in the outer list
corresponds to one entire command line launching a different file editor, here
_Typora_ and _VSCode_.  When launching, _Tp-Note_ searches through this list
until it finds an installed file editor on the system.

In this example, we register the _Mark Text_ editor at the first place in this
list, by inserting '`['flatpak', 'run', 'com.github.marktext.marktext'],`:


```toml
editor_args = [
    [
    'flatpak',
    'run',
    'com.github.marktext.marktext',
],
    ['typora'],
    [
    'code',
    '-w',
    '-n',
],
#...
]
```

Save the modified configuration file.  Next time you launch _Tp-Note_, the
_Mark Text_-editor will open with your note.


## Change the default markup language

_Tp-Note_ identifies the note's markup language by its file extension and
renders the content accordingly (see '`note_file_extension_*`' variables).
This ensures interoperability between authors using different markup
languages. Although _Tp-Note_ is flexible in opening existing note files, new
notes are always created in the same markup language, which is by default
_Markdown_. How to change this is shown in the following section.

### Change the way how new notes are created

_Tp-Note_'s core function is a template system and as such it depends
very little on the used markup language. The default templates are
designed in a way that they contain almost no markup specific code. There
is one little exception though. The following configuration variables
affect the way new notes are created:

1. Change the default file extension for new notes from:

       extension_default='md'

   to:

       extension_default='rst'

2. Replace the following line in the template '`tmpl_clipboard_content`'
   that defines a hyperlink in Markdown format:

       [{{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }}](<{{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }}>)

   with the following line encoded in _RestructuredText_:

       `<{{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }}>`_

As a result, all future notes are created as '`*.rst`' files.

### Change the markup language for one specific note only

You can change the Markup language of a specific note by adding the variable
'`file_ext:`' to its YAML header. For example, for _ReStructuredText_ add:

```yaml
---
title:    "some note"
file_ext: "rst"
---
```

The above change only applies to the current note.


## Customize the built-in note viewer

### Change the way how note files are rendered for viewing

Besides its core function, _Tp-Note_ comes with several built-in markup
renderer and viewer, allowing to work with different markup languages at the
same time. The configuration file variables '`note_file_extension_*`' determine
which markup renderer is used for which note file extension. Depending on the
markup language, this feature is more or less advanced and complete: _Markdown_
(cf. '`note_file_extension_md`') is best supported and feature complete: It
complies with the _Common Mark_ specification. The _ReStructuredText_ renderer
(cf.  '`note_file_extension_rst`') is quit new and still in experimental state.
For all other supported markup languages _Tp-Note_ provides a built-in markup
source text viewer (cf.  '`note_file_extension_txt`') that shows the note as
typed (without markup), but renders all hyperlinks to make them clickable.  In
case none of the above rendition engines suit you, it is possible to disable
the viewer feature selectively for some particular note file extensions: just
place these extensions in the '`note_file_extension_no_viewer`' variable. If
you wish to disable the viewer feature overall, set the variable
`viewer_enabled = false`.

### Change the HTML rendition template

After the markup rendition process, _Tp-Note_'s built-in viewer generates its
final HTML rendition through the customizable HTML templates
'`viewer_rendition_tmpl`' and '`viewer_error_tmpl`'. The following code
example taken from '`viewer_rendition_tmpl`' illustrates the available variables:

```html
viewer_rendition_tmpl = '''<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
  </head>
  <body>
  <pre class="noteHeader">{{ fm_all_yaml }}</pre>
  <hr>
  <div class="noteBody">{{ noteBody }}</div>
  <script>{{ noteJS }}</script>
</body>
</html>
'''
```

Specifically:

* '`{{ fm_* }}`' are the deserialized header variables. All content
  template variables and filters are available. See section _Template
  variables_ above.

* '`{{ fm_all_yaml }}`' is the raw UTF-8 copy of the header. Not to be
  confounded with the dictionary variable '`{{ fm_all }}`'.

* '`{{ noteBody }}`' is the note's body as HTML rendition.

* '`{{ noteJS }}`' is the Java-Script browser code for
  live updates.

Alternatively, the header enclosed by '`<pre>...</pre>`' can also be rendered
as a table:

```html
  <table>
    <tr><th>title:</th><th>{{ fm_title }}</th> </tr>
    <tr><th>subtitle:</th><th>{{ fm_subtitle | default(value='') }}</th></tr>
  {% for k, v in fm_all| remove(var='fm_title')| remove(var='fm_subtitle') %}
    <tr><th>{{ k }}:</th><th>{{ v }}</th></tr>
  {% endfor %}
  </table>
```

The error page template '`viewer_error_tmpl`' (see below) does not provide '`fm_*`'
variables, because of possible header syntax errors. Instead, the variable
'`{{ noteError }}`' contains the error message as raw UTF-8 and the variable
'`{{ noteErrorContent }}`' the HTML rendition of the text source with
clickable hyperlinks:

```html
viewer_error_tmpl = '''<!DOCTYPE html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<title>Syntax error</title>
</head>
<body>
<h3>Syntax error</h3>
<p> in note file: <pre>{{ file }}</pre><p>
<hr>
<pre class=\"noteError\">{{ noteError }}</pre>
<hr>
{{ noteErrorContent }}
<script>{{ noteJS }}</script>
</body>
</html>
'''
```

### Choose your favorite web browser as note viewer

Once the note is rendered into HTML, _Tp-Note_'s internal HTTP server connects
to a random port at the `localhost` interface where the rendition is served to
be viewed with a web browser. _Tp-Note_'s configuration file contains a list
'`browser_args`' with common web browsers and their usual location on disk.
This list is executed top down until a web browser is found and launched. If
you want to view your notes with a different web browser, simply modify the
'`browser_args`' list and put your favorite web browser on top.

In case none of the listed browsers can be found, _Tp-Note_ switches into a
fall back mode with limited functionality, where it tries to open the system's
default web browser. A disadvantage is, that in fall back mode _Tp-Note_ is not
able to detect when the user closes the web browser. This might lead to
situations, where _Tp-Note_'s internal HTTP server shuts down to early. 
In order to check if _Tp-Note_ finds the selected web browser properly, start 
_Tp-Note_ with '`--debug`'.


## Customize the built-in HTML exporter

Customizing _Tp-Note_'s HTML export function works the same way than
customizing the built-in viewer. There are some slight differences though:
The role of the '`viewer_rendition_tmpl`' template - discussed above - is
taken over by the '`exporter_rendition_tmpl`' template. In this template the
same _Tera_ variables are available, except '`{{ noteJS }}`' which does not
make sense in this context. As the exporter prints possible rendition error
messages on the console, there is no equivalent to the '`viewer_error_tmpl`'
template.



# SECURITY AND PRIVACY CONSIDERATIONS

As discussed above, _Tp-Note_'s built-in viewer sets up an HTTP server on the
'`localhost`' interface with a random port number. This HTTP server runs as
long as the as long as the launched web browser window is open. It should be
remembered, that the HTTP server not only exposes the rendered note, but also
some other (image) files in the same directory (and all subdirectories) where
the note file resides. For security reasons symbolic links to files outside the
note's directory are not followed. Furthermore, _Tp-Note_'s built-in HTTP
server only serves files that are referenced in the note document and whose
file extensions are registered with the '`viewer_served_mime_type`'
configuration file variable. As _Tp-Note_'s built-in viewer binds to the
'`localhost`' interface, the exposed files are in principle accessible to all
processes running on the computer. As long as only one user is logged into the
computer at a given time, no privacy concern is raised: any potential note
reader must be logged in, in order to access the `localhost` HTTP server.

This is why on systems where multiple users are logged in at the same time, it
is recommended to disable _Tp-Note_'s viewer feature by setting the
configuration file variable '`viewer_enabled = false`'. Alternatively, you can
also compile _Tp-Note_ without the '`viewer`' feature. Note, that even with the
viewer feature disabled, one can still render the note manually with the
'`--export`' option.

**Summary**: As long as _Tp-Note_'s built-in note viewer is running, the note
file and all its referenced (image) files are exposed to all users logged into
the computer at that given time. This concerns only local users, _Tp-Note_
never exposes any information to the network.


# EXIT STATUS

Normally the exit status is '`0`' when the note file was processed without
error or '`1`' otherwise. If _Tp-Note_ can not read or write its
configuration file, the exit status is '`5`'.



# RESOURCES

_Tp-Note_ it hosted on:

* Gitlab: <https://gitlab.com/getreu/tp-note>.

* Github (mirror): <https://github.com/getreu/tp-note> and on



# COPYING

Copyright (C) 2016-2021 Jens Getreu

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

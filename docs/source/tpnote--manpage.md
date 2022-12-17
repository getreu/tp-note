% TP-NOTE(1) Version 1.19.7 | Tp-Note documentation

---
title:      "tpnote"
subtitle:   "manpage"
author:     "Jens Getreu"
date:       "2022-11-08"
orig_name:  "tpnote--manpage.md"
sort_tag:   ""
lang:       "en-US"
---


# NAME

_Tp-Note_ - save and edit your clipboard content as a note file.



# SYNOPSIS

    tpnote [-a ] [-b] [-c <FILE>] [-d] [-e] [-p <NUM>]
            [-n] [-v] [-V] [-x <DIR>|''|'-'] [<DIR>|<FILE>]



# DESCRIPTION

Tp-Note is a note-taking tool and a template system, that synchronizes the
note's metadata with its filename. Tp-Note analyses its environment and the
clipboard content and stores the result in variables. New notes are
created by filling these variables in predefined and customizable
_Tera_-templates. In case the first positional parameter '`<FILE>`'
points to an existing Tp-Note file, the note's metadata is parsed
and, if necessary, its filename is adjusted. For all other file types,
Tp-Note creates a new note in the same directory annotating the file. If
the positional parameter '`<DIR>`' points to an existing directory (or,
when omitted, the current working directory), a new note is created in
that directory. After creation, Tp-Note launches the systems file editor.
Although the configurable default templates are written for Markdown, Tp-Note
is not tied to any specific markup language. However, Tp-Note comes
with an optional viewer feature, that currently renders only Markdown,
ReStructuredText and HTML input. In addition, there is some limited
support for Asciidoc and WikiText. Finally, the note's rendition is live
updated and displayed in the user's web browser.

After the user finished editing, Tp-Note analyses eventual changes in the
notes metadata and renames, if necessary, the file, so that its metadata and
filename are in sync again. Finally, the resulting path is printed to
'`stdout`', log and error messages are dumped to '`stderr`'.

This document is Tp-Note's technical reference. More information
can be found in [Tp-Note's user manual] and at [Tp-Note's project page].

[Tp-Note's user manual]: https://blog.getreu.net/projects/tp-note/tpnote--manual.html
[Tp-Note's project page]: https://blog.getreu.net/projects/tp-note/



# OPERATION MODES

Tp-Note operates in 5 different modes, depending on its command line arguments
and the clipboard state. Each mode is associated with one content template and
one filename template.


## Create a new note with empty clipboard

In case the clipboard is empty while starting, the new note is created
with the templates: '`tmpl.new_content`' and '`tmpl.new_filename`'.  By
default, the new note's title is the parent's directory name. The newly created
file is then opened with an external text editor, allowing it to change the
proposed title and add other content. When the text editor closes, Tp-Note
synchronizes the note's metadata and its filename. This operation is performed
with the '`tmpl.sync_filename`' template.

Example: the clipboard is empty and `<path>` is a directory (or empty):

``` sh
> tpnote "./03-Favorite Readings/"
```

or

``` sh
> cd "./03-Favorite Readings"
> tpnote
```

creates the document:

    "./03-Favorite Readings/20211031-Favorite Readings--Note.md"

with the content:

``` yaml
---
title:      "Favorite Readings"
subtitle:   "Note"
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
---
```


## Create a new note based on clipboard data

When '`<path>`' is a directory and the clipboard is not empty, the clipboard's
content is stored in the variable '`{{ clipboard }}`'. In addition, if the
content contains an hyperlink in Markdown format, the hyperlink's name can be
accessed with '`{{ clipboard | link_text }}`', its URL with
'`{{ clipboard | link_dest }}`' and its title with
'`{{ clipboard | link_title }}`'. The new note is then created with the
'`tmpl.from_clipboard_content`' and the '`tmpl.from_clipboard_filename`'
templates. Finally, the newly created note file is opened again with some
external text editor. When the user closes the text editor, Tp-Note
synchronizes the note's metadata and its filename with the template
'`tmpl.sync_filename`'.

> Note: this operation mode also empties the clipboard (configurable feature).

**Clipboard simulation**

When no mouse and clipboard is available, the clipboard feature can be
simulated by feeding the clipboard data into `stdin`:

```sh
> echo "[The Rust Book](<https://doc.rust-lang.org/book/>)" | tpnote
```

Tp-Note behaves here as if the clipboard contained the string:
"`[The Rust Book](<https://doc.rust-lang.org/book/>)`".



### The clipboard contains a string

Example: While launching Tp-Note the clipboard contains the string:
"`Who Moved My Cheese?\n\nChapter 2`" and `<path>` is a directory.

``` sh
> tpnote "./03-Favorite Readings/"
```

or

``` sh
> cd "./03-Favorite Readings/"
> tpnote
```

This creates the document:

    "./03-Favorite Readings/20211031-Who Moved My Cheese--Note.md"

with the content:

```yaml
---
title:      "Who Moved My Cheese"
subtitle:   "Note"
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
---

Who Moved My Cheese?

Chapter 2
```

We see from the above example, how the '`tmpl.from_clipboard_content`' content
template extracts the first line of the clipboards content and inserts it into
the header's '`title:`' field. Then, it copies the entire clipboard content into
the body of the document.  However, if desired or necessary, it is possible to
modify all templates in Tp-Note's configuration file. Note, that not only the
note's content is created with a template, but also its filename: The
'`tmpl.from_clipboard_filename`' filename template concatenates the current date,
the note's title and subtitle.


### The clipboard contains a hyperlink

Example: `<path>` is a directory, the clipboard is not empty and it contains
the string: "`I recommend:\n[The Rust Book](https://doc.rust-lang.org/book/)`".

```sh
> tpnote './doc/Lecture 1'
```

Tp-Note's templates '`tmpl.from_clipboard_content`' and
'`tmpl.from_clipboard_filename`' create the following document:

    ./doc/Lecture 1/20211031-The Rust Book--Notes.md

```yaml
---
title:      "The Rust Book"
subtitle:   "URL"
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
---

I recommend:
[The Rust Book](<https://doc.rust-lang.org/book/>)
```

When analyzing the clipboard's content, Tp-Note searches for hyperlinks in
Markdown, ReStructuredText, Asciidoc and HTML format. When successful, the
content template uses the link text of the first hyperlink found as document
title.


### The clipboard contains a string with a YAML header

Example: '`<path>`' is a directory, the clipboard is not empty and contains
the string: "`---\ntitle: Todo\nfile_ext: mdtxt\n---\n\nnothing`".

```sh
> tpnote
```

This creates the note: '`20211031-Todo.mdtxt`' with the following
content:

```yaml
---
title:      "Todo"
subtitle:   ""
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
file_ext:   "mdtxt"
---

nothing
```

Technically, the creation of the new note is performed
using the YAML header variables: '`{{ fm_title }}`',
'`{{ fm_subtitle }}`', '`{{ fm_author }}`', '`{{ fm_date }}`',
'`{{ fm_lang }}`', '`{{ fm_sort_tag }}`' and
'`{{ fm_file_ext }}`' which are evaluated with the
'`tmpl.from_clipboard_yaml_content`' and the
'`tmpl.from_clipboard_yaml_filename`' templates.

Note, that the same result can also be achieved without clipboard input by
typing in a terminal:

```sh
> echo -e "---\ntitle: Todo\nfile_ext: mdtxt\n---\n\nnothing" | tpnote
```

Furthermore, this operation mode is very handy with pipes in general, as shows the
following example: it downloads some webpage, converts it to Markdown and copies
the result into a Tp-Note file. The procedure preserves the webpage's title in the
note's title:

```sh
curl 'https://blog.getreu.net' | pandoc --standalone -f html -t markdown_strict+yaml_metadata_block | tpnote
```

creates the note file '`20211031-Jens Getreu's blog.md`' with the webpage's
content converted to Markdown:

```yaml
---
title:      "Jens Getreu's blog"
subtitle:   ""
author:     "getreu"
date:       "2021-10-31"
lang:       "en"
---


<a href="/" class="logo">Jens Getreu's blog</a>

-   [Home](https://blog.getreu.net)
-   [Categories](https://blog.getreu.net/categories)
```


## Create a new note annotating a non-Tp-Note file

When '`<path>`' points to an existing file, whose file extension is other than
'`.md`', a new note is created with a similar filename and a reference to the
original file is copied into the new note's body. If the clipboard contains some
text, it is appended there also. The logic of this is implemented in the
templates: '`tmpl.annotate_file_content`' and
'`tmpl.annotate_file_filename`'. Once the file is created, it is opened with
an external text editor. After editing the file, it will be - if necessary -
renamed to be in sync with the note's metadata.

Example:

``` sh
> tpnote "Classic Shell Scripting.pdf"
```

creates the note:

    Classic Shell Scripting.pdf--Note.md"

with the content:

``` yaml
---
title:      "Classic Shell Scripting.pdf"
subtitle:   "Note"
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
---

[Classic Shell Scripting.pdf](<Classic Shell Scripting.pdf>)
```

The configuration file variables '`filename.extensions_*`' list all the file
extensions that Tp-Note recognizes as own file types. Only foreign file types
can be annotated.

Note that the file annotation mode also reads the clipboard's content: when it
is not empty, its data is appended to the new note's body.



## Convert a text file into a Tp-Note file

Consider the content of the following text file
'`Ascii-Hangman--A game for children.md`' whose creation date is 13 March 2022:

    A little game designed for primary kids to revise vocabulary in classroom.

To convert the text file into a Tp-Note file type:

```bash
> tpnote --add-header --batch "Ascii-Hangman--A game for children.md"
```

NB: the '`--add-header`' flag is actually not necessary, as it is enabled by
default through the configuration file variable
'`arg_default.add_header = true`'.

As a result of the above command, Tp-Note converts the filename into:

    20220313-Ascii-Hangman--A game for children.md

and prepends a YAML header to the file's content:

```yaml
---
title:      "Ascii-Hangman "
subtitle:   "A game for children"
author:     "getreu"
date:       "2022-03-13"
lang:       "en-GB"
orig_name:  "Ascii-Hangman--A game for children.md"
---

A little game designed for primary kids to revise vocabulary in classroom.
```


## Use Tp-Note in shell scripts

* **Use case: download a webpage and store it as Tp-Note file**

  Using the method displayed above you can save time and create a script with:

  ```
  > sudo nano /usr/local/bin/download
  ```

  Insert the following content:

  ```sh
  #!/bin/sh
  curl "$1" | pandoc --standalone -f html -t markdown_strict+yaml_metadata_block | tpnote
  ```

  and make it executable:

  ```
  > sudo chmod a+x /usr/local/bin/download
  ```

  To execute the script type:

  ```shell
  > download 'https://blog.getreu.net'
  ```

* **Use case: synchronize recursively filenames and metadata**

  The following synchronizes bidirectionally all filenames with the note's YAML
  header data.

  ```shell
  > TPNOTEUSER="John" find . -type f -name '*.md' -exec tpnote -a -b {} > /dev/null \;
  ```

  The direction of the synchronization depends on whether the '`.md`' file has
  a valid YAML header or not:

  * A YAML header is present and valid: the header fields might update
    the filename (see template '`tmpl.sync_filename`'). A possible _sort-tag_
    at the beginning of the filename remains untouched.

  * No YAML header: a new header is prepended (see template
    '`from_text_file_content`') and the filename might change slightly
    (see template '`from_text_file_filename`'). A possible _sort-tag_
    at the beginning of the filename remains untouched. If the filename
    does not start with a sort tag, the current date is prepended.



## Editing notes

Unless invoked with '`--batch`' or '`--view`', Tp-Note launches an external
text editor after creating a new note. This also happens when '`<path>`' points
to an existing '`.md`'-file.

Example: edit the note from the previous example:

``` bash
> cd "./03-Favorite Readings"
> tpnote 20211031-Favorite Readings--Note.md
```


## Automatic filename synchronization before and after editing

Before launching the text editor and after closing it, Tp-Note synchronizes
the filename with the note's metadata. When the user changes the metadata of a
note, Tp-Note will replicate that change in the note's filename. As a result,
*all your note's filenames always correspond to their metadata*, which helps
to retrieve your notes in large data pools.

Example:

``` sh
> tpnote "20200306-Favorite Readings--Note.md"
```

The way how Tp-Note synchronizes the note's metadata and filename is defined
in the template '`tmpl.sync_filename`'.

Once Tp-Note opens the file in an text editor, the person taking notes may
decide updating the title in the note's YAML metadata section from '`title:
"Favorite Readings"`' to '`title: "Introduction to bookkeeping"`'.  After
closing the text editor the filename is automatically updated too and looks
like:

    "20200306-Introduction to bookkeeping--Note.md"

Note: the sort tag '`20200306-`' has not changed. The filename synchronization
mechanism by default never does. (See below for more details about filename
synchronization).



# OPTIONS

**-a**, **\--add-header**

:   Prepend a YAML header in case the text file does not have one.
    The default template, deduces the '`title:`' and '`subtitle:`'
    header field from the filename. It's sort-tag and file extension
    remain untouched. In case the filename is lacking a _sort-tag_,
    the file creation date in numerical format is prepended.
    As this option is activated by default, it has no effect unless
    you set '`arg_default.add_header = false`' in the configuration
    file.


**-b**, **\--batch**

:   Do not launch the external text editor or viewer. All other operations
    are available and are executed in the same way. In batch mode, error
    messages are dumped on the console only and no alert windows pop up.

:   Tp-Note ignores the clipboard when run in batch mode with '`--batch`'.
    Instead, if available, it reads the `stdin` stream as if the data came
    from the clipboard.

**-c** *FILE*, **\--config**=*FILE*

:   Load the alternative config file *FILE* instead of the default one.

**-d** *LEVEL*, **\--debug**=*LEVEL*

:   Print additional log messages.  The debug level *LEVEL* must be one out of
    '`trace`', '`debug`', '`info`', '`warn`', '`error`' (default) or '`off`'.
    The level '`trace`' reports the most detailed information, while '`error`'
    informs you only about failures.  A '`warn`' level message means, that not
    all functionality might be available or work as expected.

:   Use '`-b -d trace`' for debugging templates. If the HTTP server
    (viewer) does not work as expected: '`-n -d debug`'. If your text editor
    does not open as expected: '`-n -d info --edit`'. Or, to observe the
    launch of the web browser: '`-n -d info --view`'. The option
    '`-d trace`' shows all available template variables, the templates
    used and the rendered result of the substitution. This is
    particularly useful for debugging new templates. The option
    '`-d off`' silences all error message reporting and also suppresses the
    error popup windows.

:   All error messages are dumped in the error stream `stderr` and appear
    on the console from where Tp-Note was launched:

        tpnote.exe --debug info my_note.md

    On Windows the output must be redirected into a file to see it:

        tpnote.exe --debug info my_note.md >debug.md 2>&1

    Alternatively, you can redirect all log file entries into popup alert
    windows.

        tpnote.exe --popup --debug info my_note.md

    The same can be achieved by setting following configuration file
    variables (especially useful with Windows):

        [arg_default]
        debug = 'info'
        popup = true

    The value for '`arg_default.debug`' must be one out of '`trace`',
    '`debug`', '`info`', '`warn`', '`error`' (default) and '`off`'. They have
    the same meaning as the corresponding command line options.

**-e**, **\--edit**

:   Edit only mode: opens the external text editor, but not the file
    viewer. This disables Tp-Note's internal file watcher and web server,
    unless '`-v`' is given. Another way to permanently disable the web server
    is to set the configuration variable '`arg_default.edit=true`'.
    When '`--edit --view`' appear together, both the editor and the viewer
    will open and the `arg_default.edit` variable is ignored.

**-p**, **\--port**=*PORT*

:   Sets the server port that the web browser connects to, to the specified
    value *PORT*.
    If not given, a random available port is chosen automatically.

**-n**, **\--no-filename-sync**

:   Whenever Tp-Note opens a note file, it synchronizes its YAML-metadata with
    its filename. '`--no-filename-sync`' disables the synchronization.  In
    addition, the "flag in scripts" can be especially useful for testing
    '`.md`'-files.  See section EXIT STATUS for more details.  The section
    METADATA FILENAME SYNCHRONIZATION shows alternative ways to disable
    synchronization.

**-t**, **\--tty**

:   Tp-Note tries different heuristics to detect whether a graphic environment
    is available or not. For example, under Linux, the '`DISPLAY`' environment
    variable is evaluated. The '`--tty`' flag disables the automatic detection
    and sets Tp-Note into "console only" mode: now only the non GUI editor
    (see configuration variable: '`app_args.editor_console`') and no viewer
    is launched.

**-u**, **\--popup**

:   Redirect log file entries into popup alert windows. Must be used together
    with the **\--debug** option to have an effect. Note, that debug level
    '`error`' conditions will always trigger popup messages, regardless of
    **\--popup** and **\--debug** (unless '`--debug off`'). Popup alert windows
    are queued and will never interrupt _Tp-Note_. To better associate a
    particular action with its log events, read through all upcoming popup alert
    windows until they fail to appear.

**-v**, **\--view**

:   View only mode: do not open the external text editor. This flag instructs
    Tp-Note to start an internal file watcher and web server and connect
    the system's default web browser to view the note file and to observe live
    file modifications. The configuration setting '`arg_default.edit=true`'
    disables the viewer. However, with '`--view`' given at the command line,
    the viewer appears, regardless of the value of '`arg_default.edit`'.

:   As most users do not expect the viewed file to change, '`--view`'
    is usually used together with '`--no-filename-sync`'.

**-V**, **\--version**

:   Print Tp-Note's version, its built-in features and the path to the
    sourced configuration file. The output is YAML formatted for further
    automatic processing.

**-x** *DIRECTORY*, **\--export**=*DIRECTORY*

:   Print the note as HTML rendition into _DIRECTORY_. '`-x -`' prints to
    _stdout_. The empty string, e.g. '`--export= `' or '`-x ""`', defaults to
    the directory where the note file resides. No external text editor or
    viewer is launched. Can be combined with '`--batch`' to avoid popup
    error alert windows.

**\--export-link-rewriting**=*MODE*

:   Choose how local links in the exported HTML file are written out: '`off`',
    '`short`' or '`long`' (default). No link rewriting occurs, for the *MODE*
    '`off`'. The *MODE* '`short`' rewrites all local relative links to absolute
    links, whose base is the first parent directory containing the marker
    file '`.tpnoteroot`' (filename customizable). NB, the directory of the
    marker file defines the base for all absolute local links in your Tp-Note
    file! The mode '`long`' rewrites *all* local links to absolute links whose
    base is the system's root directory '`/`'. For relative local links this
    is performed by prepending the path to the note file. Absolute local links
    get the path to the marker file '`.tpnoteroot`' prepended. In case you
    do not place a '`.tpnoteroot`' file in a parent directory, the base for
    absolute local links in your note file is interpreted as '`/`'. The right
    mode to choose depends on how you view the resulting HTML: if you publish
    on a web server, then '`short`' is a good choice. If you view the HTML
    file directly in your web browser, better choose '`long`'. NB: You can
    also set this option via Tp-Note's configuration file with the key 
    '`arg_default.export_link_rewriting`'.



# THE NOTE'S DOCUMENT STRUCTURE

Tp-Note considers a text file to be a valid note file, if its:

* file extension is listed in one of the configuration file variables
  '`filename.extension_*`'; if its

* content has a valid YAML header and

* the YAML header contains a key whose name is defined in the configuration
  file variable '`tmpl.compulsory_header_field`' (default '`title`').

A Tp-Note note file is always UTF-8 encoded. As newline, either the Unix
standard '`\n`' or the Windows standard '`\r\n`' is accepted. Tp-Note writes
out newlines according the operating system it runs on.



## The document's header and body

Tp-Note is designed to be compatible with '`Pandoc`'s and '`RMarkdown`s
document structure as shown in the figure below. In this documentation
the terms "YAML header", "header" and "front matter" are used as
synonyms to designate to document's metadata block at the beginning
of the text file:

```
---
<YAML-front-matter>
---
<document-body>
```

The YAML front-matter starts at the beginning of the document with '`---`'
and ends with '`...`' or '`---`'. Note that according to the YAML standard,
string literals are always encoded as JSON strings. By convention, a valid
Tp-Note file has at least one YAML field named '`title:`' (the name of this
compulsory field is defined by the '`tmpl.compulsory_header_field`'
variable in the configuration file and can be changed there).

Note that prepended text, placed before the YAML front-matter, is ignored. There
are however certain restrictions: If present, the skipped text should not be too
long (cf. constant '`BEFORE_HEADER_MAX_IGNORED_CHARS`' in the source code of
Tp-Note) and it must be followed by at least one blank line:

```
Prepended text is ignored.

---
<YAML-front-matter>
---
<document-body>
```

There is no restriction about the markup language being used in the note's text
body. However, the default templates assume Markdown and the file extension
'`.md`'. Both can be changed easily by adapting Tp-Note's configuration file.
Besides the requirements concerning its header, a valid Tp-Note file must have
a filename extension that is listed in one of the configuration file variables:
'`filename.extension_*`'. The latter also determine which internal markup
language render is called for Tp-Note's internal viewer.



## Links to resources and other documents

The document's body often contains (inline) links to resources e.g. images and
links to other documents. The link syntax depends on the markup language used
in the Tp-Note file.

Here some example links written in Markdown:

* Website: '`[blog](https://blog.getreu.net)`'
* Inline image with relative local URL: '`![Alt text](<images/my logo.png>)`'.
* Link to another Tp-Note document with relative local link:
  '`[my doc](<../../notes/my doc.md>)`'
* The same as above, but using the short autolink syntax:
  '`<http:../../notes/my%20doc.md>`'
* Link to another Tp-Note document with absolute local link:
  '`[my doc](</home/kanban/documents/my note.md)`'
  The base for absolute local links is the first parent directory containing
  the marker file '`.tpnoteroot`'. If absent, absolute local links refer
  to the root directory '`/`'.

Although Tp-Note's built in viewer follows absolute and relative URLs, usually
the latter are preferred, because it allows you to move more easily documents
and its resources in the file system.

Tp-Note's exporter function '`--export`' converts a given Tp-Note file into
HTML and adds '`.html`' to the output filename. Links in the documents
content to other Tp-Note files are hereby rewritten by appending '`.html`' to their
URLs. This way you can convert groups of documents to HTML and later browse
from document to document in you web browser. The option
'`--export-link-rewriting`' allows you to finetune how local links are written
out. Valid values are: '`off`', '`short`' and '`long`'.

In order to achieve this, the user must respect  the following convention
concerning absolute local links in Tp-Note documents:  The base of absolute
local links in Tp-Note documents must be the directory where the marker file
'`.tpnoteroot`' resides (or '`/`' in non exists). The option '`--export-link-
rewriting`' decides how local links in the Tp-Note  document are converted when
the HTML is generated.  If its value is '`short`', then relative local links
are converted to absolute links. The base of the resulting links is where the
`.tpnoteroot` file resides (or `/` if none exists). Consider the following
example:

* The Tp-Note file '`/my/docs/car/bill.md`' contains
* the absolute link '`/car/scan.jpg`'.
* and the relative link '`./photo.jpg`'.
* The document root marker is: '`/my/docs/.tpnoteroot`'.

The images in the resulting HTML will appear as

* '`/car/scan.jpg`'.
* '`/car/photo.jpg`'.

For '`--export-link-rewriting=long`', in addition to the above, all absolute
local links are rebased to '`/`''. Consider the following example:

* The Tp-Note file '`/my/docs/car/bill.md`' contains
* the absolute link '`/car/scan.jpg`'.
* and the relative link '`./photo.jpg`'.
* The document root marker is: '`/my/docs/.tpnoteroot`'.

The images in the resulting HTML will appear as

* '`/my/docs/car/scan.jpg`'.
* '`/my/docs/car/photo.jpg`'.

So far we have seen how Tp-Note's viewer and HTML exporter converts the
_target_ of local links '`[text](target)`'. For the _text_ property the
situation is simpler as it never changes. However, there is one exception:
when the text contains a URL starting with '`http:`' or '`https:`' only
the file stem is displayed. For example, the link: 
'`[http:dir/my file.md](<http:dir/my file.md>)`' is rewritten into 
'`[my file](<http:dir/my file.md>)`' before being rendered. This explains 
why the autolink '`<http:dir/my file.md>.`' appears as '`my file`' in 
the browser.



# METADATA FILENAME SYNCHRONIZATION

Consider the following Tp-Note file:

    20151208-Make this world a better place--Suggestions.md

The filename has 4 parts:

    {{ fm_sort_tag }}{{ fm_title }}--{{ fm_subtitle }}.{{ fm_file_ext }}

A so-called _sort tag_ is a numerical prefix at the beginning of the
filename. It is used to order files and notes in the file system. Besides
numerical digits and whitespace, a _sort tag_ can be any combination of
`-_.`[^sort tag] and is usually used as

* *chronological sort tag*

        20140211-Reminder.doc
        20151208-Manual.pdf
        2015-12-08-Manual.pdf

* or as a *sequence number sort tag*.

        02-Invoices
        08-Tax documents
        09_02-Notes
        09.09-Notes

When Tp-Note creates a new note, it will automatically prepend a
*chronological
sort tag* of today. The '`{{ fm_title }}`' part is usually derived from the
parent directory name omitting its own *sort tag*.

[^sort tag]: The characters '`_`', '`-`', '` `', '`\t`' and '`.`' are considered
  to be part of the *sort tag* even when they appear in last position.

A note's filename is in sync with its meta data, when the following is true
(slightly simplified, see the configuration file for the complete definition):

> filename on disk without *sort tag* == '`{{ fm_title }}--{{ fm_subtitle }}.md`'
  ^[The variables '`{{ fm_title }}`' and '`{{ fm_subtitle }}`' reflect the values in
  the note's metadata.]

Consider the following document with the filename:

    20211031-My file.md

and the content:

``` yaml
---
title:      "1. The Beginning"
subtitle:   "Note"
author:     "getreu"
date:       "2021-10-31"
lang:       "en-GB"
---
```

As "`-My file.md`" is not equal to "`-'1. The Beginning--Note.md`",
Tp-Note will rename the file to "`20211031-'1. The Beginning--Note.md`".
If the filename had been "`05_02-My file.md`", it would rename it to
"`05_02-'1. The Beginning--Note.md`".

Note: When the YAML front-matter does not contain the optional '`sort_tag`'
variable, Tp-Note will never change a sort tag. Nevertheless, it might
change the rest of the filename!

The reason why by default Tp-Note does not change sort tags is, that they
define their order in the file listing. In general this order is independent of
the notes content. The simplest way to organize the sort tags of your files is
by renaming them directly in your file system. Nevertheless, in some cases you
might want to have full control over the whole filename through the note's YAML
front-matter. For example, if — for some reason — you have changed the
document's date in the front-matter and you want to change the chronological
sort tag in one go. In order to overwrite the note's sort tag on disk, you can
add a '`sort_tag`' variable to its front-matter:

``` yaml
---
title:      "1. The Beginning"
date:       "2021-10-31"
sort_tag:   "20211101-"
---
```

When Tp-Note synchronizes the note's metadata with its filename, it will also
change the sort tag from '`20211031-`' to '`20211101-`'. The resulting filename
becomes "`20211101-'1. The Beginning--Note.md`".

The '`sort_tag`' variable also becomes handy, when you want to create one single
note without any sort tag:

``` yaml
---
title:      "1. The Beginning"
sort_tag:   ""
---
```

In the same way, how it is possible to pin the sort tag of the note from within
the note's metadata, you can also change the file extension by adding the
optional '`file_ext`' variable into the note's front-matter:

``` yaml
---
title:      "1. The Beginning"
file_ext:   "rst"
---
```

This will change the file extension from '`.md`' to '`.rst`. The resulting
filename becomes "`20211101-'1. The Beginning--Note.rst`".

Important: '`rst`' must be one of the registered file extensions
listed in the '`filename.extensions_rst`' variables in Tp-Note's configuration
file. If needed you can add more extensions there. If the new filename extension
is not listed in one of these variables, Tp-Note will not be able to
recognize the note file as such and will not open it in the external text editor
and viewer.

Note: When a '`sort_tag`' variable is defined in the note's YAML header, you
should not change the sort tag string in the note's file name manually by
renaming the file, as your change will be overwritten next time you open the
note with Tp-Note.  However, you can switch back to Tp-Note's default
behavior any time by deleting the '`sort_tag`' line in the note's metadata.
The same applies to the '`file_ext`' variable.

The metadata filename synchronization feature can be disabled permanently
by setting the configuration file variable
'`arg_default.no_filename_sync = true`'. To disable this feature for one time
only, invoke Tp-Note with '`--no-filename-sync`'. To exclude a particular note
from filename synchronization, add the YAML header field '`filename_sync:
false`'.

``` yaml
---
title:      "1. The Beginning"
filename_sync: false
---
```

Note, that in the above described examples, the information flow goes always
from the YAML note header towards the note's filename. However, when Tp-Note
opens a text file without a YAML header, a new header is added automatically.
In this case the information flow goes from the filename towards the header,
namely in the opposite direction. Once the new header is prepended to the text
file, a regular filename synchronization - as described above - is triggered and
executed as described above.

Technically, all rules and logic of how the synchronisation is executed, are
encoded in customizable so called filename templates (cf. section _Templates_).





# CUSTOMIZATION

Tp-Note's configuration file resides typically in
'`~/.config/tpnote/tpnote.toml`' on Unix or in
'`C:\Users\<LOGIN>\AppData\Roaming\tpnote\config\tpnote.toml>`' on Windows.
'`tpnote --version`' prints the current configuration file path.
When Tp-Note starts, it first tries to find its configuration file. If it
fails, it writes a default configuration file. Tp-Note is best customized by
starting it once, and then modifying its default configuration. For a detailed
description of the available configuration variables, please consult the
'`const`' definitions in Tp-Note's source code file '`config.rs`'
The configuration file is encoded according to the TOML standard.


## Register your own text editor

The configuration file variables '`app_args.editor`' and
'`app_args.editor_console`' define lists of external text editors to be
launched for editing. The lists contain by default well-known text editor names
and their command line arguments.  Tp-Note tries to launch every text editor
in '`app_args.editor`' from the beginning of the list until it finds an
installed text editor. When Tp-Note is started on a Linux console, the list
'`app_args.editor_console`' is used instead. Here you can register text
editors that do not require a graphical environment, e.g. '`vim`' or '`nano`'.
In order to use your own text editor, just place it at the top of the list. To
debug your changes invoke Tp-Note with '`tpnote --debug info --popup --edit`'

The following example showcases the configuration for the _Kate_
file editor.  The entry '`kate`' launches the binary, while the command
line parameter '`--block`' guarantees, that the launched process blocks
until the user closes the editor. Tp-Note detects the end of the process,
checks if the title of the note files has changed in its YAML header
and renames the note file if necessary.

```toml
editor = [
  [
    'kate',
    '--block'
  ]
]
```

When you configure Tp-Note to work with your text editor, make sure, that your
text editor does not fork! You can check this by launching the text editor from
the command line: if the command prompt returns immediately, then the file
editor forks the process. On the other hand everything is OK, when the command
prompt only comes back at the moment the text editor is closed. Many text
editors provide an option to restrain from forking: for example the
_VScode_-editor can be launched with the '`--wait`' option, _Vim_ with
'`--nofork`' or _Kate_ with '`--block`' (see example above). However,
Tp-Note also works with forking text editors. Although this should be
avoided, there is a possible workaround:

```shell
> FILE=$(tpnote --batch) # Create the new note.
> mytexteditor "$FILE"    # The prompt returns immediatly as the editor forks.
> tpnote --view "$FILE"  # Launch Tp-Note's viewer.
>                         # After the editing is done...
> tpnote --batch "$FILE" # Synchronize the note's filename.
```

Remark for the advanced console user: In a similar way, you can launch a
different text editor than the one configured in Tp-Note's configuration file:

```shell
> FILE=$(tpnote --batch); vi "$FILE"; tpnote --batch "$FILE"
```

Whereby '`FILE=$(tpnote --batch)`' creates the note file, '`vi "$FILE"`' opens the
'`vi`'-text editor and '`tpnote --batch "$FILE"`' synchronizes the filename.



**Register a Flatpak Markdown editor**

[Flathub for Linux] is a cross-platform application repository that works well
with Tp-Note.  To showcase an example, we will add a Tp-Note launcher for
the _Mark Text_ Markdown text editor available as [Flatpak package]. Before
installing, make sure that you have [set up Flatpack] correctly. Then install
the application with:

[Flathub for Linux]: https://www.flathub.org/home
[Flatpak package]: https://www.flathub.org/apps/details/com.github.marktext.marktext
[set up Flatpack]: https://flatpak.org/setup/

    > sudo flatpak install flathub com.github.marktext.marktext

To test, run _Mark Text_ from the command line:

    > flatpak run com.github.marktext.marktext

Then open Tp-Note's configuration file `tpnote.toml` and search for the
'`app_args.editor`' variable, quoted shortened below:

```toml
[app_args]
editor = [
    [
    'code',
    '-w',
    '-n',
],
#...
]
```

The structure of this variable is a list of lists. Every item in the outer list
corresponds to one entire command line launching a different text editor, here
_VSCode_.  When launching, Tp-Note searches through this list
until it finds an installed text editor on the system.

In this example, we register the _Mark Text_ editor at the first place in this
list, by inserting '`['flatpak', 'run', 'com.github.marktext.marktext']`':


```toml
[app_args]
editor = [
    [
    'flatpak',
    'run',
    'com.github.marktext.marktext',
],
    [
    'code',
    '-w',
    '-n',
],
#...
]
```

Save the modified configuration file.  Next time you launch Tp-Note, the
_Mark Text_-editor will open with your note.


**Register a console text editor running in a terminal emulator**

In this setup Tp-Note launches the terminal emulator which is configured to
launch the text editor as child process. Neither process should fork when they
start (see above).

Examples, adjust to your needs and taste:

* _Neovim_ in _Xfce4-Terminal_:

  ```toml
  [app_args]
  editor = [
    [
      'xfce4-terminal',
      '--disable-server',
      '-x',
      'nvim',
      '+colorscheme pablo',
      '+set syntax=markdown',
    ],
  ]
  ```
* _Neovim_ in _LXTerminal_:

  ```toml
  [app_args]
  editor = [
    [
      'lxterminal',
      '--no-remote',
      '-e',
      'nvim',
      '+colorscheme pablo',
      '+set syntax=markdown',
    ],
  ]
  ```

* _Neovim_ in _Xterm_:

  ```toml
  [app_args]
  editor = [
    [
      'xterm',
      '-fa',
      'DejaVu Sans Mono',
      '-fs',
      '12',
      '-e',
      'nvim',
      '+colorscheme pablo',
      '+set syntax=markdown',
    ],
  ]
  ```
* _Neovim_ in _Alacritty_:

  ```toml
  [app_args]
  editor = [
    [
      'alacritty',
      '-e',
      'nvim',
      '+colorscheme pablo',
      '+set syntax=markdown',
    ],
  ]
  ```

* _Helix-editor_ in _XFCE4-Terminal_:

  ```toml
  [app_args]
  editor = [
    [
      'xfce4-terminal',
      '--disable-server',
      '-x',
      'hx',
    ],
  ]
  ```



## Change the file extension for new note files

Tp-Note identifies the note's markup language by its file extension and
renders the content accordingly (see '`filename.extensions_*`' variables).
For example: the variable '`filename.extensions_md`' lists all file
extensions, that are regarded as Markdown files:

```toml
[filename]
extensions_md = [ 'txt', 'md', 'markdown' ]
```

The default file extension for new note files under Windows is defined as:

```toml
[filename]
extension_default = 'txt'
```

If you prefer rather the file extension '`md`' for new notes, change this to:

```toml
[filename]
extension_default = 'md'
```

This modification does not change how the note file's content is interpreted -
in this case as Markdown - because both file extensions '`.txt`' and '`.md`'
belong to the same extension group defined in '`filename.extensions_md`'.



## Change the default markup language

Tp-Note's core functionality, the management of note file headers and
filenames, is markup language agnostic. However, there is one content template
'`tmpl.from_clipboard_content`' that generates a hyperlink. The hyperlink
syntax varies depending on the markup language. Hence, you should not forget to
modify the '`tmpl.from_clipboard_content`' content template, when you
change the default markup language defined in '`filename.extension_default`'.


### Change default markup language to ReStructuredText

Tp-Note's core function is a template system and as such it depends
very little on the used markup language. The default templates are
designed in a way that they contain almost no markup specific code. There
is one little exception though. The following configuration variables
affect the way new notes are created:

1. Change the default file extension for new notes from:

   ```toml
   [filename]
   extension_default='md'
   ```

   to:

   ```toml
   [filename]
   extension_default='rst'
   ```

2. Replace the following line in the template '`tmpl.from_clipboard_content`'
   that defines a hyperlink in Markdown format:

       [{{ path | tag }}{{ path | stem }}{{ path | ext | prepend_dot }}](<{{ path | tag }}{{ path | stem }}{{ path | ext | prepend_dot }}>)

   with the following line encoded in ReStructuredText syntax:

       `<{{ path | tag }}{{ path | stem }}{{ path | ext | prepend_dot }}>`_

As a result, all future notes are created as '`*.rst`' files.


### Change the markup language for one specific note only

You can change the Markup language of a specific note by adding the variable
'`file_ext:`' to its YAML header. For example, for ReStructuredText add:

```yaml
---
title:    "some note"
file_ext: "rst"
---
```

When Tp-Note triggers the next filename synchronization, the filename
extension of the note file will change to '`.rst`'. The above modification
applies to the current note only.




## Change the sort tag character set

*Sort tags* for new notes are generated with the '`[TMPL] *_filename`'
templates and updated with the '`[TMPL] sync_filename`' template.

By default, the characters '`_`', '`-`', _space_, '`\t`' and '`.`' are
recognized as being part of a *sort tag* when they appear at the beginning of a
filename.  This set of characters can be modified with the '`[filename]
sort_tag_chars`' configuration variable. In addition, one special character
'`filename.sort_tag_extra_separator`' (by default '`'`') is sometimes used as
"end of sort tag marker" to avoid ambiguity.



## Customize the filename synchronization scheme

The filename synchronization scheme is fully customizable through _Tp-Note's
filename templates_. To design such a custom scheme, start to set up your
synchronization rules in the '`tmpl.sync_filename`' template. Then adjust all
'`tmpl.*_filename`' templates to comply with these rules. In order to verify
your design, check that the following holds for any sequential application of
one '`tmpl.*_filename`' template followed directly by the
'`tmpl.sync_filename`' template: The latter should never change the filename
initially set up by any '`tmpl.*_filename`' template.

Secondly, make sure that the expression in '`tmpl.sync_filename`' describing
the filename's _sort tag_ e.g. '`{{ path | tag }}`' is always followed by a
variable with the '`sanit(force_alpha=true)`' filter set, e.g.:

    {{ path | tag }}{{ fm_title | sanit(force_alpha=true) }}

The first expression guarantees, that it resolves only to characters defined in
the '`filename.sort_tag_chars`' set, while the second expression is known
to not start with such a character. This way Tp-Note is able to separate
_sort tags_ in filenames and avoids cyclic filename change. Or, in other words:
the '`tmpl.sync_filname`' template should always give the same result, even
after repeated application.

To debug your '`tmpl.sync_filename`' template, create a test note file
'`test.md`' and invoke Tp-Note with '`--debug trace`' and '`--batch`':

    tpnote --batch --debug trace test.md



## Store new note files by default in a subdirectory

When you are annotating an existing file on disk, the new note file is
placed in the same directory by default. To configure Tp-Note to
store the new note file in a subdirectory, let's say '`Notes/`', instead, you
need to modify the templates '`tmpl.annotate_file_filename`' and
'`tmpl.annotate_file_content`':

Replace in '`tmpl.annotate_file_filename`' the string:

    {{ path | tag }}

with:

    Notes/{{ path | tag }}

and in '`tmpl.annotate_file_content`':

    [{{ path | filename }}](<{{ path | filename }}>)

with:

    [{{ path | filename }}](<../{{ path | filename }}>)

Please note that web browsers usually ignore leading '`../`' in relative
URL paths. To work around this limitation, Tp-Note's built-in viewer
first replaces '`../`' with the string '`ParentDir`'. When the user clicks
on this relative URL, the '`ParentDir`' string in the HTTP request replaced
back with '`..`'.  It is also worth mentioning that Tp-Note automatically
creates the subdirectory '`Notes/`' in case it does not exist.



## Customize the built-in note viewer

**Delay the launch of the web browser**

By default, Tp-Note launches two external programs: some text editor and a
web browser. If wished for, the configuration variable
'`[viewer] startup_delay`' allows to delay the launch of the web browser some
milliseconds.  This way the web browser window will always appear on top of the
editor window.  A negative value delays the start of the text editor instead.

**Change the way how note files are rendered for viewing**

Besides its core function, Tp-Note comes with several built-in markup
renderer and viewer, allowing to work with different markup languages at the
same time. The configuration file variables '`filename.extensions_*`' determine
which markup renderer is used for which note file extension. Depending on the
markup language, this feature is more or less advanced and complete: _Markdown_
(cf. '`filename.extensions_md`') is best supported and feature complete: It
complies with the _Common Mark_ specification. The _ReStructuredText_ renderer
(cf.  '`filename.extensions_rst`') is quite new and still in experimental state.
For all other supported markup languages Tp-Note provides a built-in markup
source text viewer (cf.  '`filename.extensions_txt`') that shows the note as
typed (without markup), but renders all hyperlinks to make them clickable.  In
case none of the above rendition engines suit you, it is possible to disable
the viewer feature selectively for some particular note file extensions: just
place these extensions in the '`filename.extensions_no_viewer`' variable. If
you wish to disable the viewer feature overall, set the variable
`arg_default.edit = true`.

**Change the HTML rendition template**

After the markup rendition process, Tp-Note's built-in viewer generates its
final HTML rendition through the customizable HTML templates
'`[tmpl_html] viewer`' and '`[tmpl_html] viewer_error`'. The following code
example taken from '`[tmpl_html] viewer`' illustrates the available variables:

```toml
[tmpl_html]
viewer = '''<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
<link rel="stylesheet" href="{{ note_css_path }}">
  </head>
  <body>
  <pre class="note-header">{{ note_fm_text }}</pre>
  <hr>
  <div class="note-body">{{ note_body_html }}</div>
  <script>{{ note_js }}</script>
</body>
</html>
'''
```

Specifically:

* '`{{ fm_* }}`' are the deserialized header variables. All content
  template variables and filters are available. See section _Template
  variables_ above.

* '`{{ note_css_path }}`' is the CSS stylesheet path required to 
  highlight embedded source code. This path is hard-wired and
  understood by Tp-Note's internal web server.

* '`{{ note_fm_text }}`' is the raw UTF-8 copy of the header. Not to be
  confounded with the dictionary variable '`{{ fm_all }}`'.

* '`{{ note_body_html }}`' is the note's body as HTML rendition.

* '`{{ note_js }}`' is the JavaScript browser code for
  live updates.

* '`{{ extension_default }}`' (c.f. section _Template variables_).

* '`{{ username }}`' (c.f. section _Template variables_).

* '`{{ lang }}`' (c.f. section _Template variables_).

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

The error page template '`[tmpl_html] viewer_error`' (see below)
does not provide '`fm_*`' variables, because of possible header syntax
errors. Instead, the variable '`{{ note_error }}`' contains the error
message as raw UTF-8 and the variable '`{{ note_erroneous_content_html }}`'
the HTML rendition of the text source with clickable hyperlinks:

```toml
[viewer_error]
error = '''<!DOCTYPE html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<title>Syntax error</title>
</head>
<body>
<h3>Syntax error</h3>
<p> in note file: <pre>{{ path }}</pre><p>
<hr>
<pre class="note-error">{{ note_error }}</pre>
<hr>
{{ note_erroneous_content_html }}
<script>{{ note_js }}</script>
</body>
</html>
'''
```

**Customize the built-in HTML exporter**

Customizing Tp-Note's HTML export function works the same way as
customizing the built-in viewer. There are some slight differences though:
The role of the '`[tmpl_html] viewer`' template - discussed above - is
taken over by the '`[tmpl_html] exporter`' template. In this template the
same _Tera_ variables are available, except '`{{ note_js }}`' which does not
make sense in this context. As the exporter prints possible rendition error
messages on the console, there is no equivalent to the
'`[tmpl_html] viewer_error`' template. Note, in contrast to the previous 
template '`[tmpl_html] viewer`' example, the source code highlighting CSS 
code is now embedded in the HTML output with '`<style>{{ note_css }}</style>`'


## Choose your favorite web browser as note viewer

Once the note is rendered into HTML, Tp-Note's internal HTTP server connects
to a random port at the '`localhost`' interface where the rendition is served to
be viewed with a web browser. Tp-Note's configuration file contains a list
'`app_args.browser`' with common web browsers and their usual location on disk.
This list is executed top down until a web browser is found and launched. If
you want to view your notes with a different web browser, simply modify the
'`app_args.browser`' list and put your favorite web browser on top.

In case none of the listed browsers can be found, Tp-Note switches into a
fall back mode with limited functionality, where it tries to open the system's
default web browser. A disadvantage is, that in fall back mode Tp-Note is not
able to detect when the user closes the web browser. This might lead to
situations, where Tp-Note's internal HTTP server shuts down to early.
In order to check if Tp-Note finds the selected web browser as intended,
invoke Tp-Note with '`tpnote --debug info --popup --view`'.




# TEMPLATES

All _TP-Note_'s workflows are customizable through its templates which
are grouped in the '`[tmpl]`' section of _Tp-Nots_'s configuration file.
Configuration file variables ending with '`tmpl.*_content`' and
'`tmpl.*_filename`' are _Tera_ template strings (see:
<https://tera.netlify.com/docs/#templates>).

Tp-Note captures and stores its environment in _Tera variables_. For example,
the variable '`{{ dir_path }}`' is initialized with the note's target
directory. The variable '`{{ clipboard }}`' contains the content of the
clipboard. To learn more about Tera variables, launch Tp-Note with the
'`--debug trace`' option and observe what information it captures from its
environment.



## Template types

The content of a new note is composed by one of Tp-Note's internal customizable
templates, hence the name Tp-Note, where _Tp_ stands for “template”. Which of
the internal templates is applied depends on the context in which Tp-Note is
invoked: e.g. the template for clipboard text input is called
'`tmpl.from_clipboard_content`'. If the clipboard contains text with a YAML
header, the template '`tmpl.from_clipboard_yaml_content`' is used.

In total, there are 5 different '`tmpl.*_content`' templates:

* '`tmpl.new_content`'
* '`tmpl.from_clipboard_content`'
* '`tmpl.from_clipboard_yaml_content`'
* '`tmpl.from_text_file_content`'
* '`tmpl.annotate_file_content`'

In general, the templates are designed in a way, that the text input
stream -usually originating from the clipboard- ends up in the body of the
note file, whereas the environment -such as the username- ends up in
the header of the note file.

Once the content of the new note is set by one of the content templates,
another template type comes into play: the so-called _filename template_.
Each content template has a corresponding filename template, e.g.:

* '`tmpl.new_filename`'
* '`tmpl.from_clipboard_filename`'
* '`tmpl.from_clipboard_yaml_filename`'
* '`tmpl.from_text_file_filename`'
* '`tmpl.annotate_file_filename`'
* '`tmpl.sync_filename`' (no corresponding content template)

As the name suggests, the role of a filename template is to determine the
filename of the new note. This is done by evaluating (deserializing) it's YAML
header. The values of the note's YAML header fields are can be accessed in
filename templates through various '`{{ fm_<key> }}`' dynamically created
template variables. For example the value of the YAML header field '`title:`'
can be accessed with '`{{ fm_title }}`'. Once the filename is set, Tp-Note
writes out the new note on disk.

Most of the above templates are dedicated to the creation of new note files.
However, two of them have a special role: _prepend header to text file_
and _synchronize filename_:

* _Prepend header to text file_ (new feature in Tp-Note v1.16.0): When Tp-Note
  opens a regular text file without a YAML header, a new header is prepended
  automatically. It's data origins mainly form the filename of the
  text file. The templates applied in this use case are:
  '`tmpl.from_text_file_content`' and '`tmpl.from_text_file_filename`'.

* _Synchronize filename_: This function mode is invoked when [Tp-Note] opens an
  existing note file, after it's YAML header is evaluated. The extracted header
  information is then applied to the '`tmpl.sync_filename`' template and the
  resulting filename is compared to the actual filename on disk. If they differ,
  [Tp-Note] renames the note file. The '`tmpl.sync_filename`' template operates on
  its own without a corresponding content template.

Note, that in the operation mode _synchronize filename_, the header data
overwrites the filename of the note, whereas in the operation mode _prepend
header_ the filename data is copied into the new prepended header. Keep in mind,
that even in the latter mode the filename might change slightly. This is because
after the header creation with the '`tmpl.from_text_file_content`' template,
the '`tmpl.from_text_file_filename`' template is applied, which might cause a
slight filename modification due to its sanitization filters (cf. '`sanit()`'
in the section _Template filters_).

You can disable the _prepend header_ feature by setting the configuration file
variable '`arg_default.add_header = false`'. To disable all filename
synchronization, set '`arg_default.no_filename_sync = true`'. This guarantees,
that Tp-Note will never change neither the filename nor the YAML header of an
existing file.

For a more detailed description of templates and their defaults, please
consult the '`const`' definitions in Tp-Note's source code file '`note.rs`'.



## Template variables

All [Tera template variables and functions](https://tera.netlify.com/docs/#templates)
can be used within Tp-Note's templates. For example '`{{ get_env(name='LANG') }}'`
gives you access to the '`LANG`' environment variable.

In addition, Tp-Note defines the following variables:

* '`{{ path }}`' is the canonicalized fully qualified path name corresponding
  to Tp-Note's positional command line parameter '`<path>`'. If none was
  given on the command line, '`{{ path }}`' contains the current working
  directory path.

* '`{{ dir_path }}`' is identical to '`{{ path }}`' with one exception: if
  '`{{ path }}`' points to a file, the last component (the file name) is omitted
  and only the directory path is retained. If '`{{ path }}`' points to a
  directory, '`{{ dir_path }}`' equals '`{{ path }}`'.

* '`{{ note_fm_text }}`': is the header as raw text of the file '`{{ path }}`'
  points to.  Note, this variable is only available in the
  templates '`from_text_file_*`', '`sync_filename`' and the HTML templates below.

* '`{{ note_body_text }}`': is the content of the file '`{{ path }}`'
  points to. If the file does not start with a front matter, this
  variable holds the whole content. Note, this variable is only available in the
  templates '`from_text_file_*`', '`sync_filename`' and the HTML templates below.

* '`{{ note_file_date }}`': is the file system creation date of the file
  '`{{ path }}`' points to. Note, this variable is only available in the
  templates '`from_text_file_*`', '`sync_filename`' and the HTML templates below.

* '`{{ clipboard }}`' is the complete clipboard text.  In case the clipboard's
  content starts with a YAML header, the latter does not appear in this
  variable.

* '`{{ clipboard_header }}`' is the YAML section of the clipboard data, if
  one exists. Otherwise: empty string.

* '`{{ stdin }}`' is the complete text content originating from the input
  stream '`stdin`'. This stream can replace the clipboard when it is not
  available.  In case the input stream's content starts with a YAML header,
  the latter does not appear in this variable.

* '`{{ stdin_header }}`' is the YAML section of the input stream, if
  one exists. Otherwise: empty string.

* '`{{ extension_default }}`' is the default extension for new notes
  (can be changed in the configuration file),

* '`{{ username }}`' is the content of the first non-empty environment
  variable: '`TPNOTEUSER`', '`LOGNAME`', '`USER`' or '`USERNAME`'.

* '`{{ lang }}`' contains the user's language tag as defined in
  [RFC 5646](http://www.rfc-editor.org/rfc/rfc5646.txt).
  Not to be confused with the UNIX '`LANG`' environment variable from which
  this value is derived under Linux/MacOS.
  Under Windows, the user's language tag is queried through the WinAPI.
  If defined, the environment variable '`TPNOTELANG`' overwrites the value
  of '`{{ lang }}`' (all operating systems).

The following '`{{ fm_* }}`' variables are typically generated, _after_ a
content template was filled in with data: For example a field named '`title:`'
in the content template '`tmpl.new_content`' will generate the variable
'`fm_title`' which can then be used in the corresponding '`tmpl.new_filename`'
filename template. '`{{ fm_* }}`' variables are generated dynamically. This
means, a YAML front-matter variable '`foo:`' in a note will generate a
'`{{ fm_foo }}`' template variable. On the other hand, a missing '`foo:`'
will cause '`{{ fm_foo }}`' to be undefined.

Please note that '`{{ fm_* }}`' variables are available in all filename
templates and in the '`tmpl.from_clipboard_yaml_content`' content template only.

* '`{{ fm_title }}`' is the '`title:`' as indicated in the YAML front-matter of
  the note.

* '`{{ fm_subtitle }}`' is the '`subtitle:`' as indicated in the YAML front
  matter of the note.

* '`{{ fm_author }}`' is the '`author:`' as indicated in the YAML front-matter
  of the note.

* '`{{ fm_lang }}`' is the '`lang:`' as indicated in the YAML front-matter of
  the note.

* '`{{ fm_file_ext }}`' holds the value of the optional YAML header variable
  '`file_ext:`' (e.g. '`file_ext: "rst"`').

* '`{{ fm_sort_tag }}`': The sort tag variable as defined in the YAML front
  matter of this note (e.g. '`sort_tag: "20200312-"`').

* '`{{ fm_all }}`': is a collection (map) of all defined '`{{ fm_* }}`'
  variables.  It is used in the '`tmpl.from_clipboard_yaml_content`' template,
  typically in a loop like:

  ```yaml
  {% for key, value in fm_all %}{{ key }}: {{ value | json_encode }}
  {% endfor %}
  ```

Important: there is no guarantee, that any of the above '`{{ fm_* }}`'
variables are defined! Depending on the last content template result, certain
variables might be undefined. Please take into consideration, that a defined
variable might contain the empty string '`""`'.

For a more detailed description of the available template variables, please
consult the '`const`' definitions in Tp-Note's source code file '`note.rs`'.



## Template filters

In addition to _Tera_'s [built-in
filters](https://tera.netlify.app/docs/#built-in-filters), Tp-Note comes with
some additional filters, e.g.: '`tag`', '`trim_tag`', '`stem`', '`cut`', '`heading`',
'`link_text`', '`link_dest`', '`link_title`' and '`ext`'.

A filter is always used together with a variable. Here are some examples:

* '`{{ path | filename }}`' returns the final component of '`{{ path }}`'.
  If '`{{ path }}`' points to a file, the filter returns the complete
  filename including its sort tag, stem, copy-counter, dot and extension. If the
  '`<path>`' points to a directory, the filter returns the final directory name.

* '`{{ path | tag }}`' is the sort tag (numerical filename prefix) of the final
  component of '`{{ path }}`', e.g. '`01-23_9-`' or '`20191022-`'. It is similar
  to '`{{ path | filename }}`' but without returning its stem, copy-counter and
  extension.

* '`{{ path | stem }}`' is similar to '`{{ path | filename }}`' but without its
  sort tag, copy-counter and extension. Only the stem of '`{{ path }}`''s last
  component is returned.

* '`{{ path | copy_counter }}`' is similar to '`{{ path | filename }}`' but
  without its sort tag, stem and extension. Only the copy counter of
  '`{{ path }}`''s last component is returned.

* '`{{ path | ext }}`' is '`{{ path }}`'’s file extension without
  dot (period), e.g. '`txt`' or '`md`'.

* '`{{ path | ext | prepend_dot }}`' is '`{{ path }}`'’s file extension with
  dot (period), e.g. '`.md`' or '`.md`'.

* '`{{ path | trim_tag }}`' returns the final component of '`path`' which might
  be a directory name or a file name. Unlike the '`filename`' filter (which also
  returns the final component), '`trim_tag`' trims the sort tag if there is one.

* '`{{ dir_path | trim_tag }}`' returns the final component of '`dir_path`'
  (which is the final directory name in '`{{ path }}`'). Unlike the '`filename`'
  filter (which also returns the final component), '`trim_tag`' trims the sort
  tag if there is one.

* '`{{ clipboard | cut }}`' is the first 200 bytes from the clipboard.

* '`{{ clipboard | heading }}`' is the clipboard's content until the
  end of the first sentence, or the first newline.

* '`{{ clipboard | link_text }}`' is the name of the first Markdown or
  ReStructuredText formatted link in the clipboard.

* '`{{ clipboard | link_dest }}`' is the URL of the first Markdown or
  ReStruncturedText formatted link in the clipboard.

* '`{{ clipboard | link_title }}`' is the title of the first Markdown or
  ReStruncturedText formatted link in the clipboard.

* '`{{ username | capitalize | json_encode }}`' is the capitalized JSON encoded
  username. As all YAML front-matter is JSON encoded, this filter code must be
  appended to any template variable placed in the front-matter block.

* '`{{ fm_subtitle | sanit }}`' is the note's subtitle as defined in its
  front-matter, sanitized in a file system friendly form. Special characters are
  omitted or replaced by '`-`' and '`_`'. See the section _Filename template
  convention_ for more details about this filter.

* '`{{ fm_title | sanit(force_alpha=true) }}`' is the note's title as defined in its
  front-matter.  Same as above, but strings starting with a sort tag are
  prepended by an apostrophe to avoid ambiguity.

* '`{{ fm_all | remove(var='fm_title') }}`' represents a collection (map) of
  all '`fm_*`' variables, exclusive of the variable '`fm_title`'.



## Content template conventions

Tp-Note distinguishes two template types: content templates are used to create
the note's content (front-matter and body) and the corresponding
filename templates '`tmpl.*_filename`' are used to calculate the note's
filename.  By convention, content templates appear in the configuration file in
variables named '`tmpl.*_content`'.

Strings in the YAML front-matter of content templates are JSON encoded.
Therefore, all variables used in the front-matter must pass an additional
'`json_encode()`'-filter. For example, the variable '`{{ dir_path | stem }}`'
becomes '`{{ dir_path | stem() | json_encode() }}`' or just
'`{{ dir_path | stem | json_encode }}`'.



## Filename template conventions

By convention, filename templates appear in the configuration file in variables
named '`tmpl.*_filename`'.  When a content template creates a new note, the
corresponding filename template is called afterwards to calculate the filename
of the new note. Please note that, the filename template
'`tmpl.sync_filename`' has a special role as it synchronizes the filename
of existing note files. Besides this, as we are dealing with filenames we must
guarantee, that the filename templates produce only file system friendly
characters.  For this purpose Tp-Note provides the additional Tera filters
'`sanit`' and '`sanit(force_alpha=true)`':

* The '`sanit()`' filter transforms a string in a file system friendly from. This
  is done by replacing forbidden characters like '`?`' and '`\\`' with '`_`'
  or space. This filter can be used with any variable, but is most useful with
  filename templates. For example, in the '`tmpl.sync_filename`'
  template, we find the expression '`{{ subtitle | sanit }}`'.
  Note that the filter recognizes strings that represent a so-called dot file
  name and treats them a little differently by prepending them with an
  apostrophe: a dot file is a file whose name starts with '`.`' and that does
  not contain whitespace. It may or may not end with a file extension.
  The apostrophe preserves the following dot from being filtered.

* '`sanit(force_alpha=true)`' is similar to the above, with one exception: when a
  string starts with a digit '`0123456789`' or '`-_ `', the whole string is
  prepended with `'`. For example: "`1 The Show Begins`" becomes "`'1 The Show
  Begins`". This filter should always be applied to the first variable
  assembling the new filename, e.g. '`{{ title | sanit(force_alpha=true )}`'. This
  way, it is always possible to distinguish the sort tag from the actual
  filename. The default sort tag separator '`'`' can be changed with the
  configuration variable '`filename.sort_tag_extra_separator`'.

In filename templates most variables must pass either the '`sanit`' or the
'`sanit(force_alpha=true)`' filter. Exception to this rule are the sort tag variables
'`{{ path | tag }}`' and '`{{ dir_path | tag }}`'. As these are guaranteed to
contain only the file system friendly characters '`0123456789 -_`', no
additional filtering is required. Please note that in this case a
'`sanit`'-filter would needlessly restrict the value range of sort tags as
they usually end with a '`-`', a character, which the '`sanit`'-filter screens
out when it appears in leading or trailing position. For this reason no
'`sanit`'-filter is not allowed with '`{{ path | tag }}`' and
'`{{ dir_path | tag }}`'.






# SECURITY AND PRIVACY CONSIDERATIONS

As discussed above, Tp-Note's built-in viewer sets up an HTTP server on the
'`localhost`' interface with a random port number.

For security reasons, Tp-Note limits the set of files the viewer is
able to publish. To summarize, a file is only served:

1. when it is referenced in one of the currently viewed Tp-Note files,
2. when its file extension is registered with the '`[viewer] served_mime_type`'
   list,
3. if the number of so far viewed Tp-Note files,
   '`[viewer] displayed_tpnote_count_max`' is not exceeded,
4. when it's located under a directory containing a marker file named
   '`.tpnoteroot`' (without marker file this condition is void).

The HTTP server runs as long as the launched web browser window is open.
Note, that the server not only exposes the displayed note file, but also all
referenced inline images and other linked TP-Note files. Internally, the
viewer maintains a list of _referenced local URLs_. For security reasons,
only listed files are served. To limit data exfiltration in case an attacker
gains access to an account on your machine, the number of served Tp-Note files
is limited by the configurable value '`[viewer] displayed_tpnote_count_max`'.

In addition to the above quantitative restriction, Tp-Note's built-in viewer
serves only files whose file extensions are registered with the
'`[viewer] served_mime_type`' configuration file variable.  The latter allows
to disable the _follow links to other Tp-Note files_ feature by removing all
'`text/*`' mime types from that list.

Another security feature is the '`.tpnoteroot`' marker file. When Tp-Note
opens a note file, it checks all directories above, one by one, until it
finds the marker file '`tpnoteroot`'. Tp-Note's viewer will never serve a file
located outside the root directory and its children. When no '`.tpnoteroot`'
file is found, the root directory is set to '`/`', which disables this
security feature.

As Tp-Note's built-in viewer binds to the '`localhost`' interface, the exposed
files are in principle accessible to all processes running on the computer. As
long as only one user is logged into the computer at a given time, no privacy
concern is raised: any potential attacker must be logged in, in order to access
the `localhost` HTTP server.

This is why on systems where multiple users are logged in at the same time,
it is recommended to disable Tp-Note's internal HTTP server by setting the
configuration file variable '`arg_default.edit = true`'. Alternatively, you
can also compile Tp-Note without the '`viewer`' feature. Note, that even if
the viewer feature disabled, the '`--export`' command line option still works:
This allows the authorized user to render the note to HTML manually.

**Summary**: As long as Tp-Note's built-in note viewer is running, the note
file and all its referenced (image) files are exposed to all users logged into
the computer at that given time. This concerns only local users, Tp-Note
never exposes any information to the network or on the Internet.




# ENVIRONMENT VARIABLES

TPNOTELANG

:   Tp-Note stores the user's locale settings in the template variable
    '`{{ lang }}`'. When set, '`TPNOTELANG`' overwrites the locale settings.

TPNOTEUSER

:   The template variable '`{{ username }}`' is the content of the first
    non-empty variable: '`TPNOTEUSER`', '`LOGNAME`', '`USER`' or '`USERNAME`'.




# EXIT STATUS

Normally the exit status is '`0`' when the note file was processed without
error or '`1`' otherwise. If Tp-Note can not read or write its
configuration file, the exit status is '`5`'.

When '`tpnote -n -b <FILE>`' returns the code '`0`', the note file has a
valid YAML header with a '`title:`' field. In addition, when
'`tpnote -n -b -x - <FILE>`' returns the code '`0`', the note's body was
rendered without error.




# RESOURCES

Tp-Note it hosted on:

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



---
title:    "Tp-Note: save and edit your clipboard content as a note file"
subtitle: "Organize your notes with your favourite editor and markup language"
author:   "Jens Getreu"
date:     "2021-08-25"
version:  "1.19.3"
filename_sync: false
lang:     "en-GB"
---

Markup languages like *Markdown* [^1] are perfectly suited for fast
note taking. Type your notes with your favourite text editor and observe
the live rendered text in your web browser.

_Tp-Note_ helps you to quickly get started writing notes with its powerful
template system. If you like to keep your notes next to your files and you care
about expressive filenames, then _Tp-Note_ might be the tool of your choice.
As _Tp-Note_ synchronizes the note's filename with its document title, you
will find your notes more easily.

_Tp-Note_ is available for Linux, Windows and MacOS. This manual illustrates
its main use cases and how to get started:

1. Fast start note taking (when the lecture starts).
2. Take a note about an existing or downloaded file.
3. Bookmark and comment a hyperlink.
4. Copy and annotate a page from a book.
5. Best practice.
6. Note taking for system administrators.

If you want to customize _Tp-Note_ with your own templates or if you want to
use another markup language than Markdown, please consult [Tp-Note's man-page] for
more technical details. It also explains how to change _Tp-Note_'s default text
editor.

The project is hosted on Gitlab:
[getreu/tp-note](https://gitlab.com/getreu/tp-note). The project's webpage is on
[http://blog.getreu.net](http://blog.getreu.net/projects/tp-note/).
The documentation of this project is dived into two parts:

* User manual

  [Tp-Note user manual - html](https://blog.getreu.net/projects/tp-note/tpnote--manual.html)

  [Tp-Note user manual - pdf](https://blog.getreu.net/_downloads/tpnote--manual.pdf)

* Unix man-page (more technical)

  [Tp-Note manual page - html](https://blog.getreu.net/projects/tp-note/tpnote--manpage.html)

  [Tp-Note manual page - pdf](https://blog.getreu.net/_downloads/tpnote--manpage.pdf)




# How students take notes

A fellow student still uses paper and pen. I asked her why, and she replied with
"It helps me concentrate better. My computer distracts me. I will do many other
things and I wont remain concentrated on my task.".

This is certainly true. As far as I am concerned, I am not good at logistics.
For me having all my documents and notes on one little machine is a blessing.

The following sections illustrate how to work with _Tp-Note_ with my most
common workflows.



## Fast start note taking (when the lecture starts)

![The folder in which the new note will be created.](images/workflow1-1.png){width="10cm"}

Alternatively you can open the folder where you want to create a new note and
right-click on some empty white space.

![The new unmodified note created by template on disk](images/workflow1-2.png){width="10cm"}

![The new unmodified note created by template](images/workflow1-3.png){width="11cm"}

![Change the title](images/workflow1-4.png){width="11cm"}

![Add some text](images/workflow1-5.png){width="11cm"}

![The new note file on disk after closing the editor](images/workflow1-6.png){width="10cm"}

> **Note**
>
> Before and after launching the editor _Tp-Note_ renames the file to be in
> sync with the note's metadata (i.e. title and subtitle).
> For more details see [How it works: Organize your files and notes with sort-tags].



## Taking notes about a file

![We want to take a note about a pdf](images/workflow2-1.png){width="9cm"}

![The new unmodified note created automatically](images/workflow2-2.png){width="11cm"}

The source code of the note shows the link with its target. The left-click, opens the `.odt` document.

```yaml
---
title:      "Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter.odt"
subtitle:   "Note"
author:     "getreu"
date:       "March 10, 2020"
lang:       "en-GB"
---

[03-Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter.odt](<03-Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter.odt>)
```

![Annotate](images/workflow2-3.png){width="11cm"}

![The new note file on disk after closing the editor](images/workflow2-4.png){width="8cm"}



## Document the download location of a local file

The approach is similar to what we have seen in the [previous chapter](#taking-notes-about-a-file):

![Copy the location of the download page as Markdown](images/workflow6-1.png){width="9cm"}

Note: for convenience I use in this example the Firefox browser addon [Copy
Selection as Markdown] to copy the hyperlink. If this addon is not available,
you can also copy the URL directly from the search bar.

![Select the file to annotate and start Tp-Note](images/workflow6-2.png){width="8cm"}

![The new unmodified note created automatically](images/workflow6-3.png){width="11cm"}

The source-code of the note shows the links with their targets.

```yaml
---
title:      "ascii-hangman.exe"
subtitle:   "URL"
author:     "getreu"
date:       "2020-08-27"
lang:       "en-GB"
---

[ascii-hangman.exe](<ascii-hangman.exe>)

[ASCII-Hangman - hangman game for children with ASCII-art rewarding](<https://blog.getreu.net/projects/ascii-hangman/#distribution>)

```

![The new note file on disk after closing the editor](images/workflow6-4.png){width="8cm"}



## Bookmark and comment a hyperlink

![Copy a link in Markdown format](images/workflow3-2.png){width="11cm"}

To copy a link in Markdown format a browser addon is needed. I recommend the
addons [Copy as Markdown] and [Copy Selection as Markdown] available
for Firefox.

[Copy as Markdown]: https://addons.mozilla.org/en-GB/firefox/search/?q=copy%20as%20markdown
[Copy Selection as Markdown]: https://addons.mozilla.org/en-GB/firefox/addon/copy-selection-as-markdown/?src=search

![Right-click on or in the new note's destination folder and start Tp-Note](images/workflow3-3.png){width="9cm"}

![The new unmodified note created automatically](images/workflow3-4.png){width="9cm"}

The source code of the note shows the link target:

```yaml
---
title:      "Rustacean Station"
subtitle:   "URL"
author:     "getreu"
date:       "March 10, 2020"
lang:       "en-GB"
---

[Rustacean Station](<https://rustacean-station.org/>)
```

In this example we copied only one Markdown link "Rustacean Station".
Furthermore, *Tp-Note* allows you also to insert a list of Markdown links in a
template. For example with [Copy as Markdown] you could copy a link list of all
open tabs. In this case, _Tp-Note_ would retain only the name of the first link
as document title, whereas the whole link list would appear in the body of the
note.

![Annotate](images/workflow3-5.png){width="11cm"}

![The new note file on disk after closing the editor](images/workflow3-6.png){width="7cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```



## Copy a page from a book

![Copy some chapters](images/workflow4-1.png){width="10cm"}

![Right-click on or in the new note's destination folder and start Tp-Note](images/workflow4-2.png){width="7cm"}

![The new unmodified note created automatically](images/workflow4-3.png){width="9cm"}

```{=docbook}
<?dbfo-need height="4cm" ?>
```

The source code of the note shows the completed template:

```yaml
---
title:      "Winston kept his back turned to the telescreen"
subtitle:   "Note"
author:     "getreu"
date:       "2020-03-23"
lang:       "en-GB"
---

Winston kept his back turned to the telescreen. It was safer, though, as he well
knew, even a back can be revealing. A kilometer away the Ministry of Truth, his
place of work, towered vast and white above the grimy landscape...
```

In this example we copied only text. *Tp-Note* suggests the first sentence as
title. This can be changed before saving as illustrated above. Here we just save
and observe the file on the disk.

![The new note file on disk after closing the editor](images/workflow4-4.png){width="7cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```



## Best practice

_Tp-Note's_ greatest advantage is it's flexibility. It easily integrates with
your workflow.  As people work differently, there is no best usage either.
Nevertheless, after having used _Tp-Note_ for some months now, here my personal
preferences and configuration:

* [Tp-Note](https://blog.getreu.net/projects/tp-note/)
* Addon for Firefox: [Copy Selection as Markdown]
* Markdown editor : [Mark Text](https://marktext.app/)
  <!--or [Typora](https://typora.io/)-->
* Integration with the file manager (start entry in context menue) as described below.

My favourite Markdown editor at the moment is [Mark Text](https://marktext.app/).
I like its clean and simple interface helping me to stay focused on the content
while writing.[^3]

When copying extracts from a web-page, I often need to preserve its hyperlinks.
For this, the Firefox browser addon [Copy Selection as Markdown] is very handy.
It precedes the copied extract with a hyperlink to the origin of the webpage.
When _TP-Note_ reads the extract from the clipboard, it uses the first Markdown
hyperlink it can find for composing the note's title and its filename on disk.
This is why the web page's name ends up automatically in the note's title and
filename.  Here a sample work flow:

![A webpage to copy extracts from](images/workflow5-1.png){width=12cm}

![“Copy Selection as Markdown”](images/workflow5-2.png){width="12cm"}

![Right-click on or in the new note's destination folder and start
Tp-Note](images/workflow5-3.png){width="8cm"}

![The new template generated note opened with
Typora](images/workflow5-4.png){width="11cm"}

![The new note file on disk after closing the
editor](images/workflow5-5.png){width="9cm"}

Note, no content or filename was edited manually in this example. _Tp-Note_
takes care of interpreting the clipboard's content and generating the file on
disk.

[Copy Selection as Markdown]: https://addons.mozilla.org/en-GB/firefox/addon/copy-selection-as-markdown/?src=search



## Note taking for system administrators (and console lovers)

As _Tp-Note_ makes extensive use of the clipboard, it mainly targets desktop
systems running a graphical environment. But also when working on the console
_Tp-Note_ can be useful with its built-in clipboard simulation: Instead of
copying the content into your clipboard, pipe it into _Tp-Note_:

```shell
echo  "Some clipboard content" | tpnote
```


### Installation on headless systems

On headless systems, it is recommended to compile _Tp-Note_ without its
`viewer` and `message-box` features:

```shell
cargo install --no-default-features --features renderer tp-note
sudo cp ~/.cargo/bin/tpnote /usr/local/bin
```


### Typical workflows

The following examples work with the full featured version of _Tp-Note_ as
well as with the `--no-default-features` console only version.

* Document a downloaded file:

  Download the file
  [i3-extensions.zip](http://blog.getreu.net/_downloads/i3-extensions.zip):

  ```bash
  wget "http://blog.getreu.net/_downloads/i3-extensions.zip"
  ```

  Document from where you downloaded the file:

  ```bash
  echo  "[download](<http://blog.getreu.net/_downloads/i3-extensions.zip>)" | tpnote i3-extensions.zip
  ```

  This creates the file `i3-extensions.zip--URL.md` with the
  following content:

  ```yaml
  ---
  title:      "i3-extensions.zip"
  subtitle:   "URL"
  author:     "getreu"
  date:       "2020-09-03"
  lang:       "en-GB"
  ---

  [i3-extensions.zip](<i3-extensions.zip>)

  [download](<http://blog.getreu.net/_downloads/i3-extensions.zip>)
  ```

* Download a webpage, convert it to Markdown and insert the result
  into a _Tp-Note_ file. The note's title is the name of the
  first hyperlink found in the webpage.

  Install `pandoc` and `curl`:

  ```bash
  sudo apt install pandoc curl
  ```

  Download and convert:

  ```bash
  curl 'https://blog.getreu.net' | pandoc -f html -t markdown_strict | tpnote
  ```

* Download a webpage while preserving its metadata:

  Same as above, but the following preserves the webpage's metadata, e.g.
  title, author, date... :

  ```bash
  curl 'https://blog.getreu.net' | pandoc --standalone -f html -t markdown_strict+yaml_metadata_block | tpnote
  ```

  creates the note file `20200910-Jens\ Getreu\'s\ blog.md` with the webpage's
  content.

* Generate a note for a given content with YAML header:

  ```bash
  echo -e "---\ntitle: Todo\nfile_ext: mdtxt\n---\n\nnothing" | tpnote
  ```

  creates the file `20200910-Todo.mdtxt` with the content:

  ```yaml
  ---
  title:      "Todo"
  subtitle:   ""
  author:     "getreu"
  date:       "2020-09-13"
  lang:       "en-GB"
  file_ext:   "mdtxt"
  ---

  nothing
  ```

* Reformat the header of a note file:

  ```bash
  mv "20200921-My Note.md" "20200921-My Note-(1).md"
  cat "20200921-My Note-(1).md" | tpnote --batch
  ```

  creates the file `20200921-My Note.md` with a rearranged header
  and the same body.

* Launch, for once only, a different text editor.\
  The external text editor, _Tp-Note_ defaults to, is defined in the configuration
  file and can be changed there. If you want to use a different text editor
  just for a one-shot, type:

  ```bash
  FILE=$(tpnote --batch); vi "$FILE"; tpnote --batch "$FILE"
  ```

* Create a new note overwriting the template's default for `subtitle`:

  ```bash
  cd dev
  echo -e "---\nsubtitle: Draft\n---\n# Draft" | tpnote
  ```

  creates the note file `20200925-dev--Draft.md` with the content:

  ```yaml
  ---
  title:      "dev"
  subtitle:   "Draft"
  author:     "getreu"
  date:       "2020-09-25"
  lang:       "en-GB"
  ---

  # Draft
  ```

* Synchronize filenames and headers of all note files in the current directory:

  ```bash
  find . -type f -name "*.md" -exec tpnote --batch {} \; >/dev/null
  ```

* Generate an HTML rendition of an existing note file in the same directory:

  ```bash
  tpnote --export='./my_notes' './my_notes/20210209-debug--Note.md'
  ```

  or, equivalent but shorter:

  ```bash
  tpnote --export= './my_notes/20210209-debug--Note.md'
  ```

  or, even shorter:

  ```bash
  tpnote -x '' './my_notes/20210209-debug--Note.md'
  ```

* Generate a PDF rendition of an existing note file :

  Install the `wkhtmltopdf`-tool:

  ```bash
  sudo apt install wkhtmltopdf
  ```

  Generate the PDF rendition of the existing note `20210122-my--Note.md`:

  ```bash
  tpnote -x - '20210122-my--Note.md' | wkhtmltopdf - '20210209-debug--Note.md.pdf'
  ```

* View and follow hyperlinks in a note file:

  When no graphical environment is available, _Tp-Note_ disables the viewer
  feature with its internal HTTP server. However, in order to follow
  comfortably hyperlinks in you notes, you can always use _Tp-Note_'s HTML
  export.

  Install the text based web browser `lynx`:

  ```bash
  sudo apt install lynx
  ```

  Convert the existing note `20210122-my_note.md` into HTML and
  open the rendition with `lynx`:

  ```bash
  tpnote -x - '20210122-my_note.md' | lynx --stdin
  ```

  The above also works in case _Tp-Note_ was compiled with
  `--no-default-features` which is recommended for headless systems.




# How it works: Organize your files and notes with sort-tags

Consider the following _Tp-Note_-file:

    20151208-Make this world a better place--Suggestions.md

The filename has 4 parts:

    {{ fm_sort_tag }}{{ fm_title }}--{{ fm_subtitle }}.{{ fm_file_ext }}

A so called _sort-tag_ is a numerical prefix at the beginning of the
filename. It is used to order files and notes in the file system. Besides
numerical digits and whitespace, a _sort-tag_ can be any combination of
`-_.` and is usually used as:

* *chronological sort-tag*

        20140211-Reminder.doc
        20151208-Manual.pdf
        2015-12-08-Manual.pdf

* or as a *sequence number sort-tag*.

        02-Invoices
        08-Tax documents
        09_02-Notes
        09.02-Notes

The figures below illustrate organizing files with *sort-tags".

![Folders with sequence number sort-tag](images/filing-system1.png){width="10cm"}

![File with chronological sort-tag](images/filing-system2.png){width="7cm"}

When _Tp-Note_ creates a new note, it automatically prepends a *chronological
sort-tag* of today. The `{{ fm_title }}` part is usually derived from the parent
directory's name omitting its own *sort-tag*.




# Installation

Depending on the operating system, the installation process is more
or less automated and can be divided into two steps:

1. [Minimum setup with no file manager integration]\
   This step consists of downloading _Tp-Note_'s binary and copying it to your hard-disk.
   See section [Distribution](https://blog.getreu.net/projects/tp-note/#distribution)
   on _Tp-Note_'s [project page](https://blog.getreu.net/projects/tp-note/#distribution)
   for a list of available packages and binaries.

2. [Optional integration with your file manager].

At the moment of this writing, an installer automating steps 1. and 2. is available for
Windows only. Packages for Debian Linux and Ubuntu help you with step 1. For other operating
systems check section [Distribution](https://blog.getreu.net/projects/tp-note/#distribution)
for precompiled binaries or
[build Tp-Note](https://blog.getreu.net/projects/tp-note/#building)
yourself.


```{=docbook}
<?dbfo-need height="6cm" ?>
```



## Minimum setup without file manager integration

_Tp-Note_'s template engine can be tested and used without window manager
integration. This section shows a minimum setup to get started quickly.[^inst]

[^inst]: Please also consult the section
  [Installation](https://blog.getreu.net/projects/tp-note/#installation) in the
  projects Readme-document to learn how to install a complete setup.

* **Windows**

  Download the
  [tpnote executable for Windows](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tpnote.exe) [^4]
  and place it on your desktop.

* **Linux**

  Download the _Tp-Note_-binary for Linux and place it on your Desktop:

  ``` sh
  > cd ~/Desktop
  > wget https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tpnote
  > chmod 755 tpnote
  ```

A new _Tp-Note_-icon appears on your desktop.



## Usage of the minimum setup

To create a new note, either double-click on the _Tp-Note_-icon, or drag a
folder or file and drop it on the _Tp-Note_-icon. This opens an editor with
your new note.

For more comfort, I recommend integrating _Tp-Note_ into the file manager's
context menu. See section [Optional integration with your file manager] for
more details.  There you also will find a list of tested Markdown editors, if
you wish to use one. _Tp-Note_ works with every Unicode text editor and
Markdown editor (see section [Optional customization] and man-page for more
details).

_Tp-Note_'s note files can be printed directly from the viewer (webbrowser)
window or first converted into `.html` with `tpnote -x '' mynote.md`. For other
formats e.g. `.docx`, `.odt` and `.pdf` use [Pandoc](https://pandoc.org/)
or `wkhtmltopdf`.


```{=docbook}
<?dbfo-need height="6cm" ?>
```



## Troubleshooting


### Incompatible configuration files

While upgrading _Tp-Note_, new features may cause a change in _Tp-Notes_'s
configuration file structure and the program may fail to start displaying an
error message. Please consult the following section
[Upgrading](https://blog.getreu.net/projects/tp-note/#upgrading) in the
project's Readme document for more information about incompatible configuration
files.


### Debugging

`Tp-Note`'s logging feature is controlled with the command line-options:
`--debug` and `--popup` or by the corresponding configuration file variables:
`[arg_default] debug` and `[arg_default] popup`.

Please consult _Tp-Note_'s manual page for more information about the
debugging options `--debug` and `--popup` and how to use them.

BTW: Under Windows, when you see only Chinese characters in the _Notepad_
text editor, update Windows to the latest version or install
[Notepad++](https://notepad-plus-plus.org/).



## Optional customization


### Change the default markup language

* Your preferred markup language is not *Markdown*, but ReStructuredText*,
  *Asciidoc*, *T2t*, *Textile*, *Wiki* or *Mediawiki*? Change it!

  _Tp-Note_'s core function is a template system and as such it is
  markup language agnostic. The default templates largely abstain from
  markup specific code, which makes it easy to switch the default new note's
  markup language. Please refer to [Tp-Note's man-page] to learn how to
  change its templates in the configuration file.

  In addition, _Tp-Note_ comes with a build in note viewer which is optional and
  independent from its core functionality. When _Tp-Note_ opens a note file, it
  detects the markup language through the note file extension and launches the
  associated builtin markup renderer. The whole process can be customized in
  _Tp-Note_'s configuration file. Please refer to [Tp-Note's man-page] for
  details.


### Chose your favourite text editor and make it default

* Your preferred text editor is not *Notepad*? Change it![^1]

  Note taking with _Tp-Note_ is more fun with a good markup (Markdown)
  text editor, although any Unicode text editor will do (even Notepad >=
  Windows 10-update 1903). _Tp-Note_ is preconfigured to work with:

  - [Mark Text](https://marktext.app/) — a simple and elegant Markdown editor
    focused on speed and usability.
  <!-- [Typora — a Markdown editor, Markdown reader.](https://typora.io/)
    Make sure that you have at least version 0.9.89 installed, as this version
    contains a [bugfix](https://github.com/typora/typora-issues/issues/3348)
    required to work properly with _Tp-Note_.
   -->

  - [ReText — Simple but powerful editor for Markdown and reStructuredText](https://github.com/retext-project/retext)

  - _VS-Code_, _Atom_ ...

  Please refer to [Tp-Note's man-page] to learn how to register your text
  editor with _Tp-Note_'s configuration file.


### Integrate _Tp-Note_ with your file manager

* You prefer working in a desktop environment instead of working on a shell?

  Read the following section [Optional integration with your file manager] to
  learn how to configure your file manager's context menu to launch _Tp-Note_.


### Choose the web browser for note viewing and make it your default

* Is your preferred web browser is not *Firefox*? Change it![^1]

  After opening the text editor, _Tp-Note_ internally renders the note file
  and opens a web browser to display the note's content. Which web browser on
  your system will be launched, depends on which of them _Tp-Note_ finds
  first by searching through a configurable list of well known web browsers.

  ![Tp-Note with open text editor (left) and viewer (right)](images/editor_and_viewer.png){width="12cm"}

  Please refer to [Tp-Note's man-page] to learn how change which web browser
  _Tp-Note_ launches as note viewer.

### Customize the way how _Tp-Note_'s viewer renders the note's content

The way the note will appear in your web browser depends on:

* which of _Tp-Note_'s internal markup renderer is used and

* the HTML template, that defines the visual appearance
  (colours, fonts etc.) of the rendition.

Please refer to [Tp-Note's man-page] to learn how to register a file
extension with a particular markup renderer or to learn
how to change the HTML-template that renders the note's content.




# Optional integration with your file manager

This section shows how to integrate _Tp-Note_ in the context menu of your
file manager. The context menu appears, when you click right on a file icon,
on a directory icon or on the white space in between (cf. figure below). In
the following we will configure the file manager to launch _Tp-Note_ with the
path to the selected icon.

![Tp-note in the context-menu (right-click menu)](images/workflow2-1.png){width="9cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```


## Windows file explorer configuration

_Tp-Note_ is distributed with a Microsoft Windows Installer package
`tpnote-x.x.x-x86_64.msi`, which automates the following key registration.
Omit this section if you have installed _Tp-Note_ through this `.msi` package!

1. Make the directory `C:\Windows\tpnote\bin\` and move `tpnote.exe`
   into it.

2. Open the *notepad* text editor and paste the following registry key into
   it.

        Windows Registry Editor Version 5.00

        [HKEY_CLASSES_ROOT\Directory\Background\shell\New Tp-Note]

        [HKEY_CLASSES_ROOT\Directory\Background\shell\New Tp-Note\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\""

        [HKEY_CLASSES_ROOT\*\OpenWithList\tpnote.exe]
        @=""


        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\edit.tpnote.exe]
        @="Edit Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\edit.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"%1\""

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\view.tpnote.exe]
        @="View Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\view.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"-v\" \"-n\" \"%1\""

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\export.tpnote.exe]
        @="Export Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.txt\shell\export.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"--export=\" \"%1\""


        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\edit.tpnote.exe]
        @="Edit Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\edit.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"%1\""

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\view.tpnote.exe]
        @="View Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\view.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"-v\" \"-n\" \"%1\""

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\export.tpnote.exe]
        @="Export Tp-Note"

        [HKEY_CLASSES_ROOT\SystemFileAssociations\.md\shell\export.tpnote.exe\command]
        @="\"C:\\Program Files\\tpnote\\bin\\tpnote.exe\" \"--export=\" \"%1\""

3. Save the file as:

   * File name: `tpnote.reg`
   * Save as type: `All files`
   * Encoding:  `UTF-16 LE`


4. Double-click on `tpnote.reg` and confirm several times.

5. Assign `tpnote` as default application for `.txt`-files

   ![Click-right on .txt file to open file properties](images/Properties-Opens_with-Notepad.png){width="8cm"}

   ![Press "Change ..." and choose "Tp-Note"](images/Properties-Opens_with-tp-note.png){width="8cm"}



## Linux file manager configuration

To simplify the configuration we first place the binary _Tp-Note_
in our `$PATH`:

```sh
cd /usr/local/bin
sudo wget https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tpnote
sudo chmod 755 tpnote
```

_Debian_ and _Ubuntu_ user can also download [Debian/Ubuntu package] and install it with:

``` sh
sudo dpkg -i tp-note_X.X.X_amd64.deb
```


### Configure Thunar's custom actions

Most file manager allow extending the context menu. As an example, the
following images show the configuration of the *Thunar* file manger.

#### Add context menu entry: Edit Tp-Note

In *Thunar*'s menu go to:

    Edit -> Configure custom actions...

![Thunar's custom action configuration](images/custom_actions1.png){width="8cm"}

![Edit custom action](images/edit_action.png){width="8cm"}

![Appearance Condition](images/appearance-condition.png){width="8cm"}

![Thunar's custom action configuration with Tp-Note](images/custom_actions2.png){width="8cm"}

[Debian/Unbuntu package]: https://blog.getreu.net/projects/tp-note/#tp-note-debianubuntu-installer-package

#### Add context menu entry: View Tp-Note

The following context menu entry allows us to view the rendered
note in the system's default web browser. This is very handy
when your note contains hyperlinks.

In Thunar, we add a custom action the same way as we
did before:

![Edit custom action](images/viewer-edit_action.png){width="8cm"}

![Appearance Condition](images/viewer-appearance-condition.png){width="8cm"}

```{=docbook}
<?dbfo-need height="4cm" ?>
```


### Configure Thunar's custom actions system-wide

Alternatively, instead of manually adding custom actions for each user, you can
do this system-wide:

    sudo nano /etc/xdg/Thunar/uca.xml

Search for `</actions>` and replace it with:

```xml
<action>
  <icon>accessories-text-editor</icon>
  <name>Tp-Note</name>
  <command>tpnote %f</command>
  <description>Tp-Note</description>
  <patterns>*</patterns>
  <directories/>
  <audio-files/>
  <image-files/>
  <other-files/>
  <text-files/>
  <video-files/>
</action>
<action>
  <icon>accessories-text-editor</icon>
  <name>Tp-Note View</name>
  <command>tpnote -v -n %f</command>
  <description>Tp-Note View</description>
  <patterns>*.txt; *.md;*.rst;*.adoc</patterns>
  <text-files/>
</action>
</actions>
```

The change becomes effective only after the user deletes his own configuration
file in `~/.config/Thunar/uca.xml`:

```shell
killall thunar
rm ~/.config/Thunar/uca.xml
thunar
```

```{=docbook}
<?dbfo-need height="8cm" ?>
```

**Optional bonus: add a menu entry "Download webpage as Markdown"**

In addition to the above, the following adds a context menu
entry for fast downloading and converting a webpage to a Markdown
Tp-Note file.

First install some helper programs:

    sudo apt install xclip curl pandoc

Then edit the system-wide Thunar configuration file:

    sudo nano /etc/xdg/Thunar/uca.xml

Search for `</actions>` and replace it with:

```xml
<action>
  <icon>accessories-text-editor</icon>
  <name>Download URL here</name>
  <command>curl $(xclip -o)| pandoc --standalone -f html -t markdown_strict+yaml_metadata_block+pipe_tables | tpnote  %F</command>
  <description>Download URL</description>
  <patterns>*</patterns>
  <directories/>
</action>
</actions>
```

The change becomes effective only after the user deletes his own configuration
file in `~/.config/Thunar/uca.xml`:

```shell
killall thunar
rm ~/.config/Thunar/uca.xml
thunar
```

```{=docbook}
<?dbfo-need height="8cm" ?>
```

**Optional bonus 2: add a menu entry "Export note as Pdf"**

First install the `wkhtmltopdf` filter program: [^5]

    sudo apt install wkhtmltopdf

Then edit the system-wide Thunar configuration file:

    sudo nano /etc/xdg/Thunar/uca.xml

Search for `</actions>` and replace it with: [^6]

```xml
<action>
  <icon>accessories-text-editor</icon>
  <name>Tp-Note Export</name>
  <command>tpnote --export=- %f | sed 's_&lt;_\r&lt;_g' -  | wkhtmltopdf --footer-center "[page]/[topage]" -B 2cm -L 2cm -R 2cm -T 2cm - %f.pdf</command>
  <description>Tp-Note Export</description>
  <patterns>*.txt; *.md;*.rst;*.adoc</patterns>
  <text-files/>
</action>
</actions>

```

The change becomes effective only after the user deletes his own configuration
file in `~/.config/Thunar/uca.xml`:

```shell
killall thunar
rm ~/.config/Thunar/uca.xml
thunar
```

```{=docbook}
<?dbfo-need height="8cm" ?>
```


### Configure Pcmanfm's custom actions system-wide

_Pcmanfm_ is the default file manager in _Lubuntu_ and in _Raspbian_ on the
Raspberry Pi.

Create the configuration file:

    sudo nano /usr/local/share/file-manager/actions/tpnote.desktop

with the following content:

```
[Desktop Entry]
Type=Action
Name[en]=Tp-Note
Tooltip=Tp-Note
Icon=package-x-generic
Profiles=profile-zero;

[X-Action-Profile profile-zero]
Name[en]=Default profile
Exec=tpnote %f
```

The above creates the custom context menu item _Tp-Note_.

#### Note viewer

Create the configuration file:

    sudo nano /usr/local/share/file-manager/actions/tpnote-view.desktop

with the following content:

```
[Desktop Entry]
Type=Action
Name[en]=Tp-Note View
Tooltip=Tp-Note View
Icon=package-x-generic
Profiles=profile-zero;

[X-Action-Profile profile-zero]
Name[en]=Default profile
Exec=tpnote -v -n %f
```

The above creates the custom context menu item _Tp-Note View_.


### Configure the text based file manager MidnightCommander

The Ncurses library based file manager _MidnightCommander_ `mc` enjoys great
popularity among people working on the console.
As _Tp-Note_ stores the note's content in UTF-8 encoded plain text, `mc`
can be used for full text searches in all note files of a directory.
Start  the full text search with the keys `[Esc]` `[?]`.

The following instructions configure `mc`'s `[F3]`-key to open `.md` files for
viewing. This is where _Tp-Note_ generates the HTML rendition of the note
file and opens the rendition with the _Lynx_ web browser. The `[Enter]`-key
runs _Tp-Note_ in editing mode.

1. First install the _Midnight Commander_ and the _Lynx_ web browser:

   ```bash
   sudo apt install mc lynx
   ```

2. Edit `mc`'s configuration file `/etc/mc/mc.ext`:

   ```bash
   sudo nano /etc/mc/mc.ext
   ```

3. Find the following lines (Debian 11+):

   ```
   shell/i/.md
          Include=editor
   ```

   and disable them:

   ```
   #shell/i/.md
   #       Include=editor
   ```

4. Replace the line `default/*` with:

   ```bash
   regex/i/.(txt|md|rst)$
       Open=tpnote %f
       View=if HTML=`tpnote -b -n -x - %f`; then (echo $"HTML" | lynx --stdin); else less    %f; fi

   default/*
   ```

5. Restart all instances of `mc` :

   ```bash
   sudo killall mc
   mc
   ```

To test the configuration, navigate to some `.md` note file and
press `[F3]` or `[Enter]`.

---

[^1]: _Tp-Note_ is preconfigured to work with many well-known external text
      editors: e.g.: `code`, `atom`, `retext`, `geany`, `gedit`, `mousepad`,
      `leafpad`, `nvim-qt`, and `gvim` under Linux and `notpad++` and `notepad`
      under Windows.  To register your own text editor, please consult the
      man-page.  For best user experience, I recommend text editors with
      internal markup previewer.

[^3]: At the moment I do not recommend using [Typora](<https://typora.io/#download>) because of
      this bug: [Errornous rendering when the value of the last header var
      is the empty string · Issue #4633 · typora/typora-issues](https://github.com/typora/typora-issues/issues/4633)

[^4]: Versions for other operating systems and a Debian package are
      [available here](<https://blog.getreu.net/projects/tp-note/_downloads/>).

[^5]: In order to get the page number rendered in Debian Buster, you need to upgrade
      the package `wkhtmltopdf`: first remove the current version:
      `sudo apt remove --purge wkhtmltopdf`, download:
      `wget https://github.com/wkhtmltopdf/packaging/releases/download/0.12.6-1/wkhtmltox_0.12.6-1.buster_amd64.deb` and
      install the new version:
      `sudo dpkg -i wkhtmltox_0.12.6-1.buster_amd64.deb`

[^6]: The `sed` filter is a workaround of a [bug in wkhtmltopdf](<https://github.com/wkhtmltopdf/wkhtmltopdf/issues/4960>).
      Once this is solved, you can remove the `sed` command. Then the line 4 of the above listing becomes:
      `<command>tpnote --export=- %f | wkhtmltopdf --footer-center "[page]/[topage]" -B 2cm -L 2cm -R 2cm -T 2cm - %f.pdf</command>`


[Tp-Note's man-page]: http://blog.getreu.net/projects/tp-note/tpnote--manpage.html#customization

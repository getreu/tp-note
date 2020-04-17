---
title:  "Tp-Note: fast note taking with templates and filename synchronization"
subtitle: "Organize your notes with your favourite editor and markup-language"
author: "Jens Getreu"
date:   "2020-03-08"
version: "1.4.3"
---

Markup languages like *Markdown*, *ReStructuredText*, *asciidoc*, *textile*,
*txt2tags* or *mediawiki* are perfectly suited for fast note-taking. Type
your notes with your favourite editor and chose your favourite markup
language[^1].

_Tp-Note_ helps you to quickly get started writing notes with its powerful
template system. As _Tp-Note_ takes care that the note's filename is always
synchronized with its document title, you will find back your notes easily.

_Tp-Note_ is available for Linux, Windows and iOS. This manual illustrates
its main use-cases and how to get started:

1. Fast start note-taking (when the lecture starts).
2. Take a note about an existing (downloaded) file.
3. Bookmark and comment a hyperlink.

If you want to customize _Tp-Note_ with own templates, another markup
language, please consult the [man-page] for more technical details.

The project is hosted on Github:
[getreu/tp-note](https://github.com/getreu/tp-note). The project's webpage is on
[http://blog.getreu.net](http://blog.getreu.net/projects/tp-note/).
The documentation of this project is dived into tow parts:

* User manual

  [tp-note user manual - html](https://blog.getreu.net/projects/tp-note/tp-note--manual.html)

  [tp-note user manual - pdf](https://blog.getreu.net/_downloads/tp-note--manual.pdf)

* Unix man-page (more technical)

  [tp-note manual page - html](https://blog.getreu.net/projects/tp-note/tp-note--manpage.html)

  [tp-note manual page - pdf](https://blog.getreu.net/_downloads/tp-note--manpage.pdf)


# How students take notes

A fellow student still uses paper and pen. I ask her why, and she replied "I can
better concentrate. My computer distracts me. I will do all other things, but
not listening.".

This is certainly true. As far as I am concerned, I am not good at logistics.
For me having all documents and notes in one little machine is a blessing.

To illustrate how to work with _Tp-Note_ here my most common workflows.



## Fast start note-taking (when the lecture starts)

![The folder in which the new note will be created.](images/workflow1-1.png){width="12cm"}

Alternatively you can open the folder you want to create a new note in and
right-click on some empty white space.

![The new unmodified note created by template on disk](images/workflow1-2.png){width="12cm"}

![The new unmodified note created by template](images/workflow1-3.png){width="13cm"}

![Change the title](images/workflow1-4.png){width="13cm"}

![Add some text](images/workflow1-5.png){width="13cm"}

![The new note file on disk after closing the editor](images/workflow1-6.png){width="12cm"}

> **Note**
>
> Before and after launching the editor _Tp-Note_ renames the file to be in
> sync with the note's metadata (i.e. title and subtitle).
> For more details see [Document title - filename sync]


## Taking notes about a file

![We want to take a note about a pdf](images/workflow2-1.png){width="11cm"}

![The new unmodified note created by template](images/workflow2-2.png){width="13cm"}

The source-code of the note shows, that the link has a  target. The left-click, opens the `.odt` document.

```yaml
---
title:      "03-Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter"
subtitle:   "Note"
author:     "getreu"
date:       "March 10, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.0"
---

[03-Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter.odt](03-Lied-Das_ist_mein_Teddybär - Lernstationen - Arbeitsblätter.odt)
```

![Annotate](images/workflow2-3.png){width="13cm"}

![The new note file on disk after closing the editor](images/workflow2-4.png){width="12cm"}


## Bookmark and comment a hyperlink


![Copy a link in markdown format](images/workflow3-2.png){width="14cm"}

To copy a link in markdown format a browser addon is needed. I recommend the
addons [Copy as Markdown] and [Copy Selection as Markdown] available
for Firefox.

[Copy as Markdown]: https://addons.mozilla.org/en-GB/firefox/search/?q=copy%20as%20markdown
[Copy Selection as Markdown]: https://addons.mozilla.org/en-GB/firefox/addon/copy-selection-as-markdown/?src=search

![Right-click on or in the new note's destination folder and start tp-note](images/workflow3-3.png){width="11cm"}

![The new unmodified note created by template](images/workflow3-4.png){width="13cm"}

The source code of the note shows the link target:

```yaml
---
title:      "Rustacean Station"
subtitle:   "URL"
author:     "getreu"
date:       "March 10, 2020"
lang:       "en_GB.UTF-8"
revision:   "1.0"
---

[Rustacean Station](https://rustacean-station.org/)
```

In this example we copied only one Markdown-link "Rustacean Station".
Furthermore, *Tp-Note* allows you also to insert a list of Markdown links in a
template. For example with [Copy as Markdown] you could copy a link list of all
open tabs. In this case, _Tp-Note_ would retain only the name of the first link
as document title, whereas the whole link-list would appear in the body of the
note.

![Annotate](images/workflow3-5.png){width="13cm"}

![The new note file on disk after closing the editor](images/workflow3-6.png){width="9cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```
## Copy a page from a book


![Copy some chapters](images/workflow4-1.png){width="14cm"}

![Right-click on or in the new note's destination folder and start tp-note](images/workflow4-2.png){width="9cm"}

![The new unmodified note created by template](images/workflow4-3.png){width="13cm"}

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
lang:       "en_GB.UTF-8"
revision:   "1.0"
---

Winston kept his back turned to the telescreen. It was safer, though, as he well
knew, even a back can be revealing. A kilometre away the Ministry of Truth, his
place of work, towered vast and white above the grimy landscape...
```

In this example we copied only text. *Tp-Note* suggests the first sentence as
title. This can be changed before saving as illustrated above. Here we just save
and observe the file on the disk.

![The new note file on disk after closing the editor](images/workflow4-4.png){width="9cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```


# How it works: Organize your files and notes with sort-tags

Consider the following _Tp-Note_-file:

    20151208-Make this world a better place--Suggestions.md

The filename has 4 parts:

    {{ sort-tag }}-{{ title }}--{{ subtitle }}.{{ extension }}

A so called _sort-tag_ is a numerical prefix at the beginning of the
filename. It is used to order files and notes in the file system. Besides
numerical digits, a _sort-tag_ can be any combination of
`0123456789-_`[^sort-tag] and is usually used as:

* *chronological sort-tag*

        20140211-Reminder.doc
        20151208-Manual.pdf

* or as a *sequence number sort-tag*.

        02-Invoices
        08-Tax documents
        09_02-Notes

The figures below illustrate organizing files with *sort-tags".

![Folders with sequence number sort-tag](images/filing-system1.png){width="12cm"}

![File with chronological sort-tag](images/filing-system2.png){width="8cm"}

When _Tp-Note_ creates a new note, it prepends automatically a *chronological
sort-tag* of today. The `{{ title }}` part is usually derived from the parent
directory's name omitting its own *sort-tag*.

[^sort-tag]: The characters `_` and `-` are not considered to be
part of the *sort-tag* when they appear in first or last position.



# Quickstart

_Tp-Note_ can be easily configured for your personal preferences and
needs[^2]. However, this section explains the basic standard setup to get you
started quickly.

```{=docbook}
<?dbfo-need height="6cm" ?>
```

## Installation

* **Windows**

  Download the
  [tp-note executable for Windows](https://blog.getreu.net/projects/tp-note/_downloads/x86_64-pc-windows-gnu/release/tp-note.exe) [^4]
  and place it on your desktop.

* **Linux**

  Download the _Tp-Note_-binary for Linux and place it on your Desktop:

  ``` sh
  > cd ~/Desktop
  > wget https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note
  > chmod 755 tp-note
  ```

A new _Tp-Note_-icon appears on your desktop.


## Usage

To create a new note, either double-click on the _Tp-Note_-icon, or drag a
folder or file and drop it on the _to-note_-icon. This opens an editor with
your new note.

For more comfort, I recommend integrating _Tp-Note_ into the file-manager's
context menu. See section [Integration with file manager] for more details.
There you also will find a list of tested Markdown-editors, if you wish to use
one. _Tp-Note_ works with every Unicode text editor and Markdown-editor (see
section [Optional customization] and man-page for more details).

_Tp-Note_'s note-files can be converted into e.g. `.docx`, `.odt`, `.html` with
[Pandoc](https://pandoc.org/) and then printed.


```{=docbook}
<?dbfo-need height="6cm" ?>
```

## Troubleshooting

* **Windows**

  When you see only Chinese characters in notepad, update Windows to the
  latest version or install [Notepad++](https://notepad-plus-plus.org/).

  Display _Tp-Note_'s error messages:

  1. Open the command-prompt: Click on *Windows-Start*, type `cmd` and [Enter].

  2. Type:

         Desktop\tp-note.exe -d >Desktop\debug.txt 2>&1

     This creates the file `debug.txt` on your desktop. Open the file
     and scroll to the end.

* **Linux**

  Display _Tp-Note_'s error messages:

  1. Open a console and change to the directory where you saved the
     _Tp-Note_-executable.

  2. Type:

         > tp-note -d 2>&1 | less

```{=docbook}
<?dbfo-need height="6cm" ?>
```

## Optional customization

* Your preferred markup language is not *Markdown*, but *ReStructuredText*,
  *T2t*, *Textile*, *Wiki*, *Mediawiki* or *Asciidoc*? Change it!
  
  Please refer to _Tp-Note_'s man-page to learn how to change its
  templates in the configuration file.

* Your preferred text editor is not *ReText*? Change it![^1]

  Note-taking with _Tp-Note_ is more fun with a good markup (Markdown)
  text editor, although any Unicode text editor will do (even Notepad >=
  Windows 10-update 1903). _Tp-Note_ it preconfigured to work with:
  
  - [Typora — a markdown editor, markdown reader.](https://typora.io/)

  - [ReText: Simple but powerful editor for Markdown and reStructuredText](https://github.com/retext-project/retext)

  - _VS-Code_, _Atom_ ...

  Please refer to _Tp-Note_'s man-page to learn how to register your text
  editor in _Tp-Note_'s configuration file.

* You prefer working in a desktop environment instead of working on a shell?

  Read [Integration with file manager].


# Integration with file manager

This section shows how to integrate _Tp-Note_ in the context menu of your
file manager. The context menu appears, when you click right on a file icon,
on a directory icon or on the white space in between (cf. figure below). In
the following we will configure the file-manager to launch _Tp-Note_ with the
path to the selected icon.

![Tp-note in the context-menu (right-click menu)](images/workflow2-1.png){width="12cm"}

```{=docbook}
<?dbfo-need height="6cm" ?>
```


## Windows file explorer configuration

The following works for me with Windows-version `10.0.18362`.

1. Make the directory `C:\Windows\tp-note\` and move `tp-note.exe`
   into it.

2. Open the *notepad* text editor and paste the following registry-key into
   it.

        Windows Registry Editor Version 5.00
        
        [HKEY_CLASSES_ROOT\Directory\Background\shell\Tp-Note]
        
        [HKEY_CLASSES_ROOT\Directory\Background\shell\Tp-Note\command]
        @="\"C:\\Program Files\\tp-note\\tp-note\""
        
        [HKEY_CLASSES_ROOT\*\OpenWithList\tp-note.exe]
        @=""

3. Save the file as: 

   * File name: `tp-note.reg`
   * Save as type: `All files`
   * Encoding:  `UTF-16 LE`


4. Double-click on `tp-note.reg` and confirm several times.




## Linux file manager configuration

To simplify the configuration we first place the binary _Tp-Note_
in our `$PATH`:

```sh
> cd /usr/local/bin
> sudo wget https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/release/tp-note
> sudo chmod 755 tp-note
```

_Debian_ and _Ubuntu_ user can also download a binary package from [debian/tp-note] and install it with:

``` sh
> sudo dpkg -i tp-note_X.X.X_amd64.deb
```


### Configure Thunar's custom actions

Most file-manager allow extending the context menu. As an example, the
following images show the configuration of the *Thunar*-file-manger.
In *Thunar*'s menu go to:

    Edit -> Configure custom actions...

![Thunar's custom action configuration](images/custom_actions1.png){width="10cm"}

![Edit custom action](images/edit_action.png){width="10cm"}

![Appearance Condition](images/appearance-condition.png){width="10cm"}

![Thunar's custom action configuration with tp-note](images/custom_actions2.png){width="10cm"}

[debian/tp-note]: https://blog.getreu.net/projects/tp-note/_downloads/x86_64-unknown-linux-gnu/debian/

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
        <command>tp-note %f</command>
        <description>Tp-Note</description>
        <patterns>*</patterns>
        <directories/>
        <audio-files/>
        <image-files/>
        <other-files/>
        <text-files/>
        <video-files/>
</action>
</actions>
```

The change becomes effective only after the user deletes his own configuration
file in `~/.config/Thunar/uca.xml`.

```{=docbook}
<?dbfo-need height="8cm" ?>
```


### Configure Pcmanfm's custom actions system-wide

_Pcmanfm_ is the default file-manager _Lubuntu_ and in _Raspbian_ on the 
Raspberry Pi.

Create the configuration file:

    sudo nano /usr/local/share/file-manager/actions/tp-note.desktop

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
Exec=tp-note %f
```

The above creates the custom context-menu _Tp-Note_.



[^1]: _Tp-Note_ is preconfigured to work with many well-known external text
editors: e.g.: `code`, `atom`, `retext`, `geany`, `gedit`, `mousepad`,
`leafpad`, `nvim-qt`, and `gvim` under Linux and `notpad++` and `notepad`
under Windows.  To register your own text editor, please consult the man-page.
For best user experience, I recommend text editors with internal markup
previewer.

[^2]: For a personalized setup read _Tp-Note_'s man-page.

[^4]: Versions for other operating systems and a Debian package are 
[available here](https://blog.getreu.net/projects/tp-note/_downloads/).


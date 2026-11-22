//! Annotating a file that is not a note: the binary creates a note beside it
//! and links back to it.

use crate::harness::{Scheme, Tp};

/// The YAML header the clipboard tests of 020 and 120 pipe in. It must override
/// the new note's `title:`, `author:` and `date:`, and with them the filename.
const PIPED_HEADER: &str = "---\ntitle: Clipboard Title\nauthor: Clip Author\n\
                            keywords: [alpha, beta]\ndate: 2026-01-15\n---\n\
                            Pasted body text.\n";

/// Port of test 003. The piped markdown link becomes the note's body, while the
/// title still comes from the annotated file. `--force-lang=-` pins the language
/// to the one `LANG` names instead of detecting it from that short body.
///
/// The `+` in the filename is not a character the binary considers safe, so it
/// is dropped from the title it derives -- which is why the reference file's
/// `title:` keeps it but the note's own name does not.
#[test]
fn an_annotation_takes_the_piped_text_as_its_body() {
    const INPUT: &str = "test003-annotate+clipboard-input-dummy.pdf";
    let tp = Tp::new(Scheme::Default).empty_file(INPUT);

    tp.run_stdin(
        ["--batch", "--force-lang=-", INPUT],
        "[ab:cd\"ef](https://getreu.net)\n",
    )
    .success()
    .file_eq(
        &format!("{INPUT}--Note.md"),
        fixture!("test003-annotate+clipboard"),
    );
}

/// Port of test 007. With nothing piped in, the note is just a link back to the
/// annotated file.
#[test]
fn an_annotation_without_input_links_back_to_the_file() {
    const INPUT: &str = "test007-some english file to annotate.pdf";
    let tp = Tp::new(Scheme::Default).empty_file(INPUT);

    tp.run(["--batch", INPUT])
        .success()
        .file_eq(&format!("{INPUT}--Note.md"), fixture!("test007-annotate"));
}

/// Port of test 018. `TPNOTE_EXTENSION_DEFAULT` picks the markup language, so
/// the note comes out as reStructuredText -- the link back is
/// `` `<file>`_ `` rather than `[file](<file>)``.
#[test]
fn the_default_extension_decides_the_markup_language() {
    const INPUT: &str = "test018-some english file to annotate.pdf";
    let tp = Tp::new(Scheme::Default)
        .empty_file(INPUT)
        .env("TPNOTE_EXTENSION_DEFAULT", "rst");

    tp.run(["--batch", INPUT])
        .success()
        .file_eq(&format!("{INPUT}--Note.rst"), fixture!("test018-annotate"));
}

/// Port of test 020. A piped YAML header overrides the header the template
/// would have produced, and therefore the filename too: the note is named after
/// the piped `title:`, not after the annotated file. `keywords:` has no bearing
/// on a filename in the `default` scheme.
#[test]
fn a_piped_header_overrides_the_new_notes_header() {
    const INPUT: &str = "test020-annotate-header-override-dummy.pdf";
    let tp = Tp::new(Scheme::Default).empty_file(INPUT);

    tp.run_stdin(["--batch", "--force-lang=-", INPUT], PIPED_HEADER)
        .success()
        .file_eq(
            "Clipboard Title--Note.md",
            fixture!("test020-annotate-header-override"),
        );
}

/// Port of test 103, the `zettel` counterpart of test 003.
#[test]
fn zettel_an_annotation_takes_the_piped_text_as_its_body() {
    const INPUT: &str = "test103-annotate+clipboard-input-dummy.pdf";
    let tp = Tp::new(Scheme::Zettel).empty_file(INPUT);

    tp.run_stdin(
        ["--batch", "--force-lang=-", INPUT],
        "[ab:cd\"ef](https://getreu.net)\n",
    )
    .success()
    .file_eq(
        &format!("{INPUT}__note.md"),
        fixture!("test103-annotate+clipboard"),
    );
}

/// Port of test 107, the `zettel` counterpart of test 007.
#[test]
fn zettel_an_annotation_without_input_links_back_to_the_file() {
    const INPUT: &str = "test107-some english file to annotate.pdf";
    let tp = Tp::new(Scheme::Zettel).empty_file(INPUT);

    tp.run(["--batch", INPUT])
        .success()
        .file_eq(&format!("{INPUT}__note.md"), fixture!("test107-annotate"));
}

/// Port of test 118, the `zettel` counterpart of test 018.
#[test]
fn zettel_the_default_extension_decides_the_markup_language() {
    const INPUT: &str = "test118-some english file to annotate.pdf";
    let tp = Tp::new(Scheme::Zettel)
        .empty_file(INPUT)
        .env("TPNOTE_EXTENSION_DEFAULT", "rst");

    tp.run(["--batch", INPUT])
        .success()
        .file_eq(&format!("{INPUT}__note.rst"), fixture!("test118-annotate"));
}

/// Port of test 120, the `zettel` counterpart of test 020. Here `keywords:` do
/// reach the filename, so the piped header decides both halves of it.
#[test]
fn zettel_a_piped_header_overrides_the_new_notes_header() {
    const INPUT: &str = "test120-annotate-header-override-dummy.pdf";
    let tp = Tp::new(Scheme::Zettel).empty_file(INPUT);

    tp.run_stdin(["--batch", "--force-lang=-", INPUT], PIPED_HEADER)
        .success()
        .file_eq(
            "Clipboard Title__alpha_beta.md",
            fixture!("test120-annotate-header-override"),
        );
}

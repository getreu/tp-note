//! Creating a note from piped input: plain text, a markdown link, HTML, or a
//! whole YAML header.
//!
//! The shell scripts call these the clipboard tests. The binary ignores the
//! system clipboard in batch mode, so they pipe the same text to standard input
//! instead, which is what `run_stdin` does here.

use crate::harness::{Scheme, Tp, today};

/// Three lines in three languages, for the detection tests. `fr` is offered as
/// a candidate but loses: the reference files record `de-DE`.
const MULTILINGUAL: &str = "Parlez-vous français?\n\
                            Ich spreche Französisch nur ein bisschen.\n\
                            A little bit is better than nothing.\n";

/// A complete note streamed in, header and all. The `EOF` line is not a
/// mistake: the shell script builds this with a quoted string that swallowed
/// its own heredoc terminator, so the body really is the word `EOF`, and the
/// reference files record it.
const STREAMED_NOTE: &str = "---\ntitle: aaa\nsubtitle: bbb\ndate: 2020-01-02\n\
                             lang: en\nrevision: '2.0'\nsort_tag: '222'\n\
                             file_ext: mdtxt\nmy_own_long_var: foo\n...\nEOF\n";

/// Port of test 004. A piped markdown link gives the note its title, taken from
/// the link's text. `:` and `"` are not characters the binary considers safe in
/// a filename, so `ab:cd"ef` becomes `ab_cd ef` there while the header keeps it
/// verbatim. An empty `TPNOTE_LANG_DETECTION` switches detection off.
#[test]
fn a_piped_markdown_link_titles_the_note() {
    let tp = Tp::new(Scheme::Default).env("TPNOTE_LANG_DETECTION", "");

    tp.run_stdin(["--batch", "."], "[ab:cd\"ef](https://getreu.net)\n")
        .success()
        .file_eq(
            &format!("{}-ab_cd ef--Note.md", today()),
            fixture!("test004-clipboard"),
        );
}

/// Port of test 005. `LANG` says Finnish, but detection is offered only
/// `en-US`, so that is what the note records.
#[test]
fn language_detection_picks_the_only_candidate() {
    let tp = Tp::new(Scheme::Default)
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US");

    tp.run_stdin(["--batch", "."], "Good morning\n")
        .success()
        .file_eq(
            &format!("{}-Good morning--Note.md", today()),
            fixture!("test005-clipboard"),
        );
}

/// Port of test 006. A title that is all digits has to be quoted in YAML, and
/// the filename gets the `'` the binary uses to keep a numeric title from
/// reading as a sort tag.
#[test]
fn a_numeric_title_is_quoted_and_escaped() {
    let tp = Tp::new(Scheme::Default).env("TPNOTE_LANG_DETECTION", "");

    tp.run_stdin(["--batch", "."], "123456\n")
        .success()
        .file_eq(
            &format!("{}-'123456--Note.md", today()),
            fixture!("test006-clipboard"),
        );
}

/// Port of test 009. A streamed note's own header decides everything: the sort
/// tag, the file extension, and an unknown `my_own_long_var:` which has to
/// survive into the stored header.
#[test]
fn a_streamed_header_decides_the_filename_and_is_kept_whole() {
    let tp = Tp::new(Scheme::Default);

    tp.run_stdin(["--batch", "."], STREAMED_NOTE)
        .success()
        .file_eq("222-aaa--bbb.mdtxt", fixture!("test009-stream-content"));
}

/// Port of test 017. Piped HTML is recognized as such and converted to markup,
/// so `<h1>Good news</h1>` becomes `# Good news`. The seeded `03b-test.md` is
/// load bearing: with a sort tag already in the directory the new note counts
/// on from it rather than taking today's date.
#[test]
fn piped_html_is_converted_to_markup() {
    let tp = Tp::new(Scheme::Default)
        .empty_file("03b-test.md")
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US");

    tp.run_stdin(["--batch", "."], "<html><h1>Good news</h1></html>\n")
        .success()
        .file_eq("03c-Good news--Note.md", fixture!("test017-clipboard"));
}

/// Port of test 019. Offered `en-US`, `de-DE` and `fr`, detection settles on
/// `de-DE` and records the candidates it considered in `languages:`.
#[test]
fn language_detection_records_the_candidates_it_weighed() {
    let tp = Tp::new(Scheme::Default)
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US,de-DE,fr");

    tp.run_stdin(["--batch", "."], MULTILINGUAL)
        .success()
        .file_eq(
            &format!("{}-Parlez-vous français--Note.md", today()),
            fixture!("test019-clipboard"),
        );
}

/// Port of test 104, the `zettel` counterpart of test 004.
#[test]
fn zettel_a_piped_markdown_link_titles_the_note() {
    let tp = Tp::new(Scheme::Zettel)
        .empty_file("03b--test.md")
        .env("TPNOTE_LANG_DETECTION", "");

    tp.run_stdin(["--batch", "."], "[ab:cd\"ef](https://getreu.net)\n")
        .success()
        .file_eq("03c--ab_cd ef__note.md", fixture!("test104-clipboard"));
}

/// Port of test 105, the `zettel` counterpart of test 005.
#[test]
fn zettel_language_detection_picks_the_only_candidate() {
    let tp = Tp::new(Scheme::Zettel)
        .empty_file("03b--test.md")
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US");

    tp.run_stdin(["--batch", "."], "Good morning\n")
        .success()
        .file_eq("03c--Good morning__note.md", fixture!("test105-clipboard"));
}

/// Port of test 106, the `zettel` counterpart of test 006.
#[test]
fn zettel_a_numeric_title_is_quoted_and_escaped() {
    let tp = Tp::new(Scheme::Zettel)
        .empty_file("03b--test.md")
        .env("TPNOTE_LANG_DETECTION", "");

    tp.run_stdin(["--batch", "."], "123456\n")
        .success()
        .file_eq("03c--'123456__note.md", fixture!("test106-clipboard"));
}

/// Port of test 109, the `zettel` counterpart of test 009.
#[test]
fn zettel_a_streamed_header_decides_the_filename_and_is_kept_whole() {
    let tp = Tp::new(Scheme::Zettel);

    tp.run_stdin(
        ["--batch", "."],
        "---\ntitle: aaa\nkeywords:\n- bbb\ndate: 2020-01-02\nlang: en\n\
         revision: '2.0'\nsort_tag: '222'\nfile_ext: mdtxt\nscheme: zettel\n\
         my_own_long_var: foo\n...\nEOF\n",
    )
    .success()
    .file_eq("222--aaa__bbb.mdtxt", fixture!("test109-stream-content"));
}

/// Port of test 117, the `zettel` counterpart of test 017.
#[test]
fn zettel_piped_html_is_converted_to_markup() {
    let tp = Tp::new(Scheme::Zettel)
        .empty_file("03b--test.md")
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US");

    tp.run_stdin(["--batch", "."], "<html><h1>Good news</h1></html>\n")
        .success()
        .file_eq("03c--Good news__note.md", fixture!("test117-clipboard"));
}

/// Port of test 119, the `zettel` counterpart of test 019.
///
/// With no sort tag to count on from, the `zettel` scheme falls back to today's
/// date, which the stored header records. The shell script drops those lines
/// before comparing and so does this, since the reference file cannot carry a
/// date that changes daily.
#[test]
fn zettel_language_detection_records_the_candidates_it_weighed() {
    let tp = Tp::new(Scheme::Zettel)
        .env("LANG", "fi_FI.UTF-8")
        .env("TPNOTE_LANG_DETECTION", "en-US,de-DE,fr");

    tp.run_stdin(["--batch", "."], MULTILINGUAL)
        .success()
        .file_eq_without(
            &format!("{}--Parlez-vous français__note.md", today()),
            "sort_tag:",
            fixture!("test119-clipboard"),
        );
}

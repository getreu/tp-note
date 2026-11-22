//! Exporting a note to HTML, and how local links are rewritten on the way.

use crate::harness::{Scheme, Tp};

/// The note that test 011 and test 111 export.
const BODY_NOTE: &str = "---\ntitle: xxx\nsubtitle: yyy\nauthor:\n- Jens\n- Hans\n\
                         sort_tag: '111'\n...\n# Body\n";

/// The link matrices of tests 016 and 116, read from a file rather than spelled
/// out here. Almost every line ends in trailing spaces, which markdown reads as
/// a hard line break -- the references contain twenty `<br />` because of them.
/// Trailing spaces inside a Rust string literal are invisible and an editor
/// that trims them on save would quietly change what is under test, so they
/// live in a file and `hard_line_breaks` below checks they are still there.
const INPUT_016: &str = include_str!("inputs/016-link-rewriting.md");
const INPUT_116: &str = include_str!("inputs/116-link-rewriting.md");

/// Number of lines ending in two or more spaces, i.e. markdown hard line
/// breaks. Guards the inputs above against whitespace trimming: were these to
/// come out as zero, the HTML comparison would fail with a puzzling diff
/// instead of naming the cause.
fn hard_line_breaks(input: &str) -> usize {
    input.lines().filter(|l| l.ends_with("  ")).count()
}

/// Port of test 011. Two runs: the first synchronizes the filename with the
/// front matter, the second exports the renamed note. The export has to carry
/// the two `author:` entries and the rendered body.
#[test]
fn a_note_exports_to_html() {
    let tp = Tp::new(Scheme::Default).file("123-abc--edf.md", BODY_NOTE);

    tp.run(["--batch", "123-abc--edf.md"]).success();
    tp.run(["--batch", "--export", ".", "111-xxx--yyy.md"])
        .success()
        .file_eq("111-xxx--yyy.md.html", fixture!("test011-html"));
}

/// Port of test 016. Every shape of local link the binary understands, each
/// rewritten against the document root because of
/// `--export-link-rewriting=short`.
///
/// The shell script writes a `.tpnote.toml` here under the comment "This is the
/// base dir for the exporter". The root marker has no leading dot, so that line
/// does nothing; the root comes from the `tpnote.toml` the harness writes.
#[test]
fn local_links_are_rewritten_against_the_document_root() {
    assert_eq!(
        hard_line_breaks(INPUT_016),
        21,
        "the hard line breaks in inputs/016-link-rewriting.md have been trimmed"
    );
    let tp = Tp::new(Scheme::Default).file("01ac-Tulips--red, yellow.md", INPUT_016);

    tp.run([
        "--batch",
        "--export",
        ".",
        "--export-link-rewriting=short",
        "01ac-Tulips--red, yellow.md",
    ])
    .success()
    .file_eq("01ac-Tulips--red, yellow.md.html", fixture!("test016-html"));
}

/// Port of test 111. The same note and the same export as test 011, run with
/// `TPNOTE_SCHEME=zettel` in the environment. Its header names no scheme, so
/// the result is identical -- the two reference files are byte for byte the
/// same -- which is the point: the environment alone does not reach a note that
/// already has a header.
#[test]
fn zettel_a_note_exports_to_html() {
    let tp = Tp::new(Scheme::Zettel).file("123-abc--edf.md", BODY_NOTE);

    tp.run(["--batch", "123-abc--edf.md"]).success();
    tp.run(["--batch", "--export", ".", "111-xxx--yyy.md"])
        .success()
        .file_eq("111-xxx--yyy.md.html", fixture!("test111-html"));
}

/// Port of test 116, the `zettel` counterpart of test 016.
#[test]
fn zettel_local_links_are_rewritten_against_the_document_root() {
    assert_eq!(
        hard_line_breaks(INPUT_116),
        21,
        "the hard line breaks in inputs/116-link-rewriting.md have been trimmed"
    );
    let tp = Tp::new(Scheme::Zettel).file("01ac--Tulips__red_yellow.md", INPUT_116);

    tp.run([
        "--batch",
        "--export",
        ".",
        "--export-link-rewriting=short",
        "01ac--Tulips__red_yellow.md",
    ])
    .success()
    .file_eq("01ac--Tulips__red_yellow.md.html", fixture!("test116-html"));
}

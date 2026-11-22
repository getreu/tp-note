//! Auto-generated heading ids, under the default `Gfm` policy.

use crate::harness::{Scheme, Tp};

/// Nine headings, each pinning one slugging rule: a plain heading, an explicit
/// `{#ch1}`, an em dash, a leading number, a repeat that has to be
/// disambiguated, inline markup to be stripped, umlauts, and trailing
/// punctuation.
const HEADINGS: &str = "---\ntitle: heading ids\nsubtitle: Note\n---\n\n\
                        ## Chapter one\n\n\
                        ## Chapter one {#ch1}\n\n\
                        ## S9 — Check the PIN\n\n\
                        ## 2. Second section\n\n\
                        ## Duplicate\n\n\
                        ## Duplicate\n\n\
                        ## *Emphasis* and `code`\n\n\
                        ## Ümlaut und Größe\n\n\
                        ## Trailing punctuation!\n";

/// Port of test 027.
#[test]
fn heading_ids_are_generated_from_the_heading_text() {
    let tp = Tp::new(Scheme::Default).file("027-heading-ids--Note.md", HEADINGS);

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "027-heading-ids--Note.md",
    ])
    .success()
    .file_eq("027-heading-ids--Note.md.html", fixture!("test027-html"));
}

/// Port of test 127. The same note under `TPNOTE_SCHEME=zettel`; slugging does
/// not depend on the scheme, and the two reference files are byte for byte
/// identical.
#[test]
fn zettel_heading_ids_are_generated_from_the_heading_text() {
    let tp = Tp::new(Scheme::Zettel).file("127-heading-ids--Note.md", HEADINGS);

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "127-heading-ids--Note.md",
    ])
    .success()
    .file_eq("127-heading-ids--Note.md.html", fixture!("test127-html"));
}

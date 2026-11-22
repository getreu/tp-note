//! Synchronizing a note's filename with its front matter.

use crate::harness::{Scheme, Tp};

/// Port of test 001 of `tests/functional/run-tests`. The front matter's
/// `title:` and `subtitle:` win over the filename, which is renamed to match
/// while its sort tag stays put. The content itself must come out unchanged.
#[test]
fn front_matter_renames_the_file_and_keeps_the_sort_tag() {
    let tp = Tp::new(Scheme::Default).file(
        "123-abc--edf.md",
        "---\ntitle: xxx\nsubtitle: yyy\n...\n",
    );
    let input = tp.at("123-abc--edf.md");

    tp.arg("--batch")
        .arg(&input)
        .run()
        .success()
        .file_eq("123-xxx--yyy.md", fixture!("test001-synchronize"))
        .file_absent("123-abc--edf.md");
}

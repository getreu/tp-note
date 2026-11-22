//! Prepending a YAML header to a file that has none.

use crate::harness::{Scheme, Tp};

/// Port of test 013. `--add-header` derives a header from the filename, and
/// records the name it derived it from in an `# orig_name:` comment.
///
/// The input's `-- edf` has a space after the separator, which survives into
/// the `subtitle: ' edf'` -- quoted, because of that leading space -- while the
/// filename itself is tidied to `--edf`.
#[test]
fn add_header_derives_a_header_from_the_filename() {
    let tp = Tp::new(Scheme::Default).file("20220312-abc-- edf.md", "Only body\n");

    tp.run(["--batch", "--add-header", "20220312-abc-- edf.md"])
        .success()
        .file_eq("20220312-abc--edf.md", fixture!("test013-add-header"))
        .file_absent("20220312-abc-- edf.md");
}

/// Port of test 113, the `zettel` counterpart of test 013. The filename's two
/// `_`-separated parts become two `keywords:`, and its leading `20220312`
/// becomes an explicit `sort_tag:`. Nothing about the name needs tidying, so it
/// stays as it is.
#[test]
fn zettel_add_header_derives_a_header_from_the_filename() {
    const INPUT: &str = "20220312--abc__edf_hij.md";
    let tp = Tp::new(Scheme::Zettel).file(INPUT, "Only body\n");

    tp.run(["--batch", "--add-header", INPUT])
        .success()
        .file_eq(INPUT, fixture!("test113-add-header"));
}

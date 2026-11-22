//! Synchronizing a note's filename with its front matter.
//!
//! These runs never rewrite the note's content, so each reference file is the
//! input echoed back; what is under test is the filename the note ends up with.

use crate::harness::{Scheme, Tp};

/// Port of test 001. The front matter's `title:` and `subtitle:` win over the
/// filename, which is renamed to match while its sort tag stays put.
#[test]
fn front_matter_renames_the_file_and_keeps_the_sort_tag() {
    const INPUT: &str = "123-abc--edf.md";
    let tp = Tp::new(Scheme::Default).file(INPUT, "---\ntitle: xxx\nsubtitle: yyy\n...\n");

    tp.run(["--batch", INPUT])
        .success()
        .file_eq("123-xxx--yyy.md", fixture!("test001-synchronize"))
        .file_absent(INPUT);
}

/// Port of test 008. A `sort_tag:` in the front matter pins the sort tag, so
/// the filename's own `123-` gives way to `111-`.
///
/// The shell script passes `LANG="en_US.UTF-008"` here, a typo for
/// `en_US.UTF-8`. It cannot matter: a synchronizing run copies the content
/// through untouched and never inserts a `lang:`, which is why the reference
/// file has none. The harness default is used instead.
#[test]
fn a_sort_tag_in_the_front_matter_pins_the_sort_tag() {
    const INPUT: &str = "123-abc--edf.md";
    let tp = Tp::new(Scheme::Default).file(
        INPUT,
        "---\ntitle: xxx\nsubtitle: yyy\nsort_tag: '111'\nfile_ext: md\n...\n",
    );

    tp.run(["--batch", INPUT])
        .success()
        .file_eq("111-xxx--yyy.md", fixture!("test008-tag"))
        .file_absent(INPUT);
}

/// Port of test 012. With `--no-filename-sync` the very same input that test
/// 001 renames is left where it is -- hence the shared reference file, which
/// states that the content came through untouched either way.
#[test]
fn no_filename_sync_leaves_the_filename_alone() {
    const INPUT: &str = "123-abc--edf.md";
    let tp = Tp::new(Scheme::Default).file(INPUT, "---\ntitle: xxx\nsubtitle: yyy\n...\n");

    tp.run(["--batch", "--no-filename-sync", INPUT])
        .success()
        .file_eq("123-abc--edf.md", fixture!("test001-synchronize"))
        .file_absent("123-xxx--yyy.md");
}

/// Port of test 014. A filename already carrying a copy counter and already
/// agreeing with its front matter must survive a run unchanged: the path the
/// binary prints is the path it was given.
#[test]
fn a_copy_counter_survives_a_synchronizing_run() {
    const INPUT: &str = "20220312-abc--edf(4).md";
    let tp = Tp::new(Scheme::Default).file(INPUT, "---\ntitle: abc\nsubtitle: edf\n---\nBody\n");

    let out = tp.run(["--batch", "--add-header", INPUT]).success();

    assert_eq!(
        out.last_stdout_line(),
        tp.at(INPUT).to_string_lossy(),
        "the binary settled on a different path than the one it was given"
    );
}

/// Port of test 101, the `zettel` counterpart of test 001: `keywords:` drive
/// the `__`-separated part of the filename.
#[test]
fn zettel_front_matter_renames_the_file_and_keeps_the_sort_tag() {
    const INPUT: &str = "123--abc__edf_ghi.md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        "---\ntitle: xxx\nkeywords:\n- yyy\n- zzz\nscheme: zettel\n...\n",
    );

    tp.run(["--batch", INPUT])
        .success()
        .file_eq("123--xxx__yyy_zzz.md", fixture!("test101-synchronize"))
        .file_absent(INPUT);
}

/// Port of test 108, the `zettel` counterpart of test 008.
///
/// The `keywords:` line keeps its trailing space on purpose. A synchronizing
/// run copies the content through byte for byte, so the trailing space is in
/// the reference file too; it is spelled `\x20` here so that no editor or
/// formatter can quietly strip it.
#[test]
fn zettel_a_sort_tag_in_the_front_matter_pins_the_sort_tag() {
    const INPUT: &str = "123--abc__edf.md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        "---\ntitle: xxx\nkeywords:\x20\n- yyy\nsort_tag: '111'\nscheme: zettel\nfile_ext: md\n...\n",
    );

    tp.run(["--batch", INPUT])
        .success()
        .file_eq("111--xxx__yyy.md", fixture!("test108-tag"))
        .file_absent(INPUT);
}

/// Port of test 112, the `zettel` counterpart of test 012. The filename is in
/// the `default` scheme's shape while the header asks for `zettel`;
/// `--no-filename-sync` means that disagreement is left standing.
#[test]
fn zettel_no_filename_sync_leaves_the_filename_alone() {
    const INPUT: &str = "123-abc--edf.md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        "---\ntitle: xxx\nkeywords:\n- yyy\nscheme: zettel\n...\n",
    );

    tp.run(["--batch", "--no-filename-sync", INPUT])
        .success()
        .file_eq("123-abc--edf.md", fixture!("test112-synchronize"));
}

/// Port of test 114, the `zettel` counterpart of test 014.
#[test]
fn zettel_a_copy_counter_survives_a_synchronizing_run() {
    const INPUT: &str = "20220312--abc__edf(4).md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        "---\ntitle: abc\nkeywords:\n- edf\nscheme: zettel\n---\nBody\n",
    );

    let out = tp.run(["--batch", "--add-header", INPUT]).success();

    assert_eq!(
        out.last_stdout_line(),
        tp.at(INPUT).to_string_lossy(),
        "the binary settled on a different path than the one it was given"
    );
}

/// Port of test 115. Here the header names no `scheme:`, so the one from
/// `TPNOTE_SCHEME` applies, and the filename's `__`-separated part is dropped
/// rather than rebuilt from `keywords:`.
///
/// The shell script seeds a `03b--test.md` here as its new-note tests do. This
/// run only renames an existing note and derives everything from that note, so
/// the seed cannot bear on the result -- and this test passing without it says
/// so.
#[test]
fn zettel_a_header_without_a_scheme_drops_the_keyword_part() {
    const INPUT: &str = "123-abc__edf_ghi.md";
    let tp = Tp::new(Scheme::Zettel).file(INPUT, "---\ntitle: xxx\nkeywords:\n- yyy\n- zzz\n...\n");

    tp.run(["--batch", INPUT])
        .success()
        .file_eq("123-xxx.md", fixture!("test115-synchronize"))
        .file_absent(INPUT);
}

//! Creating a new note in a directory, with nothing to go on but that
//! directory itself.

use crate::harness::{Scheme, Tp, today};

/// Port of test 002. Pointed at a directory, the binary creates a note there
/// and takes the title from the directory's own name -- which is why the
/// sandbox's note directory has to be called `test_output`, as the shell
/// scripts call theirs.
#[test]
fn a_directory_yields_a_note_titled_after_it() {
    let tp = Tp::new(Scheme::Default);

    tp.run(["--batch", "."]).success().file_eq(
        &format!("{}-test_output--Note.md", today()),
        fixture!("test002-new"),
    );
}

/// Port of test 010. Two runs in the same directory produce the same header
/// twice, so the second note cannot have the first one's filename: it gets a
/// copy counter. The reference is the *second* note, which must be a byte for
/// byte match of the first.
#[test]
fn a_second_identical_note_gets_a_copy_counter() {
    let tp = Tp::new(Scheme::Default);

    tp.run(["--batch", "."]).success();
    tp.run(["--batch", "."])
        .success()
        .file_exists(&format!("{}-test_output--Note.md", today()))
        .file_eq(
            &format!("{}-test_output--Note(1).md", today()),
            fixture!("test010-new-new"),
        );
}

/// Port of test 102, the `zettel` counterpart of test 002. Here the sort tag is
/// counted on from what the directory already holds, so the seeded `03--test.md`
/// is load bearing: the reference file records `sort_tag: '04'`.
#[test]
fn zettel_a_new_note_takes_the_next_sort_tag() {
    let tp = Tp::new(Scheme::Zettel).empty_file("03--test.md");

    tp.run(["--batch", "."])
        .success()
        .file_eq("04--test_output__note.md", fixture!("test102-new"));
}

/// Port of test 110, the `zettel` counterpart of test 010. Counting on from the
/// seeded `03b--test.md` gives `03c` and then `03d`; unlike the `default`
/// scheme there is no copy counter, because each note gets its own sort tag.
/// The reference is the second note, and records `sort_tag: 03d`.
#[test]
fn zettel_a_second_note_takes_the_sort_tag_after_it() {
    let tp = Tp::new(Scheme::Zettel).empty_file("03b--test.md");

    tp.run(["--batch", "."]).success();
    tp.run(["--batch", "."])
        .success()
        .file_exists("03c--test_output__note.md")
        .file_eq("03d--test_output__note.md", fixture!("test110-new-new"));
}

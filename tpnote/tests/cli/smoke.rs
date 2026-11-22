//! Checks on the harness itself, so that a failure elsewhere in this suite
//! points at the binary rather than at the scaffolding.

use crate::harness::{Scheme, Tp, today};

/// The generated test configuration loads, and a note created from a directory
/// matches the shell scripts' reference byte for byte. This exercises the whole
/// scaffolding at once: `--config-defaults` generation, the three edits applied
/// to it, the sandbox named `test_output`, and reference file lookup.
#[test]
fn harness_reproduces_the_reference_output() {
    let out = Tp::new(Scheme::Default)
        .arg("--batch")
        .arg(".")
        .run()
        .success();

    out.file_eq(
        &format!("{}-test_output--Note.md", today()),
        fixture!("test002-new"),
    );
}

/// Standard input is `/dev/null` unless a test asks for it. Were it inherited
/// from the test runner the binary would read whatever the harness happens to
/// be writing and treat it as the note's body, which would corrupt most of this
/// suite in a way that is tedious to trace back. Here that would show up as a
/// body after the header, or as a title taken from the piped text.
#[test]
fn standard_input_is_not_inherited() {
    let out = Tp::new(Scheme::Default)
        .arg("--batch")
        .arg(".")
        .run()
        .success();

    out.file_eq(
        &format!("{}-test_output--Note.md", today()),
        fixture!("test002-new"),
    );
}

/// The counterpart: when a test does pipe something in, it arrives. Keeping
/// both directions here means the default above is a real guarantee and not
/// just an untested claim.
#[test]
fn standard_input_arrives_when_given() {
    let out = Tp::new(Scheme::Default)
        .args(["--batch", "--force-lang=-", "."])
        .stdin("Good morning")
        .run()
        .success();

    out.file_exists(&format!("{}-Good morning--Note.md", today()));
}

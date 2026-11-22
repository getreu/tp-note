//! One check on the harness itself, so that a failure elsewhere in this suite
//! points at the binary rather than at the scaffolding.

use crate::harness::{Scheme, Tp, today};

/// Standard input reaches the binary when a test asks for it.
///
/// The opposite direction -- that an un-redirected run does *not* inherit the
/// test runner's standard input -- needs no test of its own: every run that
/// does not call `run_stdin` asserts it implicitly, because inherited input
/// would be read as the note's body and the content comparison would fail.
/// This test guards the other way round, that `run_stdin` is actually wired up,
/// which no content comparison would catch.
#[test]
fn standard_input_reaches_the_binary() {
    let tp = Tp::new(Scheme::Default);

    tp.run_stdin(["--batch", "--force-lang=-", "."], "Good morning")
        .success()
        .file_exists(&format!("{}-Good morning--Note.md", today()));
}

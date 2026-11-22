//! Mermaid diagrams, and what the exporter does with a broken one.

use crate::harness::{Scheme, Tp};

/// Port of test 023 of `tests/functional/run-tests`. Exporting in batch mode
/// (`--export` together with `--batch`) forces the embedded content error
/// policy to `HardError` whatever the configuration says, so a diagram that
/// does not parse fails the whole export: the message lands on standard error
/// and no document is written.
#[test]
fn a_broken_diagram_fails_a_batch_export() {
    let tp = Tp::new(Scheme::Default).file(
        "023-mermaid-bad--Note.md",
        "---\ntitle: mermaid bad\nsubtitle: Note\n---\n\n\
         ```mermaid\nthis is not a valid mermaid diagram\n```\n",
    );
    let input = tp.at("023-mermaid-bad--Note.md");

    tp.args(["--batch", "--no-filename-sync", "--export", "."])
        .arg(&input)
        .run()
        // The shell script never checks this; a failed export that still
        // reported success would pass there and fail here.
        .failure()
        // The reference holds only the stable prefix of the message; what
        // follows "Additional technical details:" lists absolute paths that
        // differ from one machine to the next.
        .stderr_eq_trimmed(fixture!("test023-mermaid-hard"))
        .file_absent("023-mermaid-bad--Note.md.html");
}

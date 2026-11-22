//! Mermaid diagrams, and the three ways the exporter can treat a broken one.

use crate::harness::{Scheme, Tp};

/// A diagram that parses.
const GOOD: &str = "```mermaid\ngraph TD\n    A --> B\n```\n";
/// A diagram that does not.
const BAD: &str = "```mermaid\nthis is not a valid mermaid diagram\n```\n";

/// Switches the exporter's error policy to `Inline`, the shell scripts'
/// `sed 's/^exporter_embedded_content_error_policy = .*/... = "Inline"/'`.
fn inline_policy(line: &str) -> Option<String> {
    line.starts_with("exporter_embedded_content_error_policy = ")
        .then(|| "exporter_embedded_content_error_policy = \"Inline\"".to_string())
}

/// Port of test 021. A diagram that parses is rendered to SVG and embedded in
/// the exported document.
#[test]
fn a_diagram_renders_to_inline_svg() {
    let tp = Tp::new(Scheme::Default).file(
        "021-mermaid--Note.md",
        &format!("---\ntitle: mermaid\nsubtitle: Note\n---\n\n{GOOD}"),
    );

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "021-mermaid--Note.md",
    ])
    .success()
    .file_contains("021-mermaid--Note.md.html", "<div class=\"mermaid\">")
    .file_contains("021-mermaid--Note.md.html", "<svg");
}

/// Port of test 022. Under the `Inline` policy a broken diagram becomes an
/// error box in the document and the export still succeeds.
///
/// `--batch` is deliberately absent: batch exporting forces the policy to
/// `HardError` whatever the configuration says, which is what test 023
/// exercises.
#[test]
fn an_inline_policy_turns_a_broken_diagram_into_an_error_box() {
    let tp = Tp::new(Scheme::Default)
        .file(
            "022-mermaid-bad--Note.md",
            &format!("---\ntitle: mermaid bad\nsubtitle: Note\n---\n\n{BAD}"),
        )
        .config_edited("tpnote-mermaid-inline.toml", inline_policy);

    tp.run([
        "--no-filename-sync",
        "--export",
        ".",
        "022-mermaid-bad--Note.md",
    ])
    .success()
    .file_contains("022-mermaid-bad--Note.md.html", "mermaid-error");
}

/// Port of test 023. Exporting in batch mode forces the policy to `HardError`,
/// so a diagram that does not parse fails the whole export: the message lands
/// on standard error and no document is written.
#[test]
fn a_broken_diagram_fails_a_batch_export() {
    const INPUT: &str = "023-mermaid-bad--Note.md";
    let tp = Tp::new(Scheme::Default).file(
        INPUT,
        &format!("---\ntitle: mermaid bad\nsubtitle: Note\n---\n\n{BAD}"),
    );

    tp.run(["--batch", "--no-filename-sync", "--export", ".", INPUT])
        // The shell script never checks this; a failed export that still
        // reported success would pass there and fail here.
        .failure()
        // The reference holds only the stable prefix of the message; what
        // follows "Additional technical details:" lists absolute paths that
        // differ from one machine to the next.
        .stderr_eq_trimmed(fixture!("test023-mermaid-hard"))
        .file_absent("023-mermaid-bad--Note.md.html");
}

/// Port of test 121, the `zettel` counterpart of test 021.
#[test]
fn zettel_a_diagram_renders_to_inline_svg() {
    let tp = Tp::new(Scheme::Zettel).file(
        "121--mermaid__diagram.md",
        &format!("---\ntitle: mermaid\nkeywords:\n- diagram\nscheme: zettel\n---\n\n{GOOD}"),
    );

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "121--mermaid__diagram.md",
    ])
    .success()
    .file_contains("121--mermaid__diagram.md.html", "<div class=\"mermaid\">")
    .file_contains("121--mermaid__diagram.md.html", "<svg");
}

/// Port of test 122, the `zettel` counterpart of test 022.
#[test]
fn zettel_an_inline_policy_turns_a_broken_diagram_into_an_error_box() {
    let tp = Tp::new(Scheme::Zettel)
        .file(
            "122--mermaid-bad__diagram.md",
            &format!("---\ntitle: mermaid bad\nkeywords:\n- diagram\nscheme: zettel\n---\n\n{BAD}"),
        )
        .config_edited("tpnote-mermaid-inline.toml", inline_policy);

    tp.run([
        "--no-filename-sync",
        "--export",
        ".",
        "122--mermaid-bad__diagram.md",
    ])
    .success()
    .file_contains("122--mermaid-bad__diagram.md.html", "mermaid-error");
}

/// Port of test 123, the `zettel` counterpart of test 023. The message does not
/// depend on the scheme, so this shares test 023's reference file -- the shell
/// scripts keep two byte for byte identical copies of it.
#[test]
fn zettel_a_broken_diagram_fails_a_batch_export() {
    const INPUT: &str = "123--mermaid-bad__diagram.md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        &format!("---\ntitle: mermaid bad\nkeywords:\n- diagram\nscheme: zettel\n---\n\n{BAD}"),
    );

    tp.run(["--batch", "--no-filename-sync", "--export", ".", INPUT])
        .failure()
        .stderr_eq_trimmed(fixture!("test123-mermaid-hard"))
        .file_absent("123--mermaid-bad__diagram.md.html");
}

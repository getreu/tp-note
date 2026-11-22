//! LaTeX formulas, and the three ways the exporter can treat a broken one.

use crate::harness::{Scheme, Tp};

/// A formula that parses.
const GOOD: &str = "```math\nx^n + y^n = z^n\n```\n";
/// A formula that does not: `\left(` is never closed, which makes the
/// conversion to MathML fail.
const BAD: &str = "```math\n\\left( x\n```\n";

/// Switches the exporter's error policy to `Inline`.
fn inline_policy(line: &str) -> Option<String> {
    line.starts_with("exporter_embedded_content_error_policy = ")
        .then(|| "exporter_embedded_content_error_policy = \"Inline\"".to_string())
}

/// Port of test 024. A formula that parses is converted to MathML and embedded
/// in the exported document.
#[test]
fn a_formula_renders_to_inline_mathml() {
    let tp = Tp::new(Scheme::Default).file(
        "024-latex--Note.md",
        &format!("---\ntitle: latex\nsubtitle: Note\n---\n\n{GOOD}"),
    );

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "024-latex--Note.md",
    ])
    .success()
    .file_line_contains_all("024-latex--Note.md.html", &["<math ", "MathML"]);
}

/// Port of test 025. Under the `Inline` policy a broken formula becomes an
/// error box in the document and the export still succeeds. `--batch` is
/// deliberately absent, as it would force `HardError`.
#[test]
fn an_inline_policy_turns_a_broken_formula_into_an_error_box() {
    let tp = Tp::new(Scheme::Default)
        .file(
            "025-latex-bad--Note.md",
            &format!("---\ntitle: latex bad\nsubtitle: Note\n---\n\n{BAD}"),
        )
        .config_edited("tpnote-latex-inline.toml", inline_policy);

    tp.run([
        "--no-filename-sync",
        "--export",
        ".",
        "025-latex-bad--Note.md",
    ])
    .success()
    .file_contains("025-latex-bad--Note.md.html", "math-error");
}

/// Port of test 026. Exporting in batch mode forces `HardError`, so a formula
/// that does not parse fails the whole export.
#[test]
fn a_broken_formula_fails_a_batch_export() {
    const INPUT: &str = "026-latex-bad--Note.md";
    let tp = Tp::new(Scheme::Default).file(
        INPUT,
        &format!("---\ntitle: latex bad\nsubtitle: Note\n---\n\n{BAD}"),
    );

    tp.run(["--batch", "--no-filename-sync", "--export", ".", INPUT])
        // Not checked by the shell script, cf. the mermaid counterpart.
        .failure()
        .stderr_eq_trimmed(fixture!("test026-latex-hard"))
        .file_absent("026-latex-bad--Note.md.html");
}

/// Port of test 124, the `zettel` counterpart of test 024.
#[test]
fn zettel_a_formula_renders_to_inline_mathml() {
    let tp = Tp::new(Scheme::Zettel).file(
        "124--latex__formula.md",
        &format!("---\ntitle: latex\nkeywords:\n- formula\nscheme: zettel\n---\n\n{GOOD}"),
    );

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "124--latex__formula.md",
    ])
    .success()
    .file_line_contains_all("124--latex__formula.md.html", &["<math ", "MathML"]);
}

/// Port of test 125, the `zettel` counterpart of test 025.
#[test]
fn zettel_an_inline_policy_turns_a_broken_formula_into_an_error_box() {
    let tp = Tp::new(Scheme::Zettel)
        .file(
            "125--latex-bad__formula.md",
            &format!("---\ntitle: latex bad\nkeywords:\n- formula\nscheme: zettel\n---\n\n{BAD}"),
        )
        .config_edited("tpnote-latex-inline.toml", inline_policy);

    tp.run([
        "--no-filename-sync",
        "--export",
        ".",
        "125--latex-bad__formula.md",
    ])
    .success()
    .file_contains("125--latex-bad__formula.md.html", "math-error");
}

/// Port of test 126, the `zettel` counterpart of test 026. Shares test 026's
/// reference file: the message does not depend on the scheme.
#[test]
fn zettel_a_broken_formula_fails_a_batch_export() {
    const INPUT: &str = "126--latex-bad__formula.md";
    let tp = Tp::new(Scheme::Zettel).file(
        INPUT,
        &format!("---\ntitle: latex bad\nkeywords:\n- formula\nscheme: zettel\n---\n\n{BAD}"),
    );

    tp.run(["--batch", "--no-filename-sync", "--export", ".", INPUT])
        .failure()
        .stderr_eq_trimmed(fixture!("test126-latex-hard"))
        .file_absent("126--latex-bad__formula.md.html");
}

//! What a comment, a `<script>` or a `<style>` holds is not markup.

use crate::harness::{Scheme, Tp};

/// A heading and a link inside each kind of inert region, next to a real one of
/// each. The last heading closes an unopened `</h2>` inside a comment, to check
/// the comment is skipped rather than parsed.
const INERT: &str = r#"---
title: inert
subtitle: Note
---

<!-- <h2>Summary</h2> -->

## Summary

<!-- <a href="other.md">commented link</a> -->

[real link](other.md)

<script>var a = '<h3>Scripted</h3>'; var b = '<a href="hidden.md">x</a>';</script>

<style>/* <h3>Styled</h3> */</style>

## Sum<!-- </h2> -->mary two
"#;

/// Port of test 028. A heading inside an inert region gets no id and reserves
/// no slug -- so the real `## Summary` keeps the plain `summary` id rather than
/// being pushed to `summary-1` -- and a link in there is left alone instead of
/// being rewritten.
#[test]
fn headings_and_links_inside_inert_regions_are_left_alone() {
    let tp = Tp::new(Scheme::Default).file("028-inert--Note.md", INERT);

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "--export-link-rewriting=short",
        "028-inert--Note.md",
    ])
    .success()
    .file_eq("028-inert--Note.md.html", fixture!("test028-html"));
}

/// Port of test 128. The same note under `TPNOTE_SCHEME=zettel`; the two
/// reference files are byte for byte identical.
#[test]
fn zettel_headings_and_links_inside_inert_regions_are_left_alone() {
    let tp = Tp::new(Scheme::Zettel).file("128-inert--Note.md", INERT);

    tp.run([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "--export-link-rewriting=short",
        "128-inert--Note.md",
    ])
    .success()
    .file_eq("128-inert--Note.md.html", fixture!("test128-html"));
}

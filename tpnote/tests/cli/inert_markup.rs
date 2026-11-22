//! What a comment, a `<script>` or a `<style>` holds is not markup.

use crate::harness::{Scheme, Tp};

/// Port of test 028 of `tests/functional/run-tests`. A heading inside an inert
/// region gets no id and reserves no slug -- so the real `## Summary` keeps the
/// plain `summary` id rather than being pushed to `summary-1` -- and a link in
/// there is left alone instead of being rewritten.
#[test]
fn headings_and_links_inside_inert_regions_are_left_alone() {
    let tp = Tp::new(Scheme::Default).file(
        "028-inert--Note.md",
        r#"---
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
"#,
    );
    let input = tp.at("028-inert--Note.md");

    tp.args([
        "--batch",
        "--no-filename-sync",
        "--export",
        ".",
        "--export-link-rewriting=short",
    ])
    .arg(&input)
    .run()
    .success()
    .file_eq("028-inert--Note.md.html", fixture!("test028-html"));
}

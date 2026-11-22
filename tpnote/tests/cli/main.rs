//! Command line tests for the `tpnote` binary.
//!
//! These are the Rust port of the shell suite that used to live in
//! `tests/functional/`. Every test here was ported by comparing against that
//! suite's own reference files, unmodified -- they are the ones in `fixtures/`,
//! moved rather than copied, so a green run has always meant agreement with the
//! exact bytes the shell suite asserted.
//!
//! Those scripts are kept on the `functional-tests-reference` branch rather
//! than deleted. To check this suite against them again, rebase that branch
//! onto master and run `tests/functional/run-tests`; it reads the same
//! `fixtures/` files from here.
//!
//! Everything lives in one test binary, with a module per theme. Cargo compiles
//! each top level file under `tests/` into a separate executable, each linking
//! the whole dependency tree, so a module tree costs a great deal less to build
//! than a file tree.
//!
//! The compile matrix the shell scripts cover as their tests 900 to 906 is not
//! expressible as a `#[test]`; `cargo hack check --each-feature` covers it
//! instead, cf. `scripts/11-test.nu`.

#[macro_use]
mod harness;

mod smoke;

mod add_header;
mod annotate;
mod export_html;
mod heading_ids;
mod inert_markup;
mod latex;
mod mermaid;
mod new_note;
mod stdin_input;
mod sync_filename;

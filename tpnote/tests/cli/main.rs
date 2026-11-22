//! Command line tests for the `tpnote` binary.
//!
//! These are the Rust port of `tests/functional/run-tests` and `run-tests2`.
//! Until that port is complete both suites are kept in service and the shell
//! scripts remain the reference: every expectation here is compared against the
//! very same `tests/functional/test*-expected-output` files, unmodified, so a
//! failure here always means the port is wrong rather than the expectation.
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

mod inert_markup;
mod mermaid;
mod sync_filename;

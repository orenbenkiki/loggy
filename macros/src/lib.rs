//! Procedural macros for the loggy framework.
//!
//! If/when Rust allows placing procedural macros inside a library crate, this crate should be merged into the overall
//! loggy crate.

#![deny(missing_docs)]

extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::ItemFn;
use syn::Stmt;

/// Create a test case using `loggy`.
///
/// To use this, prefix the test with `[#loggy]`. In the test, invoke one of [`assert_logged`],
/// [`assert_logged_panics`], [`assert_panics`] or, if you wish to ignore the captured log
/// altogether, [`ignore_log`].
///
/// Since `loggy` collects messages from all threads, `test_loggy!` tests must be run with
/// `RUST_TEST_THREADS=1`, otherwise "bad things will happen". However, such tests may freely spawn
/// multiple new threads.
///
/// If the environment variable `LOGGY_MIRROR_TO_STDERR` is set to any non empty value, then all
/// log messages will be mirrored to the standard error stream, in addition to being captured. This
/// places the `Level::Debug` messages in the context of the other log messages, to help in
/// debugging.
#[proc_macro_attribute]
pub fn loggy(attributes: TokenStream, stream: TokenStream) -> TokenStream {
    assert!(attributes.is_empty(), "unexpected arguments");
    let mut input = parse_macro_input!(stream as ItemFn);
    let prefix: Stmt = parse_quote! { loggy::before_test(); };
    let suffix: Stmt = parse_quote! { loggy::after_test(); };
    input.block.stmts.insert(0, prefix);
    input.block.stmts.push(suffix);
    let output = quote! { #input };
    output.into()
}

//! Procedural macros for the loggy framework.
//!
//! If/when Rust allows placing procedural macros inside a library crate, this crate should be merged into the overall
//! loggy crate.

#![deny(missing_docs)]

extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::ItemFn;
use syn::LitStr;
use syn::Result;
use syn::Stmt;

/// How to parse a scope name argument.
struct ScopeName {
    string: LitStr,
}

impl Parse for ScopeName {
    fn parse(stream: ParseStream) -> Result<Self> {
        Ok(Self {
            string: stream.parse()?,
        })
    }
}

/// Mark a function as a scope.
///
/// To use this, prefix the test with `#[loggy::scope]` or `#[loggy::scope("name")]`. All log messages generated in the
/// code invoked by the function will be prefixed by the scope name (by default, the function name) instead of the
/// default (module name).
///
/// # Panics
///
/// If the code invoked by the function generated any error messages.
#[proc_macro_attribute]
pub fn scope(attributes: TokenStream, stream: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(stream as ItemFn);
    let name = if attributes.is_empty() {
        input.sig.ident.to_string()
    } else {
        parse_macro_input!(attributes as ScopeName).string.value()
    };
    let prefix: Stmt = parse_quote! { let _loggy_scope = loggy::Scope::new(#name); };
    input.block.stmts.insert(0, prefix);
    let output = quote! { #input };
    output.into()
}

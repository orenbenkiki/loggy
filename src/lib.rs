// Copyright (C) 2017-2021 Oren Ben-Kiki. See the LICENSE.txt file at the top-level directory of this distribution and
// at http://rust-lang.org/COPYRIGHT.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option. This file may not
// be copied, modified, or distributed except according to those terms.

// TODO test_no_loggy?
// TODO: proc macros
// TODO: check debug vs. trace

//! An opinionated library for developing and testing rust applications that use logging.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::perf)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]

pub use loggy_macros::loggy;

use crate as loggy;
use lazy_static::lazy_static;
use log::{Level, LevelFilter, Log, Metadata, Record};
use parking_lot::{Mutex, Once};
use std::cell::Cell;
use std::fmt::Write;
use std::io::{stderr, Write as IoWrite};
use std::panic::{catch_unwind, set_hook, take_hook, AssertUnwindSafe};
use unindent::unindent;

/// Log a structured message.
///
/// Usage: `log!(level, "some text {}", 1; field => value, label { sub_field => value }, ...)` results in
/// a log message:
/// ```yaml
/// some text:
///   field: value
///   label:
///     sub field: value
/// ```
///
/// This is an extension of the [slog](https://github.com/slog-rs/slog) structured message format to support nesting.
/// Note that here there's no way to control the final message format, which was chosen to target human readability.
#[macro_export]
macro_rules! log {
    ( $level:expr , $format:literal $( ; $( $tail:tt )* )? ) => {
        {
            if $level == log::Level::Error || log::log_enabled!($level) {
                #[allow(unused_mut)]
                let mut string = format!($format);
                $(
                    let mut indent = "  ".to_owned();
                    log!( @collect string , indent , $( $tail )* );
                )?
                log::log!( $level , "{}" , string );
            }
        }
    };

    ( $level:expr , $format:literal $( , $value:expr )* $( ; $( $tail:tt )* )? ) => {
        {
            if $level == log::Level::Error || log::log_enabled!($level) {
                #[allow(unused_mut)]
                let mut string = format!($format $( , $value )* );
                $(
                    let mut indent = "  ".to_owned();
                    log!( @collect string , indent , $( $tail )* );
                )?
                log::log!( $level , "{}" , string );
            }
        }
    };

    ( @collect $string:ident , $indent:ident , $name:ident $( , )? ) => {
        $string.push_str(format!("\n{}{}: {}", $indent, stringify!($name), $name).as_str());
    };

    ( @collect $string:ident , $indent:ident , $name:ident , $( $tail:tt )* ) => {
        log!( @collect $string , $indent , $name );
        log!( @collect $string , $indent , $( $tail )* );
    };

    ( @collect $string:ident, $indent:ident , $name:ident => $value:expr $( , )? ) => {
        $string.push_str(format!("\n{}{}: {}", $indent, stringify!($name), $value).as_str());
    };

    ( @collect $string:ident, $indent:ident , $name:ident => $value:expr , $( $tail:tt )* ) => {
        log!( @collect $string , $indent , $name => $value );
        log!( @collect $string , $indent , $( $tail )* );
    };

    ( @collect $string:ident , $indent:ident, $name:ident { $( $nest:tt )* } $( , )? ) => {
        $string.push_str(format!("\n{}{}:", $indent, stringify!($name)).as_str());
        $indent.push_str("  ");
        log!( @collect $string , $indent , $( $nest )* );
        $indent.pop();
        $indent.pop();
    };

    ( @collect $string:ident , $indent:ident, $name:ident { $( $nest:tt )* } , $( $tail:tt )* ) => {
        log!( @collect $string , $indent , $name { $( $nest )* } );
        log!( @collect $string , $indent , $( $tail )* );
    };
}

/// Log a countable error message.
///
/// This is identical to invoking `log!(log::Level::Error, ...)`.
///
/// Note that error messages are special. By default they are converted to a `std::panic!`, unless running inside a
/// `[#loggy]` test or inside [`count_errors`].
#[macro_export]
macro_rules! error { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Error , $( $arg )* ) } }

/// Log an error message when it is known we are not counting errors.
///
/// This allows the compiler to know that any following code is unreachable (which isn't always the case for errors).
#[macro_export]
macro_rules! panic {
    ( $( $arg:tt )* ) => {
        {
            loggy::error!( $( $arg )* );
            std::panic!("counting an uncountable error");
        }
    }
}

/// Log a warning message.
///
/// This is identical to invoking `log!(log::Level::Warn, ...)`.
#[macro_export]
macro_rules! warn { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Warn , $( $arg )* )
} }

/// Log either an error or a warning, depending on some configuration parameter.
///
/// Invoking `note!(is_error, ...)` is identical to invoking `error!(...)` if `is_error` is `true`, or `warn!(...)` if
/// `is_error` is `false`. This allows easily handling conditions whose handling depends on command line arguments or
/// other considerations.
#[macro_export]
macro_rules! note {
    ( $is_error:expr, $( $arg:tt )* ) => {
        loggy::log!( if $is_error { log::Level::Error } else { log::Level::Warn } , $( $arg )* )
    };
}

/// Log an informational message..
///
/// This is identical to invoking `log!(log::Level::Info, ...)`.
#[macro_export]
macro_rules! info { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Info , $( $arg )* ) } }

/// Log a debugging message..
///
/// This is identical to invoking `log!(log::Level::Debug, ...)`.
///
/// Debug messages are special. The are always emitted in debug builds, regardless of the requested log level. They are
/// not captured by tests, and instead are always sent to the standard error. The idea being that debug messages are
/// used for, well, debugging.
#[macro_export]
macro_rules! debug { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Debug , $( $arg )* ) } }

/// Log a debugging message..
///
/// This is identical to invoking `debug!(...)`. Renaming it `todox!` ensures all uses will be reported by `cargo
/// todox`, to ensure their removal once their usefulness is past.
#[macro_export]
macro_rules! todox { ( $( $arg:tt )* ) => { loggy::debug!( $( $arg )* ) } }

/// Log a tracing message.
///
/// This is identical to invoking `log!(log::Level::Trace, ...)`.
///
/// This is originally meant for more-detailed-than-debugging messages, which doesn't really fit the model of "debug
/// messages are used for actual debugging". It may be useful if you want debug-like (very detailed) messages that are
/// not subject to the special handling of actual debug messages.
#[macro_export]
macro_rules! trace { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Trace , $( $arg )* ) } }

/// Provide program-wide control over whether some note(s) are considered to be an error.
///
/// This is typically invoked as `pub mod foo { is_an_error!(true /* or false */); }`.
///
/// This will define a nested module which exposes the following:
///
/// * `pub fn is_an_error() -> bool` returns whether the event is considered to be an error. This is initialized to the
///   value passed to the macro.
///
///   This is intended to be used in `note!(foo::is_an_error(), ...)`, using the `note!` macro provided by `loggy`. It
///   will behave as either an `error!` or a `warn!` depending on the value.
///
/// * `pub fn set_is_an_error(new_value: bool) -> bool` sets whether the event is considered to be an error, returning
///   the old value for convenience.
///
///   This allows the program to modify the setting at run-time, for example by parsing the command line arguments.
///   TODO: Provide additional functions to automate this.
#[macro_export]
macro_rules! is_an_error {
    ($default:expr) => {
        lazy_static! {
            static ref IS_AN_ERROR: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new($default);
        }

        pub fn set_is_an_error(is_error: bool) -> bool {
            IS_AN_ERROR.swap(is_error, std::sync::atomic::Ordering::Relaxed)
        }

        pub fn is_an_error() -> bool {
            IS_AN_ERROR.load(std::sync::atomic::Ordering::Relaxed)
        }
    };
}

/// Control the behavior of the `loggy` logger.
pub struct Loggy {
    /// A prefix appended to each message.
    ///
    /// This typically contains the name of the program.
    pub prefix: &'static str,

    /// Whether to include the date and time in the log message.
    pub show_time: bool,

    /// Whether to include the thread id in the log message.
    pub show_thread: bool,
}

lazy_static! {
    // BEGIN NOT TESTED
    static ref TIME_FORMAT: Vec<time::format_description::FormatItem<'static>> =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap();
    // END NOT TESTED
    static ref TOTAL_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static ref TOTAL_ERRORS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
}

thread_local!(
    static THREAD_ID: Cell<Option<usize>> = Cell::new(None);
    static NAMED_SCOPE: Cell<Option<&'static str>> = Cell::new(None);
    static THREAD_ERRORS: Cell<usize> = Cell::new(0);
    static IS_COUNTING_ERRORS: Cell<bool> = Cell::new(false);
);

impl Log for Loggy {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() == Level::Error
            || metadata.level() == Level::Debug
            || metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if record.level() == Level::Error {
            TOTAL_ERRORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            THREAD_ERRORS.with(|thread_errors_cell| {
                thread_errors_cell.set(thread_errors_cell.get() + 1);
            });
        }

        if record.level() == Level::Error || self.enabled(record.metadata()) {
            emit_message(record.level(), self.format_message(record).as_ref());
        }
    }

    // BEGIN NOT TESTED
    fn flush(&self) {
        stderr().flush().unwrap();
    }
    // END NOT TESTED
}

/// Execute some code in a named scope.
///
/// This scope `name` will be used to prefix log messages generated by the `code`, instead of the default (module name)
/// scope. An empty name will eliminate the prefix altogether.
///
/// Note this only affects error messages generated in the current thread.
pub fn in_named_scope<Code: FnOnce()>(name: &'static str, code: Code) {
    let old_scope = NAMED_SCOPE.with(|named_scope| named_scope.replace(Some(name)));
    code();
    NAMED_SCOPE.with(|named_scope| named_scope.set(old_scope));
}

/// Execute some code, while counting the errors.
///
/// Returns the number of errors reported by the `code`. These errors are also added to any containing scope. By
/// default, unless running in a test, error (formatted) messages are passed to `std::panic!`. This is disabled when
/// counting errors, allowing the code to emit multiple such messages. It is the caller's responsibility to examine the
/// number of errors and do a final `panic!`, or otherwise handle the situation.
///
/// Note this only counts error messages generated in the current thread.
#[must_use]
pub fn count_errors<Code: FnOnce()>(code: Code) -> usize {
    let was_counting_before =
        IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.replace(true)); // APPEARS NOT TESTED
    let errors_before = THREAD_ERRORS.with(std::cell::Cell::get);

    code();

    IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.set(was_counting_before));
    let errors_after = THREAD_ERRORS.with(std::cell::Cell::get);

    errors_after - errors_before
}

impl Loggy {
    fn format_message(&self, record: &Record<'_>) -> String {
        let now = if self.show_time {
            // BEGIN NOT TESTED
            time::OffsetDateTime::now_local()
                .unwrap()
                .format(&TIME_FORMAT)
                .unwrap()
            // END NOT TESTED
        } else {
            "".to_string()
        };

        let mut message = String::with_capacity(128);
        writeln!(&mut message, "{}", record.args()).unwrap();

        let mut buffer = String::with_capacity(128 + message.len());
        let mut level = record.level().to_string();
        for (index, line) in message.lines().enumerate() {
            if index > 0 {
                level = level.to_lowercase();
            }
            self.append_prefix(&mut buffer, now.as_ref(), level.as_ref(), record);
            writeln!(&mut buffer, " {}", line).unwrap();
        }

        buffer
    }

    fn append_prefix(&self, mut message: &mut String, now: &str, level: &str, record: &Record<'_>) {
        message.push_str(self.prefix);

        if self.show_thread {
            // BEGIN NOT TESTED
            THREAD_ID.with(|thread_id_cell| {
                if thread_id_cell.get().is_none() {
                    let total_threads =
                        TOTAL_THREADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    thread_id_cell.set(Some(total_threads));
                }
                let current_thread_id = thread_id_cell.get().unwrap();
                write!(&mut message, "[{}]", current_thread_id).unwrap();
            });
            // END NOT TESTED
        }

        message.push(':');

        if self.show_time {
            message.push(' '); // NOT TESTED
            message.push_str(now); // NOT TESTED
        }

        write!(&mut message, " [{}]", level).unwrap();

        if record.level() == Level::Debug {
            write!(
                &mut message,
                " {}:{}:",
                record.file().unwrap(),
                record.line().unwrap()
            )
            .unwrap();
        }

        let scope = NAMED_SCOPE.with(|named_scope| match named_scope.get() {
            None => record.module_path().unwrap(),
            Some(scope) => scope,
        });
        if !scope.is_empty() {
            write!(&mut message, " {}:", scope).unwrap();
        }
    }
}

enum LogSink {
    Stderr,
    Buffer,
}

lazy_static! {
    static ref LOG_BUFFER: Mutex<Cell<Option<String>>> = Mutex::new(Cell::new(None));
}

fn set_log_sink(log_sink: &LogSink) {
    match log_sink {
        LogSink::Stderr => {
            LOG_BUFFER.lock().set(None);
        }
        LogSink::Buffer => {
            LOG_BUFFER.lock().set(Some(String::new()));
        }
    };
}

/// Assert that the collected log messages are as expected.
///
/// The expected string is passed through `unindent` prior to the comparison, to enable proper indentation of the tests
/// data in the code.
///
/// This clears the log buffer following the comparison.
///
/// This is meant to be used in tests using the `[#loggy]` macro. Tests using this macro expect the log buffer being
/// clear at the end of the test, either by using this function or `ignore_log`.
///
/// # Panics
///
/// If the actual log is different from the expected log.
pub fn assert_logged(expected_log: &str) {
    let mut log_buffer = LOG_BUFFER.lock();
    match log_buffer.get_mut() {
        None => {
            std::panic!("asserting log when logging to stderr"); // NOT TESTED
        }
        Some(actual_log) => {
            let expected_log = fix_expected(expected_log);
            if *actual_log != expected_log {
                // BEGIN NOT TESTED
                print!(
                    "ACTUAL LOG:\n>>>\n{}<<<\nIS DIFFERENT FROM EXPECTED LOG:\n>>>\n{}<<<\n",
                    actual_log, expected_log
                );
                assert_eq!("ACTUAL LOG", "EXPECTED LOG");
            } // END NOT TESTED
            actual_log.clear();
        }
    }
}

/// Ignore (clear) the content of the captured log.
///
/// This is meant to be used in tests using the `[#loggy]` macro, if they do not otherwise consume the captured log
/// (e.g. by calling [`assert_logged`]).
///
/// # Panics
///
/// If no log is being collected.
pub fn ignore_log() {
    let mut log_buffer = LOG_BUFFER.lock();
    match log_buffer.get_mut() {
        None => std::panic!("clearing log when logging to stderr"), // NOT TESTED
        Some(ref mut buffer) => buffer.clear(),
    }
}

lazy_static! {
    static ref MIRROR_TO_STDERR: bool = std::env::var("LOGGY_MIRROR_TO_STDERR")
        // BEGIN NOT TESTED
        .map(|var| !var.is_empty())
        .unwrap_or(false);
        // END NOT TESTED
}

fn emit_message(level: Level, message: &str) {
    if level == Level::Debug {
        eprint!("{}", message);
        return;
    }

    if level == Level::Error && !IS_COUNTING_ERRORS.with(std::cell::Cell::get) {
        std::panic!("{}", message);
    }

    let mut log_buffer = LOG_BUFFER.lock();
    match log_buffer.get_mut() {
        None => eprint!("{}", message), // NOT TESTED
        Some(buffer) => {
            if *MIRROR_TO_STDERR {
                eprint!("{}", message); // NOT TESTED
            }
            buffer.push_str(message);
        }
    }
}

/// Return the total number of calls to `error!` in the whole program.
///
/// This is reset for each test using the `[#loggy]` macro.
#[must_use]
pub fn errors() -> usize {
    TOTAL_ERRORS.load(std::sync::atomic::Ordering::Relaxed)
}

static LOGGER_ONCE: Once = Once::new();

#[doc(hidden)]
pub fn before_test() {
    LOGGER_ONCE.call_once(|| {
        log::set_logger(&Loggy {
            prefix: "test",
            show_time: false,
            show_thread: false,
        })
        .unwrap();
        log::set_max_level(LevelFilter::Trace);
    });

    TOTAL_THREADS.store(0, std::sync::atomic::Ordering::Relaxed);
    TOTAL_ERRORS.store(0, std::sync::atomic::Ordering::Relaxed);
    THREAD_ID.with(|thread_id_cell| thread_id_cell.set(None));
    THREAD_ERRORS.with(|thread_errors_cell| thread_errors_cell.set(0));
    IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.set(true));

    set_log_sink(&LogSink::Buffer);
}

#[doc(hidden)]
pub fn after_test() {
    assert_logged("");
    set_log_sink(&LogSink::Stderr);
    IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.set(false));
}

/// Ensure that executing some code will panic with a specific error message (ignoring and clearing the log).
///
/// TODO: This isn't really the best place for this, but it is necessary to capture tests that `panic!`.
///
/// Unlike `#[should_panic(expected = "...")]`, this:
/// * Allows isolating a specific part of the test instead of decorating a whole test;
/// * Does not pollute stderr with the panic indication and back trace;
/// * Allows the caller to dynamically generate the expected panic message;
/// * Insists on the exact panic string rather than just a sub-string if it.
///
/// # Panics
///
/// If the code does not panic, or panics with a different message than expected.
pub fn assert_panics<Code: FnOnce() -> Result, Result>(expected_panic: &str, code: Code) {
    do_assert_logged_panics(None, expected_panic, code);
}

/// Same as [`assert_panics`] but also assert that the log contained the specified entries at the point of the panic.
pub fn assert_logged_panics<Code: FnOnce() -> Result, Result>(
    expected_log: &str,
    expected_panic: &str,
    code: Code,
) {
    do_assert_logged_panics(Some(expected_log), expected_panic, code);
}

fn do_assert_logged_panics<Code: FnOnce() -> Result, Result>(
    expected_log: Option<&str>,
    expected_panic: &str,
    code: Code,
) {
    let prev_hook = take_hook();
    set_hook(Box::new(|_| {}));

    let was_counting_before =
        IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.replace(false)); // APPEARS NOT TESTED
    let result = catch_unwind(AssertUnwindSafe(code));
    IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.set(was_counting_before));
    set_hook(prev_hook);

    if let Some(expected_log) = expected_log {
        assert_logged(expected_log);
    } else {
        ignore_log();
    }

    match result {
        Ok(_) => std::panic!("test did not panic"), // NOT TESTED

        Err(error) => {
            let actual_panic = if let Some(actual_panic) = error.downcast_ref::<String>() {
                actual_panic.as_str()
            // BEGIN NOT TESTED
            } else if let Some(actual_panic) = error.downcast_ref::<&'static str>() {
                actual_panic
            // END NOT TESTED
            } else {
                "unknown panic" // NOT TESTED
            };
            let expected_panic = fix_expected(expected_panic);
            assert_eq!(actual_panic, expected_panic);
        }
    }
}

/// Check that some code writes the expected results.
///
/// TODO: This isn't really the best place for this.
///
/// # Panics
///
/// If the code does not write the expected text.
pub fn assert_writes<Code: FnOnce(&mut dyn IoWrite)>(expected_string: &str, code: Code) {
    let mut actual_bytes: Vec<u8> = vec![];
    code(&mut actual_bytes);
    let actual_string = String::from_utf8(actual_bytes).ok().unwrap();
    let expected_string = fix_expected(expected_string);
    assert_eq!(actual_string, expected_string);
}

/// Assert that some code does not log any errors by counting them.
///
/// # Panics
///
/// If the code logs even one error.
pub fn assert_no_errors<Code: FnOnce()>(name: &str, code: Code) {
    let errors_count = count_errors(code);
    if errors_count > 0 {
        panic!("{} {} error(s)", errors_count, name);
    }
}

fn fix_expected(expected: &str) -> String {
    let needs_trailing_newline = expected.ends_with('\n') || expected.ends_with(' ');
    let expected = unindent(expected);
    let expected = expected.strip_prefix('\n').unwrap_or(&expected);
    match (expected.ends_with('\n'), needs_trailing_newline) {
        (true, false) => expected.strip_suffix('\n').unwrap().to_owned(), // NOT TESTED
        (false, true) => format!("{}\n", expected),
        _ => expected.to_owned(),
    }
}

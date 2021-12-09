// Copyright (C) 2017-2021 Oren Ben-Kiki. See the LICENSE.txt file at the top-level directory of this distribution and
// at http://rust-lang.org/COPYRIGHT.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option. This file may not
// be copied, modified, or distributed except according to those terms.

//! An opinionated library for developing and testing rust applications that use logging.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::perf)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]

use lazy_static::lazy_static;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::cell::Cell;
use std::fmt::Write;
use std::io::stderr;
use std::io::Write as IoWrite;
use std::sync::{Mutex, Once};
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
    ( $level:expr , $format:literal $( , $( $value:expr )* )? $( ; $( $tail:tt )* )? ) => {
        {
            if log::log_enabled!($level) {
                let mut string = format!($format $( , $( $value )* )? );
                $(
                    let mut indent = "  ".to_owned();
                    log!( @collect string , indent , $( $tail )* );
                    indent.pop();
                    indent.pop();
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

/// Log an error message.
///
/// This is identical to invoking `log!(log::Level::Error, ...)`.
///
/// Note that error messages are special. By default they are converted to a `panic!`, unless running inside a
/// [`test_loggy!`] or inside [`count_errors`].
#[macro_export]
macro_rules! error { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Error , $( $arg )* ) } }

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
macro_rules! todox { ( $( $arg:tt )* ) => { debug!( $( $arg )* ) } }

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
        metadata.level() == Level::Debug || metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if record.level() == Level::Error {
            TOTAL_ERRORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            THREAD_ERRORS.with(|thread_errors_cell| {
                thread_errors_cell.set(thread_errors_cell.get() + 1);
            });
        }

        if self.enabled(record.metadata()) {
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
/// default, unless running in a test, error (formatted) messages are passed to `panic!`. This is disabled when counting
/// errors, allowing the code to emit multiple such messages. It is the caller's responsibility to examine the number of
/// errors and do a final `panic!`, or otherwise handle the situation.
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
            LOG_BUFFER.lock().unwrap().set(None);
        }
        LogSink::Buffer => {
            LOG_BUFFER.lock().unwrap().set(Some(String::new()));
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
/// This is meant to be used in tests using the `test_loggy!` macro. Tests using this macro expect the log buffer being
/// clear at the end of the test, either by using this function or `clear_log`.
///
/// # Panics
///
/// If the actual log is different from the expected log.
pub fn assert_log(expected: &str) {
    let expected = unindent(expected);
    let mut log_buffer = LOG_BUFFER.lock().unwrap();
    match log_buffer.get_mut() {
        None => {
            panic!("asserting log when logging to stderr"); // NOT TESTED
        }
        Some(actual) => {
            if *actual != expected {
                // BEGIN NOT TESTED
                print!(
                    "ACTUAL LOG:\n{}\nIS DIFFERENT FROM EXPECTED LOG:\n{}\n",
                    actual, expected
                );
                assert_eq!("ACTUAL LOG", "EXPECTED LOG");
            } // END NOT TESTED
            actual.clear();
        }
    }
}

/// Clear the log buffer following the comparison.
///
/// This is meant to be used in tests using the `test_loggy!` macro. Tests using this macro expect the log buffer being
/// clear at the end of the test, either by using this function or `assert_log`.
///
/// # Panics
///
/// If no log is being collected.
pub fn clear_log() {
    let mut log_buffer = LOG_BUFFER.lock().unwrap();
    match log_buffer.get_mut() {
        None => panic!("clearing log when logging to stderr"), // NOT TESTED
        Some(buffer) => buffer.clear(),
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
    let mut log_buffer = LOG_BUFFER.lock().unwrap();
    match log_buffer.get_mut() {
        None => {
            // BEGIN NOT TESTED
            if IS_COUNTING_ERRORS.with(std::cell::Cell::get) {
                eprint!("{}", message);
            } else {
                panic!("{}", message);
            }
            // END NOT TESTED
        }
        Some(buffer) => {
            if *MIRROR_TO_STDERR {
                eprint!("{}", message); // NOT TESTED
            }
            buffer.push_str(message);
        }
    }
}

static LOGGER_ONCE: Once = Once::new();

/// Create a test case using `loggy`.
///
/// `test_loggy!(name, { ... });` creates a test case which captures all log messages (except for `Level::Debug`
/// messages). It is expected to use either `assert_log` or `clear_log` to clear the buffered log before the test ends.
/// It is possible to provide additional attributes in addition to `#[test]` by specifying them before the name, as in
/// `test_loggy!(#[cfg(debug_assertions)], name, { ... });`
///
/// Since `loggy` collects messages from all threads, `test_loggy!` tests must be run with `RUST_TEST_THREADS=1`,
/// otherwise "bad things will happen". However, such tests may freely spawn multiple new threads.
///
/// If the environment variable `LOGGY_MIRROR_TO_STDERR` is set to any non empty value, then all log messages will be
/// mirrored to the standard error stream, in addition to being captured. This places the `Level::Debug` messages in the
/// context of the other log messages, to help in debugging.
#[macro_export]
macro_rules! test_loggy {
    ($(#[$attr:meta])*, $name:ident, $test:block) => {
        #[test]
        $( #[$attr] )*
        fn $name() {
            loggy::before_test();
            $test
            loggy::after_test();
        }
    };
    ($name:ident, $test:block) => {
        #[test]
        fn $name() {
            loggy::before_test();
            $test
            loggy::after_test();
        }
    };
}

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
    assert_log("");
    set_log_sink(&LogSink::Stderr);
    IS_COUNTING_ERRORS.with(|is_counting_errors| is_counting_errors.set(false));
}

/// Return the total number of calls to `error!` in the whole program.
///
/// This is reset for each test using the `test_loggy!` macro.
#[must_use]
pub fn errors() -> usize {
    TOTAL_ERRORS.load(std::sync::atomic::Ordering::Relaxed)
}

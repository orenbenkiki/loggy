// Copyright (C) 2017-2021 Oren Ben-Kiki. See the LICENSE.txt file at the top-level directory of this distribution and
// at http://rust-lang.org/COPYRIGHT.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option. This file may not
// be copied, modified, or distributed except according to those terms.

#![doc = include_str!("../README.md")]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::perf)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]

pub use loggy_macros::scope;

use lazy_static::lazy_static;
use log::{logger, set_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;
use std::cell::Cell;
use std::fmt::Write;
use std::io::{stderr, Write as IoWrite};
use std::marker::PhantomData;
use std::panic::{catch_unwind, set_hook, take_hook, AssertUnwindSafe};
use std::sync::atomic::AtomicBool;
use std::thread::panicking;
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

/// Log an error message, which will trigger a panic at the end of the current [`scope`].
///
/// This is identical to invoking `log!(log::Level::Error, ...)`.
///
/// # Panics
///
/// If invoked outside any scope, this will immediately panic with a generic message.
#[macro_export]
macro_rules! error { ( $( $arg:tt )* ) => {
    loggy::log!( log::Level::Error , $( $arg )* );
} }

/// Generate a panic error message and immediately terminate the current thread.
///
/// # Panics
///
/// Always.
#[macro_export]
macro_rules! panic {
    ( $( $arg:tt )* ) => {
        {
            loggy::force_panic();
            loggy::error!( $( $arg )* );
            std::unreachable!();
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
        if $is_error {
            loggy::error!( $( $arg )* );
        } else {
            loggy::warn!( $( $arg )* );
        }
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
/// Debug messages are special. The are always emitted in debug builds, regardless of the requested
/// log level. They are never captured by tests, and instead are always sent to the standard error.
/// The idea being that debug messages are used for, well, debugging.
#[macro_export]
macro_rules! debug { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Debug , $( $arg )* ) } }

/// Log a debugging message that must be removed from the code once some debugging task is
/// complete.
///
/// This is identical to invoking `debug!(...)`. Renaming it `todox!` ensures all uses will be reported by `cargo
/// todox`, to ensure their removal once their usefulness is past.
#[macro_export]
macro_rules! todox { ( $( $arg:tt )* ) => { loggy::debug!( $( $arg )* ) } }

/// Log a tracing message.
///
/// This is identical to invoking `log!(log::Level::Trace, ...)`.
#[macro_export]
macro_rules! trace { ( $( $arg:tt )* ) => { loggy::log!( log::Level::Trace , $( $arg )* ) } }

/// Provide program-wide control over whether some note(s) are considered to be an error.
///
/// This is typically invoked as `pub mod some_condition { is_an_error!(true /* or false */); }`.
///
/// This will define a nested module which exposes the following:
///
/// * `pub fn is_an_error() -> bool` returns whether the event is considered to be an error. This is initialized to the
///   value passed to the macro.
///
///   This is intended to be used in [`note!`], which will behave as either an [`error!`] or a
///   [`warn!`] depending on the value.
///
/// * `pub fn set_is_an_error(new_value: bool) -> bool` sets whether the event is considered to be an error, returning
///   the old value for convenience.
///
///   This allows the program to modify the setting at run-time, for example by parsing the command line arguments.
///   TODO: Provide additional functions to automate this.
#[macro_export]
macro_rules! is_an_error {
    ($default:expr) => {
        static IS_AN_ERROR: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new($default);

        pub fn set_is_an_error(is_error: bool) -> bool {
            IS_AN_ERROR.swap(is_error, std::sync::atomic::Ordering::Relaxed)
        }

        pub fn is_an_error() -> bool {
            IS_AN_ERROR.load(std::sync::atomic::Ordering::Relaxed)
        }
    };
}

/// A named scope for log messages and [`error`]s.
#[derive(Clone, Copy)]
struct NamedScope {
    /// The scope name (to replace the module name in the messages).
    name: &'static str,

    /// The number of errors we've seen in the scope.
    errors: usize,
}

thread_local! {
    static NAMED_SCOPE: Cell<Option<NamedScope>> = Cell::new(None);
}

/// An RAII scope for log messages and [`error`]s.
pub struct Scope<'a> {
    /// The previous scope in effect before this one.
    previous: Option<NamedScope>,

    /// Ensure the scope name outlives the scope.
    name_lifetime: PhantomData<&'a str>,
}

impl<'a> Scope<'a> {
    /// Create a new logging scope.
    #[must_use]
    pub fn new(name: &'a str) -> Self {
        let name_ptr: *const str = name;
        let static_name_ref: &'static str = unsafe { &*name_ptr };
        let next: NamedScope = NamedScope {
            name: static_name_ref,
            errors: 0,
        };
        let previous = NAMED_SCOPE.with(|named_scope| named_scope.replace(Some(next)));
        Scope {
            previous,
            name_lifetime: PhantomData,
        }
    }

    /// Execute some code with in a named scope.
    pub fn with<T, Code: FnOnce() -> T>(name: &'a str, code: Code) -> T {
        let _scope = Scope::new(name);
        code()
    }
}

impl<'a> Drop for Scope<'a> {
    fn drop(&mut self) {
        let current = NAMED_SCOPE
            .with(|named_scope| named_scope.replace(self.previous))
            .unwrap();
        if current.errors > 0 && !panicking() {
            std::panic!(
                "{}: [ERROR] {}: failed with {} error(s)",
                Loggy::global().prefix,
                current.name,
                current.errors
            );
        }
    }
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
    /// The format to use for the time in emitted log messages.
    static ref TIME_FORMAT: Vec<time::format_description::FormatItem<'static>> =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap(); // NOT TESTED
}

static TOTAL_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

thread_local!(
    static THREAD_ID: Cell<Option<usize>> = Cell::new(None);
    static FORCE_PANIC: Cell<bool> = Cell::new(false);
);

impl Log for Loggy {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() == Level::Error
            || metadata.level() == Level::Debug
            || metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
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

impl Loggy {
    fn global() -> &'static Self {
        let logger_ptr: *const dyn Log = logger();
        #[allow(clippy::cast_ptr_alignment)]
        #[allow(clippy::ptr_as_ptr)]
        unsafe {
            &*(logger_ptr as *const Self)
        }
    }

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
            // BEGIN MAYBE TESTED
            write!(
                // END MAYBE TESTED
                &mut message,
                " {}:{}:",
                record.file().unwrap(), // MAYBE TESTED
                record.line().unwrap()  // MAYBE TESTED
            )
            .unwrap();
        }

        let scope = NAMED_SCOPE.with(|named_scope| match named_scope.get() {
            None => record.module_path().unwrap(),
            Some(scope) => scope.name,
        });
        if !scope.is_empty() {
            write!(&mut message, " {}:", scope).unwrap();
        }
    }
}

lazy_static! {
    /// The buffer capturing the log messages for assertions.
    static ref LOG_BUFFER: Mutex<Cell<Option<String>>> = Mutex::new(Cell::new(None));

    /// Whether to mirror captured log messages to stderr.
    static ref MIRROR_TO_STDERR: bool = std::env::var("LOGGY_MIRROR_TO_STDERR")
        // BEGIN MAYBE TESTED
        .map(|var| !var.is_empty())
        .unwrap_or(false);
    // END MAYBE TESTED
}

/// Whether we already setup loggy as the global logger.
static DID_SET_LOGGER: AtomicBool = AtomicBool::new(false);

/// Force the next error-level message to be emitted as a panic.
#[doc(hidden)]
pub fn force_panic() {
    FORCE_PANIC.with(|force_panic| {
        force_panic.set(true);
    });
}

/// Actually emit (or capture) a log message.
fn emit_message(level: Level, message: &str) {
    if level == Level::Debug {
        eprint!("{}", message); // MAYBE TESTED
        return;
    }

    if level == Level::Error {
        if FORCE_PANIC.with(|force_panic| force_panic.replace(false)) {
            std::panic!("{}", message);
        } else {
            NAMED_SCOPE.with(|maybe_named_scope| {
                if let Some(ref mut named_scope) = maybe_named_scope.get() {
                    named_scope.errors += 1;
                    maybe_named_scope.set(Some(*named_scope));
                } else {
                    std::panic!(
                        "{}: error! called outside a named scope",
                        Loggy::global().prefix,
                    );
                }
            });
        }
    }

    let mut log_buffer = LOG_BUFFER.lock();
    match log_buffer.get_mut() {
        None => eprint!("{}", message), // NOT TESTED
        Some(buffer) => {
            if *MIRROR_TO_STDERR {
                eprint!("{}", message); // MAYBE TESTED
            }
            buffer.push_str(message);
        }
    }
}

/// RAII for capturing the log content.
struct Capture {}

impl Capture {
    fn new() -> Self {
        if !DID_SET_LOGGER.swap(true, std::sync::atomic::Ordering::Relaxed) {
            set_logger(&Loggy {
                prefix: "test",
                show_time: false,
                show_thread: false,
            })
            .unwrap();
            set_max_level(LevelFilter::Trace);
        }

        LOG_BUFFER.lock().set(Some(String::new()));

        TOTAL_THREADS.store(0, std::sync::atomic::Ordering::Relaxed);
        THREAD_ID.with(|thread_id_cell| thread_id_cell.set(None));

        Self {}
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        LOG_BUFFER.lock().set(None);
    }
}

lazy_static! {
    /// Ensure there is only a single test which is capturing log entries.
    static ref SINGLE_TEST: Mutex<()> = Mutex::new(());
}

/// Assert that the collected log messages are as expected.
///
/// The expected string is passed through `unindent` prior to the comparison, to enable proper indentation of the tests
/// data in the code.
///
/// # Notes
///
/// The rust `log` facade mandates using a single global logger. Therefore, only one test can capture the log at any
/// given time, using a a global `Mutex`. Therefore, nesting this inside itself, [`assert_panics`] or
/// [`assert_logs_panics`] will deadlock.
///
/// # Panics
///
/// If the actual log is different from the expected log.
pub fn assert_logs<Code: FnOnce() -> Result, Result>(expected_log: &str, code: Code) -> Result {
    let _single_test = SINGLE_TEST.lock();
    do_assert_logs_panics(Some(expected_log), None, code).unwrap()
}

/// Ensure that executing some code will panic with a specific error message (ignoring the log).
///
/// TODO: This crate isn't really the best place for this.
///
/// Unlike `#[should_panic(expected = "...")]`, this:
/// * Allows isolating a specific part of the test instead of decorating a whole test;
/// * Does not pollute stderr with the panic indication and back trace;
/// * Allows the caller to dynamically generate the expected panic message;
/// * Insists on the exact panic string rather than just a sub-string if it.
///
/// # Notes
///
/// The rust `log` facade mandates using a single global logger. Therefore, only one test can capture the log at any
/// given time, using a a global `Mutex`. Therefore, nesting this inside itself, [`assert_logs`] or
/// [`assert_logs_panics`] will deadlock.
///
/// # Panics
///
/// If the code does not panic, or panics with a different message than expected.
pub fn assert_panics<Code: FnOnce() -> Result, Result>(expected_panic: &str, code: Code) {
    let _single_test = SINGLE_TEST.lock();
    do_assert_logs_panics(None, Some(expected_panic), code);
}

/// Combine [`assert_logs`] and [`assert_panics`], that is, assert that the expected log is generated and then the
/// expected panic is triggered.
///
/// # Notes
///
/// The rust `log` facade mandates using a single global logger. Therefore, only one test can capture the log at any
/// given time, using a a global `Mutex`. Therefore, nesting this inside itself, [`assert_panics`] or
/// [`assert_logs_panics`] will deadlock.
///
/// # Panics
///
/// If the code does generate the expected log, or does not panic, or panics with a different message than expected.
pub fn assert_logs_panics<Code: FnOnce() -> Result, Result>(
    expected_log: &str,
    expected_panic: &str,
    code: Code,
) {
    let _single_test = SINGLE_TEST.lock();
    do_assert_logs_panics(Some(expected_log), Some(expected_panic), code);
}

fn do_assert_logs_panics<Code: FnOnce() -> Result, Result>(
    expected_log: Option<&str>,
    expected_panic: Option<&str>,
    code: Code,
) -> Option<Result> {
    let _capture = Capture::new();

    if let Some(expected_panic) = expected_panic {
        let prev_hook = take_hook();
        set_hook(Box::new(|_| {}));
        let result = catch_unwind(AssertUnwindSafe(code));
        set_hook(prev_hook);

        do_assert_logs(expected_log);

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
                if actual_panic != expected_panic {
                    // BEGIN NOT TESTED
                    print!(
                        "ACTUAL PANIC:\n>>>\n{}<<<\nIS DIFFERENT FROM EXPECTED PANIC:\n>>>\n{}<<<\n",
                        actual_panic, expected_panic
                    );
                    assert_eq!("ACTUAL PANIC", "EXPECTED PANIC");
                } // END NOT TESTED
            }
        }
        None
    } else {
        let result = code();
        do_assert_logs(expected_log);
        Some(result)
    }
}

fn do_assert_logs(expected_log: Option<&str>) {
    if let Some(expected_log) = expected_log {
        let actual_log = LOG_BUFFER.lock().take().unwrap();
        let expected_log = fix_expected(expected_log);
        if actual_log != expected_log {
            // BEGIN NOT TESTED
            print!(
                "ACTUAL LOG:\n>>>\n{}<<<\nIS DIFFERENT FROM EXPECTED LOG:\n>>>\n{}<<<\n",
                actual_log, expected_log
            );
            assert_eq!("ACTUAL LOG", "EXPECTED LOG");
        } // END NOT TESTED
    }
}

/// Check that some code writes the expected results.
///
/// TODO: This crate isn't really the best place for this.
///
/// # Panics
///
/// If the code does not write the expected text.
pub fn assert_writes<Code: FnOnce(&mut dyn IoWrite)>(expected_string: &str, code: Code) {
    let mut actual_bytes: Vec<u8> = vec![];
    code(&mut actual_bytes);
    let actual_string = String::from_utf8(actual_bytes).ok().unwrap();
    let expected_string = fix_expected(expected_string);
    if actual_string != expected_string {
        // BEGIN NOT TESTED
        print!(
            "ACTUAL WRITTEN:\n>>>\n{}<<<\nIS DIFFERENT FROM EXPECTED WRITTEN:\n>>>\n{}<<<\n",
            actual_string, expected_string
        );
        assert_eq!("ACTUAL WRITTEN", "EXPECTED WRITTEN");
        // END NOT TESTED
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

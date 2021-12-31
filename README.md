# loggy v0.5.1-dev
[![Verify](https://github.com/orenbenkiki/loggy/actions/workflows/on_push.yml/badge.svg)](https://github.com/orenbenkiki/loggy/actions/workflows/on_push.yml) [![Monthly audit](https://github.com/orenbenkiki/loggy/actions/workflows/monthly_audit.yml/badge.svg)](https://github.com/orenbenkiki/loggy/actions/workflows/on_updated_dependencies.yml) [![codecov](https://codecov.io/gh/orenbenkiki/loggy/branch/master/graph/badge.svg)](https://codecov.io/gh/orenbenkiki/loggy) [![Api Docs](https://docs.rs/loggy/badge.svg)](https://docs.rs/crate/loggy)

An opinionated library for developing and testing rust applications that use logging.

This was initially inspired by [simple-logging](https://github.com/Ereski/simple-logging) implementation, with
additional features focusing on development of applications (as opposed to libraries). Structured messages were
influenced by [slog](https://github.com/slog-rs/slog), but allow for nested structures and provide only a single format
targeting human readability.

## Motivation

This library was written to support the development of a non-trivial Rust binary application(s) that uses logging. The
functionality provided here was factored out, both to keep it isolated from the application's code itself, and in the
hope it might prove useful to others.

In essence, the focus of this library is on using logging to provide information to the human user running the console
application, rather than providing logging events for analysis of the behavior of server-like application. For the
latter use case, you would want something like  [slog](https://github.com/slog-rs/slog).

Technically, this library is an implementation for the Rust log facade, with a few additional features thrown in.

## Features

The provided features reflect the library's opinionated nature.

### Message formatting

Messages are always emitted to the standard error (except for in tests, where they may be captured for use in
assertions). The message format is `<prefix>[<thread>]: <time> [<level]>] <module or scope>: <message>`, where the
thread and time may be omitted when you set up the global logger. For example:

```ignore
extern crate loggy;

fn main() {
    log::set_logger(&loggy::Loggy {
        prefix: "...", // Typically, the name of the program.
        show_time: true, // Or false, if you prefer.
        show_thread: true, // Or false, if you prefer.
    }).unwrap();
    log::set_max_level(log::LevelFilter::Info); // Or whatever level you want.

    // Use loggy facilities in the rest of the code.
    // ...
}
```

Logging multi-line messages (that contain `\n`) will generate multiple log lines, which will always be consecutive (even
when logging from multiple threads). The first line will include the log level in upper case (e.g., `[ERROR]`), all the
following will specify it in lower case (e.g., `[error]`). The time stamp, if included, will be identical for all these
lines. This makes log messages easily `grep`-able, countable, etc.

Logging a message provides a structured way to format relevant additional information. The syntax is an extension of
`slog`, allowing for nested structures. However unlike in `slog`, the output format is fixed. For example:

```ignore
#[macro_use]
extern crate loggy;

fn foo() {
    let value = "bar";
    loggy::info!(
        "some text {}", 1;
        value,
        label {
            sub_field => value,
        }
    );
}
```

Will generate the message:

```yaml
program name: [INFO] scope name: some text 1
program name: [info]   value: bar
program name: [info]   label:
program name: [info]     sub_field: bar
```

### Named scopes

By default, log messages are annotated with the name of the module generating them. To better identify specific
processing stages and/or tasks, it is common to replace this by an explicit scope name; note this only applies to the
current thread. Scopes can be established in three different ways:

```ignore
#[macro_use]
extern crate loggy;

#[loggy::scope("scope name")]
fn foo() {
    // Log messages generated here will be prefixed by the scope name instead of the module name.
    // ...
}

#[loggy::scope]
fn bar() {
    // Log messages generated here will be prefixed by the function name `bar` instead of the module name.
    // ...
}

fn baz() {
    loggy::with_scope("scope name", || {
        // Log messages generated here will be prefixed by the scope name instead of the module name.
        // ...
    });

    if some_condition {
        let _scope = loggy::Scope::new("scope name");
        // Log messages generated here will be prefixed by the scope name instead of the module name.
        // ...
    } else {
    }
}
```

### Logging levels

Log levels are given stronger semantics:

* A `loggy::panic!` is logged using the `Error` level (that is, it is formatted as a log message), but is always
  converted to a `std::panic!` (that is, terminates the current thread).

* A `loggy::error!` serves a different purpose. It also indicates a non-recoverable error, but allows the code to
  continue, possibly reporting additional errors, and automatically calls `panic!` at the end of the current named
  scope, indicating this scope has failed and reporting the total number of errors. Calling `loggy::error!` outside a
  named scope is not allowed; it is converted to a generic "error! must only be used inside a scope" `std::panic!`
  message. Errors, like panics, are always reported, regardless of the logging level.

* A `loggy::warn!` is only reported if the logging level is at least `Warn`, and is otherwise silently ignored. Warnings
  designate abnormal situations where the code has a reasonable way to recover and continue normal execution. This may
  be used outside any named scope.

* A `loggy::note!` allows the code to implement the "treat warnings as errors" functionality. It takes an additional
  boolean flag specifying whether this should be treated as a `error!` or a `warn!`. Since this may be treated as an
  `error!` it must only be used inside a scope. It is often useful to have program-wide flags determining whether
  a certain class of warnings should become errors, for example using a command-line flag. This can be done as follows:

```ignore
#[macro_use]
extern crate loggy;

mod some_condition {
    loggy::is_an_error!(false); // By default, not an error.
}

fn main() {
    // ...
    some_condition::set_is_an_error(based_on_the_command_line_flags);
    // ...
    loggy::with_scope("scope name", || { // Errors must be inside some scope.
        // ...
        if test_for_some_condition {
            note!(some_condition::is_an_error(), "some condition"); // Will be an error or a warning depending on the command line flag.
        }
        // ...
    });
    // ...
}
```

* A `loggy::info!` is only reported if the logging level is at least `Info`, and is otherwise silently ignored.
  Information messages should be few and far between, to indicate overall program progress.

* A `loggy::debug!` is meant specifically for debugging the program, and targets the code developers rather than the
  program's users. Debug messages are always emitted in debug builds; in release builds they are only emitted if the
  logging level is at least `Debug`. The format of debug messages includes an additional `<file>:<line>:` prefix to
  identify their exact source code location. Finally, debug messages are always emitted to the standard error, and are
  never captured in tests (see below), which makes it possible to debug tests that examine the expected log.

* A `loggy::todox!` is identical to `loggy::debug!`. It allows using the `cargo todox` extension to ensure no leftover
  debug messages are left in the code when debugging is over.

* A `loggy::trace!` is only reported if the logging level is at least `Trace`, and is otherwise silently ignored. Trace
  messages describe program progress in high detail so may generate a very large log.

You can also use `loggy::log!(level, ...)` to specify the level of a message. Note that if this level is `Error`, the
message can only be generated inside a named scope. There is no way to force a `panic!` this way (use `note!` instead).

### Testing

Testing logging faces the following inconvenient truths:

* The rust `log` facade mandates using a single global logger shared by all threads. You can't even replace it once you
  have set it up once.

* By default, `cargo test` uses multiple threads to run tests in parallel.

* A test capturing logged messages should capture everything generated from all sub-threads spawned by the test.

Therefore, the following following assertions take a global lock to ensure messages from different tests do not
interfere with each other. This has several implications:

* The test assertions have to setup a logger that captures the messages, so do not combine logging tests with any code
  that sets up the global logger.

* The test assertions will run serially, one at a time, regardless of the number of threads spawned by `cargo
  test`. This still allows non-logging tests (that do not use the following assertions) to run in parallel.

* Nesting the logging assertions will cause a deadlock. It doesn't make sense to do this in the 1st place, so just
  don't.

All that said, testing the actual log messages generated by some code is a convenient and surprisingly powerful way of
ensuring it behaves as expected. It also ensures that the log messages contain the expected data, something that is
otherwise difficult to verify. The following assertions are available to support this:

* `assert_logs(expected_log, || { ... })` executes some code and asserts that the actual log is identical to the
  (unindented) `expected_log`. Crucially, this can be nested, so you can examine the log in parts. The collected log for
  an outer `assert_logs` (or `assert_logs_panics`)  does not include the log captured by an internal `assert_logs`.

* `assert_panics(expected_panic, || { ... })` executes some code and asserts that it panics with the (unindented)
  `expected_panic`, ignoring the log.

* `assert_logs_panics(expected_log, expected_panic, || { ... })` executes some code and asserts that both the actual log
  is as expected, and that the code also panics with the expected message.

* `assert_writes(expected_text, |writer| { ... })` is provided for convenience, asserting that the code writes the
  (unindented) `expected_text` to the `writer: &mut dyn IoWrite`. This really should be in a more generic crate.

These are intentionally not attribute macros attached to the test (like the standard `#[should_panic]`. This allows the
expected texts to be dynamically formatted.

Setting the `LOGGY_MIRROR_TO_STDERR` environment variable to any non-empty value will cause all messages to be emitted
to the standard error stream, together with any debug messages, even in tests. This places the debug messages in the
context of the other messages, helping in debugging of tests.

Ideally, the standard error content is only reported for failing tests (this includes any debug messages). In practice,
the rust mechanism for capturing the standard error does not work properly when the test spawns new threads, so any
debug messages emitted from worker threads will be visible even for passing tests. This isn't a show stopper given such
messages and the `LOGGY_MIRROR_TO_STDERR` variable are only used when actively debugging an issue.

## License

`loggy` is licensed under the [MIT License](LICENSE.txt).

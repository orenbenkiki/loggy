# loggy [![Build Status](https://api.travis-ci.org/orenbenkiki/loggy.svg?branch=master)](https://travis-ci.org/orenbenkiki/loggy) [![codecov](https://codecov.io/gh/orenbenkiki/loggy/branch/master/graph/badge.svg)](https://codecov.io/gh/orenbenkiki/loggy)

An opinionated library for developing and testing rust applications that use
logging.

This is inspired by [simple-logging](https://github.com/Ereski/simple-logging)
implementation, with additional features meant to support development of
applications (as opposed to libraries).

## Installing and Building

To install, run `cargo install loggy`.

To build run either `cargo build` or `cargo make build`.

To test, run either `RUST_TEST_THREADS=1 cargo test` or `cargo make test`.
Single thread testing is required due to the rust `log` facade mandating
the use of a single global logger.

## Examples

The main program will need to set up the logger:

```rust
extern crate loggy;

loggy::init(Loggy {
    prefix: "...", // Typically, the name of the program.
    show_time: true, // Or false.
    log_level: LogLevel::Warn, // Or Info, or Error.
});
```

To provide user control over handling issues:

```rust
#[macro_use]
extern crate loggy;

let is_issue_an_error = decide_based_on_command_line_arguments();
if did_issue_occur() {
    note!(is_issue_an_error, "issue occured: {}", issue_data());
    provide_workaround();
} else {
    proceed_normally();
}
```

To deal with errors in processing stages:

```rust
extern crate loggy;

{
    let scope = loggy::ErrorsScope::new();
    run_processing_stage_with_potential_errors();
    if scope.had_errors() {
        return;
    }
}
continue_to_next_processing_stage();
```

To test code that emits log messages;

```rust
#[macro_use]
extern crate loggy;

test_loggy!(test_name, {
    run_some_code();
    assert_log("\
test: [<level>] <module>: <message>
...
");
});

```

## Motivation

This library was written to support the development of a non-trivial Rust
application that uses logging. The functionality provided here was factored
out, both to keep it isolated from the application itself, and in the hope
it might prove useful to others.

Technically, this library is an implementation for the Rust log facade,
with a few additional features thrown in. The implementation and features
were designed to support a specific development workflow.

## Features

As an implementation of the log facade, this library is pretty basic and
standard. Messages are emitted to the standard error stream. The message format
is `<prefix>[<thread>]: <time> [<level]>] <module>: <message>`, where the time
may be omitted.

Additional features reflect the library's opinionated nature:

### Logging Features

* It is assumed that either the program is single-threaded, or, if
  multi-threaded, then the main thread spawns off worker threads slaved to the
  main one, to perform transient tasks. Therefore, the thread index is only
  reported for log messages generated from the worker threads.

* In debug builds, debug messages are always emitted, regardless of the
  setting of the log level threshold (other messages do obey the threshold).
  This is under the assumption that debug builds are only used for, well,
  debugging. In contrast, in release builds, the threshold applies to debug
  messages as well. It is assumed that there would be none (or that they
  would be very rare), since release builds are meant for, well, release
  rather than debugging.

* The format of the debug messages replaces the `<module>:` name with
  `<file>:<line>:`. This is under the assumption that such messages would hardly
  ever be seen by users. Developers, on the other hand, would benefit from
  having the exact code location generating each message.

### Development Features

* An additional `todox!` macro is provided, which behaves exactly like
  `debug!`. If using the `cargo todox` extension, this prevents leftover
  debugging messages from being inadvertently left in the code.

* An additional `note!` macro is provided, which behaves either like `error!`
  or `warn!`, depending on the value of its first (additional) parameter. This
  boolean parameter is typically derived from a command line argument (ideally,
  this should be automated as well). This makes it easy to allow the users
  to determine whether different conditions warrant aborting the program.

* Every call to `error!` (including calls via `note!`), from any thread, is
  counted. The `errors` function returns the total number of errors, and the
  `had_errors` function just returns whether any such calls occurred. This
  allows the program to easily decide on its final exit status.

* In addition, using `ErrorsScope` allows counting the errors that occur in
  specific regions of the code (in the current thread), also providing `errors`
  count and a `had_errors` test. This allows the code to easily report multiple
  errors from some processing stage, deferring aborting the program until the
  whole processing stage is done.

### Testing Features

* A `test_loggy!` macro allows creating a test for code that emits log messages.
  All messages (except for debug messages) are captured to a buffer. The test
  should use `assert_log` to examine this buffer, or `clear_log` to explicitly
  discard it. Examining the log is an effective way to gain insights and verify
  the behavior of the tested code.

* Setting the `LOGGY_MIRROR_TO_STDERR` environment variable to any non-empty
  value will cause all messages to be emitted to the standard error stream,
  together with any debug messages. This places the debug messages in the
  context of the other messages, helping in debugging.

  Note that the standard error contents are only reported for failing tests.
  Well, actually, the rust mechanism for capturing the standard error seems to
  not work properly when the test spawns new threads, so any debug or mirrored
  messages emitted from worker threads will be visible even for passing tests.
  This isn't a show stopper given such messages and the `LOGGY_MIRROR_TO_STDERR`
  variable are only used when actively debugging an issue.

* The rust `log` facade mandates using a single global logger. This, combined
  with `loggy` handling multiple threads at once (counting errors, capturing
  messages), means that `test_loggy!` tests must run with `RUST_TEST_THREADS=1
  cargo test`. This can be automated by providing an `env` section in
  `Makefile.toml` and running `cargo make test` from the command line, and
  similarly by providing an `env` section in the `.travis.yml` file (both
  methods are used by this package).

## License

`loggy` is licensed under the [MIT License](LICENSE.txt).

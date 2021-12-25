#[macro_use]
extern crate loggy;

use loggy::{assert_logs, assert_logs_panics, assert_panics, assert_writes, Scope};
use std::thread;

#[test]
fn assert_panics_are_captured() {
    assert_panics("test: [ERROR] test_log: foo\n", || panic!("foo"));
}

#[test]
fn assert_writes_are_captured() {
    assert_writes("foo", |writer| {
        writer.write_all("foo".as_bytes()).unwrap();
    });
}

#[test]
fn scoped_errors_should_be_captured() {
    assert_logs_panics(
        "test: [ERROR] scope: error\n",
        "test: [ERROR] scope: failed with 1 error(s)",
        || {
            Scope::with("scope", || {
                error!("error");
            });
        },
    );
}

#[test]
fn emit_structured_log_messages() {
    assert_logs(
        r#"
        test: [INFO] test_log: simple
        test: [WARN] test_log: format 0
        test: [TRACE] test_log: both 0
        test: [trace] test_log:   foo: 1
        test: [trace] test_log:   bar:
        test: [trace] test_log:     baz: 2
    "#,
        || {
            info!("simple");
            warn!("format {}", 0);
            trace!("both {}", 0; foo => 1, bar { baz => 2 });
        },
    );
}

#[test]
fn named_scope_should_replace_module() {
    assert_logs(
        r#"
        test: [WARN] scope: warning
    "#,
        || {
            Scope::with("scope", || {
                warn!("warning");
            })
        },
    );
}

#[test]
fn multi_line_should_be_captured() {
    assert_logs(
        r#"
        test: [INFO] test_log: info
        test: [info] test_log: continuation
        test: [info] test_log: lines
    "#,
        || {
            info!("info\ncontinuation\nlines");
        },
    );
}

#[test]
fn warning_should_be_captured() {
    assert_logs(
        r#"
        test: [WARN] test_log: warning
    "#,
        || {
            warn!("warning");
        },
    );
}

#[test]
fn info_should_be_captured() {
    assert_logs(
        r#"
        test: [INFO] test_log: information
    "#,
        || {
            info!("information");
        },
    );
}

#[test]
fn debug_should_not_be_captured() {
    debug!("debug");
    todox!("debug");
}

#[test]
fn notice_should_be_captured() {
    assert_logs_panics(
        r#"
        test: [ERROR] scope: error
        test: [WARN] scope: warning
        "#,
        "test: [ERROR] scope: failed with 1 error(s)",
        || {
            let _scope = Scope::new("scope");
            note!(true, "error");
            note!(false, "warning");
        },
    );
}

mod foo {
    is_an_error!(false);
}

#[test]
fn notice_should_be_controlled() {
    assert!(!foo::is_an_error());
    assert!(!foo::set_is_an_error(true));
    assert!(foo::is_an_error());
    assert!(foo::set_is_an_error(false));
}

#[test]
fn worker_threads_should_be_reported() {
    assert_logs(
        r#"
        test: [INFO] test_log: before
        test: [INFO] test_log: child
        test: [INFO] test_log: after
    "#,
        || {
            info!("before");
            let child = thread::spawn(|| {
                info!("child");
            });
            child.join().unwrap();
            info!("after");
        },
    );
}

#[loggy::scope]
fn scoped() {
    info!("message");
}

#[test]
fn scoped_functions_should_work() {
    assert_logs(
        r#"
        test: [INFO] scoped: message
        "#,
        scoped,
    );
}

#[loggy::scope("scope name")]
fn name_scoped() {
    info!("message");
}

#[test]
fn named_scoped_functions_should_work() {
    assert_logs(
        r#"
        test: [INFO] scope name: message
        "#,
        name_scoped,
    );
}

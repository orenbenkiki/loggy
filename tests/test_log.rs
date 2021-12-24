#[macro_use]
extern crate loggy;

use loggy::{
    assert_logged, assert_logged_panics, assert_no_errors, assert_panics, assert_writes,
    count_errors, in_named_scope,
};
use std::thread;

#[loggy]
#[test]
fn test_assert_panics() {
    assert_panics("test: [ERROR] test_log: foo\n", || panic!("foo"));
}

#[loggy]
#[test]
fn test_assert_writes() {
    assert_writes("foo", |writer| {
        writer.write("foo".as_bytes()).unwrap();
    });
}

#[loggy]
#[test]
fn test_assert_no_errors_should_detect_an_error() {
    assert_no_errors("foo", || {});
    assert_logged_panics(
        "test: [ERROR] test_log: error\n",
        "test: [ERROR] loggy: 1 foo error(s)\n",
        || {
            assert_no_errors("foo", || {
                error!("error");
            });
        },
    );
}

#[loggy]
#[test]
fn test_error_should_be_captured() {
    error!("error");
    assert_logged("test: [ERROR] test_log: error\n");
}

#[loggy]
#[test]
fn emit_structured_log_messages() {
    info!("simple");
    warn!("format {}", 0);
    error!("fields"; foo => 1, bar { baz => 2 });
    trace!("both {}", 0; foo => 1, bar { baz => 2 });
    assert_logged(
        r#"
        test: [INFO] test_log: simple
        test: [WARN] test_log: format 0
        test: [ERROR] test_log: fields
        test: [error] test_log:   foo: 1
        test: [error] test_log:   bar:
        test: [error] test_log:     baz: 2
        test: [TRACE] test_log: both 0
        test: [trace] test_log:   foo: 1
        test: [trace] test_log:   bar:
        test: [trace] test_log:     baz: 2
    "#,
    );
}

#[loggy]
#[test]
fn named_scope_should_replace_module() {
    in_named_scope("scope", || error!("error"));
    assert_logged(
        r#"
        test: [ERROR] scope: error
    "#,
    );
}

#[loggy]
#[test]
fn multi_line_should_be_captured() {
    error!("error\ncontinuation\nlines");
    assert_logged(
        r#"
        test: [ERROR] test_log: error
        test: [error] test_log: continuation
        test: [error] test_log: lines
    "#,
    );
}

#[loggy]
#[test]
fn warning_should_be_captured() {
    warn!("warning");
    assert_logged(
        r#"
        test: [WARN] test_log: warning
    "#,
    );
}

#[loggy]
#[test]
fn info_should_be_captured() {
    info!("information");
    assert_logged(
        r#"
        test: [INFO] test_log: information
    "#,
    );
}

#[loggy]
#[test]
fn debug_should_not_be_captured() {
    debug!("debug");
    todox!("debug");
}

#[loggy]
#[test]
fn notice_should_be_captured() {
    note!(true, "error");
    note!(false, "warning");
    assert_logged(
        r#"
        test: [ERROR] test_log: error
        test: [WARN] test_log: warning
    "#,
    );
}

mod foo {
    is_an_error!(false);
}

#[loggy]
#[test]
fn notice_should_be_controlled() {
    assert!(!foo::is_an_error());
    assert!(!foo::set_is_an_error(true));
    assert!(foo::is_an_error());
    assert!(foo::set_is_an_error(false));
}

#[loggy]
#[test]
fn worker_threads_should_be_reported() {
    info!("before");
    let child = thread::spawn(|| {
        info!("child");
    });
    child.join().unwrap();
    info!("after");
    assert_logged(
        r#"
        test: [INFO] test_log: before
        test: [INFO] test_log: child
        test: [INFO] test_log: after
    "#,
    );
}

#[loggy]
#[test]
fn errors_should_be_counted() {
    assert_eq!(loggy::errors(), 0);
    error!("unscoped");
    assert_eq!(loggy::errors(), 1);

    let outer_errors = count_errors(|| {
        assert_eq!(loggy::errors(), 1);

        let inner_errors = count_errors(|| {
            assert_eq!(loggy::errors(), 1);
            error!("inner");
            assert_eq!(loggy::errors(), 2);
        });

        assert_eq!(inner_errors, 1);
        assert_eq!(loggy::errors(), 2);

        error!("outer");
        assert_eq!(loggy::errors(), 3);
    });

    assert_eq!(outer_errors, 2);
    assert_eq!(loggy::errors(), 3);

    let child = thread::spawn(|| {
        let errors_count = count_errors(|| {
            assert_eq!(loggy::errors(), 3);
            error!("child");
            assert_eq!(loggy::errors(), 4);
        });
        assert_eq!(errors_count, 1);
    });

    child.join().unwrap();
    assert_eq!(loggy::errors(), 4);

    assert_logged(
        r###"
        test: [ERROR] test_log: unscoped
        test: [ERROR] test_log: inner
        test: [ERROR] test_log: outer
        test: [ERROR] test_log: child
        "###,
    );
}

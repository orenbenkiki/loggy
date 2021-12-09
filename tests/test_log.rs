#[macro_use]
extern crate loggy;

use loggy::{assert_log, clear_log, count_errors, in_named_scope};
use std::thread;

test_loggy!(error_should_be_captured, {
    error!("error");
    assert_log(
        r#"
        test: [ERROR] test_log: error
    "#,
    );
});

test_loggy!(emit_structured_log_messages, {
    info!("simple");
    warn!("format {}", 0);
    error!("fields"; foo => 1, bar { baz => 2 });
    trace!("both {}", 0; foo => 1, bar { baz => 2 });
    assert_log(
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
});

test_loggy!(named_scope_should_replace_module, {
    in_named_scope("scope", || error!("error"));
    assert_log(
        r#"
        test: [ERROR] scope: error
    "#,
    );
});

test_loggy!(multi_line_should_be_captured, {
    error!("error\ncontinuation\nlines");
    assert_log(
        r#"
        test: [ERROR] test_log: error
        test: [error] test_log: continuation
        test: [error] test_log: lines
    "#,
    );
});

test_loggy!(warning_should_be_captured, {
    warn!("warning");
    assert_log(
        r#"
        test: [WARN] test_log: warning
    "#,
    );
});

test_loggy!(info_should_be_captured, {
    info!("information");
    assert_log(
        r#"
        test: [INFO] test_log: information
    "#,
    );
});

test_loggy!(debug_should_not_be_captured, {
    debug!("debug");
    todox!("debug");
});

test_loggy!(notice_should_be_captured, {
    note!(true, "error");
    note!(false, "warning");
    assert_log(
        r#"
        test: [ERROR] test_log: error
        test: [WARN] test_log: warning
    "#,
    );
});

mod foo {
    is_an_error!(false);
}

test_loggy!(notice_should_be_controlled, {
    assert!(!foo::is_an_error());
    assert!(!foo::set_is_an_error(true));
    assert!(foo::is_an_error());
    assert!(foo::set_is_an_error(false));
});

test_loggy!(worker_threads_should_be_reported, {
    info!("before");
    let child = thread::spawn(|| {
        info!("child");
    });
    child.join().unwrap();
    info!("after");
    assert_log(
        r#"
        test: [INFO] test_log: before
        test: [INFO] test_log: child
        test: [INFO] test_log: after
    "#,
    );
});

test_loggy!(errors_should_be_counted, {
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
        assert_eq!(loggy::errors(), 3);
        error!("child");
        assert_eq!(loggy::errors(), 4);
    });

    child.join().unwrap();
    assert_eq!(loggy::errors(), 4);

    clear_log();
});

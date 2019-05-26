#[macro_use] // NOT TESTED
extern crate loggy;

#[macro_use]
extern crate log;

use loggy::{assert_log, clear_log, ErrorsScope};
use std::thread;

test_loggy!(error_should_be_captured, {
    error!("error");
    assert_log(
        r#"
        test: [ERROR] test_log: error
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
        test[1]: [INFO] test_log: child
        test: [INFO] test_log: after
    "#,
    );
});

test_loggy!(errors_should_be_counted, {
    let outer_scope = ErrorsScope::new();

    assert_eq!(loggy::errors(), 0);
    assert!(!loggy::had_errors());

    assert_eq!(outer_scope.errors(), 0);
    assert!(!outer_scope.had_errors());

    error!("outer");

    assert_eq!(loggy::errors(), 1);
    assert!(loggy::had_errors());

    assert_eq!(outer_scope.errors(), 1);
    assert!(outer_scope.had_errors());

    let child = thread::spawn(|| {
        let inner_scope = ErrorsScope::new();

        assert_eq!(inner_scope.errors(), 0);
        assert!(!inner_scope.had_errors());

        error!("inner");

        assert_eq!(inner_scope.errors(), 1);
        assert!(inner_scope.had_errors());
    });
    child.join().unwrap();

    assert_eq!(loggy::errors(), 2);
    assert!(loggy::had_errors());

    assert_eq!(outer_scope.errors(), 1);
    assert!(outer_scope.had_errors());

    clear_log();
});

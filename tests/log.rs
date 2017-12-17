// FILE NOT TESTED

#[macro_use]
extern crate loggy;

#[macro_use]
extern crate log;

use loggy::{ErrorsScope, assert_log, clear_log};
use std::thread;

test_loggy!(error_should_be_captured, {
    error!("error");
    assert_log("test: [ERROR] log: error\n");
});

test_loggy!(warning_should_be_captured, {
    warn!("warning");
    assert_log("test: [WARN] log: warning\n");
});

test_loggy!(info_should_be_captured, {
    info!("information");
    assert_log("test: [INFO] log: information\n");
});

test_loggy!(debug_should_not_be_captured, {
    debug!("debug");
    todox!("debug");
});

test_loggy!(notice_should_be_captured, {
    note!(true, "error");
    note!(false, "warning");
    assert_log(
        "\
test: [ERROR] log: error
test: [WARN] log: warning
",
    );
});

test_loggy!(worker_threads_should_be_reported, {
    info!("before");
    let child1 = thread::spawn(|| {
        info!("child");
    });
    let child2 = thread::spawn(|| {
        info!("child");
    });
    child1.join().unwrap();
    child2.join().unwrap();
    info!("after");
    assert_log(
        "\
test: [INFO] log: before
test[1]: [INFO] log: child
test[2]: [INFO] log: child
test: [INFO] log: after
",
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

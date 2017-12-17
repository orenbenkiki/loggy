// FILE NOT TESTED

#[macro_use]
extern crate log;

#[macro_use]
extern crate loggy;

use log::LogLevel;
use loggy::Loggy;

fn main() {
    loggy::init(Loggy {
        prefix: "example",
        show_time: true,
        log_level: LogLevel::Warn,
    }).unwrap();

    note!(false, "This is a example message.");
}

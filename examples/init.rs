// FILE NOT TESTED

#[macro_use]
extern crate loggy;

use log::LevelFilter;
use loggy::Loggy;

fn main() {
    log::set_logger(&Loggy {
        prefix: "example",
        show_time: true,
        show_thread: true,
    })
    .unwrap();
    log::set_max_level(LevelFilter::Warn);

    note!(false, "This is a example message.");
}

use log::{info, debug, error};
use crate::core::logger;

#[test]
fn test_logger() {
    logger::setup(log::LevelFilter::Info, None);
    info!("info: testing....");
    error!("debug: testing...");
    assert!(true);
    logger::teardown();
}

#[test]
fn test_logger_disable() {
    logger::setup(log::LevelFilter::Info, None);
    logger::revert_console_output();
    info!("info: testing....");
    debug!("debug: testing...");
    assert!(true);
    logger::teardown();
}

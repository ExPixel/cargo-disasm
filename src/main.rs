mod app;

use std::error::Error;

fn main() {
    log::set_logger(app::logging::AppLogger::init()).expect("failed to set logger");
    let has_err = if let Err(err) = app::run() {
        log::error!("{}", err);
        let mut last_source: &dyn Error = &*err;
        while let Some(source) = last_source.source() {
            log::error!("  caused by {}", source);
            last_source = source;
        }
        true
    } else {
        false
    };
    log::logger().flush();

    if has_err {
        std::process::exit(-1);
    }
}

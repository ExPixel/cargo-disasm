mod app;
mod disasm;
mod util;

fn main() {
    log::set_logger(app::logging::AppLogger::init()).expect("failed to set logger");
    let has_err = if let Err(err) = app::run() {
        log::error!("{:?}", err);
        true
    } else {
        false
    };
    log::logger().flush();

    if has_err {
        std::process::exit(-1);
    }
}

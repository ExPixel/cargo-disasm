use log::{Level, LevelFilter};
use std::io::Write as _;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor as _};

static mut APP_LOGGER_LEVEL: LevelFilter = LevelFilter::Error;
static mut APP_LOGGER_COLOR: ColorChoice = ColorChoice::Auto;

pub struct AppLogger;

impl AppLogger {
    pub fn init() -> &'static AppLogger {
        log::set_max_level(unsafe { Self::instance().level() });
        Self::instance()
    }

    pub fn instance() -> &'static AppLogger {
        static INSTANCE: AppLogger = AppLogger;
        &INSTANCE
    }

    pub unsafe fn level(&self) -> LevelFilter {
        APP_LOGGER_LEVEL
    }

    pub unsafe fn color_choice(&self) -> ColorChoice {
        APP_LOGGER_COLOR
    }

    pub unsafe fn set_level(&self, level: LevelFilter) {
        APP_LOGGER_LEVEL = level;
        log::set_max_level(APP_LOGGER_LEVEL);
    }

    pub unsafe fn set_color_choice(&self, color: ColorChoice) {
        APP_LOGGER_COLOR = color;
    }

    fn write_log(&self, record: &log::Record) -> std::io::Result<()> {
        let (level, color, use_stderr) = match record.level() {
            Level::Error => ("error", Color::Red, true),
            Level::Warn => ("warning", Color::Yellow, true),
            Level::Info => ("info", Color::Blue, false),
            Level::Debug => ("debug", Color::Green, true),
            Level::Trace => ("trace", Color::Magenta, true),
        };

        let mut output = if use_stderr {
            StandardStream::stderr(unsafe { self.color_choice() })
        } else {
            StandardStream::stdout(unsafe { self.color_choice() })
        };

        let mut level_color = ColorSpec::new();
        level_color.set_fg(Some(color)).set_bold(true);
        let mut reset_color = ColorSpec::new();
        reset_color.set_reset(true);

        output.set_color(&level_color)?;
        write!(output, "{:>width$}(", level, width = 7)?;
        output.set_color(&reset_color)?;
        write!(output, "{}", record.target())?;
        output.set_color(&level_color)?;
        write!(output, "): ")?;
        output.set_color(&reset_color)?;
        writeln!(output, "{}", record.args())?;

        Ok(())
    }
}

impl log::Log for AppLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= unsafe { self.level() }
    }

    fn log(&self, record: &log::Record) {
        self.write_log(record).expect("failed to write log")
    }

    fn flush(&self) {
        std::io::stdout().flush().expect("failed to flush stdout");
        std::io::stderr().flush().expect("failed to flush stderr");
    }
}

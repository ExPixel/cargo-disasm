pub struct DurationDisplay(pub std::time::Duration);

impl std::fmt::Display for DurationDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::time::Duration;

        if self.0 >= Duration::from_secs(1) {
            write!(f, "{:.3} seconds", self.0.as_secs_f64())
        } else if self.0 >= Duration::from_millis(1) {
            const NANOS_PER_MS: f64 = 1_000_000.0f64;
            write!(f, "{:.3} ms", self.0.as_nanos() as f64 / NANOS_PER_MS)
        } else if self.0 >= Duration::from_micros(1) {
            const NANOS_PER_US: f64 = 1_000.0f64;
            write!(f, "{:.3} us", self.0.as_nanos() as f64 / NANOS_PER_US)
        } else {
            write!(f, "{:.3} ns", self.0.as_nanos())
        }
    }
}

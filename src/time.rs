pub struct Clock {
    pub start_time: std::time::Instant,
    pub previous_tick: std::time::Instant,
    pub last_delta_time: std::time::Duration,
}
impl Clock {
    pub fn start() -> Self {
        let now = std::time::Instant::now();

        Self {
            start_time: now,
            previous_tick: now,
            last_delta_time: std::time::Duration::from_secs(1),
        }
    }

    /// Sets previous_time to the current time and returns the duration since the previously set
    /// previous_time.
    pub fn tick(&mut self) -> std::time::Duration {
        self.last_delta_time = self.previous_tick.elapsed();
        self.previous_tick = std::time::Instant::now();
        self.last_delta_time
    }
}

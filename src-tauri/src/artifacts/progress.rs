use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(serde::Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum LaunchProgress {
    Status {
        message: &'static str,
    },
    Checking {
        current: u64,
        total: u64,
    },
    Downloading {
        message: &'static str,
        current: u64,
        total: Option<u64>,
    },
}

// Keep reports ordered across workers and avoid flooding the WebView with tiny reads.
pub struct ProgressCounter<F> {
    state: Mutex<CounterState>,
    report: F,
}

struct CounterState {
    current: u64,
    published: u64,
    last: Instant,
}

impl<F: Fn(u64)> ProgressCounter<F> {
    pub fn new(report: F) -> Self {
        report(0);
        Self {
            state: Mutex::new(CounterState {
                current: 0,
                published: 0,
                last: Instant::now(),
            }),
            report,
        }
    }

    pub fn advance(&self, amount: u64) {
        let mut state = self.state.lock().unwrap();
        state.current += amount;
        if state.last.elapsed() >= Duration::from_millis(120) {
            (self.report)(state.current);
            state.published = state.current;
            state.last = Instant::now();
        }
    }

    pub fn finish(&self) {
        let mut state = self.state.lock().unwrap();
        if state.published != state.current {
            (self.report)(state.current);
            state.published = state.current;
        }
    }
}

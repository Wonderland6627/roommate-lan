//! GUI-side lease keepalive so WebView timer throttling cannot drop the tunnel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::service::ServiceClient;

const HEARTBEAT_INTERVAL_SECS: u64 = 10;

struct LeaseHeartbeat {
    stop: Arc<AtomicBool>,
}

impl LeaseHeartbeat {
    fn start_new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        thread::spawn(move || {
            let client = ServiceClient::new();
            while !stop_flag.load(Ordering::SeqCst) {
                let _ = client.heartbeat();
                // Sleep in short slices so stop is responsive.
                for _ in 0..HEARTBEAT_INTERVAL_SECS * 2 {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        });
        Self { stop }
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn slot() -> &'static Mutex<Option<LeaseHeartbeat>> {
    static SLOT: OnceLock<Mutex<Option<LeaseHeartbeat>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Start (or restart) the GUI-side lease heartbeat after a successful connect.
pub fn start() {
    let mut guard = slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(prev) = guard.take() {
        prev.stop();
    }
    *guard = Some(LeaseHeartbeat::start_new());
}

/// Stop the GUI-side lease heartbeat (disconnect / reset / process exit).
pub fn stop() {
    let mut guard = slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(prev) = guard.take() {
        prev.stop();
    }
}

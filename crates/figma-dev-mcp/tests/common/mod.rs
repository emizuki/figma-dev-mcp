//! Shared helpers for production-binary stdio protocol tests.

use std::net::TcpListener;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use figma_dev_mcp_broker::config::{FRONTEND_ADDRESS, PLUGIN_ADDRESS};

pub static STDIO_ERA_LOCK: Mutex<()> = Mutex::new(());

pub fn plugin_port() -> u16 {
    PLUGIN_ADDRESS.port()
}

pub fn frontend_port() -> u16 {
    FRONTEND_ADDRESS.port()
}

pub fn assert_production_ports_free() {
    TcpListener::bind(PLUGIN_ADDRESS).unwrap_or_else(|error| {
        panic!(
            "plugin listener {} must be free after stdio cleanup: {error}",
            PLUGIN_ADDRESS
        )
    });
    TcpListener::bind(FRONTEND_ADDRESS).unwrap_or_else(|error| {
        panic!(
            "frontend listener {} must be free after stdio cleanup: {error}",
            FRONTEND_ADDRESS
        )
    });
}

pub fn wait_until(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    ready()
}

use std::sync::{Arc, Mutex};

use tokio::sync::watch;
use uuid::Uuid;

use crate::BrokerState;

#[derive(Debug)]
pub(crate) struct Activity {
    state: Mutex<ActivityState>,
    changes: watch::Sender<u64>,
}

#[derive(Debug)]
struct ActivityState {
    counts: ActivityCounts,
    closing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActivityCounts {
    pub(crate) frontends: usize,
    pub(crate) plugins: usize,
}

impl Default for Activity {
    fn default() -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            state: Mutex::new(ActivityState {
                counts: ActivityCounts {
                    frontends: 0,
                    plugins: 0,
                },
                closing: false,
            }),
            changes,
        }
    }
}

impl Activity {
    pub(crate) fn frontend_lease(self: &Arc<Self>) -> Option<FrontendLease> {
        let mut state = self.state.lock().expect("activity state mutex poisoned");
        if state.closing {
            return None;
        }
        state.counts.frontends += 1;
        drop(state);
        self.changed();
        Some(FrontendLease {
            activity: Arc::clone(self),
        })
    }

    pub(crate) fn plugin_lease(self: &Arc<Self>) -> Option<PluginLease> {
        let mut state = self.state.lock().expect("activity state mutex poisoned");
        if state.closing {
            return None;
        }
        state.counts.plugins += 1;
        drop(state);
        self.changed();
        Some(PluginLease {
            activity: Arc::clone(self),
        })
    }

    pub(crate) fn counts(&self) -> ActivityCounts {
        self.state
            .lock()
            .expect("activity state mutex poisoned")
            .counts
    }

    pub(crate) fn begin_closing_if_idle(&self) -> bool {
        let mut state = self.state.lock().expect("activity state mutex poisoned");
        if state.counts.frontends != 0 || state.counts.plugins != 0 {
            return false;
        }
        state.closing = true;
        true
    }

    pub(crate) fn changed(&self) {
        self.changes
            .send_modify(|generation| *generation = generation.wrapping_add(1));
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }
}

#[derive(Debug)]
pub struct FrontendLease {
    activity: Arc<Activity>,
}

impl Drop for FrontendLease {
    fn drop(&mut self) {
        let mut counts = self
            .activity
            .state
            .lock()
            .expect("activity state mutex poisoned");
        debug_assert!(
            counts.counts.frontends > 0,
            "frontend lease count underflow"
        );
        counts.counts.frontends = counts.counts.frontends.saturating_sub(1);
        drop(counts);
        self.activity.changed();
    }
}

#[derive(Debug)]
pub(crate) struct PluginLease {
    activity: Arc<Activity>,
}

impl Drop for PluginLease {
    fn drop(&mut self) {
        let mut counts = self
            .activity
            .state
            .lock()
            .expect("activity state mutex poisoned");
        debug_assert!(counts.counts.plugins > 0, "plugin lease count underflow");
        counts.counts.plugins = counts.counts.plugins.saturating_sub(1);
        drop(counts);
        self.activity.changed();
    }
}

pub(crate) async fn cleanup_socket(state: &BrokerState, socket_id: Uuid) {
    state.registry.lock().await.remove_socket(socket_id);
    state.pending.lock().await.remove_socket(socket_id);
    state.queues.lock().await.remove(&socket_id);
    state.activity.changed();
}

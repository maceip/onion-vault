//! Bootstrap progress reporting.
//!
//! Arti exposes the bootstrap state through [`TorClient::bootstrap_status`].
//! The monitor turns it into a percentage that the native Android UI shows on
//! the readiness bar before any navigation is allowed.

use std::{sync::Arc, time::Duration};

use arti_client::TorClient;
use tokio::sync::broadcast;
use tor_rtcompat::PreferredRuntime;

/// Progress of the Tor bootstrap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapProgress {
    /// Completion in percent, 0..=100.
    pub percent: u8,
    /// Human readable stage description.
    pub stage: String,
    /// `true` once the client can carry traffic.
    pub ready: bool,
}

/// Watch the bootstrap state of a client and publish the progress.
pub struct BootstrapMonitor {
    client: Arc<TorClient<PreferredRuntime>>,
    events: broadcast::Sender<BootstrapProgress>,
}

impl BootstrapMonitor {
    /// Create a monitor for `client`.
    pub fn new(
        client: Arc<TorClient<PreferredRuntime>>,
        events: broadcast::Sender<BootstrapProgress>,
    ) -> Self {
        Self { client, events }
    }

    /// The current bootstrap state.
    pub fn current(&self) -> BootstrapProgress {
        let status = self.client.bootstrap_status();
        let fraction = status.as_frac().clamp(0.0, 1.0);
        let stage = match status.blocked() {
            Some(blockage) => format!("blocked: {}", blockage.message()),
            None => {
                if status.ready_for_traffic() {
                    "ready".to_string()
                } else {
                    "connecting to the Tor network".to_string()
                }
            }
        };
        BootstrapProgress {
            percent: (fraction * 100.0).round().clamp(0.0, 100.0) as u8,
            stage,
            ready: status.ready_for_traffic(),
        }
    }

    /// Poll the bootstrap state until the client is ready or the task is
    /// dropped.
    pub async fn run(self, interval: Duration) {
        let mut last = -1i32;
        loop {
            let progress = self.current();
            let percent = progress.percent as i32;
            let ready = progress.ready;
            if percent != last {
                last = percent;
                let _ = self.events.send(progress);
            }
            if ready {
                return;
            }
            tokio::time::sleep(interval).await;
        }
    }
}

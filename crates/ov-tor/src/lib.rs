//! Embedded Tor transport for PlugTalk.
//!
//! The crate owns a complete Arti client inside the application process:
//!
//!   * the client bootstraps against the Tor network using the application
//!     sandbox for its state and cache,
//!   * a SOCKS5 listener on `127.0.0.1:<ephemeral>` exposes the client to the
//!     CEF network stack (Chromium is pointed at it with `--proxy-server`),
//!   * every `.onion` apex domain is placed on its own circuit.
//!
//! Nothing is shelled out to an external `tor` process: the transport lives
//! in the same address space as the browser, which keeps the traffic inside
//! the app sandbox and lets the leak-prevention rules (no DNS outside the
//! tunnel) be enforced by Chromium's host resolver rules.

pub mod bootstrap;
pub mod config;
pub mod isolation;
pub mod socks5;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use arti_client::TorClient;
use tokio::sync::broadcast;
use tor_rtcompat::PreferredRuntime;

pub use bootstrap::{BootstrapMonitor, BootstrapProgress};
pub use config::TorConfig;
pub use isolation::IsolationRegistry;
pub use socks5::{Socks5Decision, Socks5Error, Socks5Request, Socks5Server};

/// Number of buffered bootstrap events.
const EVENT_BUFFER: usize = 32;

/// Install the cryptography provider of rustls.
///
/// rustls does not carry a cryptography provider itself: the provider has to
/// be installed once per process, before the first TLS connection is made.
/// The Arti client panics without it, so every entry point of this crate
/// calls this function first.  It is safe to call it more than once.
pub fn install_crypto_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            if let Err(err) = rustls::crypto::ring::default_provider().install_default() {
                log::error!("cannot install the rustls cryptography provider: {err:?}");
            } else {
                log::info!("the rustls cryptography provider is installed");
            }
        }
    });
}

/// The embedded Tor service.
pub struct TorService {
    client: Arc<TorClient<PreferredRuntime>>,
    isolation: Arc<IsolationRegistry>,
    socks: Arc<Socks5Server>,
    events: broadcast::Sender<BootstrapProgress>,
    config: TorConfig,
}

impl TorService {
    /// Bootstrap the client and start the SOCKS5 listener.
    ///
    /// The returned service owns the listener; the chosen ephemeral port is
    /// available through [`TorService::socks_endpoint`].
    pub async fn start(config: TorConfig) -> anyhow::Result<Arc<Self>> {
        install_crypto_provider();
        config.validate()?;

        let tor_config = config.build_tor_config()?;

        // `PreferredRuntime` is Arti's tokio backed runtime; it is selected
        // explicitly so that the runtime type is part of this crate's API
        // surface instead of an implicit default.
        let runtime =
            tor_rtcompat::PreferredRuntime::current().context("cannot obtain the Tor runtime")?;
        let client = TorClient::with_runtime(runtime)
            .config(tor_config)
            .bootstrap_behavior(arti_client::BootstrapBehavior::Manual)
            .create_unbootstrapped_async()
            .await
            .context("cannot create the Arti Tor client")?;

        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let monitor = BootstrapMonitor::new(client.clone(), events.clone());

        // Bootstrapping runs in the background: the UI shows the readiness bar
        // while the user is already able to type an address.
        {
            let client = client.clone();
            tokio::spawn(async move {
                if let Err(err) = client.bootstrap().await {
                    log::error!("Tor bootstrap failed: {err}");
                    return;
                }
                log::info!("Tor client bootstrapped");
            });
        }

        tokio::spawn(async move {
            monitor.run(Duration::from_millis(500)).await;
        });

        let isolation = Arc::new(IsolationRegistry::new(
            client.clone(),
            config.isolate_per_apex,
        ));

        let socks = Socks5Server::start(
            config.socks_bind,
            isolation.clone(),
            config.allow_clearnet,
            config.connect_timeout,
        )
        .await?;

        log::info!("SOCKS5 listener bound to {}", socks.local_addr());

        Ok(Arc::new(Self {
            client,
            isolation,
            socks,
            events,
            config,
        }))
    }

    /// The address of the SOCKS5 listener, including the ephemeral port.
    pub fn socks_endpoint(&self) -> SocketAddr {
        self.socks.local_addr()
    }

    /// The `socks5://` proxy URL that Chromium has to be pointed at.
    pub fn proxy_url(&self) -> String {
        format!("socks5://{}", self.socks_endpoint())
    }

    /// Subscribe to the bootstrap progress events.
    pub fn subscribe(&self) -> broadcast::Receiver<BootstrapProgress> {
        self.events.subscribe()
    }

    /// The current bootstrap progress.
    pub fn bootstrap_progress(&self) -> BootstrapProgress {
        BootstrapMonitor::new(self.client.clone(), self.events.clone()).current()
    }

    /// `true` when the client can carry traffic.
    pub fn is_ready(&self) -> bool {
        self.client.bootstrap_status().ready_for_traffic()
    }

    /// The main (non isolated) Arti client.
    pub fn client(&self) -> Arc<TorClient<PreferredRuntime>> {
        self.client.clone()
    }

    /// The circuit isolation registry.
    pub fn isolation(&self) -> Arc<IsolationRegistry> {
        self.isolation.clone()
    }

    /// The configuration this service was started with.
    pub fn config(&self) -> &TorConfig {
        &self.config
    }

    /// Stop the listener.
    pub fn shutdown(&self) {
        self.socks.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_listener_binds_to_loopback_only() {
        install_crypto_provider();
        let config = TorConfig {
            state_dir: std::env::temp_dir().join("ov-tor-test-state"),
            cache_dir: std::env::temp_dir().join("ov-tor-test-cache"),
            ..TorConfig::default()
        };
        config.validate().unwrap();
        assert!(config.socks_bind.ip().is_loopback());
    }

    #[test]
    fn a_public_bind_address_is_rejected() {
        let config = TorConfig {
            socks_bind: "0.0.0.0:9050".parse().unwrap(),
            ..TorConfig::default()
        };
        assert!(config.validate().is_err());
    }
}

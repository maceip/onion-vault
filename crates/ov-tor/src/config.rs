//! Configuration of the embedded Tor client.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use anyhow::{bail, Context, Result};

/// Everything the embedded Tor client needs to know.
#[derive(Clone, Debug)]
pub struct TorConfig {
    /// Directory that holds the persistent Arti state (consensus cache, keys).
    pub state_dir: PathBuf,
    /// Directory that holds the Arti cache.
    pub cache_dir: PathBuf,
    /// Address the SOCKS5 listener binds to.  The port `0` lets the kernel
    /// pick a free ephemeral port; the chosen port is read back from
    /// `Socks5Server::local_addr`.
    pub socks_bind: SocketAddr,
    /// When `false` the client refuses every clearnet destination: the browser
    /// can then only reach `.onion` services (hidden service only mode).
    pub allow_clearnet: bool,
    /// When `true` every `.onion` apex domain gets its own circuit.
    pub isolate_per_apex: bool,
    /// Timeout for a single SOCKS5 CONNECT.
    pub connect_timeout: Duration,
}

impl Default for TorConfig {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::from("arti_state"),
            cache_dir: PathBuf::from("arti_cache"),
            // Loopback only: the listener must never be reachable from
            // outside the application sandbox.
            socks_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            allow_clearnet: false,
            isolate_per_apex: true,
            connect_timeout: Duration::from_secs(60),
        }
    }
}

impl TorConfig {
    /// Reject configurations that would leak traffic outside the tunnel.
    pub fn validate(&self) -> Result<()> {
        if !self.socks_bind.ip().is_loopback() {
            bail!(
                "the SOCKS5 listener must be bound to a loopback address, got {}",
                self.socks_bind.ip()
            );
        }
        if self.state_dir == self.cache_dir {
            bail!("state_dir and cache_dir must differ");
        }
        Ok(())
    }

    /// Create the state/cache directories and build the Arti configuration.
    pub fn build_tor_config(&self) -> Result<arti_client::TorClientConfig> {
        std::fs::create_dir_all(&self.state_dir)
            .with_context(|| format!("cannot create {}", self.state_dir.display()))?;
        std::fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("cannot create {}", self.cache_dir.display()))?;

        // The hidden service directory is what makes `.onion` addresses
        // resolvable; Arti fetches it as part of the regular consensus.
        let config = arti_client::config::TorClientConfigBuilder::from_directories(
            &self.state_dir,
            &self.cache_dir,
        )
        .build()
        .map_err(|e| anyhow::anyhow!("cannot build the Arti configuration: {e}"))?;
        Ok(config)
    }
}

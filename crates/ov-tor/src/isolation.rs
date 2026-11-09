//! Circuit isolation for hidden services.
//!
//! Tor's "stream isolation" guarantees that two different destinations never
//! share a circuit.  Arti implements this with an [`IsolationToken`]; a
//! client created with [`TorClient::isolated_client`] carries its own token
//! and therefore its own circuits.
//!
//! OnionVault isolates per *apex domain* of a v3 hidden service address: the
//! first 16 characters of the 56 character base32 address identify the
//! service, everything after them is the version specific part.  Visiting two
//! different hidden services therefore never puts them on the same circuit,
//! and neither does visiting the same service twice.

use std::{collections::HashMap, sync::Arc};

use arti_client::TorClient;
use tokio::sync::Mutex;
use tor_rtcompat::PreferredRuntime;

/// Number of base32 characters of a v3 onion address that identify the apex
/// domain (the service itself); the remaining 40 characters are the version
/// specific part.
const V3_APEX_LEN: usize = 16;

/// Keeps one isolated [`TorClient`] per isolation key.
pub struct IsolationRegistry {
    base: Arc<TorClient<PreferredRuntime>>,
    clients: Mutex<HashMap<String, Arc<TorClient<PreferredRuntime>>>>,
    isolate: bool,
}

impl IsolationRegistry {
    /// Create a registry on top of the main client.
    pub fn new(base: Arc<TorClient<PreferredRuntime>>, isolate: bool) -> Self {
        Self {
            base,
            clients: Mutex::new(HashMap::new()),
            isolate,
        }
    }

    /// The client that is used for destinations without an isolation group.
    pub fn base_client(&self) -> Arc<TorClient<PreferredRuntime>> {
        self.base.clone()
    }

    /// Return the client for `key`, creating an isolated one on first use.
    pub async fn client_for(&self, key: &str) -> Arc<TorClient<PreferredRuntime>> {
        if !self.isolate || key.is_empty() {
            return self.base.clone();
        }

        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get(key) {
            return client.clone();
        }
        // `isolated_client` clones the client but gives it a fresh isolation
        // token, which means fresh circuits for every new hidden service.
        let client = self.base.isolated_client();
        clients.insert(key.to_string(), client.clone());
        log::info!("created an isolated Tor circuit group for {key}");
        client
    }

    /// Number of isolation groups that are currently in use.
    pub async fn group_count(&self) -> usize {
        self.clients.lock().await.len()
    }

    /// The apex domain of a `.onion` address.
    ///
    /// `abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwxyz2345.onion`
    /// yields `abcdefghijklmnop`.  `None` is returned for anything that is not
    /// an onion address.
    pub fn apex_domain(&self, host: &str) -> Option<String> {
        apex_domain(host)
    }
}

/// Free function variant of [`IsolationRegistry::apex_domain`].
pub fn apex_domain(host: &str) -> Option<String> {
    let lower = host.trim_end_matches('.').to_ascii_lowercase();
    let base = lower.strip_suffix(".onion")?;
    if base.is_empty() {
        return None;
    }
    if base.len() <= V3_APEX_LEN {
        // v2 addresses are exactly 16 characters long.
        Some(base.to_string())
    } else {
        Some(base[..V3_APEX_LEN].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_apex_is_the_first_sixteen_characters() {
        let address = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion";
        assert_eq!(apex_domain(address).as_deref(), Some("2gzyxa5ihm7nsggf"),);
    }

    #[test]
    fn trailing_dot_and_case_are_normalised() {
        assert_eq!(
            apex_domain("2GZYXA5IHM7NSGGFXNU52RCK2VV4RVMDLKIU3ZZUI5DU4XYCLEN53WID.ONION.")
                .as_deref(),
            Some("2gzyxa5ihm7nsggf"),
        );
    }

    #[test]
    fn clearnet_hosts_have_no_apex() {
        assert_eq!(apex_domain("example.com"), None);
        assert_eq!(apex_domain("onion.example.com"), None);
    }
}

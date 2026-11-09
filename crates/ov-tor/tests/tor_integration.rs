//! Integration tests for the SOCKS5 front end of the embedded Tor client.
//!
//! The tests prove three properties of the front end:
//!
//!   * the handshake is answered exactly as RFC 1928 and RFC 1929 prescribe,
//!     byte for byte on the wire,
//!   * the traffic policy is leak proof: in strict hidden service mode a
//!     clearnet destination is refused, even when the browser asks for it
//!     through the proxy, while a hidden service address is accepted,
//!   * every hidden service gets its own circuit: two distinct apex
//!     domains never share an isolation token, the same apex always does.
//!
//! The suite is hermetic.  A local HTTP server plays the role of the hidden
//! service that serves the compose page of the browser, and the SOCKS5
//! listener is backed by an Arti client with a manual bootstrap behaviour:
//! it never touches the network, so every dial fails immediately and no
//! traffic ever leaves the machine.  The test that needs a live Tor network
//! is ignored by default.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use arti_client::{BootstrapBehavior, DataStream, TorClient};
use futures_util::StreamExt;
use ov_tor::{
    isolation::apex_domain,
    socks5::{
        decide, isolation_key, parse_request, reply_frame, Socks5Decision, Socks5Error,
        ATYP_DOMAIN, ATYP_IPV4, CMD_CONNECT, REP_HOST_UNREACHABLE, REP_NOT_ALLOWED, REP_SUCCESS,
        VERSION,
    },
    IsolationRegistry, Socks5Server, TorConfig, TorService,
};
use safelog::DisplayRedacted;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream as AsyncTcpStream,
    time::Instant,
};
use tor_cell::relaycell::msg::Connected;
use tor_hsservice::{config::OnionServiceConfigBuilder, HsNickname};
use tor_rtcompat::PreferredRuntime;

/// "No authentication required", RFC 1928.
const METHOD_NONE: u8 = 0x00;
/// "User/password authentication", RFC 1928.
const METHOD_USERPASS: u8 = 0x02;
/// The user/password sub negotiation version, RFC 1929.
const USERPASS_VERSION: u8 = 0x01;

/// The v3 address of the Tor Project onion service.
const TOR_PROJECT_ONION: &str = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion";
/// The v3 address of the DuckDuckGo onion service.
const DUCKDUCKGO_ONION: &str = "duckduckgogg42xjoc72x3sjasowoarfbgcmvfimaftt6twagsw3ad.onion";

/// The compose page of the OnionVault browser.
///
/// This is the page a hidden service serves: a text area for the plaintext
/// and a text area that carries an armored OpenPGP message.
const COMPOSE_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>OnionVault</title>
  </head>
  <body>
    <h1>OnionVault</h1>
    <form id="compose">
      <label for="plaintext">Message</label>
      <textarea id="plaintext" name="plaintext" rows="8" cols="64"></textarea>
      <label for="ciphertext">Encrypted message</label>
      <textarea id="ciphertext" name="ciphertext" rows="8" cols="64">-----BEGIN PGP MESSAGE-----

xA0DAAoWR6JVzBLxGQcBwV4DcrkFpMFLCBYSAQdA3vxCHFkrYH47UcYLQfuje75N
NDCT6Kv9L/FzAdz0ikgw25rxRkf0KWPsuzur3IXyYAq9btMJflvE64AUVHSKavVE
RUFFUUEQTjwo39t1Ghzg0jwBP5r18nDdTr6Jti0jpkbcEMJDOJhlRFIGsoyokfmU
BAChP8edxJy4nVouds21XIjtxfu4lyspci+9ik/CvQQAFgoAbwWCavD+FQkQR6JV
zBLxGQdHFAAAAAAAHgAgc2FsdEBub3RhdGlvbnMuc2VxdW9pYS1wZ3Aub3JnBdqi
J2cm8hrHx6VxWk6C6P7vF3dR7Vpjw8ZHxIaw/esWIQSReRElvIS7LhB1pNFHolXM
EvEZBwAA1BwA/38f9/yonrLWBR0q7vuKVWzfrn/mONF4UrnppZyFu3pGAQDgir0z
xVSxzONUmbzao0vCQTFBNNfB3Jjp+diiYlMZDA==
=YgPp
-----END PGP MESSAGE-----</textarea>
      <button type="submit">Encrypt</button>
    </form>
  </body>
</html>
"#;

/// A temporary directory that is removed when it goes out of scope.
struct TempDir(std::path::PathBuf);

impl TempDir {
    /// Create a temporary directory that belongs to this test.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ov-tor-it-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("cannot create the test directory");
        Self(path)
    }

    /// The path of the directory.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A minimal HTTP/1.1 server that serves the compose page.
struct HttpFixture {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl HttpFixture {
    /// Start the server on an ephemeral loopback port.
    fn start(body: String) -> Self {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("cannot bind the HTTP fixture");
        let addr = listener
            .local_addr()
            .expect("cannot read the fixture address");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _peer)) = listener.accept() else {
                    break;
                };
                serve_request(&mut stream, &body);
            }
        });
        Self {
            addr,
            stop,
            handle: Some(handle),
        }
    }

    /// The IPv4 address of the server.
    fn ipv4(&self) -> Ipv4Addr {
        match self.addr.ip() {
            std::net::IpAddr::V4(ip) => ip,
            std::net::IpAddr::V6(_) => unreachable!("the fixture binds to IPv4"),
        }
    }

    /// The port of the server.
    fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Fetch a page with a plain HTTP/1.1 request.
    fn fetch(&self, path: &str) -> String {
        let mut stream = TcpStream::connect(self.addr).expect("cannot reach the HTTP fixture");
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.addr
        );
        stream
            .write_all(request.as_bytes())
            .expect("cannot send the HTTP request");
        stream.flush().expect("cannot flush the HTTP request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("cannot read the HTTP response");
        response
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Unblock the accept loop of the fixture thread.
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Answer a single HTTP/1.1 request.
fn serve_request(stream: &mut TcpStream, body: &str) {
    let mut request = [0u8; 2048];
    let _ = stream.read(&mut request);
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// The bytes of a CONNECT request for a domain name (RFC 1928, `ATYP` 3).
fn connect_request_domain(host: &str, port: u16) -> Vec<u8> {
    let mut request = vec![VERSION, CMD_CONNECT, 0x00, ATYP_DOMAIN];
    request.push(host.len() as u8);
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    request
}

/// The bytes of a CONNECT request for an IPv4 address (RFC 1928, `ATYP` 1).
fn connect_request_ipv4(host: Ipv4Addr, port: u16) -> Vec<u8> {
    let mut request = vec![VERSION, CMD_CONNECT, 0x00, ATYP_IPV4];
    request.extend_from_slice(&host.octets());
    request.extend_from_slice(&port.to_be_bytes());
    request
}

/// A minimal SOCKS5 client that speaks RFC 1928 and RFC 1929.
struct Socks5Client {
    stream: AsyncTcpStream,
}

impl Socks5Client {
    /// Connect to a SOCKS5 listener.
    async fn connect(addr: SocketAddr) -> Self {
        let stream = AsyncTcpStream::connect(addr)
            .await
            .expect("cannot connect to the SOCKS5 listener");
        Self { stream }
    }

    /// Send the RFC 1928 greeting and read the selected method.
    async fn greet(&mut self, methods: &[u8]) -> [u8; 2] {
        let mut greeting = Vec::with_capacity(2 + methods.len());
        greeting.push(VERSION);
        greeting.push(methods.len() as u8);
        greeting.extend_from_slice(methods);
        self.stream
            .write_all(&greeting)
            .await
            .expect("cannot send the greeting");
        self.stream
            .flush()
            .await
            .expect("cannot flush the greeting");
        let mut answer = [0u8; 2];
        self.stream
            .read_exact(&mut answer)
            .await
            .expect("cannot read the method selection");
        answer
    }

    /// Run the RFC 1929 user/password exchange.
    async fn authenticate(&mut self, user: &str, password: &str) -> [u8; 2] {
        let mut message = vec![USERPASS_VERSION, user.len() as u8];
        message.extend_from_slice(user.as_bytes());
        message.push(password.len() as u8);
        message.extend_from_slice(password.as_bytes());
        self.stream
            .write_all(&message)
            .await
            .expect("cannot send the user name");
        self.stream
            .flush()
            .await
            .expect("cannot flush the user name");
        let mut answer = [0u8; 2];
        self.stream
            .read_exact(&mut answer)
            .await
            .expect("cannot read the user/password answer");
        answer
    }

    /// Send a request over the tunnel and read the whole answer.
    async fn exchange(&mut self, request: &[u8]) -> String {
        self.stream
            .write_all(request)
            .await
            .expect("cannot send the request over the tunnel");
        self.stream.flush().await.expect("cannot flush the tunnel");
        let mut answer = Vec::new();
        self.stream
            .read_to_end(&mut answer)
            .await
            .expect("cannot read the answer of the tunnel");
        String::from_utf8_lossy(&answer).into_owned()
    }

    /// Send a request and read the reply of the front end.
    async fn request(&mut self, request: &[u8]) -> [u8; 10] {
        self.stream
            .write_all(request)
            .await
            .expect("cannot send the request");
        self.stream.flush().await.expect("cannot flush the request");
        let mut reply = [0u8; 10];
        self.stream
            .read_exact(&mut reply)
            .await
            .expect("cannot read the reply");
        reply
    }
}

/// Create a directory that only its owner can see.
///
/// The Arti client refuses a state directory that the other users of the
/// machine can read: the directory holds the keys of the hidden services.
fn private_dir(path: &Path) {
    std::fs::create_dir_all(path).expect("cannot create the directory");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("cannot restrict the permissions of the directory");
}

/// Install the rustls crypto provider that the Arti client needs.
///
/// The production build never installs one, but a test that creates an
/// Arti client has to.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Start a SOCKS5 front end that is backed by an Arti client which never
/// bootstraps.
///
/// The client is created with a manual bootstrap behaviour, so it never
/// touches the network: every dial fails immediately.  That is enough to
/// exercise the whole protocol state machine of the front end.
async fn start_front_end(
    state_dir: &Path,
    cache_dir: &Path,
    allow_clearnet: bool,
) -> anyhow::Result<(Arc<Socks5Server>, Arc<IsolationRegistry>)> {
    install_crypto_provider();

    private_dir(state_dir);
    private_dir(cache_dir);

    let config = TorConfig {
        state_dir: state_dir.to_path_buf(),
        cache_dir: cache_dir.to_path_buf(),
        allow_clearnet,
        connect_timeout: Duration::from_secs(30),
        ..TorConfig::default()
    };
    config.validate()?;

    let tor_config = config.build_tor_config()?;
    let runtime = PreferredRuntime::current().context("cannot obtain the Tor runtime")?;
    let client = TorClient::with_runtime(runtime)
        .config(tor_config)
        .bootstrap_behavior(BootstrapBehavior::Manual)
        .create_unbootstrapped_async()
        .await
        .context("cannot create the Arti client")?;

    let registry = Arc::new(IsolationRegistry::new(client, true));
    let server = Socks5Server::start(
        config.socks_bind,
        registry.clone(),
        config.allow_clearnet,
        config.connect_timeout,
    )
    .await?;
    Ok((server, registry))
}

/// The hidden service that serves the compose page is reachable over plain
/// HTTP, and the page carries the elements the browser side needs.
#[test]
fn the_compose_page_is_served_over_http() {
    let fixture = HttpFixture::start(COMPOSE_PAGE.to_string());
    let response = fixture.fetch("/compose");

    assert!(
        response.starts_with("HTTP/1.1 200 OK\r\n"),
        "the fixture must answer with a valid status line: {response:?}"
    );
    assert!(response.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert!(response.contains(&format!("Content-Length: {}\r\n", COMPOSE_PAGE.len())));

    let (_head, body) = response
        .split_once("\r\n\r\n")
        .expect("the response must have a body");
    assert_eq!(body, COMPOSE_PAGE);

    // The elements the compose screen of OnionVault is built from.
    assert!(body.contains("<textarea"));
    assert!(body.contains("-----BEGIN PGP MESSAGE-----"));
    assert!(body.contains("-----END PGP MESSAGE-----"));
}

/// The client greeting of RFC 1928 is answered with the selected method.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_client_greeting_selects_the_no_authentication_method() -> anyhow::Result<()> {
    let dir = TempDir::new("greeting");
    let (server, _registry) =
        start_front_end(&dir.path().join("state"), &dir.path().join("cache"), false).await?;
    let addr = server.local_addr();

    let mut client = Socks5Client::connect(addr).await;
    let answer = client.greet(&[METHOD_NONE]).await;
    assert_eq!(
        answer,
        [VERSION, METHOD_NONE],
        "the server must select 'no authentication required'"
    );

    Ok(())
}

/// The optional user/password exchange of RFC 1929 is answered with the
/// success status, and the user name becomes the isolation key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_user_password_exchange_follows_rfc_1929() -> anyhow::Result<()> {
    let dir = TempDir::new("userpass");
    let (server, _registry) =
        start_front_end(&dir.path().join("state"), &dir.path().join("cache"), false).await?;
    let addr = server.local_addr();

    let mut client = Socks5Client::connect(addr).await;
    let answer = client.greet(&[METHOD_USERPASS]).await;
    assert_eq!(
        answer,
        [VERSION, METHOD_USERPASS],
        "the server must select the user/password method"
    );

    let status = client.authenticate("vault", "onion").await;
    assert_eq!(
        status,
        [USERPASS_VERSION, 0x00],
        "the user/password exchange must succeed"
    );

    // The user name is the isolation key of the connection.
    assert_eq!(isolation_key(TOR_PROJECT_ONION, "vault"), "vault");

    Ok(())
}

/// In strict hidden service mode the front end refuses a clearnet
/// destination, so that the browser cannot leak traffic out of the tunnel.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_strict_policy_refuses_a_clearnet_destination() -> anyhow::Result<()> {
    let dir = TempDir::new("strict");
    let (server, _registry) =
        start_front_end(&dir.path().join("state"), &dir.path().join("cache"), false).await?;
    let addr = server.local_addr();

    // The compose page is served over plain HTTP; asking for it through
    // the proxy must not be possible.
    let fixture = HttpFixture::start(COMPOSE_PAGE.to_string());

    let mut client = Socks5Client::connect(addr).await;
    client.greet(&[METHOD_NONE]).await;
    let reply = client
        .request(&connect_request_ipv4(fixture.ipv4(), fixture.port()))
        .await;

    assert_eq!(
        reply[1], REP_NOT_ALLOWED,
        "the front end must refuse the clearnet destination"
    );
    assert_eq!(
        reply.to_vec(),
        reply_frame(REP_NOT_ALLOWED),
        "the reply must be exactly the frame of RFC 1928"
    );

    Ok(())
}

/// A hidden service address is accepted by the strict policy, and the
/// request is decoded exactly as RFC 1928 prescribes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hidden_service_destination_is_accepted_by_the_strict_policy() -> anyhow::Result<()> {
    let dir = TempDir::new("onion");
    let (server, _registry) =
        start_front_end(&dir.path().join("state"), &dir.path().join("cache"), false).await?;
    let addr = server.local_addr();

    let request = connect_request_domain(TOR_PROJECT_ONION, 80);
    assert_eq!(
        &request[..4],
        &[VERSION, CMD_CONNECT, 0x00, ATYP_DOMAIN],
        "the request must use the DOMAINNAME address type"
    );

    // The policy accepts the hidden service address ...
    let decoded = parse_request(&request).expect("the request must be parsed");
    assert!(decoded.is_onion());
    assert_eq!(
        decide(&decoded, false),
        Socks5Decision::Proceed,
        "the strict policy must accept a hidden service address"
    );

    // ... so the front end dials it.  The test environment has no Tor
    // network, so the dial fails and the failure is reported.
    let mut client = Socks5Client::connect(addr).await;
    client.greet(&[METHOD_NONE]).await;
    let reply = client.request(&request).await;

    assert_ne!(
        reply[1], REP_SUCCESS,
        "the front end must not report a successful tunnel"
    );
    assert_eq!(
        reply[1], REP_HOST_UNREACHABLE,
        "the dial of the hidden service must fail without a Tor network"
    );
    assert_eq!(
        reply.to_vec(),
        reply_frame(REP_HOST_UNREACHABLE),
        "the reply must be exactly the frame of RFC 1928"
    );

    Ok(())
}

/// A request frame is decoded exactly as RFC 1928 prescribes, and the
/// malformed frames are rejected with a precise error.
#[test]
fn a_request_frame_is_parsed_exactly_as_rfc_1928_prescribes() {
    // A CONNECT to a hidden service, carried as a domain name.
    let request = connect_request_domain(TOR_PROJECT_ONION, 443);
    let decoded = parse_request(&request).expect("the request must be parsed");
    assert_eq!(decoded.command, CMD_CONNECT);
    assert_eq!(decoded.host, TOR_PROJECT_ONION);
    assert_eq!(decoded.port, 443);
    assert!(decoded.is_onion());

    // A CONNECT to an IPv4 address.
    let request = connect_request_ipv4(Ipv4Addr::new(127, 0, 0, 1), 8080);
    let decoded = parse_request(&request).expect("the request must be parsed");
    assert_eq!(decoded.command, CMD_CONNECT);
    assert_eq!(decoded.host, "127.0.0.1");
    assert_eq!(decoded.port, 8080);
    assert!(!decoded.is_onion());

    // The malformed frames.
    assert_eq!(parse_request(&[]), Err(Socks5Error::Truncated));
    assert_eq!(
        parse_request(&[0x04, CMD_CONNECT, 0x00, ATYP_IPV4]),
        Err(Socks5Error::UnsupportedVersion(0x04))
    );
    assert_eq!(
        parse_request(&[VERSION, CMD_CONNECT, 0x00, 0x09]),
        Err(Socks5Error::UnsupportedAddressType(0x09))
    );
    assert_eq!(
        parse_request(&[VERSION, CMD_CONNECT, 0x00, ATYP_DOMAIN, 0x00]),
        Err(Socks5Error::InvalidDomainLength(0x00))
    );
    assert_eq!(
        parse_request(&[VERSION, CMD_CONNECT, 0x00, ATYP_DOMAIN, 0x03, b'a']),
        Err(Socks5Error::Truncated)
    );
    assert_eq!(
        parse_request(&[VERSION, CMD_CONNECT, 0x00, ATYP_IPV4, 127, 0, 0, 1]),
        Err(Socks5Error::Truncated)
    );
}

/// The isolation key of a connection is the apex domain of the hidden
/// service, unless the client supplied a user name.
#[test]
fn the_isolation_key_is_the_apex_of_the_hidden_service() {
    // Without a user name the apex domain of the hidden service is used.
    assert_eq!(
        isolation_key(TOR_PROJECT_ONION, ""),
        apex_domain(TOR_PROJECT_ONION).expect("the address must have an apex domain")
    );
    assert_eq!(isolation_key(TOR_PROJECT_ONION, ""), "2gzyxa5ihm7nsggf");

    // The user name of the RFC 1929 exchange wins.
    assert_eq!(isolation_key(TOR_PROJECT_ONION, "vault-1"), "vault-1");

    // A clearnet destination has no apex domain.
    assert_eq!(isolation_key("example.com", ""), "");

    // Two addresses that share their apex domain share the isolation key.
    let sibling = format!("{}{}.onion", &TOR_PROJECT_ONION[..16], "a".repeat(40));
    assert_eq!(
        isolation_key(&sibling, ""),
        isolation_key(TOR_PROJECT_ONION, "")
    );
}

/// Two hidden services never share a circuit, and the same hidden service
/// always ends up on the same one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_hidden_services_never_share_a_circuit() -> anyhow::Result<()> {
    let dir = TempDir::new("isolation");
    let (_server, registry) =
        start_front_end(&dir.path().join("state"), &dir.path().join("cache"), false).await?;

    let first_key = apex_domain(TOR_PROJECT_ONION).expect("the address must have an apex domain");
    let second_key = apex_domain(DUCKDUCKGO_ONION).expect("the address must have an apex domain");
    assert_ne!(
        first_key, second_key,
        "the two services must have different apex domains"
    );

    let first = registry.client_for(&first_key).await;
    let second = registry.client_for(&second_key).await;
    assert!(
        !Arc::ptr_eq(&first, &second),
        "two hidden services must not share an isolation token"
    );
    assert_eq!(registry.group_count().await, 2);

    // The same apex domain always yields the same isolation token.
    let again = registry.client_for(&first_key).await;
    assert!(
        Arc::ptr_eq(&first, &again),
        "the same hidden service must always use the same circuit"
    );
    assert_eq!(registry.group_count().await, 2);

    // Two addresses that share their apex domain share the circuit.  (The
    // checksum of the address is not validated by the apex domain, only
    // the first sixteen characters matter.)
    let sibling = format!("{}{}.onion", &TOR_PROJECT_ONION[..16], "a".repeat(40));
    let sibling_key = apex_domain(&sibling).expect("the address must have an apex domain");
    assert_eq!(sibling_key, first_key);
    let sibling_client = registry.client_for(&sibling_key).await;
    assert!(
        Arc::ptr_eq(&first, &sibling_client),
        "the same hidden service must always use the same circuit"
    );
    assert_eq!(registry.group_count().await, 2);

    Ok(())
}

/// The real client boots against the Tor network and exposes the SOCKS5
/// listener on the loopback interface.
///
/// The test needs a live Tor network, so it is ignored by default.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a live Tor network"]
async fn the_live_client_boots_and_exposes_a_loopback_socks_endpoint() -> anyhow::Result<()> {
    install_crypto_provider();

    let dir = TempDir::new("live");
    private_dir(&dir.path().join("state"));
    private_dir(&dir.path().join("cache"));

    let config = TorConfig {
        state_dir: dir.path().join("state"),
        cache_dir: dir.path().join("cache"),
        socks_bind: "127.0.0.1:0".parse()?,
        allow_clearnet: false,
        connect_timeout: Duration::from_secs(120),
        ..TorConfig::default()
    };

    let service = TorService::start(config).await?;

    let endpoint = service.socks_endpoint();
    assert!(
        endpoint.ip().is_loopback(),
        "the SOCKS5 listener must stay on the loopback interface, got {endpoint}"
    );
    assert_ne!(endpoint.port(), 0, "the listener must have a real port");

    // A cold Tor client needs a couple of minutes to bootstrap.
    let deadline = Instant::now() + Duration::from_secs(600);
    while !service.is_ready() {
        assert!(
            Instant::now() < deadline,
            "the Tor client did not bootstrap in time: {}",
            service.bootstrap_progress().stage
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert!(service.is_ready(), "the client must be ready for traffic");
    let progress = service.bootstrap_progress();
    assert!(progress.ready);
    assert_eq!(progress.percent, 100);

    service.shutdown();

    Ok(())
}

/// Answer the request of a hidden service with the compose page.
async fn serve_onion_service(mut stream: DataStream, body: String) {
    let mut request = [0u8; 2048];
    let _ = stream.read(&mut request).await;
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

/// The end to end proof of the transport: the client publishes a hidden
/// service, the SOCKS5 front end opens a tunnel to it and the compose page
/// of the hidden service comes back through the tunnel.
///
/// This is what the browser does: it asks for a hidden service, the front
/// end puts the request on a circuit and the service answers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires a live Tor network"]
async fn the_client_tunnels_to_a_real_onion_service() -> anyhow::Result<()> {
    let dir = TempDir::new("onion-service");
    private_dir(&dir.path().join("state"));
    private_dir(&dir.path().join("cache"));

    let config = TorConfig {
        state_dir: dir.path().join("state"),
        cache_dir: dir.path().join("cache"),
        socks_bind: "127.0.0.1:0".parse()?,
        allow_clearnet: false,
        ..TorConfig::default()
    };

    let service = TorService::start(config).await?;
    let endpoint = service.socks_endpoint();

    // Wait for the bootstrap of the client.
    let deadline = Instant::now() + Duration::from_secs(300);
    while !service.is_ready() {
        anyhow::ensure!(
            Instant::now() < deadline,
            "the Tor client did not bootstrap in time"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // The hidden service is published by the same client, so the test needs
    // only one directory cache.
    let service_config = OnionServiceConfigBuilder::default()
        .nickname(HsNickname::new("onionvault-test".to_string())?)
        .build()?;
    let (running, requests) = service
        .client()
        .launch_onion_service(service_config)?
        .context("the onion service must be enabled")?;
    let address = running
        .onion_address()
        .context("the onion service must have an address")?;
    let host = address.display_unredacted().to_string();

    // Answer the requests of the hidden service with the compose page.
    tokio::spawn(async move {
        let mut requests = Box::pin(requests);
        while let Some(request) = requests.next().await {
            let mut streams = match request.accept().await {
                Ok(streams) => streams,
                Err(err) => {
                    log::warn!("cannot accept the request of the hidden service: {err}");
                    continue;
                }
            };
            while let Some(stream_request) = streams.next().await {
                let stream = match stream_request.accept(Connected::new_empty()).await {
                    Ok(stream) => stream,
                    Err(err) => {
                        log::warn!("cannot accept the stream of the hidden service: {err}");
                        continue;
                    }
                };
                tokio::spawn(serve_onion_service(stream, COMPOSE_PAGE.to_string()));
            }
        }
    });

    // The descriptor of the hidden service has to be published to the
    // hidden service directories and fetched again, so the tunnel is
    // retried for a few minutes.
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut client = loop {
        let mut client = Socks5Client::connect(endpoint).await;
        client.greet(&[METHOD_NONE]).await;
        let reply = client.request(&connect_request_domain(&host, 80)).await;
        if reply[1] == REP_SUCCESS {
            break client;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "the hidden service {host} did not become reachable"
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
    };

    // The compose page comes through the tunnel.
    let response = client
        .exchange(
            format!("GET /compose HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await;

    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the hidden service must answer with a valid status line: {response:?}"
    );
    assert!(
        response.contains("<textarea"),
        "the compose page must have a text area"
    );
    assert!(
        response.contains("-----BEGIN PGP MESSAGE-----"),
        "the compose page must carry the armored OpenPGP message"
    );

    service.shutdown();
    Ok(())
}

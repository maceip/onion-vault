//! A minimal, self-contained SOCKS5 (RFC 1928 / RFC 1929) front end for the
//! embedded Arti client.
//!
//! Chromium's network stack is pointed at this listener with
//! `--proxy-server=socks5://127.0.0.1:<port>`; every browser connection is
//! therefore tunnelled through Arti and never leaves the device in the clear.
//!
//! The server also acts as the isolation point: a SOCKS5 username/password
//! pair (RFC 1929) or the destination's `.onion` apex domain selects the
//! circuit that the connection is placed on.
//!
//! The wire format and the traffic policy are implemented in the pure
//! helpers [`parse_request`], [`isolation_key`], [`decide`] and
//! [`reply_frame`]; they can be exercised without a running Tor client.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use arti_client::{DataStream, StreamPrefs};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
};

use crate::isolation::{apex_domain, IsolationRegistry};

/// SOCKS5 protocol version.
pub const VERSION: u8 = 0x05;
/// RFC 1929 user/password authentication version.
const USERPASS_VERSION: u8 = 0x01;

/// "No authentication required" method.
const METHOD_NONE: u8 = 0x00;
/// "User/password authentication" method.
const METHOD_USERPASS: u8 = 0x02;
/// "No acceptable methods" answer.
const METHOD_UNACCEPTABLE: u8 = 0xff;

/// CONNECT command.
pub const CMD_CONNECT: u8 = 0x01;
/// IPv4 address type.
pub const ATYP_IPV4: u8 = 0x01;
/// Domain name address type.
pub const ATYP_DOMAIN: u8 = 0x03;
/// IPv6 address type.
pub const ATYP_IPV6: u8 = 0x04;

/// Reply codes.
pub const REP_SUCCESS: u8 = 0x00;
pub const REP_GENERAL_FAILURE: u8 = 0x01;
pub const REP_NOT_ALLOWED: u8 = 0x02;
pub const REP_HOST_UNREACHABLE: u8 = 0x04;
pub const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub const REP_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

/// Maximum length of a SOCKS5 domain name.
const MAX_DOMAIN_LEN: u8 = 255;

/// Errors that can occur while a SOCKS5 message is parsed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum Socks5Error {
    /// The client used a SOCKS version other than 5.
    #[error("unsupported SOCKS version {0}")]
    UnsupportedVersion(u8),
    /// The client used an address type that RFC 1928 does not define.
    #[error("unsupported address type {0}")]
    UnsupportedAddressType(u8),
    /// The length of the domain name is zero or larger than 255.
    #[error("invalid domain length {0}")]
    InvalidDomainLength(u8),
    /// The message ends before the request is complete.
    #[error("the SOCKS5 request is truncated")]
    Truncated,
}

/// A decoded SOCKS5 request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Socks5Request {
    /// Command byte (only `CONNECT` is supported).
    pub command: u8,
    /// Destination host, either a domain name or a textual IP address.
    pub host: String,
    /// Destination port.
    pub port: u16,
}

impl Socks5Request {
    /// `true` when the destination is a Tor hidden service address.
    pub fn is_onion(&self) -> bool {
        self.host.to_ascii_lowercase().ends_with(".onion")
            || self.host.to_ascii_lowercase().ends_with(".onion.")
    }
}

/// What the front end does with a decoded request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Socks5Decision {
    /// The request is allowed: the connection is placed on a circuit.
    Proceed,
    /// The request is rejected; the given reply code is sent to the client.
    Reject(u8),
}

/// Parse a SOCKS5 request message.
///
/// The message is the one described by RFC 1928, section 3: the version, the
/// command, a reserved byte, the address type, the address and the port.
/// The function is pure, so that the wire format can be tested without a
/// running Tor client.
pub fn parse_request(bytes: &[u8]) -> Result<Socks5Request, Socks5Error> {
    if bytes.len() < 4 {
        return Err(Socks5Error::Truncated);
    }
    let version = bytes[0];
    if version != VERSION {
        return Err(Socks5Error::UnsupportedVersion(version));
    }
    let command = bytes[1];
    let atyp = bytes[3];

    let (host, port_offset) = match atyp {
        ATYP_IPV4 => {
            if bytes.len() < 8 {
                return Err(Socks5Error::Truncated);
            }
            let octets = [bytes[4], bytes[5], bytes[6], bytes[7]];
            (std::net::Ipv4Addr::from(octets).to_string(), 8)
        }
        ATYP_IPV6 => {
            if bytes.len() < 20 {
                return Err(Socks5Error::Truncated);
            }
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&bytes[4..20]);
            (std::net::Ipv6Addr::from(octets).to_string(), 20)
        }
        ATYP_DOMAIN => {
            if bytes.len() < 5 {
                return Err(Socks5Error::Truncated);
            }
            let len = bytes[4];
            // The length of a domain name is a single byte, so it can never
            // be longer than 255 characters; only the empty name is invalid.
            if len == 0 {
                return Err(Socks5Error::InvalidDomainLength(len));
            }
            let end = 5 + len as usize;
            if bytes.len() < end {
                return Err(Socks5Error::Truncated);
            }
            (String::from_utf8_lossy(&bytes[5..end]).into_owned(), end)
        }
        other => return Err(Socks5Error::UnsupportedAddressType(other)),
    };

    if bytes.len() < port_offset + 2 {
        return Err(Socks5Error::Truncated);
    }
    let port = u16::from_be_bytes([bytes[port_offset], bytes[port_offset + 1]]);

    Ok(Socks5Request {
        command,
        host,
        port,
    })
}

/// The isolation key of a request.
///
/// The SOCKS5 user name (RFC 1929) wins when the client supplied one;
/// otherwise the apex domain of the hidden service is used.  Two different
/// hidden services therefore never share a circuit.
pub fn isolation_key(host: &str, username: &str) -> String {
    if username.is_empty() {
        apex_domain(host).unwrap_or_default()
    } else {
        username.to_string()
    }
}

/// Apply the traffic policy to a decoded request.
///
/// In hidden service only mode every destination that is not a hidden
/// service address is refused: the browser must not be able to leak traffic
/// outside of the tunnel.
pub fn decide(request: &Socks5Request, allow_clearnet: bool) -> Socks5Decision {
    if request.command != CMD_CONNECT {
        return Socks5Decision::Reject(REP_COMMAND_NOT_SUPPORTED);
    }
    if !request.is_onion() && !allow_clearnet {
        return Socks5Decision::Reject(REP_NOT_ALLOWED);
    }
    Socks5Decision::Proceed
}

/// Build the reply message for `code`.
///
/// The bound address is always reported as 0.0.0.0:0, which is what every
/// SOCKS5 client expects from a CONNECT reply.
pub fn reply_frame(code: u8) -> Vec<u8> {
    let mut reply = vec![VERSION, code, 0x00, ATYP_IPV4];
    reply.extend_from_slice(&[0, 0, 0, 0]);
    reply.extend_from_slice(&0u16.to_be_bytes());
    reply
}

/// The SOCKS5 listener.
pub struct Socks5Server {
    local_addr: SocketAddr,
    shutdown: watch::Sender<bool>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Socks5Server {
    /// Bind the listener and start accepting connections.
    ///
    /// The socket is bound to `bind`; passing port `0` makes the kernel pick
    /// an ephemeral port, which is then reported by [`Socks5Server::local_addr`].
    pub async fn start(
        bind: SocketAddr,
        isolation: Arc<IsolationRegistry>,
        allow_clearnet: bool,
        connect_timeout: Duration,
    ) -> anyhow::Result<Arc<Self>> {
        let listener = TcpListener::bind(bind)
            .await
            .map_err(|e| anyhow::anyhow!("cannot bind the SOCKS5 listener on {bind}: {e}"))?;
        let local_addr = listener.local_addr()?;
        let (shutdown, mut shutdown_rx) = watch::channel(false);

        let accept_task = {
            let shutdown = shutdown.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        changed = shutdown_rx.changed() => {
                            if changed.is_err() || *shutdown_rx.borrow() {
                                break;
                            }
                        }
                        accepted = listener.accept() => {
                            let Ok((stream, peer)) = accepted else { continue };
                            let isolation = isolation.clone();
                            let shutdown = shutdown.subscribe();
                            tokio::spawn(async move {
                                if let Err(err) = handle_connection(
                                    stream,
                                    peer,
                                    isolation,
                                    allow_clearnet,
                                    connect_timeout,
                                    shutdown,
                                )
                                .await
                                {
                                    log::debug!("socks5 connection from {peer} ended: {err:#}");
                                }
                            });
                        }
                    }
                }
            })
        };

        Ok(Arc::new(Self {
            local_addr,
            shutdown,
            accept_task,
        }))
    }

    /// The address the listener is bound to, including the ephemeral port.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop accepting new connections.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
        self.accept_task.abort();
    }
}

impl Drop for Socks5Server {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// Serve a single SOCKS5 client connection.
async fn handle_connection(
    mut stream: TcpStream,
    _peer: SocketAddr,
    isolation: Arc<IsolationRegistry>,
    allow_clearnet: bool,
    connect_timeout: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    stream.set_nodelay(true)?;

    let username = negotiate_auth(&mut stream).await?;

    let request = match read_request(&mut stream).await {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(err) => {
            // The client sent something that is not a SOCKS5 request.
            log::debug!("malformed SOCKS5 request: {err:#}");
            write_reply(&mut stream, REP_ADDRESS_TYPE_NOT_SUPPORTED).await?;
            return Ok(());
        }
    };

    match decide(&request, allow_clearnet) {
        Socks5Decision::Reject(REP_NOT_ALLOWED) => {
            log::warn!(
                "refusing clearnet connection to {}:{} in hidden service only mode",
                request.host,
                request.port
            );
            write_reply(&mut stream, REP_NOT_ALLOWED).await?;
            return Ok(());
        }
        Socks5Decision::Reject(code) => {
            log::debug!("refusing the SOCKS5 request with reply code {code}");
            write_reply(&mut stream, code).await?;
            return Ok(());
        }
        Socks5Decision::Proceed => {}
    }

    // The isolation key is the SOCKS5 user name when the client supplied one,
    // otherwise the apex domain of the hidden service.  Two different hidden
    // services therefore never share a circuit.
    let key = isolation_key(&request.host, &username);

    let client = isolation.client_for(&key).await;

    let prefs = StreamPrefs::default();
    let target = (request.host.as_str(), request.port);
    let tor_stream = tokio::select! {
        biased;
        changed = shutdown.changed() => {
            if changed.is_err() || *shutdown.borrow() {
                return Ok(());
            }
            return Ok(());
        }
        result = tokio::time::timeout(connect_timeout, client.connect_with_prefs(target, &prefs)) => {
            match result {
                Ok(Ok(stream)) => stream,
                Ok(Err(err)) => {
                    log::warn!("cannot reach {}:{}: {err}", request.host, request.port);
                    write_reply(&mut stream, REP_HOST_UNREACHABLE).await?;
                    return Ok(());
                }
                Err(_) => {
                    log::warn!("timeout while connecting to {}:{}", request.host, request.port);
                    write_reply(&mut stream, REP_GENERAL_FAILURE).await?;
                    return Ok(());
                }
            }
        }
    };

    write_reply(&mut stream, REP_SUCCESS).await?;
    stream.flush().await?;

    relay(stream, tor_stream).await
}

/// Copy bytes in both directions until either side closes.
async fn relay(mut client: TcpStream, mut tor: DataStream) -> anyhow::Result<()> {
    // Two buffers: the select! below borrows both of them at the same time.
    let mut to_tor = vec![0u8; 32 * 1024];
    let mut to_client = vec![0u8; 32 * 1024];
    loop {
        tokio::select! {
            read = client.read(&mut to_tor) => {
                let n = read?;
                if n == 0 {
                    break;
                }
                tor.write_all(&to_tor[..n]).await?;
                tor.flush().await?;
            }
            write = tor.read(&mut to_client) => {
                let n = write?;
                if n == 0 {
                    break;
                }
                client.write_all(&to_client[..n]).await?;
                client.flush().await?;
            }
        }
    }
    Ok(())
}

/// Perform the method negotiation and, when the client asks for it, the
/// RFC 1929 user/password exchange.  Returns the user name.
async fn negotiate_auth(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != VERSION {
        anyhow::bail!("unsupported SOCKS version {}", header[0]);
    }
    let method_count = header[1];
    if method_count == 0 {
        anyhow::bail!("client offered no authentication method");
    }
    let mut methods = vec![0u8; method_count as usize];
    stream.read_exact(&mut methods).await?;

    if methods.contains(&METHOD_USERPASS) {
        stream.write_all(&[VERSION, METHOD_USERPASS]).await?;
        stream.flush().await?;
        let username = read_userpass(stream).await?;
        Ok(username)
    } else if methods.contains(&METHOD_NONE) {
        stream.write_all(&[VERSION, METHOD_NONE]).await?;
        stream.flush().await?;
        Ok(String::new())
    } else {
        stream.write_all(&[VERSION, METHOD_UNACCEPTABLE]).await?;
        anyhow::bail!("client offered no supported authentication method")
    }
}

/// RFC 1929 user/password sub-negotiation.  The user name doubles as the
/// isolation key, the password is not used for authentication (the listener
/// only exists inside the application sandbox).
async fn read_userpass(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != USERPASS_VERSION {
        anyhow::bail!("unsupported user/password version {}", header[0]);
    }
    let name_len = header[1];
    let mut username = vec![0u8; name_len as usize];
    stream.read_exact(&mut username).await?;

    let mut pass_len = [0u8; 1];
    stream.read_exact(&mut pass_len).await?;
    let mut password = vec![0u8; pass_len[0] as usize];
    stream.read_exact(&mut password).await?;

    // Status 0 == success.
    stream.write_all(&[USERPASS_VERSION, 0x00]).await?;
    stream.flush().await?;

    Ok(String::from_utf8_lossy(&username).into_owned())
}

/// Read the SOCKS5 request message.
///
/// The message is collected in a frame buffer and handed to
/// [`parse_request`], so that the wire format is implemented exactly once.
async fn read_request(stream: &mut TcpStream) -> anyhow::Result<Option<Socks5Request>> {
    let mut header = [0u8; 4];
    if stream.read_exact(&mut header).await.is_err() {
        return Ok(None);
    }

    let mut frame = Vec::with_capacity(4 + MAX_DOMAIN_LEN as usize + 3);
    frame.extend_from_slice(&header);

    match header[3] {
        ATYP_IPV4 | ATYP_IPV6 => {
            let len = if header[3] == ATYP_IPV4 { 4 } else { 16 };
            let mut address = vec![0u8; len];
            stream.read_exact(&mut address).await?;
            frame.extend_from_slice(&address);
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            if len[0] == 0 {
                anyhow::bail!("invalid domain length {}", len[0]);
            }
            let mut name = vec![0u8; len[0] as usize];
            stream.read_exact(&mut name).await?;
            frame.push(len[0]);
            frame.extend_from_slice(&name);
        }
        other => anyhow::bail!("unsupported address type {other}"),
    }

    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await?;
    frame.extend_from_slice(&port);

    let request =
        parse_request(&frame).map_err(|err| anyhow::anyhow!("malformed SOCKS5 request: {err}"))?;
    Ok(Some(request))
}

/// Write a SOCKS5 reply.
async fn write_reply(stream: &mut TcpStream, code: u8) -> anyhow::Result<()> {
    stream.write_all(&reply_frame(code)).await?;
    stream.flush().await?;
    Ok(())
}

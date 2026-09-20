use super::{
    DemoReport, EnvelopeCodec, MAX_ENVELOPE_BYTES, MAX_SESSION_PREFACE_BYTES, RuntimeError,
    SESSION_ACCEPTED, SESSION_REJECTED, SessionPreface, Transport, WireEncoding, WireEnvelope,
    decode_session_preface, demo_report, encode_session_preface, local_session_preface,
    require_session_field, run_generate_client_plan, run_generate_server_plan,
};
use crate::Conversation;
use crate::plan::PreparedPlan;
use iroh::endpoint::{Connection, ConnectionError, RecvStream, SendStream, VarInt, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, SecretKey};
use std::net::SocketAddr;

/// Eve's application protocol identifier over Iroh QUIC.
pub const EVE_IROH_ALPN: &[u8] = b"eve/0.1";
const CHANNEL_BINDING_LABEL: &[u8] = b"EXPORTER-eve-session-v0";

/// A persistent-identity Iroh endpoint before it becomes one connected Eve transport.
pub struct IrohNode {
    endpoint: Endpoint,
    runtime: tokio::runtime::Runtime,
}

impl IrohNode {
    /// Bind a direct-only endpoint. This is appropriate for a datacenter or Miren overlay where
    /// addresses are already routable and no public relay should be contacted.
    pub fn bind_direct(secret_key: SecretKey, address: SocketAddr) -> Result<Self, RuntimeError> {
        let runtime = iroh_runtime()?;
        let endpoint = runtime
            .block_on(
                Endpoint::builder(presets::N0)
                    .clear_ip_transports()
                    .bind_addr(address)
                    .map_err(|error| RuntimeError::Iroh(error.to_string()))?
                    .secret_key(secret_key)
                    .alpns(vec![EVE_IROH_ALPN.to_vec()])
                    .relay_mode(RelayMode::Disabled)
                    .bind(),
            )
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        Ok(Self { endpoint, runtime })
    }

    /// Bind with Iroh's default address discovery, NAT traversal, and relay fallback.
    pub fn bind_default(secret_key: SecretKey) -> Result<Self, RuntimeError> {
        let runtime = iroh_runtime()?;
        let endpoint = runtime
            .block_on(
                Endpoint::builder(presets::N0)
                    .secret_key(secret_key)
                    .alpns(vec![EVE_IROH_ALPN.to_vec()])
                    .bind(),
            )
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        Ok(Self { endpoint, runtime })
    }

    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.endpoint.secret_key().to_bytes()
    }

    pub fn close(self) {
        let Self { endpoint, runtime } = self;
        runtime.block_on(async { endpoint.close().await });
    }

    pub fn accept(self, expected_peer: EndpointId) -> Result<IrohTransport, RuntimeError> {
        self.accept_with_codec(&[expected_peer], EnvelopeCodec::Reference)
    }

    pub fn accept_compact(
        self,
        expected_peer: EndpointId,
        plan: &PreparedPlan,
    ) -> Result<IrohTransport, RuntimeError> {
        self.accept_with_codec(
            &[expected_peer],
            EnvelopeCodec::for_plan(WireEncoding::Compact, plan),
        )
    }

    /// Accept one connection only when its cryptographic EndpointID is in the supplied policy set.
    pub fn accept_authorized(
        self,
        authorized_peers: &[EndpointId],
    ) -> Result<IrohTransport, RuntimeError> {
        self.accept_with_codec(authorized_peers, EnvelopeCodec::Reference)
    }

    pub fn accept_authorized_compact(
        self,
        authorized_peers: &[EndpointId],
        plan: &PreparedPlan,
    ) -> Result<IrohTransport, RuntimeError> {
        self.accept_with_codec(
            authorized_peers,
            EnvelopeCodec::for_plan(WireEncoding::Compact, plan),
        )
    }

    fn accept_with_codec(
        self,
        authorized_peers: &[EndpointId],
        codec: EnvelopeCodec,
    ) -> Result<IrohTransport, RuntimeError> {
        let Self { endpoint, runtime } = self;
        let incoming = runtime
            .block_on(async { endpoint.accept().await })
            .ok_or_else(|| RuntimeError::Iroh("Iroh endpoint closed".to_string()))?;
        let connection = runtime
            .block_on(async move {
                let accepting = incoming.accept().map_err(|error| error.to_string())?;
                accepting.await.map_err(|error| error.to_string())
            })
            .map_err(RuntimeError::Iroh)?;
        if let Err(error) = verify_remote_authorized(&connection, authorized_peers) {
            connection.close(VarInt::from_u32(1), b"unauthorized Eve endpoint");
            runtime.block_on(endpoint.close());
            return Err(error);
        }
        let (send, receive) = runtime
            .block_on(async { connection.accept_bi().await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        Ok(IrohTransport::new(
            endpoint, connection, send, receive, runtime, codec,
        ))
    }

    pub fn connect(
        self,
        remote: EndpointAddr,
        expected_peer: EndpointId,
    ) -> Result<IrohTransport, RuntimeError> {
        self.connect_with_codec(remote, expected_peer, EnvelopeCodec::Reference)
    }

    pub fn connect_compact(
        self,
        remote: EndpointAddr,
        expected_peer: EndpointId,
        plan: &PreparedPlan,
    ) -> Result<IrohTransport, RuntimeError> {
        self.connect_with_codec(
            remote,
            expected_peer,
            EnvelopeCodec::for_plan(WireEncoding::Compact, plan),
        )
    }

    fn connect_with_codec(
        self,
        remote: EndpointAddr,
        expected_peer: EndpointId,
        codec: EnvelopeCodec,
    ) -> Result<IrohTransport, RuntimeError> {
        if remote.id != expected_peer {
            let actual = remote.id;
            self.close();
            return Err(RuntimeError::IrohPeerIdentity {
                expected: expected_peer.to_string(),
                actual: actual.to_string(),
            });
        }
        let Self { endpoint, runtime } = self;
        let connection = runtime
            .block_on(async { endpoint.connect(remote, EVE_IROH_ALPN).await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        verify_remote_identity(&connection, expected_peer)?;
        let (send, receive) = runtime
            .block_on(async { connection.open_bi().await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        Ok(IrohTransport::new(
            endpoint, connection, send, receive, runtime, codec,
        ))
    }
}

pub struct IrohTransport {
    endpoint: Endpoint,
    connection: Connection,
    send: SendStream,
    receive: RecvStream,
    runtime: tokio::runtime::Runtime,
    codec: EnvelopeCodec,
    remote_identity: EndpointId,
    session_established: bool,
    finished: bool,
}

impl IrohTransport {
    fn new(
        endpoint: Endpoint,
        connection: Connection,
        send: SendStream,
        receive: RecvStream,
        runtime: tokio::runtime::Runtime,
        codec: EnvelopeCodec,
    ) -> Self {
        let remote_identity = connection.remote_id();
        Self {
            endpoint,
            connection,
            send,
            receive,
            runtime,
            codec,
            remote_identity,
            session_established: false,
            finished: false,
        }
    }

    pub fn remote_identity(&self) -> EndpointId {
        self.remote_identity
    }

    pub fn session_established(&self) -> bool {
        self.session_established
    }

    /// Exchange a preface that is bound to both authenticated EndpointIDs and this exact TLS
    /// connection. Replaying the bytes on another connection changes the exporter binding.
    pub fn establish_session(
        &mut self,
        plan: &PreparedPlan,
        local_role: &str,
        peer_role: &str,
    ) -> Result<SessionPreface, RuntimeError> {
        let channel_binding = channel_binding(&self.connection, plan)?;
        let mut local = local_session_preface(&self.codec, plan, local_role, peer_role)?;
        local.endpoint_identity = Some(self.endpoint.id().to_string());
        local.channel_binding = Some(channel_binding.clone());
        let encoded = encode_session_preface(&local)?;
        let length =
            u32::try_from(encoded.len()).map_err(|_| RuntimeError::SessionPrefaceTooLarge {
                actual: encoded.len(),
                limit: u32::MAX as usize,
            })?;

        let peer_length = self
            .runtime
            .block_on(async {
                self.send
                    .write_all(&length.to_be_bytes())
                    .await
                    .map_err(|error| error.to_string())?;
                self.send
                    .write_all(&encoded)
                    .await
                    .map_err(|error| error.to_string())?;
                let mut peer_length = [0_u8; 4];
                self.receive
                    .read_exact(&mut peer_length)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>(u32::from_be_bytes(peer_length) as usize)
            })
            .map_err(RuntimeError::Iroh)?;
        if peer_length > MAX_SESSION_PREFACE_BYTES {
            self.abort();
            return Err(RuntimeError::SessionPrefaceTooLarge {
                actual: peer_length,
                limit: MAX_SESSION_PREFACE_BYTES,
            });
        }
        let mut peer_encoded = vec![0; peer_length];
        self.runtime
            .block_on(async { self.receive.read_exact(&mut peer_encoded).await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;

        let peer = decode_session_preface(&peer_encoded).and_then(|peer| {
            verify_iroh_session_preface(
                &peer,
                plan,
                peer_role,
                self.codec.encoding(),
                self.remote_identity,
                &channel_binding,
            )?;
            Ok(peer)
        });
        let peer = match peer {
            Ok(peer) => peer,
            Err(error) => {
                let _ = self
                    .runtime
                    .block_on(self.send.write_all(&[SESSION_REJECTED]));
                self.abort();
                return Err(error);
            }
        };

        let mut peer_status = [SESSION_REJECTED];
        let status = self.runtime.block_on(async {
            self.send
                .write_all(&[SESSION_ACCEPTED])
                .await
                .map_err(|error| error.to_string())?;
            self.receive
                .read_exact(&mut peer_status)
                .await
                .map_err(|error| error.to_string())
        });
        if let Err(error) = status {
            self.abort();
            return Err(RuntimeError::Iroh(error));
        }
        if peer_status[0] != SESSION_ACCEPTED {
            self.abort();
            return Err(RuntimeError::SessionRejected("Iroh"));
        }
        self.session_established = true;
        Ok(peer)
    }

    fn require_session(&self) -> Result<(), RuntimeError> {
        if !self.session_established {
            return Err(RuntimeError::SessionRequired("Iroh"));
        }
        Ok(())
    }
}

impl Transport for IrohTransport {
    fn plan(&self) -> &'static str {
        match self.codec.encoding() {
            WireEncoding::Reference => "iroh",
            WireEncoding::Compact => "iroh+compact",
        }
    }

    fn send(&mut self, envelope: &WireEnvelope) -> Result<(), RuntimeError> {
        self.require_session()?;
        let encoded = self.codec.encode(envelope)?;
        let length = u32::try_from(encoded.len()).map_err(|_| RuntimeError::EnvelopeTooLarge {
            actual: encoded.len(),
            limit: u32::MAX as usize,
        })?;
        self.runtime
            .block_on(async {
                self.send.write_all(&length.to_be_bytes()).await?;
                self.send.write_all(&encoded).await
            })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))
    }

    fn receive(&mut self) -> Result<WireEnvelope, RuntimeError> {
        self.require_session()?;
        let mut length = [0_u8; 4];
        self.runtime
            .block_on(async { self.receive.read_exact(&mut length).await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        let length = u32::from_be_bytes(length) as usize;
        if length > MAX_ENVELOPE_BYTES {
            return Err(RuntimeError::EnvelopeTooLarge {
                actual: length,
                limit: MAX_ENVELOPE_BYTES,
            });
        }
        let mut encoded = vec![0; length];
        self.runtime
            .block_on(async { self.receive.read_exact(&mut encoded).await })
            .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
        self.codec.decode(&encoded)
    }

    fn finish(&mut self, role: &str) -> Result<(), RuntimeError> {
        if self.finished {
            return Ok(());
        }
        match role {
            "client" => {
                self.runtime
                    .block_on(async {
                        self.send
                            .write_all(&0_u32.to_be_bytes())
                            .await
                            .map_err(|error| error.to_string())?;
                        read_close_marker(&mut self.receive).await?;
                        self.send
                            .write_all(&0_u32.to_be_bytes())
                            .await
                            .map_err(|error| error.to_string())
                    })
                    .map_err(RuntimeError::Iroh)?;
                self.send
                    .finish()
                    .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
                let reason = self.runtime.block_on(self.connection.closed());
                match reason {
                    ConnectionError::ApplicationClosed(closed)
                        if closed.error_code == VarInt::from_u32(0) => {}
                    reason => {
                        return Err(RuntimeError::Iroh(format!(
                            "Iroh connection closed before the server acknowledged completion: {reason}"
                        )));
                    }
                }
                self.runtime.block_on(self.endpoint.close());
            }
            "server" => {
                self.runtime
                    .block_on(async {
                        read_close_marker(&mut self.receive).await?;
                        self.send
                            .write_all(&0_u32.to_be_bytes())
                            .await
                            .map_err(|error| error.to_string())?;
                        read_close_marker(&mut self.receive).await
                    })
                    .map_err(RuntimeError::Iroh)?;
                self.send
                    .finish()
                    .map_err(|error| RuntimeError::Iroh(error.to_string()))?;
                self.runtime.block_on(self.endpoint.close());
            }
            role => {
                return Err(RuntimeError::Iroh(format!(
                    "Iroh close handshake does not support role {role}"
                )));
            }
        }
        self.finished = true;
        Ok(())
    }

    fn abort(&mut self) {
        self.connection
            .close(VarInt::from_u32(1), b"eve typed failure");
    }
}

pub fn run_iroh_demo(
    conversation: &Conversation,
    prompt: &str,
    token_limit: usize,
    cancel_after: Option<usize>,
) -> Result<DemoReport, RuntimeError> {
    let plan = PreparedPlan::compile(conversation)?;
    run_iroh_plan_demo(&plan, prompt, token_limit, cancel_after)
}

pub fn run_iroh_plan_demo(
    plan: &PreparedPlan,
    prompt: &str,
    token_limit: usize,
    cancel_after: Option<usize>,
) -> Result<DemoReport, RuntimeError> {
    run_iroh_plan_demo_with_encoding(
        plan,
        prompt,
        token_limit,
        cancel_after,
        WireEncoding::Reference,
    )
}

pub fn run_iroh_plan_demo_with_encoding(
    plan: &PreparedPlan,
    prompt: &str,
    token_limit: usize,
    cancel_after: Option<usize>,
    encoding: WireEncoding,
) -> Result<DemoReport, RuntimeError> {
    let server_node = IrohNode::bind_direct(
        SecretKey::generate(),
        "127.0.0.1:0".parse().expect("valid address"),
    )?;
    let client_node = IrohNode::bind_direct(
        SecretKey::generate(),
        "127.0.0.1:0".parse().expect("valid address"),
    )?;
    let server_addr = server_node.addr();
    let server_id = server_node.id();
    let client_id = client_node.id();
    let prompt = prompt.to_string();
    let (client, server) = std::thread::scope(|scope| {
        let server_worker = scope.spawn(move || {
            let mut transport = match encoding {
                WireEncoding::Reference => server_node.accept(client_id)?,
                WireEncoding::Compact => server_node.accept_compact(client_id, plan)?,
            };
            transport.establish_session(plan, "server", "client")?;
            run_generate_server_plan(plan, &mut transport, token_limit)
        });
        let client_worker = scope.spawn(move || {
            let mut transport = match encoding {
                WireEncoding::Reference => client_node.connect(server_addr, server_id)?,
                WireEncoding::Compact => {
                    client_node.connect_compact(server_addr, server_id, plan)?
                }
            };
            transport.establish_session(plan, "client", "server")?;
            run_generate_client_plan(plan, &mut transport, &prompt, cancel_after)
        });
        let client = client_worker
            .join()
            .map_err(|_| RuntimeError::WorkerPanicked)??;
        let server = server_worker
            .join()
            .map_err(|_| RuntimeError::WorkerPanicked)??;
        Ok::<_, RuntimeError>((client, server))
    })?;
    let name = match encoding {
        WireEncoding::Reference => "iroh",
        WireEncoding::Compact => "iroh+compact",
    };
    Ok(demo_report(name, client, server))
}

fn verify_remote_identity(
    connection: &Connection,
    expected: EndpointId,
) -> Result<(), RuntimeError> {
    let actual = connection.remote_id();
    if actual != expected {
        return Err(RuntimeError::IrohPeerIdentity {
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn verify_remote_authorized(
    connection: &Connection,
    authorized: &[EndpointId],
) -> Result<(), RuntimeError> {
    let actual = connection.remote_id();
    if authorized.contains(&actual) {
        return Ok(());
    }
    Err(RuntimeError::IrohPeerIdentity {
        expected: authorized
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        actual: actual.to_string(),
    })
}

fn verify_iroh_session_preface(
    peer: &SessionPreface,
    plan: &PreparedPlan,
    peer_role: &str,
    encoding: WireEncoding,
    remote_identity: EndpointId,
    channel_binding: &str,
) -> Result<(), RuntimeError> {
    peer.verify_peer(plan, peer_role, encoding)?;
    require_session_field(
        "endpoint_identity",
        &remote_identity.to_string(),
        peer.endpoint_identity.as_deref().unwrap_or("missing"),
    )?;
    require_session_field(
        "channel_binding",
        channel_binding,
        peer.channel_binding.as_deref().unwrap_or("missing"),
    )
}

fn channel_binding(connection: &Connection, plan: &PreparedPlan) -> Result<String, RuntimeError> {
    let mut binding = [0_u8; 32];
    connection
        .export_keying_material(
            &mut binding,
            CHANNEL_BINDING_LABEL,
            plan.plan_identity().as_bytes(),
        )
        .map_err(|error| RuntimeError::Iroh(format!("TLS exporter failed: {error:?}")))?;
    let mut encoded = String::with_capacity(binding.len() * 2);
    for byte in binding {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing into a string cannot fail");
    }
    Ok(format!("tls-exporter:{encoded}"))
}

async fn read_close_marker(receive: &mut RecvStream) -> Result<(), String> {
    let mut marker = [0_u8; 4];
    receive
        .read_exact(&mut marker)
        .await
        .map_err(|error| error.to_string())?;
    if u32::from_be_bytes(marker) != 0 {
        return Err("expected Eve Iroh close marker".to_string());
    }
    Ok(())
}

fn iroh_runtime() -> Result<tokio::runtime::Runtime, RuntimeError> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        // One two-node run opens two runtimes in one process. Unbounded worker
        // threads oversubscribe a small CI machine and starve the connection.
        .worker_threads(2)
        .enable_all()
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::network_test_guard;

    fn conversation() -> Conversation {
        serde_json::from_str(include_str!("../../examples/generate.eveconv.json")).unwrap()
    }

    #[test]
    fn iroh_runs_the_same_plan_with_mutual_endpoint_identity() {
        let _guard = network_test_guard();
        let report = run_iroh_demo(&conversation(), "authenticated peers", 3, None).unwrap();
        assert_eq!(report.transport_plan, "iroh");
        assert!(report.semantic_trace_equivalent);
        assert!(report.outcome_equivalent);
        assert_eq!(report.client.tokens, vec![1, 2, 3]);
    }

    #[test]
    fn iroh_compact_wire_preserves_semantics() {
        let _guard = network_test_guard();
        let plan = PreparedPlan::compile(&conversation()).unwrap();
        let report = run_iroh_plan_demo_with_encoding(
            &plan,
            "compact authenticated peers",
            2,
            None,
            WireEncoding::Compact,
        )
        .unwrap();
        assert_eq!(report.transport_plan, "iroh+compact");
        assert!(report.semantic_trace_equivalent);
        assert_eq!(report.client.tokens, vec![1, 2]);
    }

    #[test]
    fn iroh_rejects_an_unexpected_authenticated_peer_before_eve_frames() {
        let _guard = network_test_guard();
        let server =
            IrohNode::bind_direct(SecretKey::generate(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let client =
            IrohNode::bind_direct(SecretKey::generate(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let impostor = SecretKey::generate().public();
        let result = client.connect(server.addr(), impostor);
        assert!(matches!(result, Err(RuntimeError::IrohPeerIdentity { .. })));
    }

    #[test]
    fn iroh_server_closes_an_unauthorized_peer_without_stranding_the_client() {
        let _guard = network_test_guard();
        let plan = PreparedPlan::compile(&conversation()).unwrap();
        let server =
            IrohNode::bind_direct(SecretKey::generate(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let client =
            IrohNode::bind_direct(SecretKey::generate(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let server_addr = server.addr();
        let server_id = server.id();
        let allowed = [SecretKey::generate().public()];

        let (client_result, server_result) = std::thread::scope(|scope| {
            let server_worker = scope.spawn(|| server.accept_authorized(&allowed));
            let client_result = client
                .connect(server_addr, server_id)
                .and_then(|mut transport| transport.establish_session(&plan, "client", "server"));
            (client_result, server_worker.join().unwrap())
        });
        assert!(client_result.is_err());
        assert!(matches!(
            server_result,
            Err(RuntimeError::IrohPeerIdentity { .. })
        ));
    }

    #[test]
    fn iroh_identity_can_be_persisted_and_restored() {
        let _guard = network_test_guard();
        let node =
            IrohNode::bind_direct(SecretKey::generate(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let id = node.id();
        let bytes = node.secret_key_bytes();
        node.close();
        let restored = IrohNode::bind_direct(
            SecretKey::from_bytes(&bytes),
            "127.0.0.1:0".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(restored.id(), id);
    }

    #[test]
    fn iroh_rejects_a_preface_copied_from_another_tls_connection() {
        let plan = PreparedPlan::compile(&conversation()).unwrap();
        let remote_identity = SecretKey::generate().public();
        let mut peer = SessionPreface::for_plan(&plan, "server", WireEncoding::Reference).unwrap();
        peer.endpoint_identity = Some(remote_identity.to_string());
        peer.channel_binding = Some(format!("tls-exporter:{}", "0".repeat(64)));

        let error = verify_iroh_session_preface(
            &peer,
            &plan,
            "server",
            WireEncoding::Reference,
            remote_identity,
            &format!("tls-exporter:{}", "1".repeat(64)),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::SessionMismatch {
                field: "channel_binding",
                ..
            }
        ));
    }
}

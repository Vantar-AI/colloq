//! Persistent Iroh node identities, public direct-address tickets, and Colloq role policy.
//!
//! These artifacts are operational inputs. They do not affect conversation content identity.

use crate::plan::PreparedPlan;
use iroh::{EndpointAddr, EndpointId, SecretKey, TransportAddr};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

pub const NODE_IDENTITY_FORMAT: &str = "0.1.0";
pub const ENDPOINT_TICKET_FORMAT: &str = "0.1.0";
pub const AUTHORIZATION_FORMAT: &str = "0.1.0";

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("node artifact I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("node artifact JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported {artifact} version {actual}; expected {expected}")]
    Version {
        artifact: &'static str,
        actual: String,
        expected: &'static str,
    },
    #[error("invalid Iroh endpoint identity {0}")]
    EndpointIdentity(String),
    #[error("invalid node secret key encoding")]
    SecretKey,
    #[error("node secret key does not produce declared endpoint identity {declared}")]
    IdentityMismatch { declared: String },
    #[error("endpoint ticket contains no direct IP address")]
    MissingAddress,
    #[error("peer {peer} is not authorized as role {role} for Colloq Plan {plan}")]
    Unauthorized {
        peer: String,
        role: String,
        plan: String,
    },
    #[error("authorization policy has no peers for role {role} and Colloq Plan {plan}")]
    NoAuthorizedPeers { role: String, plan: String },
}

/// Local-only Ed25519 identity material. Never send this artifact to a peer.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIdentity {
    pub colloq_node: String,
    pub endpoint_identity: String,
    pub secret_key: String,
}

impl std::fmt::Debug for NodeIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NodeIdentity")
            .field("colloq_node", &self.colloq_node)
            .field("endpoint_identity", &self.endpoint_identity)
            .field("secret_key", &"[redacted]")
            .finish()
    }
}

impl NodeIdentity {
    pub fn generate() -> Self {
        Self::from_secret_key(&SecretKey::generate())
    }

    pub fn from_secret_key(secret_key: &SecretKey) -> Self {
        Self {
            colloq_node: NODE_IDENTITY_FORMAT.to_string(),
            endpoint_identity: secret_key.public().to_string(),
            secret_key: encode_hex(&secret_key.to_bytes()),
        }
    }

    pub fn load(path: &Path) -> Result<Self, NodeError> {
        let identity: Self = serde_json::from_slice(&fs::read(path)?)?;
        identity.secret_key()?;
        Ok(identity)
    }

    /// Persist private key material with owner-only permissions on Unix.
    pub fn save_private(&self, path: &Path) -> Result<(), NodeError> {
        self.secret_key()?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn endpoint_id(&self) -> Result<EndpointId, NodeError> {
        self.secret_key().map(|secret| secret.public())
    }

    pub fn secret_key(&self) -> Result<SecretKey, NodeError> {
        require_version(
            "Colloq node identity",
            &self.colloq_node,
            NODE_IDENTITY_FORMAT,
        )?;
        let bytes = decode_32_bytes(&self.secret_key)?;
        let secret = SecretKey::from_bytes(&bytes);
        if secret.public().to_string() != self.endpoint_identity {
            return Err(NodeError::IdentityMismatch {
                declared: self.endpoint_identity.clone(),
            });
        }
        Ok(secret)
    }
}

/// Public, direct-only addressing information safe to give to another node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointTicket {
    pub colloq_endpoint: String,
    pub endpoint_identity: String,
    pub addresses: Vec<SocketAddr>,
}

impl EndpointTicket {
    pub fn from_addr(address: &EndpointAddr) -> Self {
        Self {
            colloq_endpoint: ENDPOINT_TICKET_FORMAT.to_string(),
            endpoint_identity: address.id.to_string(),
            addresses: address.ip_addrs().copied().collect(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, NodeError> {
        let ticket: Self = serde_json::from_slice(&fs::read(path)?)?;
        ticket.endpoint_addr()?;
        Ok(ticket)
    }

    pub fn save(&self, path: &Path) -> Result<(), NodeError> {
        self.endpoint_addr()?;
        write_public_json(path, self)
    }

    pub fn endpoint_id(&self) -> Result<EndpointId, NodeError> {
        parse_endpoint_id(&self.endpoint_identity)
    }

    pub fn endpoint_addr(&self) -> Result<EndpointAddr, NodeError> {
        require_version(
            "Colloq endpoint ticket",
            &self.colloq_endpoint,
            ENDPOINT_TICKET_FORMAT,
        )?;
        if self.addresses.is_empty() {
            return Err(NodeError::MissingAddress);
        }
        Ok(EndpointAddr::new(self.endpoint_id()?)
            .with_addrs(self.addresses.iter().copied().map(TransportAddr::Ip)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationRule {
    pub endpoint_identity: String,
    pub role: String,
    pub plan: String,
}

/// Local allow-list mapping authenticated transport identities onto exact Colloq authorities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationPolicy {
    pub colloq_authorization: String,
    pub rules: Vec<AuthorizationRule>,
}

impl Default for AuthorizationPolicy {
    fn default() -> Self {
        Self {
            colloq_authorization: AUTHORIZATION_FORMAT.to_string(),
            rules: Vec::new(),
        }
    }
}

impl AuthorizationPolicy {
    pub fn load(path: &Path) -> Result<Self, NodeError> {
        let policy: Self = serde_json::from_slice(&fs::read(path)?)?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn save(&self, path: &Path) -> Result<(), NodeError> {
        self.validate()?;
        write_public_json(path, self)
    }

    pub fn allow(&mut self, endpoint: EndpointId, role: &str, plan: &PreparedPlan) {
        let exists = self.rules.iter().any(|rule| {
            rule.endpoint_identity == endpoint.to_string()
                && rule.role == role
                && rule.plan == plan.plan_identity()
        });
        if !exists {
            self.rules.push(AuthorizationRule {
                endpoint_identity: endpoint.to_string(),
                role: role.to_string(),
                plan: plan.plan_identity().to_string(),
            });
            self.rules.sort_by(|left, right| {
                (&left.endpoint_identity, &left.role, &left.plan).cmp(&(
                    &right.endpoint_identity,
                    &right.role,
                    &right.plan,
                ))
            });
        }
    }

    pub fn authorize(
        &self,
        endpoint: EndpointId,
        role: &str,
        plan: &PreparedPlan,
    ) -> Result<(), NodeError> {
        self.validate()?;
        let allowed = self.rules.iter().any(|rule| {
            rule.endpoint_identity == endpoint.to_string()
                && rule.role == role
                && rule.plan == plan.plan_identity()
        });
        if allowed {
            Ok(())
        } else {
            Err(NodeError::Unauthorized {
                peer: endpoint.to_string(),
                role: role.to_string(),
                plan: plan.plan_identity().to_string(),
            })
        }
    }

    pub fn authorized_peers(
        &self,
        role: &str,
        plan: &PreparedPlan,
    ) -> Result<Vec<EndpointId>, NodeError> {
        self.validate()?;
        let peers = self
            .rules
            .iter()
            .filter(|rule| rule.role == role && rule.plan == plan.plan_identity())
            .map(|rule| parse_endpoint_id(&rule.endpoint_identity))
            .collect::<Result<Vec<_>, _>>()?;
        if peers.is_empty() {
            return Err(NodeError::NoAuthorizedPeers {
                role: role.to_string(),
                plan: plan.plan_identity().to_string(),
            });
        }
        Ok(peers)
    }

    fn validate(&self) -> Result<(), NodeError> {
        require_version(
            "Colloq authorization policy",
            &self.colloq_authorization,
            AUTHORIZATION_FORMAT,
        )?;
        for rule in &self.rules {
            parse_endpoint_id(&rule.endpoint_identity)?;
            if rule.role.is_empty() || rule.plan.is_empty() {
                return Err(NodeError::Unauthorized {
                    peer: rule.endpoint_identity.clone(),
                    role: rule.role.clone(),
                    plan: rule.plan.clone(),
                });
            }
        }
        Ok(())
    }
}

fn parse_endpoint_id(value: &str) -> Result<EndpointId, NodeError> {
    EndpointId::from_str(value).map_err(|_| NodeError::EndpointIdentity(value.to_string()))
}

fn write_public_json(path: &Path, value: &impl Serialize) -> Result<(), NodeError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn require_version(
    artifact: &'static str,
    actual: &str,
    expected: &'static str,
) -> Result<(), NodeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(NodeError::Version {
            artifact,
            actual: actual.to_string(),
            expected,
        })
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing into a string cannot fail");
    }
    encoded
}

fn decode_32_bytes(encoded: &str) -> Result<[u8; 32], NodeError> {
    if encoded.len() != 64 || !encoded.is_ascii() {
        return Err(NodeError::SecretKey);
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| NodeError::SecretKey)?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| NodeError::SecretKey)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Conversation;

    fn plan() -> PreparedPlan {
        let conversation: Conversation =
            serde_json::from_str(include_str!("../examples/generate.colloqconv.json")).unwrap();
        PreparedPlan::compile(&conversation).unwrap()
    }

    #[test]
    fn node_identity_round_trips_without_debug_secret_disclosure() {
        let identity = NodeIdentity::generate();
        assert_eq!(
            identity.endpoint_id().unwrap().to_string(),
            identity.endpoint_identity
        );
        let debug = format!("{identity:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains(&identity.secret_key));
    }

    #[test]
    fn modified_identity_is_rejected() {
        let mut identity = NodeIdentity::generate();
        identity.endpoint_identity = SecretKey::generate().public().to_string();
        assert!(matches!(
            identity.secret_key(),
            Err(NodeError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn endpoint_ticket_round_trips_direct_addresses() {
        let id = SecretKey::generate().public();
        let address = EndpointAddr::new(id).with_ip_addr("127.0.0.1:7880".parse().unwrap());
        let ticket = EndpointTicket::from_addr(&address);
        assert_eq!(ticket.endpoint_addr().unwrap(), address);
    }

    #[test]
    fn policy_is_exact_on_peer_role_and_plan() {
        let plan = plan();
        let peer = SecretKey::generate().public();
        let other = SecretKey::generate().public();
        let mut policy = AuthorizationPolicy::default();
        policy.allow(peer, "client", &plan);
        assert!(policy.authorize(peer, "client", &plan).is_ok());
        assert!(matches!(
            policy.authorize(peer, "server", &plan),
            Err(NodeError::Unauthorized { .. })
        ));
        assert!(matches!(
            policy.authorize(other, "client", &plan),
            Err(NodeError::Unauthorized { .. })
        ));
    }

    #[test]
    fn policy_growth_does_not_create_cartesian_product_grants() {
        let client_plan = plan();
        let sync_conversation: Conversation =
            serde_json::from_str(include_str!("../examples/draft-sync.colloqconv.json")).unwrap();
        let server_plan = PreparedPlan::compile(&sync_conversation).unwrap();
        let peer = SecretKey::generate().public();
        let mut policy = AuthorizationPolicy::default();
        policy.allow(peer, "client", &client_plan);
        policy.allow(peer, "server", &server_plan);
        assert!(policy.authorize(peer, "client", &client_plan).is_ok());
        assert!(policy.authorize(peer, "server", &server_plan).is_ok());
        assert!(matches!(
            policy.authorize(peer, "server", &client_plan),
            Err(NodeError::Unauthorized { .. })
        ));
        assert!(matches!(
            policy.authorize(peer, "client", &server_plan),
            Err(NodeError::Unauthorized { .. })
        ));
    }
}

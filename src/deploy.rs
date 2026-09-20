//! Deployment adapters. These translate validated Colloq semantics into infrastructure artifacts;
//! they do not become part of the language's meaning.

use crate::Conversation;
use crate::plan::{ColloqPlan, PlanError};
use crate::runtime::WireEncoding;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const MIREN_DOCKERFILE: &str = ".miren/Dockerfile.colloq";

#[derive(Debug, Error)]
pub enum DeploymentError {
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error("Miren serialization error: {0}")]
    Toml(#[from] toml::ser::Error),
    #[error("invalid Miren deployment option: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct MirenOptions {
    pub app_name: String,
    pub conversation_source: PathBuf,
    pub port: u16,
    pub tokens: usize,
    pub instances: usize,
    pub wire: WireEncoding,
    pub transport: MirenTransport,
    /// Expose the raw Colloq TCP or Iroh/UDP port on the Miren node.
    pub node_port: bool,
}

impl MirenOptions {
    pub fn for_conversation(conversation: &Conversation, source: impl Into<PathBuf>) -> Self {
        Self {
            app_name: format!("colloq-{}", miren_slug(&conversation.module.id)),
            conversation_source: source.into(),
            port: 7878,
            tokens: 3,
            instances: 1,
            wire: WireEncoding::Compact,
            transport: MirenTransport::Tcp,
            node_port: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirenTransport {
    Tcp,
    Iroh,
}

#[derive(Debug, Clone)]
pub struct MirenBundle {
    pub app_toml: String,
    pub dockerfile: String,
    pub dockerfile_path: &'static str,
    pub plan_identity: String,
    pub conversation_identity: String,
}

/// Render a self-contained Miren adapter for the current executable Generate runtime.
///
/// Miren provides build, placement, restart, and L4 forwarding. Colloq still validates the
/// conversation and owns plan/session identities. The TCP mode is an unauthenticated correctness
/// testbed. The Iroh mode exposes UDP and requires persistent identity, exact authorization, and
/// an advertised routable address through deployment configuration.
pub fn render_miren_bundle(
    conversation: &Conversation,
    options: &MirenOptions,
) -> Result<MirenBundle, DeploymentError> {
    validate_options(conversation, options)?;
    let plan = ColloqPlan::compile(conversation)?;
    let wire = match options.wire {
        WireEncoding::Reference => "reference",
        WireEncoding::Compact => "compact",
    };
    let source = options
        .conversation_source
        .to_str()
        .ok_or_else(|| DeploymentError::Invalid("conversation path must be UTF-8".to_string()))?;
    let command = match options.transport {
        MirenTransport::Tcp => format!(
            "/bin/colloq serve /etc/colloq/conversation.json --listen 0.0.0.0:{} --tokens {} --wire {} --forever",
            options.port, options.tokens, wire
        ),
        MirenTransport::Iroh => format!(
            "/bin/colloq serve-iroh /etc/colloq/conversation.json --identity /run/colloq/server.colloqnode.json --policy /run/colloq/server.authorization.json --listen 0.0.0.0:{} --ticket-out /tmp/server.colloqendpoint.json --tokens {} --wire {}",
            options.port, options.tokens, wire
        ),
    };

    let mut env = vec![
        MirenEnv::value("COLLOQ_PLAN_IDENTITY", &plan.plan_identity),
        MirenEnv::value("COLLOQ_CONVERSATION_IDENTITY", &plan.conversation_identity),
    ];
    if options.transport == MirenTransport::Iroh {
        env.extend([
            MirenEnv::required(
                "COLLOQ_NODE_IDENTITY_JSON",
                true,
                "Local Colloq node identity JSON; contains the Iroh private key.",
            ),
            MirenEnv::required(
                "COLLOQ_AUTHORIZATION_JSON",
                true,
                "Exact peer/role/plan authorization policy JSON.",
            ),
            MirenEnv::required(
                "COLLOQ_ADVERTISE_ADDRESS",
                false,
                "Public or overlay socket address advertised in the endpoint ticket.",
            ),
        ]);
    }

    let app = MirenApp {
        name: options.app_name.clone(),
        build: MirenBuild {
            dockerfile: MIREN_DOCKERFILE.to_string(),
        },
        env,
        services: BTreeMap::from([(
            "server".to_string(),
            MirenService {
                command,
                ports: vec![MirenPort {
                    port: options.port,
                    name: "colloq".to_string(),
                    protocol: match options.transport {
                        MirenTransport::Tcp => "tcp",
                        MirenTransport::Iroh => "udp",
                    }
                    .to_string(),
                    node_port: options.node_port.then_some(options.port),
                }],
                concurrency: MirenConcurrency {
                    mode: "fixed".to_string(),
                    num_instances: options.instances,
                    shutdown_timeout: "10s".to_string(),
                },
            },
        )]),
    };

    let dockerfile = format!(
        "FROM rust:1.96-bookworm AS build\nWORKDIR /src\nCOPY Cargo.toml Cargo.lock ./\nCOPY src ./src\nRUN cargo build --release --locked\n\nFROM debian:bookworm-slim\nRUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*\nCOPY --from=build /src/target/release/colloq /bin/colloq\nCOPY {source} /etc/colloq/conversation.json\nCOPY examples/draft-sync.colloqconv.json /etc/colloq/draft-sync.colloqconv.json\nUSER 65532:65532\n"
    );

    Ok(MirenBundle {
        app_toml: toml::to_string_pretty(&app)?,
        dockerfile,
        dockerfile_path: MIREN_DOCKERFILE,
        plan_identity: plan.plan_identity,
        conversation_identity: plan.conversation_identity,
    })
}

fn validate_options(
    conversation: &Conversation,
    options: &MirenOptions,
) -> Result<(), DeploymentError> {
    if !valid_miren_name(&options.app_name) {
        return Err(DeploymentError::Invalid(format!(
            "app name {:?} must contain lowercase letters, numbers, or dashes",
            options.app_name
        )));
    }
    if options.port == 0 {
        return Err(DeploymentError::Invalid(
            "the Colloq service port cannot be zero".to_string(),
        ));
    }
    if options.instances == 0 {
        return Err(DeploymentError::Invalid(
            "Miren requires at least one fixed service instance".to_string(),
        ));
    }
    if !conversation.roles.iter().any(|role| role.id == "client")
        || !conversation.roles.iter().any(|role| role.id == "server")
    {
        return Err(DeploymentError::Invalid(
            "the current deployable reference runtime requires client and server roles".to_string(),
        ));
    }
    validate_build_source(&options.conversation_source)
}

fn validate_build_source(path: &Path) -> Result<(), DeploymentError> {
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err(DeploymentError::Invalid(
            "conversation source must be a non-empty path relative to the repository".to_string(),
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(value)
                if value.to_str().is_some_and(|value| {
                    !value.is_empty()
                        && value.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                        })
                }) => {}
            _ => {
                return Err(DeploymentError::Invalid(format!(
                    "conversation source {:?} contains a component unsafe for Docker COPY",
                    path
                )));
            }
        }
    }
    Ok(())
}

fn valid_miren_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn miren_slug(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' {
                byte as char
            } else {
                '-'
            }
        })
        .collect()
}

#[derive(Serialize)]
struct MirenApp {
    name: String,
    build: MirenBuild,
    env: Vec<MirenEnv>,
    services: BTreeMap<String, MirenService>,
}

#[derive(Serialize)]
struct MirenBuild {
    dockerfile: String,
}

#[derive(Serialize)]
struct MirenEnv {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensitive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl MirenEnv {
    fn value(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: Some(value.to_string()),
            required: None,
            sensitive: None,
            description: None,
        }
    }

    fn required(key: &str, sensitive: bool, description: &str) -> Self {
        Self {
            key: key.to_string(),
            value: None,
            required: Some(true),
            sensitive: Some(sensitive),
            description: Some(description.to_string()),
        }
    }
}

#[derive(Serialize)]
struct MirenService {
    command: String,
    ports: Vec<MirenPort>,
    concurrency: MirenConcurrency,
}

#[derive(Serialize)]
struct MirenPort {
    port: u16,
    name: String,
    #[serde(rename = "type")]
    protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_port: Option<u16>,
}

#[derive(Serialize)]
struct MirenConcurrency {
    mode: String,
    num_instances: usize,
    shutdown_timeout: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conversation() -> Conversation {
        serde_json::from_str(include_str!("../examples/generate.colloqconv.json")).unwrap()
    }

    #[test]
    fn miren_bundle_is_parseable_and_plan_bound() {
        let conversation = conversation();
        let options =
            MirenOptions::for_conversation(&conversation, "examples/generate.colloqconv.json");
        let bundle = render_miren_bundle(&conversation, &options).unwrap();
        let manifest: toml::Value = toml::from_str(&bundle.app_toml).unwrap();
        assert_eq!(manifest["name"].as_str(), Some("colloq-example-generate"));
        assert_eq!(
            manifest["services"]["server"]["ports"][0]["type"].as_str(),
            Some("tcp")
        );
        assert!(bundle.app_toml.contains(&bundle.plan_identity));
        assert!(bundle.app_toml.contains(&bundle.conversation_identity));
        assert!(bundle.dockerfile.contains("FROM rust:1.96-bookworm"));
        assert!(
            bundle
                .dockerfile
                .contains("COPY examples/generate.colloqconv.json /etc/colloq/conversation.json")
        );
    }

    #[test]
    fn miren_bundle_rejects_a_source_outside_the_build_context() {
        let conversation = conversation();
        let options = MirenOptions::for_conversation(&conversation, "../secret.json");
        assert!(matches!(
            render_miren_bundle(&conversation, &options),
            Err(DeploymentError::Invalid(_))
        ));
    }

    #[test]
    fn miren_iroh_bundle_exposes_udp_and_requires_private_configuration() {
        let conversation = conversation();
        let mut options =
            MirenOptions::for_conversation(&conversation, "examples/generate.colloqconv.json");
        options.transport = MirenTransport::Iroh;
        let bundle = render_miren_bundle(&conversation, &options).unwrap();
        let manifest: toml::Value = toml::from_str(&bundle.app_toml).unwrap();
        assert_eq!(
            manifest["services"]["server"]["ports"][0]["type"].as_str(),
            Some("udp")
        );
        assert!(bundle.app_toml.contains("COLLOQ_NODE_IDENTITY_JSON"));
        assert!(bundle.app_toml.contains("sensitive = true"));
        assert!(bundle.app_toml.contains("COLLOQ_ADVERTISE_ADDRESS"));
        assert!(bundle.app_toml.contains("serve-iroh"));
    }
}

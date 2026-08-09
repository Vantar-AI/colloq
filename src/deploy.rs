//! Deployment adapters. These translate validated Eve semantics into infrastructure artifacts;
//! they do not become part of the language's meaning.

use crate::Conversation;
use crate::plan::{EvePlan, PlanError};
use crate::runtime::WireEncoding;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const MIREN_DOCKERFILE: &str = ".miren/Dockerfile.eve";

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
    /// Expose the raw Eve TCP port on the Miren node for a multi-server testbed.
    pub node_port: bool,
}

impl MirenOptions {
    pub fn for_conversation(conversation: &Conversation, source: impl Into<PathBuf>) -> Self {
        Self {
            app_name: format!("eve-{}", miren_slug(&conversation.module.id)),
            conversation_source: source.into(),
            port: 7878,
            tokens: 3,
            instances: 1,
            wire: WireEncoding::Compact,
            node_port: true,
        }
    }
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
/// Miren provides build, placement, restart, and overlay networking. Eve still validates the
/// conversation and owns the plan/session identities. The generated service uses the existing
/// TCP transport because Miren's WireGuard overlay already supplies routable cluster addresses;
/// Iroh remains an independently selectable authenticated Eve transport.
pub fn render_miren_bundle(
    conversation: &Conversation,
    options: &MirenOptions,
) -> Result<MirenBundle, DeploymentError> {
    validate_options(conversation, options)?;
    let plan = EvePlan::compile(conversation)?;
    let wire = match options.wire {
        WireEncoding::Reference => "reference",
        WireEncoding::Compact => "compact",
    };
    let source = options
        .conversation_source
        .to_str()
        .ok_or_else(|| DeploymentError::Invalid("conversation path must be UTF-8".to_string()))?;
    let command = format!(
        "/bin/eve serve /etc/eve/conversation.json --listen 0.0.0.0:{} --tokens {} --wire {} --forever",
        options.port, options.tokens, wire
    );

    let app = MirenApp {
        name: options.app_name.clone(),
        build: MirenBuild {
            dockerfile: MIREN_DOCKERFILE.to_string(),
        },
        env: vec![
            MirenEnv {
                key: "EVE_PLAN_IDENTITY".to_string(),
                value: plan.plan_identity.clone(),
            },
            MirenEnv {
                key: "EVE_CONVERSATION_IDENTITY".to_string(),
                value: plan.conversation_identity.clone(),
            },
        ],
        services: BTreeMap::from([(
            "server".to_string(),
            MirenService {
                command,
                ports: vec![MirenPort {
                    port: options.port,
                    name: "eve".to_string(),
                    protocol: "tcp".to_string(),
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
        "FROM rust:1.96-bookworm AS build\nWORKDIR /src\nCOPY Cargo.toml Cargo.lock ./\nCOPY src ./src\nRUN cargo build --release --locked\n\nFROM debian:bookworm-slim\nRUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*\nCOPY --from=build /src/target/release/eve /bin/eve\nCOPY {source} /etc/eve/conversation.json\nUSER 65532:65532\n"
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
            "the Eve service port cannot be zero".to_string(),
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
    value: String,
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
        serde_json::from_str(include_str!("../examples/generate.eveconv.json")).unwrap()
    }

    #[test]
    fn miren_bundle_is_parseable_and_plan_bound() {
        let conversation = conversation();
        let options =
            MirenOptions::for_conversation(&conversation, "examples/generate.eveconv.json");
        let bundle = render_miren_bundle(&conversation, &options).unwrap();
        let manifest: toml::Value = toml::from_str(&bundle.app_toml).unwrap();
        assert_eq!(manifest["name"].as_str(), Some("eve-example-generate"));
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
                .contains("COPY examples/generate.eveconv.json /etc/eve/conversation.json")
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
}

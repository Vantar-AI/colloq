use clap::{Parser, Subcommand, ValueEnum};
use eve::benchmark::run_reference_benchmark;
use eve::deploy::{MirenOptions, MirenTransport, render_miren_bundle};
use eve::draft_exchange::{run_draft_sync_client, run_draft_sync_server};
use eve::graph::{AutomergeDraft, DraftScalarPatch};
use eve::node::{AuthorizationPolicy, EndpointTicket, NodeIdentity};
use eve::plan::{EvePlan, PreparedPlan};
use eve::runtime::{
    ExecutionReport, FaultOperation, FaultPlan, IrohNode, QuicListener, QuicTransport,
    TcpTransport, WireEncoding, run_generate_client_plan, run_generate_server_plan, run_iroh_demo,
    run_iroh_plan_demo_with_encoding, run_memory_demo, run_memory_fault_demo,
    run_memory_plan_demo_with_encoding, run_quic_demo, run_quic_plan_demo_with_encoding,
    run_tcp_demo, run_tcp_plan_demo_with_encoding,
};
use eve::{Conversation, Frame, project, validate, verify_trace};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Parser)]
#[command(
    name = "eve",
    version,
    about = "Experimental compiler for graph-native server conversations"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate one persistent local Iroh identity (private file, mode 0600 on Unix).
    NodeInit {
        #[arg(long, default_value = "build/node.evenode.json")]
        out: PathBuf,
    },
    /// Add one exact peer/role/plan grant to a local authorization policy.
    PolicyAllow {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "build/eve.authorization.json")]
        policy: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        role: String,
    },
    /// Create two persistent identities and reciprocal Generate + draft-sync policies.
    BootstrapTwoNode {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "examples/draft-sync.eveconv.json")]
        draft_conversation: PathBuf,
        #[arg(long, default_value = "build/two-node")]
        out: PathBuf,
    },
    /// Validate a global Eve conversation.
    Check {
        conversation: PathBuf,
        /// Emit validation diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Project a global conversation into one endpoint machine per role.
    Project {
        conversation: PathBuf,
        #[arg(long, default_value = "build/endpoints")]
        out: PathBuf,
    },
    /// Compile one conversation into a reusable, verified Eve Plan.
    Compile {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "build/generate.eveplan.json")]
        out: PathBuf,
    },
    /// Create an Automerge-backed collaborative draft from a validated Eve conversation.
    DraftCreate {
        conversation: PathBuf,
        #[arg(long, default_value = "build/generate.evedraft")]
        out: PathBuf,
    },
    /// Apply one optimistic scalar JSON-pointer edit to an Automerge draft.
    DraftPatch {
        draft: PathBuf,
        /// JSON pointer such as /module/semantic_version.
        #[arg(long)]
        pointer: String,
        /// JSON scalar, for example '"0.2.0"', true, or 10.
        #[arg(long)]
        value: String,
        /// Write to another draft file instead of updating the input file.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Reject conflicts, validate the materialized graph, and emit canonical Eve artifacts.
    DraftPromote {
        draft: PathBuf,
        #[arg(long, default_value = "build/promoted.eveconv.json")]
        conversation_out: PathBuf,
        #[arg(long, default_value = "build/promoted.eveplan.json")]
        plan_out: PathBuf,
    },
    /// Generate a Miren app manifest and pinned Rust container for an Eve server.
    EmitMiren {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long)]
        app_name: Option<String>,
        #[arg(long, default_value_t = 7878)]
        port: u16,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        #[arg(long, default_value_t = 1)]
        instances: usize,
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Compact)]
        wire: WireEncodingArg,
        /// Deploy the plaintext TCP testbed or mutually authenticated Iroh over UDP.
        #[arg(long, value_enum, default_value_t = DeploymentTransportArg::Tcp)]
        transport: DeploymentTransportArg,
        /// Keep the service internal to Miren instead of allocating a node port.
        #[arg(long)]
        internal_only: bool,
    },
    /// Execute a previously compiled Eve Plan without re-projecting the conversation.
    RunPlan {
        #[arg(default_value = "build/generate.eveplan.json")]
        plan: PathBuf,
        #[arg(long, value_enum, default_value_t = DemoTransport::Memory)]
        transport: DemoTransport,
        /// Use self-describing envelopes or compact transition IDs from the plan.
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Reference)]
        wire: WireEncodingArg,
        #[arg(
            long,
            default_value = "Explain why the conversation is the computation."
        )]
        prompt: String,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        #[arg(long)]
        cancel_after: Option<usize>,
    },
    /// Verify that a frame trace follows the global conversation.
    VerifyTrace {
        conversation: PathBuf,
        trace: PathBuf,
    },
    /// Run both projected endpoints using one selectable transport plan.
    Demo {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, value_enum, default_value_t = DemoTransport::Memory)]
        transport: DemoTransport,
        #[arg(
            long,
            default_value = "Explain why the conversation is the computation."
        )]
        prompt: String,
        /// Maximum number of tokens the reference server will emit.
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        /// Ask the client to cancel after receiving this many tokens.
        #[arg(long)]
        cancel_after: Option<usize>,
    },
    /// Run the memory transport with one deterministic typed transport failure.
    FaultDemo {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "Exercise typed transport failure.")]
        prompt: String,
        /// Maximum number of tokens the reference server will attempt to emit.
        #[arg(long, default_value_t = 5)]
        tokens: usize,
        /// Ask the client to cancel after receiving this many tokens.
        #[arg(long)]
        cancel_after: Option<usize>,
        /// Endpoint whose transport operation should fail.
        #[arg(long, default_value = "server")]
        fault_role: String,
        /// Transport operation whose selected occurrence should fail.
        #[arg(long, value_enum, default_value_t = FaultOperationArg::Send)]
        fault_operation: FaultOperationArg,
        /// One-based occurrence of the selected operation to fail.
        #[arg(long, default_value_t = 2)]
        fault_at: usize,
        /// Declared Eve failure ID to observe.
        #[arg(long, default_value = "transport.closed")]
        failure: String,
        /// Failure observed by the peer after the injected side aborts.
        #[arg(long)]
        peer_failure: Option<String>,
    },
    /// Compare the Eve reference memory runtime with a hand-written JSON baseline.
    Benchmark {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "Measure the conversation runtime.")]
        prompt: String,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        #[arg(long, default_value_t = 200)]
        iterations: usize,
        #[arg(long, default_value_t = 20)]
        warmup: usize,
    },
    /// Serve a projected endpoint over TCP.
    Serve {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7878")]
        listen: SocketAddr,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        /// Bind the session to the reference or compact wire encoding.
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Reference)]
        wire: WireEncodingArg,
        /// Continue accepting conversations instead of exiting after the first session.
        #[arg(long)]
        forever: bool,
    },
    /// Connect the client endpoint to an Eve TCP server.
    Connect {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7878")]
        server: SocketAddr,
        #[arg(
            long,
            default_value = "Explain why the conversation is the computation."
        )]
        prompt: String,
        #[arg(long)]
        cancel_after: Option<usize>,
        /// Require exact agreement on the reference or compact wire encoding.
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Reference)]
        wire: WireEncodingArg,
    },
    /// Serve one projected endpoint over authenticated QUIC.
    ServeQuic {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7879")]
        listen: SocketAddr,
        /// Write the generated public certificate here for the client to pin.
        #[arg(long, default_value = "build/eve-quic-cert.der")]
        certificate_out: PathBuf,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        /// Bind the TLS-authenticated session to this wire encoding.
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Reference)]
        wire: WireEncodingArg,
    },
    /// Connect the client endpoint over QUIC using a pinned server certificate.
    ConnectQuic {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7879")]
        server: SocketAddr,
        #[arg(long, default_value = "build/eve-quic-cert.der")]
        certificate: PathBuf,
        #[arg(
            long,
            default_value = "Explain why the conversation is the computation."
        )]
        prompt: String,
        #[arg(long)]
        cancel_after: Option<usize>,
        /// Require the authenticated server to use this exact wire encoding.
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Reference)]
        wire: WireEncodingArg,
    },
    /// Serve a Generate endpoint as a separate, mutually authenticated Iroh process.
    ServeIroh {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "build/two-node/server.evenode.json")]
        identity: PathBuf,
        #[arg(long, default_value = "build/two-node/server.authorization.json")]
        policy: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7880")]
        listen: SocketAddr,
        /// Publish this routable address instead of the bind address.
        #[arg(long)]
        advertise: Option<SocketAddr>,
        #[arg(long, default_value = "build/two-node/server.eveendpoint.json")]
        ticket_out: PathBuf,
        #[arg(long, default_value_t = 3)]
        tokens: usize,
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Compact)]
        wire: WireEncodingArg,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    /// Connect a Generate endpoint to an authorized Iroh server ticket.
    ConnectIroh {
        #[arg(default_value = "examples/generate.eveconv.json")]
        conversation: PathBuf,
        #[arg(long, default_value = "build/two-node/client.evenode.json")]
        identity: PathBuf,
        #[arg(long, default_value = "build/two-node/client.authorization.json")]
        policy: PathBuf,
        #[arg(long, default_value = "build/two-node/server.eveendpoint.json")]
        server: PathBuf,
        #[arg(
            long,
            default_value = "Explain why the conversation is the computation."
        )]
        prompt: String,
        #[arg(long)]
        cancel_after: Option<usize>,
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Compact)]
        wire: WireEncodingArg,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    /// Serve an Automerge draft sync endpoint over an authorized Eve/Iroh session.
    DraftServeIroh {
        #[arg(default_value = "examples/draft-sync.eveconv.json")]
        conversation: PathBuf,
        #[arg(long)]
        draft: PathBuf,
        #[arg(long, default_value = "build/two-node/server.evenode.json")]
        identity: PathBuf,
        #[arg(long, default_value = "build/two-node/server.authorization.json")]
        policy: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7881")]
        listen: SocketAddr,
        #[arg(long)]
        advertise: Option<SocketAddr>,
        #[arg(long, default_value = "build/two-node/draft-server.eveendpoint.json")]
        ticket_out: PathBuf,
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Compact)]
        wire: WireEncodingArg,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    /// Synchronize a local Automerge draft with an authorized Eve/Iroh server.
    DraftConnectIroh {
        #[arg(default_value = "examples/draft-sync.eveconv.json")]
        conversation: PathBuf,
        #[arg(long)]
        draft: PathBuf,
        #[arg(long, default_value = "build/two-node/client.evenode.json")]
        identity: PathBuf,
        #[arg(long, default_value = "build/two-node/client.authorization.json")]
        policy: PathBuf,
        #[arg(long, default_value = "build/two-node/draft-server.eveendpoint.json")]
        server: PathBuf,
        #[arg(long, value_enum, default_value_t = WireEncodingArg::Compact)]
        wire: WireEncodingArg,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    /// Verify that independently captured client/server reports describe one Eve session.
    VerifySession {
        #[arg(long)]
        client: PathBuf,
        #[arg(long)]
        server: PathBuf,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum DemoTransport {
    Memory,
    Tcp,
    Quic,
    Iroh,
}

#[derive(Clone, Debug, ValueEnum)]
enum FaultOperationArg {
    Send,
    Receive,
}

#[derive(Clone, Debug, ValueEnum)]
enum WireEncodingArg {
    Reference,
    Compact,
}

#[derive(Clone, Debug, ValueEnum)]
enum DeploymentTransportArg {
    Tcp,
    Iroh,
}

impl From<DeploymentTransportArg> for MirenTransport {
    fn from(transport: DeploymentTransportArg) -> Self {
        match transport {
            DeploymentTransportArg::Tcp => Self::Tcp,
            DeploymentTransportArg::Iroh => Self::Iroh,
        }
    }
}

impl From<WireEncodingArg> for WireEncoding {
    fn from(encoding: WireEncodingArg) -> Self {
        match encoding {
            WireEncodingArg::Reference => Self::Reference,
            WireEncodingArg::Compact => Self::Compact,
        }
    }
}

impl From<FaultOperationArg> for FaultOperation {
    fn from(operation: FaultOperationArg) -> Self {
        match operation {
            FaultOperationArg::Send => Self::Send,
            FaultOperationArg::Receive => Self::Receive,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::NodeInit { out } => {
            ensure_absent(&out)?;
            let identity = NodeIdentity::generate();
            identity.save_private(&out)?;
            println!(
                "created node {} at {} (private, do not share)",
                identity.endpoint_identity,
                out.display()
            );
        }
        Command::PolicyAllow {
            conversation,
            policy,
            peer,
            role,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let peer = iroh::EndpointId::from_str(&peer)?;
            let mut authorization = if policy.exists() {
                AuthorizationPolicy::load(&policy)?
            } else {
                AuthorizationPolicy::default()
            };
            authorization.allow(peer, &role, &plan);
            authorization.save(&policy)?;
            println!(
                "authorized {peer} as {role} for {} in {}",
                plan.plan_identity(),
                policy.display()
            );
        }
        Command::BootstrapTwoNode {
            conversation,
            draft_conversation,
            out,
        } => {
            let generate: Conversation = read_json(&conversation)?;
            let generate = PreparedPlan::compile(&generate)?;
            let draft: Conversation = read_json(&draft_conversation)?;
            let draft = PreparedPlan::compile(&draft)?;
            let server_path = out.join("server.evenode.json");
            let client_path = out.join("client.evenode.json");
            let server_policy_path = out.join("server.authorization.json");
            let client_policy_path = out.join("client.authorization.json");
            for path in [
                &server_path,
                &client_path,
                &server_policy_path,
                &client_policy_path,
            ] {
                ensure_absent(path)?;
            }
            let server = NodeIdentity::generate();
            let client = NodeIdentity::generate();
            let server_id = server.endpoint_id()?;
            let client_id = client.endpoint_id()?;
            let mut server_policy = AuthorizationPolicy::default();
            server_policy.allow(client_id, "client", &generate);
            server_policy.allow(client_id, "client", &draft);
            let mut client_policy = AuthorizationPolicy::default();
            client_policy.allow(server_id, "server", &generate);
            client_policy.allow(server_id, "server", &draft);
            server.save_private(&server_path)?;
            client.save_private(&client_path)?;
            server_policy.save(&server_policy_path)?;
            client_policy.save(&client_policy_path)?;
            println!(
                "created two-node trust domain in {}\nserver: {}\nclient: {}",
                out.display(),
                server.endpoint_identity,
                client.endpoint_identity
            );
        }
        Command::Check { conversation, json } => {
            let conversation: Conversation = read_json(&conversation)?;
            match validate(&conversation) {
                Ok(()) => println!(
                    "valid conversation {} ({} roles, {} states)",
                    conversation.module.id,
                    conversation.roles.len(),
                    conversation.states.len()
                ),
                Err(errors) if json => {
                    println!("{}", serde_json::to_string_pretty(&errors)?);
                    std::process::exit(1);
                }
                Err(errors) => return Err(Box::new(errors)),
            }
        }
        Command::Project { conversation, out } => {
            let conversation: Conversation = read_json(&conversation)?;
            let endpoints = project(&conversation)?;
            fs::create_dir_all(&out)?;
            for endpoint in endpoints {
                let path = out.join(format!("{}.endpoint.json", endpoint.role));
                fs::write(&path, serde_json::to_vec_pretty(&endpoint)?)?;
                println!("wrote {}", path.display());
            }
        }
        Command::Compile { conversation, out } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = EvePlan::compile(&conversation)?;
            if let Some(parent) = out.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, serde_json::to_vec_pretty(&plan)?)?;
            println!(
                "compiled {} as {} ({} endpoints, {} compact transitions) to {}",
                plan.conversation,
                plan.plan_identity,
                plan.endpoints.len(),
                plan.wire.as_ref().map_or(0, |wire| wire.transitions.len()),
                out.display()
            );
        }
        Command::DraftCreate { conversation, out } => {
            let conversation: Conversation = read_json(&conversation)?;
            validate(&conversation)?;
            let mut draft = AutomergeDraft::from_conversation(&conversation)?;
            write_bytes(&out, &draft.save())?;
            println!("created collaborative Eve draft at {}", out.display());
        }
        Command::DraftPatch {
            draft,
            pointer,
            value,
            out,
        } => {
            let bytes = fs::read(&draft)?;
            let mut document = AutomergeDraft::load(&bytes)?;
            let heads = document.heads();
            let value = serde_json::from_str(&value)?;
            document.apply_scalar_patch(
                &heads,
                &DraftScalarPatch {
                    path: pointer,
                    value,
                },
            )?;
            let out = out.unwrap_or(draft);
            write_bytes(&out, &document.save())?;
            println!("updated collaborative Eve draft at {}", out.display());
        }
        Command::DraftPromote {
            draft,
            conversation_out,
            plan_out,
        } => {
            let document = AutomergeDraft::load(&fs::read(&draft)?)?;
            let promoted = document.promote()?;
            write_bytes(
                &conversation_out,
                &serde_json::to_vec_pretty(&promoted.conversation)?,
            )?;
            write_bytes(&plan_out, &serde_json::to_vec_pretty(&promoted.plan)?)?;
            println!(
                "promoted {} as {} to {} and {}",
                promoted.conversation.module.id,
                promoted.conversation_identity,
                conversation_out.display(),
                plan_out.display()
            );
        }
        Command::EmitMiren {
            conversation,
            app_name,
            port,
            tokens,
            instances,
            wire,
            transport,
            internal_only,
        } => {
            let source = conversation.clone();
            let conversation: Conversation = read_json(&conversation)?;
            let mut options = MirenOptions::for_conversation(&conversation, source);
            if let Some(app_name) = app_name {
                options.app_name = app_name;
            }
            options.port = port;
            options.tokens = tokens;
            options.instances = instances;
            options.wire = wire.into();
            options.transport = transport.into();
            options.node_port = !internal_only;
            let bundle = render_miren_bundle(&conversation, &options)?;
            write_bytes(Path::new(".miren/app.toml"), bundle.app_toml.as_bytes())?;
            write_bytes(
                Path::new(bundle.dockerfile_path),
                bundle.dockerfile.as_bytes(),
            )?;
            println!(
                "wrote .miren/app.toml and {} for plan {}",
                bundle.dockerfile_path, bundle.plan_identity
            );
        }
        Command::RunPlan {
            plan,
            transport,
            wire,
            prompt,
            tokens,
            cancel_after,
        } => {
            let artifact: EvePlan = read_json(&plan)?;
            let plan = artifact.prepare()?;
            let wire = wire.into();
            let report = match transport {
                DemoTransport::Memory => {
                    run_memory_plan_demo_with_encoding(&plan, &prompt, tokens, cancel_after, wire)?
                }
                DemoTransport::Tcp => {
                    run_tcp_plan_demo_with_encoding(&plan, &prompt, tokens, cancel_after, wire)?
                }
                DemoTransport::Quic => {
                    run_quic_plan_demo_with_encoding(&plan, &prompt, tokens, cancel_after, wire)?
                }
                DemoTransport::Iroh => {
                    run_iroh_plan_demo_with_encoding(&plan, &prompt, tokens, cancel_after, wire)?
                }
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::VerifyTrace {
            conversation,
            trace,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let frames: Vec<Frame> = read_json(&trace)?;
            let report = verify_trace(&conversation, &frames)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.complete {
                eprintln!("trace is valid but does not reach a terminal state");
                std::process::exit(2);
            }
        }
        Command::Demo {
            conversation,
            transport,
            prompt,
            tokens,
            cancel_after,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let report = match transport {
                DemoTransport::Memory => {
                    run_memory_demo(&conversation, &prompt, tokens, cancel_after)?
                }
                DemoTransport::Tcp => run_tcp_demo(&conversation, &prompt, tokens, cancel_after)?,
                DemoTransport::Quic => run_quic_demo(&conversation, &prompt, tokens, cancel_after)?,
                DemoTransport::Iroh => run_iroh_demo(&conversation, &prompt, tokens, cancel_after)?,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::FaultDemo {
            conversation,
            prompt,
            tokens,
            cancel_after,
            fault_role,
            fault_operation,
            fault_at,
            failure,
            peer_failure,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let report = run_memory_fault_demo(
                &conversation,
                &prompt,
                tokens,
                cancel_after,
                FaultPlan {
                    role: fault_role,
                    operation: fault_operation.into(),
                    occurrence: fault_at,
                    failure,
                    peer_failure,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Benchmark {
            conversation,
            prompt,
            tokens,
            iterations,
            warmup,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let report =
                run_reference_benchmark(&conversation, &prompt, tokens, iterations, warmup)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Serve {
            conversation,
            listen,
            tokens,
            wire,
            forever,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            verify_deployment_identity(&plan)?;
            let wire = wire.into();
            let listener = TcpListener::bind(listen)?;
            println!("Eve server listening on {listen}");
            loop {
                let (stream, peer) = listener.accept()?;
                println!("accepted Eve endpoint {peer}");
                let mut transport = match wire {
                    WireEncoding::Reference => TcpTransport::from_stream(stream)?,
                    WireEncoding::Compact => TcpTransport::from_stream_compact(stream, &plan)?,
                };
                transport.establish_session(&plan, "server", "client")?;
                let report = run_generate_server_plan(&plan, &mut transport, tokens)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                if !forever {
                    break;
                }
            }
        }
        Command::Connect {
            conversation,
            server,
            prompt,
            cancel_after,
            wire,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let mut transport = match wire.into() {
                WireEncoding::Reference => TcpTransport::connect(server)?,
                WireEncoding::Compact => TcpTransport::connect_compact(server, &plan)?,
            };
            transport.establish_session(&plan, "client", "server")?;
            let report = run_generate_client_plan(&plan, &mut transport, &prompt, cancel_after)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ServeQuic {
            conversation,
            listen,
            certificate_out,
            tokens,
            wire,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let listener = QuicListener::bind(listen)?;
            if let Some(parent) = certificate_out.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(&certificate_out, listener.certificate_der())?;
            println!(
                "Eve QUIC server listening on {} (certificate: {})",
                listener.local_addr()?,
                certificate_out.display()
            );
            let mut transport = match wire.into() {
                WireEncoding::Reference => listener.accept()?,
                WireEncoding::Compact => listener.accept_compact(&plan)?,
            };
            transport.establish_session(&plan, "server", "client")?;
            let report = run_generate_server_plan(&plan, &mut transport, tokens)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ConnectQuic {
            conversation,
            server,
            certificate,
            prompt,
            cancel_after,
            wire,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let trusted_certificate = fs::read(certificate)?;
            let mut transport = match wire.into() {
                WireEncoding::Reference => QuicTransport::connect(server, &trusted_certificate)?,
                WireEncoding::Compact => {
                    QuicTransport::connect_compact(server, &trusted_certificate, &plan)?
                }
            };
            transport.establish_session(&plan, "client", "server")?;
            let report = run_generate_client_plan(&plan, &mut transport, &prompt, cancel_after)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ServeIroh {
            conversation,
            identity,
            policy,
            listen,
            advertise,
            ticket_out,
            tokens,
            wire,
            report_out,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            verify_deployment_identity(&plan)?;
            let identity = load_node_identity(&identity)?;
            let authorization = load_authorization_policy(&policy)?;
            let authorized = authorization.authorized_peers("client", &plan)?;
            let node = IrohNode::bind_direct(identity.secret_key()?, listen)?;
            let ticket = endpoint_ticket(&node, advertise)?;
            ticket.save(&ticket_out)?;
            println!(
                "Eve/Iroh server {} listening; wrote {}",
                node.id(),
                ticket_out.display()
            );
            let mut transport = match wire.into() {
                WireEncoding::Reference => node.accept_authorized(&authorized)?,
                WireEncoding::Compact => node.accept_authorized_compact(&authorized, &plan)?,
            };
            transport.establish_session(&plan, "server", "client")?;
            let report = run_generate_server_plan(&plan, &mut transport, tokens)?;
            emit_report(&report, report_out.as_deref())?;
        }
        Command::ConnectIroh {
            conversation,
            identity,
            policy,
            server,
            prompt,
            cancel_after,
            wire,
            report_out,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let identity = load_node_identity(&identity)?;
            let authorization = load_authorization_policy(&policy)?;
            let ticket = load_endpoint_ticket(&server)?;
            let server_id = ticket.endpoint_id()?;
            authorization.authorize(server_id, "server", &plan)?;
            let node = IrohNode::bind_direct(
                identity.secret_key()?,
                "0.0.0.0:0".parse().expect("valid wildcard address"),
            )?;
            let mut transport = match wire.into() {
                WireEncoding::Reference => node.connect(ticket.endpoint_addr()?, server_id)?,
                WireEncoding::Compact => {
                    node.connect_compact(ticket.endpoint_addr()?, server_id, &plan)?
                }
            };
            transport.establish_session(&plan, "client", "server")?;
            let report = run_generate_client_plan(&plan, &mut transport, &prompt, cancel_after)?;
            emit_report(&report, report_out.as_deref())?;
        }
        Command::DraftServeIroh {
            conversation,
            draft,
            identity,
            policy,
            listen,
            advertise,
            ticket_out,
            wire,
            report_out,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let identity = load_node_identity(&identity)?;
            let authorization = load_authorization_policy(&policy)?;
            let authorized = authorization.authorized_peers("client", &plan)?;
            let node = IrohNode::bind_direct(identity.secret_key()?, listen)?;
            let ticket = endpoint_ticket(&node, advertise)?;
            ticket.save(&ticket_out)?;
            println!(
                "Eve draft-sync server {} listening; wrote {}",
                node.id(),
                ticket_out.display()
            );
            let mut transport = match wire.into() {
                WireEncoding::Reference => node.accept_authorized(&authorized)?,
                WireEncoding::Compact => node.accept_authorized_compact(&authorized, &plan)?,
            };
            transport.establish_session(&plan, "server", "client")?;
            let mut document = AutomergeDraft::load(&fs::read(&draft)?)?;
            let report = run_draft_sync_server(&plan, &mut transport, &mut document)?;
            write_bytes(&draft, &document.save())?;
            emit_report(&report, report_out.as_deref())?;
        }
        Command::DraftConnectIroh {
            conversation,
            draft,
            identity,
            policy,
            server,
            wire,
            report_out,
        } => {
            let conversation: Conversation = read_json(&conversation)?;
            let plan = PreparedPlan::compile(&conversation)?;
            let identity = load_node_identity(&identity)?;
            let authorization = load_authorization_policy(&policy)?;
            let ticket = load_endpoint_ticket(&server)?;
            let server_id = ticket.endpoint_id()?;
            authorization.authorize(server_id, "server", &plan)?;
            let node = IrohNode::bind_direct(
                identity.secret_key()?,
                "0.0.0.0:0".parse().expect("valid wildcard address"),
            )?;
            let mut transport = match wire.into() {
                WireEncoding::Reference => node.connect(ticket.endpoint_addr()?, server_id)?,
                WireEncoding::Compact => {
                    node.connect_compact(ticket.endpoint_addr()?, server_id, &plan)?
                }
            };
            transport.establish_session(&plan, "client", "server")?;
            let mut document = AutomergeDraft::load(&fs::read(&draft)?)?;
            let report = run_draft_sync_client(&plan, &mut transport, &mut document)?;
            write_bytes(&draft, &document.save())?;
            emit_report(&report, report_out.as_deref())?;
        }
        Command::VerifySession { client, server } => {
            let client: ExecutionReport = read_json(&client)?;
            let server: ExecutionReport = read_json(&server)?;
            let report = verify_session_reports(client, server)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

fn verify_deployment_identity(plan: &PreparedPlan) -> Result<(), std::io::Error> {
    for (key, actual) in [
        ("EVE_PLAN_IDENTITY", plan.plan_identity()),
        ("EVE_CONVERSATION_IDENTITY", plan.conversation_identity()),
    ] {
        if let Ok(expected) = std::env::var(key)
            && expected != actual
        {
            return Err(std::io::Error::other(format!(
                "{key} mismatch: deployment expects {expected}, compiled conversation produced {actual}"
            )));
        }
    }
    Ok(())
}

fn ensure_absent(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite existing identity artifact {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn endpoint_ticket(
    node: &IrohNode,
    advertise: Option<SocketAddr>,
) -> Result<EndpointTicket, Box<dyn std::error::Error>> {
    let mut ticket = EndpointTicket::from_addr(&node.addr());
    let advertise = advertise
        .map(Ok)
        .or_else(|| {
            std::env::var("EVE_ADVERTISE_ADDRESS")
                .ok()
                .map(|value| value.parse::<SocketAddr>())
        })
        .transpose()?;
    if let Some(address) = advertise {
        ticket.addresses = vec![address];
    }
    if ticket
        .addresses
        .iter()
        .any(|address| address.ip().is_unspecified())
    {
        return Err(Box::new(std::io::Error::other(
            "a wildcard Iroh listener requires --advertise or EVE_ADVERTISE_ADDRESS",
        )));
    }
    Ok(ticket)
}

fn load_node_identity(path: &Path) -> Result<NodeIdentity, Box<dyn std::error::Error>> {
    if let Ok(encoded) = std::env::var("EVE_NODE_IDENTITY_JSON") {
        let identity: NodeIdentity = serde_json::from_str(&encoded)?;
        identity.secret_key()?;
        Ok(identity)
    } else {
        Ok(NodeIdentity::load(path)?)
    }
}

fn load_authorization_policy(
    path: &Path,
) -> Result<AuthorizationPolicy, Box<dyn std::error::Error>> {
    if let Ok(encoded) = std::env::var("EVE_AUTHORIZATION_JSON") {
        Ok(serde_json::from_str(&encoded)?)
    } else {
        Ok(AuthorizationPolicy::load(path)?)
    }
}

fn load_endpoint_ticket(path: &Path) -> Result<EndpointTicket, Box<dyn std::error::Error>> {
    if let Ok(encoded) = std::env::var("EVE_SERVER_TICKET_JSON") {
        let ticket: EndpointTicket = serde_json::from_str(&encoded)?;
        ticket.endpoint_addr()?;
        Ok(ticket)
    } else {
        Ok(EndpointTicket::load(path)?)
    }
}

fn emit_report<T: Serialize>(
    report: &T,
    report_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = serde_json::to_vec_pretty(report)?;
    if let Some(path) = report_out {
        write_bytes(path, &encoded)?;
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

#[derive(Debug, Serialize)]
struct MultiNodeSessionReport {
    conversation_identity: String,
    plan_identity: String,
    semantic_trace_identity: String,
    semantic_trace_equivalent: bool,
    outcome_equivalent: bool,
    client: ExecutionReport,
    server: ExecutionReport,
}

fn verify_session_reports(
    client: ExecutionReport,
    server: ExecutionReport,
) -> Result<MultiNodeSessionReport, std::io::Error> {
    for (field, client_value, server_value) in [
        (
            "conversation_identity",
            client.conversation_identity.as_str(),
            server.conversation_identity.as_str(),
        ),
        (
            "plan_identity",
            client.plan_identity.as_str(),
            server.plan_identity.as_str(),
        ),
    ] {
        if client_value != server_value {
            return Err(std::io::Error::other(format!(
                "multi-node {field} mismatch: client {client_value}, server {server_value}"
            )));
        }
    }
    let semantic_trace_equivalent =
        client.semantic_trace_identity == server.semantic_trace_identity;
    let outcome_equivalent = client.completed
        && server.completed
        && client.successful == server.successful
        && client.failure == server.failure
        && client.tokens == server.tokens;
    if !semantic_trace_equivalent || !outcome_equivalent {
        return Err(std::io::Error::other(
            "multi-node reports do not describe an equivalent Eve execution",
        ));
    }
    Ok(MultiNodeSessionReport {
        conversation_identity: client.conversation_identity.clone(),
        plan_identity: client.plan_identity.clone(),
        semantic_trace_identity: client.semantic_trace_identity.clone(),
        semantic_trace_equivalent,
        outcome_equivalent,
        client,
        server,
    })
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, Box<dyn std::error::Error>> {
    let input = fs::read(path)?;
    Ok(serde_json::from_slice(&input)?)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

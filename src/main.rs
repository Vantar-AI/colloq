use clap::{Parser, Subcommand, ValueEnum};
use eve::benchmark::run_reference_benchmark;
use eve::deploy::{MirenOptions, render_miren_bundle};
use eve::graph::{AutomergeDraft, DraftScalarPatch};
use eve::plan::{EvePlan, PreparedPlan};
use eve::runtime::{
    FaultOperation, FaultPlan, QuicListener, QuicTransport, TcpTransport, WireEncoding,
    run_generate_client_plan, run_generate_server_plan, run_iroh_demo,
    run_iroh_plan_demo_with_encoding, run_memory_demo, run_memory_fault_demo,
    run_memory_plan_demo_with_encoding, run_quic_demo, run_quic_plan_demo_with_encoding,
    run_tcp_demo, run_tcp_plan_demo_with_encoding,
};
use eve::{Conversation, Frame, project, validate, verify_trace};
use serde::de::DeserializeOwned;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};

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

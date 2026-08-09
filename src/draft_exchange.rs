//! Incremental Automerge synchronization carried by an ordinary authenticated Eve conversation.

use crate::Frame;
use crate::graph::{AutomergeDraft, DraftError, DraftSyncSession};
use crate::plan::PreparedPlan;
use crate::runtime::{EndpointMachine, RuntimeError, Transport, WireEnvelope};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

const MAX_SYNC_ROUNDS: usize = 1024;

#[derive(Debug, Error)]
pub enum DraftExchangeError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Draft(#[from] DraftError),
    #[error("invalid Eve draft-sync payload: {0}")]
    Payload(String),
    #[error("Eve draft sync exceeded {MAX_SYNC_ROUNDS} rounds")]
    TooManyRounds,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftExchangeReport {
    pub role: String,
    pub transport: String,
    pub conversation_identity: String,
    pub plan_identity: String,
    pub messages_sent: usize,
    pub messages_received: usize,
    pub frames: u64,
    pub completed: bool,
    pub heads: Vec<String>,
}

pub fn run_draft_sync_client<T: Transport>(
    plan: &PreparedPlan,
    transport: &mut T,
    draft: &mut AutomergeDraft,
) -> Result<DraftExchangeReport, DraftExchangeError> {
    let transport_name = transport.plan().to_string();
    let mut machine = EndpointMachine::from_plan(plan, "client")?;
    let mut peer = DraftSyncSession::default();
    let mut received = 0;

    for round in 0..MAX_SYNC_ROUNDS {
        let Some(message) = draft.generate_sync_message(&mut peer) else {
            let envelope = machine.emit_select("done")?;
            send(transport, envelope)?;
            transport.finish(machine.role())?;
            return report(&machine, transport_name, draft, round, received);
        };

        let envelope = machine.emit_select("message")?;
        send(transport, envelope)?;
        let envelope = machine.emit_data(json!({ "bytes": encode_hex(&message) }))?;
        send(transport, envelope)?;
        let selection = receive(&mut machine, transport)?;
        match selection {
            Frame::Select { label, .. } if label == "message" => {
                let frame = receive(&mut machine, transport)?;
                let message = sync_payload(&frame)?;
                draft.receive_sync_message(&mut peer, &message)?;
                received += 1;
            }
            Frame::Select { label, .. } if label == "idle" => {}
            frame => {
                return Err(DraftExchangeError::Payload(format!(
                    "client expected server sync choice, received {frame:?}"
                )));
            }
        }
    }
    Err(DraftExchangeError::TooManyRounds)
}

pub fn run_draft_sync_server<T: Transport>(
    plan: &PreparedPlan,
    transport: &mut T,
    draft: &mut AutomergeDraft,
) -> Result<DraftExchangeReport, DraftExchangeError> {
    let transport_name = transport.plan().to_string();
    let mut machine = EndpointMachine::from_plan(plan, "server")?;
    let mut peer = DraftSyncSession::default();
    let mut sent = 0;
    let mut received = 0;

    for _ in 0..MAX_SYNC_ROUNDS {
        let selection = receive(&mut machine, transport)?;
        match selection {
            Frame::Select { label, .. } if label == "done" => {
                transport.finish(machine.role())?;
                return report(&machine, transport_name, draft, sent, received);
            }
            Frame::Select { label, .. } if label == "message" => {
                let frame = receive(&mut machine, transport)?;
                let message = sync_payload(&frame)?;
                draft.receive_sync_message(&mut peer, &message)?;
                received += 1;
            }
            frame => {
                return Err(DraftExchangeError::Payload(format!(
                    "server expected client sync choice, received {frame:?}"
                )));
            }
        }

        if let Some(message) = draft.generate_sync_message(&mut peer) {
            let envelope = machine.emit_select("message")?;
            send(transport, envelope)?;
            let envelope = machine.emit_data(json!({ "bytes": encode_hex(&message) }))?;
            send(transport, envelope)?;
            sent += 1;
        } else {
            let envelope = machine.emit_select("idle")?;
            send(transport, envelope)?;
        }
    }
    Err(DraftExchangeError::TooManyRounds)
}

fn send<T: Transport>(transport: &mut T, envelope: WireEnvelope) -> Result<(), RuntimeError> {
    transport.send(&envelope)
}

fn receive<T: Transport>(
    machine: &mut EndpointMachine,
    transport: &mut T,
) -> Result<Frame, RuntimeError> {
    machine.accept(transport.receive()?)
}

fn sync_payload(frame: &Frame) -> Result<Vec<u8>, DraftExchangeError> {
    let Value::Object(payload) = frame_payload(frame)? else {
        return Err(DraftExchangeError::Payload(
            "sync payload must be an object".to_string(),
        ));
    };
    let encoded = payload
        .get("bytes")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DraftExchangeError::Payload("sync payload has no bytes string".to_string())
        })?;
    decode_hex(encoded)
}

fn frame_payload(frame: &Frame) -> Result<&Value, DraftExchangeError> {
    match frame {
        Frame::Data {
            message,
            payload: Some(payload),
            ..
        } if message == "sync" => Ok(payload),
        _ => Err(DraftExchangeError::Payload(format!(
            "expected sync data, received {frame:?}"
        ))),
    }
}

fn report(
    machine: &EndpointMachine,
    transport: String,
    draft: &mut AutomergeDraft,
    messages_sent: usize,
    messages_received: usize,
) -> Result<DraftExchangeReport, DraftExchangeError> {
    Ok(DraftExchangeReport {
        role: machine.role().to_string(),
        transport,
        conversation_identity: machine.identity().to_string(),
        plan_identity: machine.plan_identity().to_string(),
        messages_sent,
        messages_received,
        frames: machine.sequence(),
        completed: machine.is_successful(),
        heads: draft
            .heads()
            .into_iter()
            .map(|head| head.to_string())
            .collect(),
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing into a string cannot fail");
    }
    encoded
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>, DraftExchangeError> {
    if !encoded.len().is_multiple_of(2) || !encoded.is_ascii() {
        return Err(DraftExchangeError::Payload(
            "sync bytes are not even-length hexadecimal".to_string(),
        ));
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).map_err(|_| {
                DraftExchangeError::Payload("sync bytes are not hexadecimal".to_string())
            })?;
            u8::from_str_radix(text, 16).map_err(|_| {
                DraftExchangeError::Payload("sync bytes are not hexadecimal".to_string())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Conversation;
    use crate::graph::DraftScalarPatch;
    use crate::runtime::memory_pair;

    fn generate() -> Conversation {
        serde_json::from_str(include_str!("../examples/generate.eveconv.json")).unwrap()
    }

    fn sync_plan() -> PreparedPlan {
        let conversation: Conversation =
            serde_json::from_str(include_str!("../examples/draft-sync.eveconv.json")).unwrap();
        PreparedPlan::compile(&conversation).unwrap()
    }

    #[test]
    fn automerge_sync_converges_through_projected_eve_endpoints() {
        let mut client_draft = AutomergeDraft::from_conversation(&generate()).unwrap();
        let saved = client_draft.save();
        let mut server_draft = AutomergeDraft::load(&saved).unwrap();
        let client_heads = client_draft.heads();
        let server_heads = server_draft.heads();
        client_draft
            .apply_scalar_patch(
                &client_heads,
                &DraftScalarPatch {
                    path: "/annotations/client".to_string(),
                    value: Value::Bool(true),
                },
            )
            .unwrap();
        server_draft
            .apply_scalar_patch(
                &server_heads,
                &DraftScalarPatch {
                    path: "/annotations/server".to_string(),
                    value: Value::Bool(true),
                },
            )
            .unwrap();

        let plan = sync_plan();
        let (mut client_transport, mut server_transport) = memory_pair();
        let (client_report, server_report) = std::thread::scope(|scope| {
            let server = scope
                .spawn(|| run_draft_sync_server(&plan, &mut server_transport, &mut server_draft));
            let client =
                run_draft_sync_client(&plan, &mut client_transport, &mut client_draft).unwrap();
            (client, server.join().unwrap().unwrap())
        });
        assert!(client_report.completed);
        assert!(server_report.completed);
        assert_eq!(
            client_draft.materialize().unwrap(),
            server_draft.materialize().unwrap()
        );
        assert_eq!(client_report.heads, server_report.heads);
    }
}

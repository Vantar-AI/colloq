//! Collaborative draft storage with an explicit promotion gate into executable Colloq semantics.
//!
//! Automerge owns draft history and merging. Colloq remains the authority for conflicts,
//! validation, canonical semantic identity, and executable-plan construction.

use crate::plan::{ColloqPlan, PlanError};
use crate::{Conversation, ValidationErrors, validate};
use automerge::hydrate::{Map as HydrateMap, Value as HydrateValue};
use automerge::sync::{self, SyncDoc};
use automerge::transaction::Transactable;
use automerge::{
    AutoCommit, AutoSerde, AutomergeError, ChangeHash, ObjId, ObjType, ROOT, ReadDoc, ScalarValue,
    Value as AutomergeValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DraftError {
    #[error("Automerge draft error: {0}")]
    Automerge(#[from] AutomergeError),
    #[error("draft JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Validation(#[from] ValidationErrors),
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error("draft transaction is based on stale Automerge heads")]
    StaleBase,
    #[error("invalid JSON pointer {path}: {detail}")]
    InvalidPointer { path: String, detail: String },
    #[error("draft patch at {0} must contain a JSON scalar")]
    NonScalarPatch(String),
    #[error("draft contains unresolved conflicts: {0}")]
    UnresolvedConflicts(String),
    #[error("Automerge synchronization error: {0}")]
    Sync(String),
}

/// One optimistic structural edit against an Automerge draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftScalarPatch {
    /// RFC 6901-style path to an existing parent map or list.
    pub path: String,
    /// A JSON scalar. Composite replacements deliberately require a future typed operation.
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftConflict {
    pub path: String,
    pub competing_values: usize,
}

/// The only form of an Automerge draft that may enter the Colloq compiler.
#[derive(Debug)]
pub struct PromotedDraft {
    pub conversation: Conversation,
    pub conversation_identity: String,
    pub plan: ColloqPlan,
}

/// A collaborative draft. Its history is intentionally distinct from Colloq content identity.
pub struct AutomergeDraft {
    document: AutoCommit,
}

/// Per-peer Automerge synchronization state. Keep one instance for each remote endpoint.
pub struct DraftSyncSession {
    state: sync::State,
}

impl Default for DraftSyncSession {
    fn default() -> Self {
        Self {
            state: sync::State::new(),
        }
    }
}

impl AutomergeDraft {
    pub fn from_conversation(conversation: &Conversation) -> Result<Self, DraftError> {
        let value = serde_json::to_value(conversation)?;
        let Value::Object(root) = value else {
            unreachable!("an Colloq conversation serializes as a JSON object");
        };
        let root = HydrateMap::from(
            root.into_iter()
                .map(|(key, value)| (key, json_to_hydrate(value)))
                .collect::<HashMap<_, _>>(),
        );
        let mut document = AutoCommit::new();
        document.init_root_from_hydrate(&root)?;
        Ok(Self { document })
    }

    pub fn load(bytes: &[u8]) -> Result<Self, DraftError> {
        Ok(Self {
            document: AutoCommit::load(bytes)?,
        })
    }

    pub fn save(&mut self) -> Vec<u8> {
        self.document.save()
    }

    pub fn heads(&mut self) -> Vec<ChangeHash> {
        self.document.get_heads()
    }

    pub fn fork(&mut self) -> Self {
        Self {
            document: self.document.fork(),
        }
    }

    pub fn merge(&mut self, other: &mut Self) -> Result<(), DraftError> {
        self.document.merge(&mut other.document)?;
        Ok(())
    }

    /// Generate the next transport-agnostic sync payload for one peer.
    pub fn generate_sync_message(&mut self, peer: &mut DraftSyncSession) -> Option<Vec<u8>> {
        self.document
            .sync()
            .generate_sync_message(&mut peer.state)
            .map(sync::Message::encode)
    }

    /// Apply one ordered sync payload received from the peer associated with `peer`.
    pub fn receive_sync_message(
        &mut self,
        peer: &mut DraftSyncSession,
        encoded: &[u8],
    ) -> Result<(), DraftError> {
        let message =
            sync::Message::decode(encoded).map_err(|error| DraftError::Sync(error.to_string()))?;
        self.document
            .sync()
            .receive_sync_message(&mut peer.state, message)?;
        Ok(())
    }

    pub fn materialize(&self) -> Result<Value, DraftError> {
        Ok(serde_json::to_value(AutoSerde::from(&self.document))?)
    }

    /// Apply a scalar patch only if the caller edited the current draft revision.
    pub fn apply_scalar_patch(
        &mut self,
        expected_heads: &[ChangeHash],
        patch: &DraftScalarPatch,
    ) -> Result<Vec<ChangeHash>, DraftError> {
        if self.document.get_heads() != expected_heads {
            return Err(DraftError::StaleBase);
        }
        let segments = parse_pointer(&patch.path)?;
        let (parent_segments, final_segment) = segments.split_at(segments.len() - 1);
        let mut object = ROOT;
        let mut traversed = String::new();
        for segment in parent_segments {
            push_pointer_segment(&mut traversed, segment);
            object = child_object(&self.document, &object, segment, &traversed)?;
        }

        let scalar = json_to_scalar(&patch.path, &patch.value)?;
        match self.document.object_type(&object)? {
            ObjType::Map | ObjType::Table => {
                self.document.put(&object, &final_segment[0], scalar)?;
            }
            ObjType::List => {
                let index = pointer_index(&patch.path, &final_segment[0])?;
                if index >= self.document.length(&object) {
                    return Err(invalid_pointer(
                        &patch.path,
                        format!("list index {index} does not exist"),
                    ));
                }
                self.document.put(&object, index, scalar)?;
            }
            ObjType::Text => {
                return Err(invalid_pointer(
                    &patch.path,
                    "text objects are not valid Colloq graph containers",
                ));
            }
        }
        Ok(self.document.get_heads())
    }

    pub fn conflicts(&self) -> Result<Vec<DraftConflict>, DraftError> {
        let mut conflicts = Vec::new();
        collect_conflicts(&self.document, &ROOT, "", &mut conflicts)?;
        Ok(conflicts)
    }

    /// Reject conflicts, deserialize with Colloq's strict schema, validate, and compile.
    pub fn promote(&self) -> Result<PromotedDraft, DraftError> {
        let conflicts = self.conflicts()?;
        if !conflicts.is_empty() {
            let paths = conflicts
                .iter()
                .map(|conflict| conflict.path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(DraftError::UnresolvedConflicts(paths));
        }
        let conversation: Conversation = serde_json::from_value(self.materialize()?)?;
        validate(&conversation)?;
        let plan = ColloqPlan::compile(&conversation)?;
        Ok(PromotedDraft {
            conversation,
            conversation_identity: plan.conversation_identity.clone(),
            plan,
        })
    }
}

fn json_to_hydrate(value: Value) -> HydrateValue {
    match value {
        Value::Null => HydrateValue::scalar(()),
        Value::Bool(value) => HydrateValue::scalar(value),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                HydrateValue::scalar(value)
            } else if let Some(value) = value.as_u64() {
                HydrateValue::scalar(value)
            } else {
                HydrateValue::scalar(value.as_f64().expect("a JSON number is finite"))
            }
        }
        Value::String(value) => HydrateValue::scalar(value),
        Value::Array(values) => {
            HydrateValue::from(values.into_iter().map(json_to_hydrate).collect::<Vec<_>>())
        }
        Value::Object(values) => HydrateValue::Map(HydrateMap::from(
            values
                .into_iter()
                .map(|(key, value)| (key, json_to_hydrate(value)))
                .collect::<HashMap<_, _>>(),
        )),
    }
}

fn json_to_scalar(path: &str, value: &Value) -> Result<ScalarValue, DraftError> {
    match value {
        Value::Null => Ok(().into()),
        Value::Bool(value) => Ok((*value).into()),
        Value::Number(value) => value
            .as_i64()
            .map(ScalarValue::from)
            .or_else(|| value.as_u64().map(ScalarValue::from))
            .or_else(|| value.as_f64().map(ScalarValue::from))
            .ok_or_else(|| DraftError::NonScalarPatch(path.to_string())),
        Value::String(value) => Ok(value.clone().into()),
        Value::Array(_) | Value::Object(_) => Err(DraftError::NonScalarPatch(path.to_string())),
    }
}

fn parse_pointer(path: &str) -> Result<Vec<String>, DraftError> {
    if !path.starts_with('/') || path == "/" {
        return Err(invalid_pointer(
            path,
            "the path must begin with '/' and name at least one field",
        ));
    }
    path[1..]
        .split('/')
        .map(|segment| decode_pointer_segment(path, segment))
        .collect()
}

fn decode_pointer_segment(path: &str, segment: &str) -> Result<String, DraftError> {
    let mut decoded = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    while let Some(character) = chars.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match chars.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => return Err(invalid_pointer(path, "invalid '~' escape")),
        }
    }
    Ok(decoded)
}

fn child_object(
    document: &AutoCommit,
    object: &ObjId,
    segment: &str,
    path: &str,
) -> Result<ObjId, DraftError> {
    let value = match document.object_type(object)? {
        ObjType::Map | ObjType::Table => {
            if document.get_all(object, segment)?.len() > 1 {
                return Err(DraftError::UnresolvedConflicts(path.to_string()));
            }
            document.get(object, segment)?
        }
        ObjType::List => {
            let index = pointer_index(path, segment)?;
            if document.get_all(object, index)?.len() > 1 {
                return Err(DraftError::UnresolvedConflicts(path.to_string()));
            }
            document.get(object, index)?
        }
        ObjType::Text => {
            return Err(invalid_pointer(
                path,
                "text objects are not valid Colloq graph containers",
            ));
        }
    };
    match value {
        Some((AutomergeValue::Object(_), child)) => Ok(child),
        Some((AutomergeValue::Scalar(_), _)) => {
            Err(invalid_pointer(path, "an intermediate value is a scalar"))
        }
        None => Err(invalid_pointer(
            path,
            "an intermediate value does not exist",
        )),
    }
}

fn collect_conflicts(
    document: &AutoCommit,
    object: &ObjId,
    path: &str,
    conflicts: &mut Vec<DraftConflict>,
) -> Result<(), AutomergeError> {
    match document.object_type(object)? {
        ObjType::Map | ObjType::Table => {
            for key in document.keys(object) {
                let mut child_path = path.to_string();
                push_pointer_segment(&mut child_path, &key);
                let all = document.get_all(object, key.as_str())?;
                if all.len() > 1 {
                    conflicts.push(DraftConflict {
                        path: child_path.clone(),
                        competing_values: all.len(),
                    });
                }
                if let Some((AutomergeValue::Object(_), child)) =
                    document.get(object, key.as_str())?
                {
                    collect_conflicts(document, &child, &child_path, conflicts)?;
                }
            }
        }
        ObjType::List => {
            for index in 0..document.length(object) {
                let child_path = format!("{path}/{index}");
                let all = document.get_all(object, index)?;
                if all.len() > 1 {
                    conflicts.push(DraftConflict {
                        path: child_path.clone(),
                        competing_values: all.len(),
                    });
                }
                if let Some((AutomergeValue::Object(_), child)) = document.get(object, index)? {
                    collect_conflicts(document, &child, &child_path, conflicts)?;
                }
            }
        }
        ObjType::Text => {}
    }
    Ok(())
}

fn pointer_index(path: &str, segment: &str) -> Result<usize, DraftError> {
    segment
        .parse::<usize>()
        .map_err(|_| invalid_pointer(path, format!("{segment:?} is not a list index")))
}

fn push_pointer_segment(path: &mut String, segment: &str) {
    path.push('/');
    path.push_str(&segment.replace('~', "~0").replace('/', "~1"));
}

fn invalid_pointer(path: &str, detail: impl Into<String>) -> DraftError {
    DraftError::InvalidPointer {
        path: path.to_string(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conversation() -> Conversation {
        serde_json::from_str(include_str!("../examples/generate.colloqconv.json")).unwrap()
    }

    #[test]
    fn draft_round_trips_and_promotes_through_eve_validation() {
        let mut draft = AutomergeDraft::from_conversation(&conversation()).unwrap();
        let saved = draft.save();
        let restored = AutomergeDraft::load(&saved).unwrap();
        let promoted = restored.promote().unwrap();
        assert_eq!(promoted.conversation.module.id, "example.generate");
        assert_eq!(
            promoted.conversation_identity,
            promoted.plan.conversation_identity
        );
    }

    #[test]
    fn non_overlapping_concurrent_edits_merge_and_validate() {
        let mut first = AutomergeDraft::from_conversation(&conversation()).unwrap();
        let base = first.heads();
        let mut second = first.fork();
        first
            .apply_scalar_patch(
                &base,
                &DraftScalarPatch {
                    path: "/module/semantic_version".to_string(),
                    value: Value::String("0.2.0".to_string()),
                },
            )
            .unwrap();
        second
            .apply_scalar_patch(
                &base,
                &DraftScalarPatch {
                    path: "/annotations/editor".to_string(),
                    value: Value::String("ai-agent".to_string()),
                },
            )
            .unwrap();
        first.merge(&mut second).unwrap();
        assert!(first.conflicts().unwrap().is_empty());
        let promoted = first.promote().unwrap();
        assert_eq!(
            promoted.conversation.module.semantic_version.as_deref(),
            Some("0.2.0")
        );
        assert_eq!(
            promoted.conversation.annotations.get("editor"),
            Some(&Value::String("ai-agent".to_string()))
        );
    }

    #[test]
    fn concurrent_meaning_conflict_cannot_be_promoted() {
        let mut first = AutomergeDraft::from_conversation(&conversation()).unwrap();
        let base = first.heads();
        let mut second = first.fork();
        for (draft, version) in [(&mut first, "0.2.0"), (&mut second, "0.3.0")] {
            draft
                .apply_scalar_patch(
                    &base,
                    &DraftScalarPatch {
                        path: "/module/semantic_version".to_string(),
                        value: Value::String(version.to_string()),
                    },
                )
                .unwrap();
        }
        first.merge(&mut second).unwrap();
        assert_eq!(
            first.conflicts().unwrap(),
            vec![DraftConflict {
                path: "/module/semantic_version".to_string(),
                competing_values: 2,
            }]
        );
        assert!(matches!(
            first.promote(),
            Err(DraftError::UnresolvedConflicts(_))
        ));
    }

    #[test]
    fn optimistic_patch_rejects_stale_heads() {
        let mut draft = AutomergeDraft::from_conversation(&conversation()).unwrap();
        let stale = draft.heads();
        draft
            .apply_scalar_patch(
                &stale,
                &DraftScalarPatch {
                    path: "/module/semantic_version".to_string(),
                    value: Value::String("0.2.0".to_string()),
                },
            )
            .unwrap();
        let error = draft
            .apply_scalar_patch(
                &stale,
                &DraftScalarPatch {
                    path: "/module/semantic_version".to_string(),
                    value: Value::String("0.3.0".to_string()),
                },
            )
            .unwrap_err();
        assert!(matches!(error, DraftError::StaleBase));
    }

    #[test]
    fn ordered_sync_messages_converge_two_drafts() {
        let mut first = AutomergeDraft::from_conversation(&conversation()).unwrap();
        let saved = first.save();
        let mut second = AutomergeDraft::load(&saved).unwrap();
        let base = first.heads();
        first
            .apply_scalar_patch(
                &base,
                &DraftScalarPatch {
                    path: "/annotations/editor".to_string(),
                    value: Value::String("sync-peer".to_string()),
                },
            )
            .unwrap();

        let mut first_peer = DraftSyncSession::default();
        let mut second_peer = DraftSyncSession::default();
        loop {
            let first_to_second = first.generate_sync_message(&mut first_peer);
            if let Some(message) = &first_to_second {
                second
                    .receive_sync_message(&mut second_peer, message)
                    .unwrap();
            }
            let second_to_first = second.generate_sync_message(&mut second_peer);
            if let Some(message) = &second_to_first {
                first
                    .receive_sync_message(&mut first_peer, message)
                    .unwrap();
            }
            if first_to_second.is_none() && second_to_first.is_none() {
                break;
            }
        }
        assert_eq!(first.materialize().unwrap(), second.materialize().unwrap());
        assert_eq!(
            second
                .promote()
                .unwrap()
                .conversation
                .annotations
                .get("editor"),
            Some(&Value::String("sync-peer".to_string()))
        );
    }
}

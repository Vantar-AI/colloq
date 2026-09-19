//! Jev as a chooser for Eve `choice` states.
//!
//! A choice state names the role that selects a branch, but Eve itself never
//! decides which label that role emits. A Jev binding lets TypeSafe's Jev
//! System One model make that selection as a typed Choice judgment. Eve keeps
//! the protocol: the model only proposes a declared label, the threshold is
//! explicit, and a low-confidence answer deterministically takes the declared
//! escalation branch. Service errors surface as typed failures instead of a
//! silent default.

use crate::{Conversation, Diagnostic, GlobalState, ValidationErrors, validate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::thread;
use std::time::Duration;
use thiserror::Error;

pub const JEV_BINDING_FORMAT: &str = "0.1.0";
pub const TYPESAFE_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
pub const TYPESAFE_API_KEY_ENV: &str = "TYPESAFE_API_KEY";

/// Question key sent to TypeSafe. Keys are not visible to the model.
const QUESTION_ID: &str = "eve_choice";

/// Binds one Eve choice state to one Jev Choice question.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JevBinding {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub eve_jev: String,
    /// Module ID of the bound conversation.
    pub conversation: String,
    /// ID of the bound `choice` state.
    pub state: String,
    pub model: String,
    /// What Jev decides. A string or structured instructions.
    pub instructions: Value,
    /// Branch label to rubric description; `null` needs no extra detail.
    pub criteria: BTreeMap<String, Option<String>>,
    /// Minimum Jev confidence to take Jev's label. Required; there is no default.
    pub threshold: f64,
    /// Branch taken when confidence is below `threshold`.
    pub escalate: String,
}

/// A binding validated against its conversation.
#[derive(Debug, Clone)]
pub struct BoundChoice {
    binding: JevBinding,
    chooser: String,
    branches: BTreeSet<String>,
}

/// Validate a binding against a conversation. Fails closed: nothing is sent to
/// Jev unless every check passes.
pub fn bind(
    conversation: &Conversation,
    binding: JevBinding,
) -> Result<BoundChoice, ValidationErrors> {
    validate(conversation)?;
    let mut diagnostics = Vec::new();
    let mut error = |code, path: &str, message: String| {
        diagnostics.push(Diagnostic {
            code,
            path: path.to_string(),
            message,
        })
    };

    if binding.eve_jev != JEV_BINDING_FORMAT {
        error(
            "EVEJ001",
            "$.eve_jev",
            format!(
                "unsupported format {}; expected {JEV_BINDING_FORMAT}",
                binding.eve_jev
            ),
        );
    }
    if binding.conversation != conversation.module.id {
        error(
            "EVEJ002",
            "$.conversation",
            format!(
                "binding targets {}, but the conversation is {}",
                binding.conversation, conversation.module.id
            ),
        );
    }
    if binding.model.trim().is_empty() {
        error("EVEJ003", "$.model", "model must not be empty".to_string());
    }
    if !binding.threshold.is_finite() || !(0.0..=1.0).contains(&binding.threshold) {
        error(
            "EVEJ004",
            "$.threshold",
            format!("threshold {} must be within 0..=1", binding.threshold),
        );
    }
    if binding.criteria.len() < 2 {
        error(
            "EVEJ005",
            "$.criteria",
            "a Choice needs at least two options".to_string(),
        );
    }

    let state = conversation
        .states
        .iter()
        .find(|state| state.id() == binding.state);
    let (chooser, branches) = match state {
        Some(GlobalState::Choice {
            chooser, branches, ..
        }) => (
            chooser.clone(),
            branches.keys().cloned().collect::<BTreeSet<_>>(),
        ),
        Some(_) => {
            error(
                "EVEJ006",
                "$.state",
                format!("state {} is not a choice", binding.state),
            );
            return Err(ValidationErrors { diagnostics });
        }
        None => {
            error(
                "EVEJ007",
                "$.state",
                format!("state {} does not exist", binding.state),
            );
            return Err(ValidationErrors { diagnostics });
        }
    };

    for option in binding.criteria.keys() {
        if !branches.contains(option) {
            error(
                "EVEJ008",
                &format!("$.criteria.{option}"),
                format!("{option} is not a branch of {}", binding.state),
            );
        }
    }
    for branch in &branches {
        if branch != &binding.escalate && !binding.criteria.contains_key(branch) {
            error(
                "EVEJ009",
                "$.criteria",
                format!("branch {branch} has no criterion, so Jev can never select it"),
            );
        }
    }
    if !branches.contains(&binding.escalate) {
        error(
            "EVEJ010",
            "$.escalate",
            format!("{} is not a branch of {}", binding.escalate, binding.state),
        );
    }

    if diagnostics.is_empty() {
        Ok(BoundChoice {
            binding,
            chooser,
            branches,
        })
    } else {
        Err(ValidationErrors { diagnostics })
    }
}

/// One Choice question, as sent to a decider.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceRequest<'a> {
    pub model: &'a str,
    pub state: &'a Value,
    pub instructions: &'a Value,
    pub criteria: &'a BTreeMap<String, Option<String>>,
}

/// A Choice answer: the top option, its distribution, and confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum JevError {
    /// The service could not answer: missing key, transport error, or HTTP error.
    #[error("jev.unavailable: {0}")]
    Unavailable(String),
    /// The service answered, but not with a usable Choice for this binding.
    #[error("jev.invalid_answer: {0}")]
    InvalidAnswer(String),
}

impl JevError {
    /// Typed failure ID, in the same style as Eve's transport failures.
    pub fn failure_id(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "jev.unavailable",
            Self::InvalidAnswer(_) => "jev.invalid_answer",
        }
    }
}

/// Anything that can answer a Choice question. The live client is
/// [`TypeSafeClient`]; tests use a fixed answer.
pub trait Decider {
    fn choose(&self, request: ChoiceRequest<'_>) -> Result<ChoiceAnswer, JevError>;
}

/// The label the chooser role emits, with the evidence behind it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JevDecision {
    pub state: String,
    pub chooser: String,
    /// Branch label to pass to `EndpointMachine::emit_select`.
    pub label: String,
    /// Jev's top option. Differs from `label` when escalated.
    pub choice: String,
    pub confidence: f64,
    pub threshold: f64,
    pub escalated: bool,
    pub probabilities: BTreeMap<String, f64>,
    pub model: String,
}

impl BoundChoice {
    pub fn binding(&self) -> &JevBinding {
        &self.binding
    }

    pub fn chooser(&self) -> &str {
        &self.chooser
    }

    pub fn branches(&self) -> &BTreeSet<String> {
        &self.branches
    }

    /// Ask the decider and turn its answer into a branch label.
    pub fn decide(&self, decider: &dyn Decider, state: &Value) -> Result<JevDecision, JevError> {
        let binding = &self.binding;
        let answer = decider.choose(ChoiceRequest {
            model: &binding.model,
            state,
            instructions: &binding.instructions,
            criteria: &binding.criteria,
        })?;

        if !binding.criteria.contains_key(&answer.choice) {
            return Err(JevError::InvalidAnswer(format!(
                "choice {} is not an option of {}",
                answer.choice, binding.state
            )));
        }
        if !answer.confidence.is_finite() || !(0.0..=1.0).contains(&answer.confidence) {
            return Err(JevError::InvalidAnswer(format!(
                "confidence {} is outside 0..=1",
                answer.confidence
            )));
        }

        let escalated = answer.confidence < binding.threshold;
        let label = if escalated {
            binding.escalate.clone()
        } else {
            answer.choice.clone()
        };
        Ok(JevDecision {
            state: binding.state.clone(),
            chooser: self.chooser.clone(),
            label,
            choice: answer.choice,
            confidence: answer.confidence,
            threshold: binding.threshold,
            escalated,
            probabilities: answer.probabilities,
            model: binding.model.clone(),
        })
    }
}

/// Blocking client for the TypeSafe System One HTTP API.
pub struct TypeSafeClient {
    api_key: String,
    endpoint: String,
    agent: ureq::Agent,
}

impl fmt::Debug for TypeSafeClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypeSafeClient")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl TypeSafeClient {
    pub fn new(api_key: impl Into<String>, timeout: Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            api_key: api_key.into(),
            endpoint: TYPESAFE_ENDPOINT.to_string(),
            agent,
        }
    }

    /// Read the API key from `TYPESAFE_API_KEY`. Keep the key server-side.
    pub fn from_env(timeout: Duration) -> Result<Self, JevError> {
        match std::env::var(TYPESAFE_API_KEY_ENV) {
            Ok(key) if !key.trim().is_empty() => Ok(Self::new(key.trim(), timeout)),
            _ => Err(JevError::Unavailable(format!(
                "{TYPESAFE_API_KEY_ENV} is not set"
            ))),
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

impl Decider for TypeSafeClient {
    fn choose(&self, request: ChoiceRequest<'_>) -> Result<ChoiceAnswer, JevError> {
        let body = choice_request_body(request);
        // 429 and 529 are retryable per the API reference; everything else is final.
        let backoff = [Duration::from_millis(250), Duration::from_millis(1000)];
        let mut attempt = 0;
        loop {
            let mut response = self
                .agent
                .post(&self.endpoint)
                .header("Authorization", &format!("Bearer {}", self.api_key))
                .send_json(&body)
                .map_err(|error| JevError::Unavailable(error.to_string()))?;
            let status = response.status().as_u16();
            if (status == 429 || status == 529) && attempt < backoff.len() {
                thread::sleep(backoff[attempt]);
                attempt += 1;
                continue;
            }
            if !(200..300).contains(&status) {
                let detail = response.body_mut().read_to_string().unwrap_or_default();
                return Err(JevError::Unavailable(format!(
                    "HTTP {status}: {}",
                    detail.chars().take(300).collect::<String>()
                )));
            }
            let value: Value = response
                .body_mut()
                .read_json()
                .map_err(|error| JevError::InvalidAnswer(error.to_string()))?;
            return parse_choice_response(&value);
        }
    }
}

/// Request body for one Choice question.
pub fn choice_request_body(request: ChoiceRequest<'_>) -> Value {
    json!({
        "state": request.state,
        "model": request.model,
        "questions": {
            QUESTION_ID: {
                "type": "choice",
                "instructions": request.instructions,
                "criteria": request.criteria,
            }
        }
    })
}

/// Extract the Choice answer from a System One response.
pub fn parse_choice_response(response: &Value) -> Result<ChoiceAnswer, JevError> {
    let answer = response
        .get("answers")
        .and_then(|answers| answers.get(QUESTION_ID))
        .ok_or_else(|| JevError::InvalidAnswer(format!("response has no answers.{QUESTION_ID}")))?;
    if answer.get("type").and_then(Value::as_str) != Some("choice") {
        return Err(JevError::InvalidAnswer(
            "answer is not a choice".to_string(),
        ));
    }
    serde_json::from_value(answer.clone())
        .map_err(|error| JevError::InvalidAnswer(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE: &str = include_str!("../examples/route.eveconv.json");
    const ROUTE_JEV: &str = include_str!("../examples/route.evejev.json");

    struct Fixed(Result<ChoiceAnswer, JevError>);

    impl Decider for Fixed {
        fn choose(&self, _: ChoiceRequest<'_>) -> Result<ChoiceAnswer, JevError> {
            self.0.clone()
        }
    }

    fn answer(choice: &str, confidence: f64) -> Fixed {
        Fixed(Ok(ChoiceAnswer {
            choice: choice.to_string(),
            probabilities: BTreeMap::from([(choice.to_string(), confidence)]),
            confidence,
        }))
    }

    fn fixtures() -> (Conversation, JevBinding) {
        (
            serde_json::from_str(ROUTE).unwrap(),
            serde_json::from_str(ROUTE_JEV).unwrap(),
        )
    }

    fn codes(result: Result<BoundChoice, ValidationErrors>) -> Vec<&'static str> {
        result
            .unwrap_err()
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn example_binding_is_valid() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        assert_eq!(bound.chooser(), "router");
        assert_eq!(bound.branches().len(), 3);
    }

    #[test]
    fn high_confidence_takes_jev_label() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        let decision = bound
            .decide(&answer("infer", 0.97), &json!({"text": "hi"}))
            .unwrap();
        assert_eq!(decision.label, "infer");
        assert!(!decision.escalated);
    }

    #[test]
    fn threshold_is_inclusive() {
        let (conversation, mut binding) = fixtures();
        binding.threshold = 0.9;
        let bound = bind(&conversation, binding).unwrap();
        let decision = bound.decide(&answer("cached", 0.9), &json!({})).unwrap();
        assert_eq!(decision.label, "cached");
    }

    #[test]
    fn low_confidence_escalates() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        let decision = bound.decide(&answer("cached", 0.41), &json!({})).unwrap();
        assert_eq!(decision.label, "review");
        assert_eq!(decision.choice, "cached");
        assert!(decision.escalated);
    }

    #[test]
    fn unknown_choice_is_rejected() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        let error = bound
            .decide(&answer("refund", 0.99), &json!({}))
            .unwrap_err();
        assert_eq!(error.failure_id(), "jev.invalid_answer");
    }

    #[test]
    fn out_of_range_confidence_is_rejected() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        let error = bound.decide(&answer("infer", 1.5), &json!({})).unwrap_err();
        assert_eq!(error.failure_id(), "jev.invalid_answer");
    }

    #[test]
    fn service_failure_is_typed_not_defaulted() {
        let (conversation, binding) = fixtures();
        let bound = bind(&conversation, binding).unwrap();
        let decider = Fixed(Err(JevError::Unavailable("HTTP 503".to_string())));
        let error = bound.decide(&decider, &json!({})).unwrap_err();
        assert_eq!(error.failure_id(), "jev.unavailable");
    }

    #[test]
    fn binding_must_target_a_choice_state() {
        let (conversation, mut binding) = fixtures();
        binding.state = "start".to_string();
        assert_eq!(codes(bind(&conversation, binding)), ["EVEJ006"]);
    }

    #[test]
    fn binding_must_target_an_existing_state() {
        let (conversation, mut binding) = fixtures();
        binding.state = "missing".to_string();
        assert_eq!(codes(bind(&conversation, binding)), ["EVEJ007"]);
    }

    #[test]
    fn criteria_must_map_to_branches() {
        let (conversation, mut binding) = fixtures();
        binding.criteria.remove("infer");
        binding
            .criteria
            .insert("refund".to_string(), Some("Money back".to_string()));
        assert_eq!(codes(bind(&conversation, binding)), ["EVEJ008", "EVEJ009"]);
    }

    #[test]
    fn escalation_branch_needs_no_criterion() {
        let (conversation, mut binding) = fixtures();
        binding.criteria.remove("review");
        assert!(bind(&conversation, binding).is_ok());
    }

    #[test]
    fn escalation_and_threshold_are_checked() {
        let (conversation, mut binding) = fixtures();
        binding.escalate = "human".to_string();
        binding.threshold = 1.2;
        assert_eq!(codes(bind(&conversation, binding)), ["EVEJ004", "EVEJ010"]);
    }

    #[test]
    fn binding_must_match_the_conversation() {
        let (conversation, mut binding) = fixtures();
        binding.conversation = "example.generate".to_string();
        assert_eq!(codes(bind(&conversation, binding)), ["EVEJ002"]);
    }

    #[test]
    fn threshold_is_required() {
        let mut value: Value = serde_json::from_str(ROUTE_JEV).unwrap();
        value.as_object_mut().unwrap().remove("threshold");
        assert!(serde_json::from_value::<JevBinding>(value).is_err());
    }

    #[test]
    fn request_body_matches_the_api_contract() {
        let (_, binding) = fixtures();
        let state = json!({"text": "hi"});
        let body = choice_request_body(ChoiceRequest {
            model: &binding.model,
            state: &state,
            instructions: &binding.instructions,
            criteria: &binding.criteria,
        });
        assert_eq!(body["model"], "jev-latest");
        assert_eq!(body["state"], state);
        assert_eq!(body["questions"][QUESTION_ID]["type"], "choice");
        assert!(body["questions"][QUESTION_ID]["criteria"]["review"].is_string());
    }

    #[test]
    fn parses_documented_choice_response() {
        let response = json!({
            "model": "jev-latest",
            "answers": {
                QUESTION_ID: {
                    "type": "choice",
                    "choice": "infer",
                    "probabilities": {"cached": 0.08, "infer": 0.85, "review": 0.07},
                    "confidence": 0.82
                }
            },
            "usage": {"input_tokens": 312, "output_tokens": 48}
        });
        let answer = parse_choice_response(&response).unwrap();
        assert_eq!(answer.choice, "infer");
        assert_eq!(answer.confidence, 0.82);
        assert_eq!(answer.probabilities.len(), 3);
    }

    #[test]
    fn rejects_non_choice_answer() {
        let response = json!({"answers": {QUESTION_ID: {"type": "noul", "noul": 0.9}}});
        assert_eq!(
            parse_choice_response(&response).unwrap_err().failure_id(),
            "jev.invalid_answer"
        );
    }

    #[test]
    fn debug_never_prints_the_key() {
        let client = TypeSafeClient::new("ts_secret_value", Duration::from_secs(1));
        assert!(!format!("{client:?}").contains("ts_secret_value"));
    }
}

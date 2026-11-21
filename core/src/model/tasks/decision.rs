//! Bounded System One inference over an owner-local model socket.

use std::collections::BTreeMap;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::DecisionConfig;

pub const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_OPTIONS: usize = 24;
pub(crate) const LOCAL_PROVIDER: &str = "system-one-local";
const MAX_QUESTIONS: usize = 8;
const DEFAULT_TIMEOUT_MS: u64 = 5000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Question {
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    Noul {
        instructions: String,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", try_from = "WireAnswer")]
pub enum Answer {
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
    },
    Noul {
        noul: f64,
    },
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
    },
}

// Decode numeric fields directly; an internally tagged enum buffers numbers
// as maps when serde_json's arbitrary_precision feature is enabled.
#[derive(Deserialize)]
struct WireAnswer {
    #[serde(rename = "type")]
    kind: AnswerKind,
    choice: Option<String>,
    probabilities: Option<BTreeMap<String, f64>>,
    noul: Option<f64>,
    score: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AnswerKind {
    Choice,
    Noul,
    Score,
}

impl TryFrom<WireAnswer> for Answer {
    type Error = &'static str;

    fn try_from(wire: WireAnswer) -> Result<Self, Self::Error> {
        match (
            wire.kind,
            wire.choice,
            wire.probabilities,
            wire.noul,
            wire.score,
        ) {
            (AnswerKind::Choice, Some(choice), Some(probabilities), None, None) => {
                Ok(Self::Choice {
                    choice,
                    probabilities,
                })
            }
            (AnswerKind::Noul, None, None, Some(noul), None) => Ok(Self::Noul { noul }),
            (AnswerKind::Score, None, Some(probabilities), None, Some(score)) => Ok(Self::Score {
                score,
                probabilities,
            }),
            _ => Err("invalid fields for the decision answer type"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecisionResult {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
}

#[derive(Clone)]
pub struct DecisionModel {
    model: String,
    socket: PathBuf,
    owner: u32,
    client: reqwest::Client,
}

impl DecisionModel {
    pub fn from_config(config: &DecisionConfig) -> Result<Option<Self>, String> {
        if config.model.is_empty() {
            return Ok(None);
        }
        let owner = unsafe { libc::geteuid() };
        if owner == 0 {
            return Err("run the decision task as the configuring non-root owner".into());
        }
        let socket = PathBuf::from(format!("/run/user/{owner}/yu-decision.sock"));
        let timeout_ms = config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if !(100..=30_000).contains(&timeout_ms) {
            return Err("decision.timeout_ms must be between 100 and 30000".into());
        }
        Self::new(
            &config.model,
            socket,
            owner,
            Duration::from_millis(timeout_ms),
        )
        .map(Some)
    }

    fn new(model: &str, socket: PathBuf, owner: u32, timeout: Duration) -> Result<Self, String> {
        validate_model_alias(model)?;
        let client = reqwest::Client::builder()
            .unix_socket(socket.clone())
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| "could not initialize the local decision client")?;
        Ok(Self {
            model: model.to_owned(),
            socket,
            owner,
            client,
        })
    }

    pub async fn decide(&self, request: &DecisionRequest) -> Result<DecisionResult, String> {
        request.validate()?;
        let body = serde_json::to_vec(&json!({
            "model": self.model,
            "state": request.state,
            "questions": request.questions,
        }))
        .map_err(|_| "could not encode the decision request")?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err("decision request is too large".into());
        }
        check_socket(&self.socket, self.owner)?;
        let mut response = self
            .client
            .post("http://localhost/v1/systemone")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| "local decision model unavailable or timed out")?;
        if !response.status().is_success() {
            return Err(format!(
                "local decision model returned HTTP {}",
                response.status()
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "could not read the local decision response")?
        {
            if chunk.len() > MAX_REQUEST_BYTES - bytes.len() {
                return Err("local decision response is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let result: DecisionResult =
            serde_json::from_slice(&bytes).map_err(|_| "invalid local decision response")?;
        if result.model != self.model || !request.matches(&result) {
            return Err("local decision response does not match the model or questions".into());
        }
        Ok(result)
    }
}

pub(crate) fn validate_model_alias(model: &str) -> Result<(), String> {
    if model.is_empty()
        || model.len() > 128
        || !model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
    {
        return Err("invalid decision model alias".into());
    }
    Ok(())
}

impl DecisionRequest {
    fn validate(&self) -> Result<(), String> {
        if self.questions.is_empty() || self.questions.len() > MAX_QUESTIONS {
            return Err("expected one to eight decision questions".into());
        }
        for (id, question) in &self.questions {
            let (instructions, valid_options) = match question {
                Question::Choice {
                    instructions,
                    criteria,
                } => (
                    instructions,
                    (2..=MAX_OPTIONS).contains(&criteria.len())
                        && criteria
                            .iter()
                            .all(|(key, label)| identifier(key) && text(label, 400)),
                ),
                Question::Noul { instructions } => (instructions, true),
                Question::Score {
                    instructions,
                    criteria,
                } => (
                    instructions,
                    (2..=10).contains(&criteria.len())
                        && criteria.iter().all(|label| text(label, 400)),
                ),
            };
            if !identifier(id) || !valid_options || !text(instructions, 1000) {
                return Err("invalid decision question id, instructions, or criteria".into());
            }
        }
        Ok(())
    }

    fn matches(&self, result: &DecisionResult) -> bool {
        result.answers.len() == self.questions.len()
            && self.questions.iter().all(|(id, question)| {
                match (question, result.answers.get(id)) {
                    (
                        Question::Choice { criteria, .. },
                        Some(Answer::Choice {
                            choice,
                            probabilities,
                        }),
                    ) => {
                        let Some(winner) = probabilities.get(choice) else {
                            return false;
                        };
                        criteria.keys().eq(probabilities.keys())
                            && distribution(probabilities)
                            && probabilities.values().all(|value| value <= winner)
                    }
                    (Question::Noul { .. }, Some(Answer::Noul { noul })) => unit(*noul),
                    (
                        Question::Score { criteria, .. },
                        Some(Answer::Score {
                            score,
                            probabilities,
                        }),
                    ) => {
                        (0.0..=(criteria.len() - 1) as f64).contains(score)
                            && probabilities.len() == criteria.len()
                            && (0..criteria.len())
                                .all(|index| probabilities.contains_key(&index.to_string()))
                            && distribution(probabilities)
                    }
                    _ => false,
                }
            })
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && !value.contains('\0') && value.chars().count() <= limit
}

fn unit(value: f64) -> bool {
    (0.0..=1.0).contains(&value)
}

fn distribution(probabilities: &BTreeMap<String, f64>) -> bool {
    probabilities.values().all(|value| unit(*value))
        && (probabilities.values().sum::<f64>() - 1.0).abs() < 0.01
}

fn check_socket(socket: &Path, owner: u32) -> Result<(), String> {
    let directory = socket
        .parent()
        .ok_or("local decision socket has no parent")?;
    let dm = std::fs::symlink_metadata(directory)
        .map_err(|_| "local decision socket directory is unavailable")?;
    if !dm.is_dir() || dm.uid() != owner || dm.mode() & 0o077 != 0 {
        return Err("local decision socket directory must be owner-only".into());
    }
    let sm =
        std::fs::symlink_metadata(socket).map_err(|_| "local decision socket is unavailable")?;
    if !sm.file_type().is_socket() || sm.uid() != owner {
        return Err("local decision socket must be owned by the configuring user".into());
    }
    Ok(())
}

pub fn status(config: &DecisionConfig) -> Value {
    let socket_present = DecisionModel::from_config(config)
        .ok()
        .flatten()
        .is_some_and(|model| check_socket(&model.socket, model.owner).is_ok());
    json!({
        "provider": LOCAL_PROVIDER,
        "model": config.model,
        "configured": !config.model.is_empty(),
        "socket_present": socket_present,
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/model/tasks/decision.rs"
    ));
}

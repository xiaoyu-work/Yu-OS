//! Closed presentation DTOs for broker-owned Activity automatic progression.

use std::collections::{BTreeMap, BTreeSet};

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkEvent {
    pub source: String,
    pub event_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkDraft {
    pub task: String,
    pub check_interval_secs: u32,
    pub wake_event: Option<AutoWorkEvent>,
}

impl AutoWorkDraft {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !text(&self.task, 8192)
            || !(60..=86400).contains(&self.check_interval_secs)
            || self.wake_event.as_ref().is_some_and(|event| {
                !text(&event.source, 128)
                    || !text(&event.event_type, 128)
                    || event.source.trim() != event.source
                    || event.event_type.trim() != event.event_type
            })
        {
            return Err("Invalid automatic-work task, check interval or event filter");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AutoWorkNext {
    Continue {
        task: String,
    },
    Wait {
        task: String,
        reason: String,
    },
    Review {
        reason: String,
    },
    Blocked {
        reason: String,
    },
    Parallel {
        tasks: Vec<AutoWorkTask>,
        then: String,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkTask {
    pub id: String,
    pub task: String,
    pub after: Vec<String>,
}

fn task_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

fn valid_plan(tasks: &[AutoWorkTask]) -> bool {
    let mut seen = BTreeSet::new();
    (2..=6).contains(&tasks.len())
        && tasks.iter().all(|task| {
            let edges: BTreeSet<_> = task.after.iter().map(String::as_str).collect();
            task_id(&task.id)
                && text(&task.task, 2048)
                && edges.len() == task.after.len()
                && edges.is_subset(&seen)
                && seen.insert(task.id.as_str())
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkCheckpoint {
    pub summary: String,
    pub made_progress: bool,
    pub completed_steps: Vec<String>,
    pub artifacts: Vec<String>,
    pub next: AutoWorkNext,
}

impl AutoWorkCheckpoint {
    fn valid(&self) -> bool {
        text(&self.summary, 8192)
            && self.completed_steps.len() <= 32
            && self.artifacts.len() <= 32
            && self.completed_steps.iter().all(|step| text(step, 512))
            && self.artifacts.iter().all(|reference| text(reference, 4096))
            && match &self.next {
                AutoWorkNext::Continue { task } => text(task, 8192),
                AutoWorkNext::Wait { task, reason } => text(task, 8192) && text(reason, 2048),
                AutoWorkNext::Review { reason } | AutoWorkNext::Blocked { reason } => {
                    text(reason, 2048)
                }
                AutoWorkNext::Parallel { tasks, then } => text(then, 8192) && valid_plan(tasks),
                AutoWorkNext::Done => true,
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoWorkPhase {
    Ready,
    Running,
    Waiting,
    Review,
    Held,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkSegment {
    pub job_id: String,
    pub session_id: String,
    pub generation: String,
    pub prepared_at: String,
    pub checkpoint: Option<AutoWorkCheckpoint>,
}

impl AutoWorkSegment {
    fn valid_for(&self, generation: Option<&str>) -> bool {
        generation == Some(self.generation.as_str())
            && text(&self.job_id, 128)
            && text(&self.session_id, 128)
            && time(&self.prepared_at)
            && self
                .checkpoint
                .as_ref()
                .is_none_or(AutoWorkCheckpoint::valid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkChild {
    pub parent_job_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkBatch {
    pub parent_job_id: String,
    pub tasks: Vec<AutoWorkTask>,
    pub running: BTreeMap<String, AutoWorkSegment>,
    pub finished: BTreeMap<String, AutoWorkRecord>,
}

impl AutoWorkBatch {
    fn valid_for(&self, generation: Option<&str>) -> bool {
        let mut jobs = BTreeSet::from([self.parent_job_id.as_str()]);
        text(&self.parent_job_id, 128)
            && valid_plan(&self.tasks)
            && self.running.len() <= 2
            && self.running.iter().all(|(id, segment)| {
                !self.finished.contains_key(id)
                    && self.tasks.iter().any(|task| &task.id == id)
                    && segment.valid_for(generation)
                    && jobs.insert(segment.job_id.as_str())
            })
            && self.finished.iter().all(|(id, record)| {
                self.tasks.iter().any(|task| &task.id == id)
                    && record.valid()
                    && jobs.insert(record.job_id.as_str())
                    && record.child.as_ref().is_some_and(|link| {
                        link.task_id == *id && link.parent_job_id == self.parent_job_id
                    })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoWorkOutcome {
    Reported,
    Failed,
    Indeterminate,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkRecord {
    pub job_id: String,
    pub session_id: String,
    pub recorded_at: String,
    pub checkpoint: Option<AutoWorkCheckpoint>,
    pub outcome: AutoWorkOutcome,
    pub message: Option<String>,
    #[serde(default)]
    pub child: Option<AutoWorkChild>,
}

impl AutoWorkRecord {
    fn valid(&self) -> bool {
        text(&self.job_id, 128)
            && text(&self.session_id, 128)
            && time(&self.recorded_at)
            && self
                .checkpoint
                .as_ref()
                .is_none_or(AutoWorkCheckpoint::valid)
            && self
                .message
                .as_deref()
                .is_none_or(|message| text(message, 2048))
            && self.child.as_ref().is_none_or(|child| {
                child.parent_job_id != self.job_id
                    && text(&child.parent_job_id, 128)
                    && task_id(&child.task_id)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityAutoWork {
    pub activity_id: String,
    pub owner_uid: u32,
    pub revision: u64,
    pub config: AutoWorkDraft,
    pub enabled: bool,
    pub generation: Option<String>,
    pub phase: AutoWorkPhase,
    pub next_task: String,
    pub next_wake_at: Option<String>,
    pub event_after: u64,
    pub current: Option<AutoWorkSegment>,
    #[serde(default)]
    pub parallel: Option<AutoWorkBatch>,
    pub last_checkpoint: Option<AutoWorkRecord>,
    pub stagnant_segments: u32,
    pub message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ActivityAutoWork {
    pub fn has_in_flight(&self) -> bool {
        self.current.is_some()
            || self
                .parallel
                .as_ref()
                .is_some_and(|batch| !batch.running.is_empty())
    }

    pub fn matches_activity(&self, id: &str) -> bool {
        self.activity_id == id
            && canonical_uuid(id)
            && (1..=i64::MAX as u64).contains(&self.revision)
            && self.config.validate().is_ok()
            && text(&self.next_task, 8192)
            && time(&self.created_at)
            && time(&self.updated_at)
            && self.next_wake_at.as_deref().is_none_or(time)
            && self.generation.as_deref().is_none_or(canonical_uuid)
            && (!self.enabled || self.generation.is_some())
            && (self.phase == AutoWorkPhase::Running) == self.has_in_flight()
            && self
                .current
                .as_ref()
                .is_none_or(|segment| segment.valid_for(self.generation.as_deref()))
            && self.parallel.as_ref().is_none_or(|batch| {
                self.current.is_none() && batch.valid_for(self.generation.as_deref())
            })
            && self
                .last_checkpoint
                .as_ref()
                .is_none_or(AutoWorkRecord::valid)
            && self.stagnant_segments <= 3
            && self
                .message
                .as_deref()
                .is_none_or(|message| text(message, 2048))
    }

    pub fn matches_owner(&self, id: &str, owner: u32) -> bool {
        self.matches_activity(id) && self.owner_uid == owner
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityAutoWorkResponse {
    pub schema: u32,
    pub activity_id: String,
    #[serde(deserialize_with = "required_nullable")]
    pub work: Option<ActivityAutoWork>,
    pub history: Vec<AutoWorkRecord>,
}

impl ActivityAutoWorkResponse {
    pub fn matches_activity(&self, id: &str) -> bool {
        self.schema == 1
            && self.activity_id == id
            && canonical_uuid(id)
            && self.history.len() <= 20
            && self
                .work
                .as_ref()
                .is_none_or(|work| work.matches_activity(id))
            && self.history.iter().all(AutoWorkRecord::valid)
    }

    pub fn matches_owner(&self, id: &str, owner: u32) -> bool {
        self.matches_activity(id)
            && self
                .work
                .as_ref()
                .is_none_or(|work| work.owner_uid == owner)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkQuery {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkSetRequest {
    #[serde(deserialize_with = "required_nullable")]
    pub expected_revision: Option<u64>,
    pub work: AutoWorkDraft,
}

impl AutoWorkSetRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        revision(self.expected_revision)?;
        self.work.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWorkEnabledRequest {
    pub expected_revision: u64,
    pub enabled: bool,
    pub next_task: Option<String>,
    pub review_note: Option<String>,
}

impl AutoWorkEnabledRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        revision(Some(self.expected_revision))?;
        if (!self.enabled && (self.next_task.is_some() || self.review_note.is_some()))
            || self
                .next_task
                .as_deref()
                .is_some_and(|task| !text(task, 8192))
            || self
                .review_note
                .as_deref()
                .is_some_and(|note| !text(note, 2048))
        {
            return Err("Invalid automatic-work resume details");
        }
        Ok(())
    }
}

fn revision(value: Option<u64>) -> Result<(), &'static str> {
    if value.is_some_and(|value| value == 0 || value >= i64::MAX as u64) {
        return Err("Work revision must be positive and incrementable");
    }
    Ok(())
}
fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= limit
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
fn time(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value).is_ok()
}
fn canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/automatic_work.rs"
    ));
}

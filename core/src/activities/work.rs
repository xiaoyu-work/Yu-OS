//! Durable, caller-reported work handoffs. Neither progress nor enablement is authority.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{parse_id, validate_text, ActivityError};

pub const MAX_WORK_HISTORY: usize = 1000;
pub const MAX_WORK_CHECKPOINT_BYTES: usize = 32 * 1024;
pub const MAX_STAGNANT_WORK_SEGMENTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkEvent {
    pub source: String,
    pub event_type: String,
}

impl ActivityWorkEvent {
    pub fn validate(&self) -> Result<(), ActivityError> {
        for (name, value) in [
            ("event source", &self.source),
            ("event type", &self.event_type),
        ] {
            validate_text(name, value, 128, false, false)?;
            if value.trim() != value {
                return Err(ActivityError::Invalid(format!("{name} must be trimmed")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkDraft {
    pub task: String,
    pub check_interval_secs: u32,
    pub wake_event: Option<ActivityWorkEvent>,
}

impl ActivityWorkDraft {
    pub fn validate(&self) -> Result<(), ActivityError> {
        validate_text("work task", &self.task, 8192, false, true)?;
        if !(60..=86400).contains(&self.check_interval_secs) {
            return Err(ActivityError::Invalid(
                "work check interval must be between 60 and 86400 seconds".into(),
            ));
        }
        if let Some(event) = &self.wake_event {
            event.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivityWorkNext {
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
        tasks: Vec<ActivityWorkTask>,
        then: String,
    },
    Done,
}

pub const MAX_PARALLEL_TASKS: usize = 6;
const MAX_IN_FLIGHT_CHILDREN: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkTask {
    pub id: String,
    pub task: String,
    pub after: Vec<String>,
}

fn valid_task_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

fn validate_plan(tasks: &[ActivityWorkTask]) -> Result<(), ActivityError> {
    if !(2..=MAX_PARALLEL_TASKS).contains(&tasks.len()) {
        return Err(ActivityError::Invalid(
            "parallel work requires two to six subtasks".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for task in tasks {
        validate_text("parallel subtask", &task.task, 2048, false, true)?;
        let parents: BTreeSet<_> = task.after.iter().map(String::as_str).collect();
        if !valid_task_id(&task.id)
            || seen.contains(task.id.as_str())
            || parents.len() != task.after.len()
            || !parents.is_subset(&seen)
        {
            return Err(ActivityError::Invalid(
                "parallel task IDs must be unique and dependencies must name distinct earlier subtasks".into(),
            ));
        }
        seen.insert(task.id.as_str());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkCheckpoint {
    pub summary: String,
    pub made_progress: bool,
    pub completed_steps: Vec<String>,
    pub artifacts: Vec<String>,
    pub next: ActivityWorkNext,
}

impl ActivityWorkCheckpoint {
    pub(crate) fn validate_role(&self, child: bool) -> Result<(), ActivityError> {
        if (child
            && !matches!(
                self.next,
                ActivityWorkNext::Done
                    | ActivityWorkNext::Review { .. }
                    | ActivityWorkNext::Blocked { .. }
            ))
            || (!child && matches!(self.next, ActivityWorkNext::Done))
        {
            return Err(ActivityError::Invalid(
                "invalid handoff for this work role".into(),
            ));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ActivityError> {
        validate_text("work summary", &self.summary, 8192, false, true)?;
        if self.completed_steps.len() > 32 || self.artifacts.len() > 32 {
            return Err(ActivityError::Invalid(
                "a work handoff allows at most 32 completed steps and 32 artifact references"
                    .into(),
            ));
        }
        for step in &self.completed_steps {
            validate_text("completed step", step, 512, false, false)?;
        }
        let mut references = std::collections::BTreeSet::new();
        for reference in &self.artifacts {
            let parsed = crate::objects::parse_reference(reference)
                .map_err(|error| ActivityError::Invalid(error.to_string()))?;
            if crate::objects::format_reference(&parsed)
                .map_err(|error| ActivityError::Invalid(error.to_string()))?
                != *reference
                || !references.insert(reference)
            {
                return Err(ActivityError::Invalid(
                    "artifact references must be canonical and unique".into(),
                ));
            }
        }
        match &self.next {
            ActivityWorkNext::Continue { task } => {
                validate_text("next work task", task, 8192, false, true)?;
            }
            ActivityWorkNext::Wait { task, reason } => {
                validate_text("next work task", task, 8192, false, true)?;
                validate_text("wait reason", reason, 2048, false, true)?;
            }
            ActivityWorkNext::Review { reason } | ActivityWorkNext::Blocked { reason } => {
                validate_text("work stop reason", reason, 2048, false, true)?;
            }
            ActivityWorkNext::Parallel { tasks, then } => {
                validate_plan(tasks)?;
                validate_text("work after parallel tasks", then, 8192, false, true)?;
            }
            ActivityWorkNext::Done => {}
        }
        if serde_json::to_vec(self)?.len() > MAX_WORK_CHECKPOINT_BYTES {
            return Err(ActivityError::Invalid("work handoff exceeds 32 KiB".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityWorkPhase {
    Ready,
    Running,
    Waiting,
    Review,
    Held,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkSegment {
    pub job_id: String,
    pub session_id: String,
    pub generation: String,
    pub prepared_at: String,
    pub checkpoint: Option<ActivityWorkCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkChild {
    pub parent_job_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkBatch {
    pub parent_job_id: String,
    pub tasks: Vec<ActivityWorkTask>,
    pub running: BTreeMap<String, ActivityWorkSegment>,
    pub finished: BTreeMap<String, ActivityWorkRecord>,
}

impl ActivityWorkBatch {
    pub(crate) fn failed(&self) -> bool {
        self.finished
            .values()
            .any(|record| !record.child_reported_done())
    }

    fn succeeded(&self) -> bool {
        self.finished.len() == self.tasks.len() && !self.failed()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWorkRecord {
    pub job_id: String,
    pub session_id: String,
    pub recorded_at: String,
    pub checkpoint: Option<ActivityWorkCheckpoint>,
    pub outcome: String,
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<ActivityWorkChild>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWork {
    pub activity_id: String,
    pub owner_uid: u32,
    pub revision: u64,
    pub config: ActivityWorkDraft,
    pub enabled: bool,
    pub generation: Option<String>,
    pub phase: ActivityWorkPhase,
    pub next_task: String,
    pub next_wake_at: Option<String>,
    pub event_after: u64,
    pub current: Option<ActivityWorkSegment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<ActivityWorkBatch>,
    pub last_checkpoint: Option<ActivityWorkRecord>,
    pub stagnant_segments: u32,
    pub message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ActivityWork {
    pub fn in_flight(&self) -> impl Iterator<Item = &ActivityWorkSegment> {
        self.current.iter().chain(
            self.parallel
                .iter()
                .flat_map(|batch| batch.running.values()),
        )
    }

    pub fn has_in_flight(&self) -> bool {
        self.in_flight().next().is_some()
    }

    pub fn segment(&self, job_id: &str) -> Option<&ActivityWorkSegment> {
        self.in_flight().find(|segment| segment.job_id == job_id)
    }

    pub(crate) fn next_child(&self) -> Option<&ActivityWorkTask> {
        let batch = self.parallel.as_ref()?;
        if !self.enabled
            || !matches!(
                self.phase,
                ActivityWorkPhase::Ready | ActivityWorkPhase::Running
            )
            || self.current.is_some()
            || batch.running.len() >= MAX_IN_FLIGHT_CHILDREN
            || batch.failed()
        {
            return None;
        }
        batch.tasks.iter().find(|task| {
            !batch.running.contains_key(&task.id)
                && !batch.finished.contains_key(&task.id)
                && task.after.iter().all(|id| {
                    batch
                        .finished
                        .get(id)
                        .is_some_and(ActivityWorkRecord::child_reported_done)
                })
        })
    }

    pub fn due_at(&self, now: DateTime<Utc>) -> Result<bool, ActivityError> {
        if !self.enabled
            || self.has_in_flight()
            || self
                .parallel
                .as_ref()
                .is_some_and(|batch| !batch.succeeded())
        {
            return Ok(false);
        }
        if !matches!(
            self.phase,
            ActivityWorkPhase::Ready | ActivityWorkPhase::Waiting
        ) {
            return Ok(false);
        }
        self.next_wake_at
            .as_deref()
            .map(|value| work_time(value).map(|at| at <= now))
            .transpose()
            .map(|due| due.unwrap_or(self.phase == ActivityWorkPhase::Ready))
    }

    pub fn validate(&self) -> Result<(), ActivityError> {
        if parse_id(&self.activity_id)? != self.activity_id
            || self.revision == 0
            || self.revision > i64::MAX as u64
        {
            return Err(ActivityError::Invalid(
                "invalid work identity or revision".into(),
            ));
        }
        self.config.validate()?;
        validate_text("next work task", &self.next_task, 8192, false, true)?;
        work_time(&self.created_at)?;
        if work_time(&self.updated_at)? < work_time(&self.created_at)? {
            return Err(ActivityError::Invalid(
                "work timestamps are reversed".into(),
            ));
        }
        if let Some(at) = &self.next_wake_at {
            work_time(at)?;
        }
        if let Some(generation) = &self.generation {
            if parse_id(generation)? != *generation {
                return Err(ActivityError::Invalid("invalid work generation".into()));
            }
        }
        if self.enabled && self.generation.is_none() {
            return Err(ActivityError::Invalid(
                "enabled work has no scheduler binding".into(),
            ));
        }
        if (self.phase == ActivityWorkPhase::Running) != self.has_in_flight()
            || (self.current.is_some() && self.parallel.is_some())
        {
            return Err(ActivityError::Invalid(
                "work phase and current Job disagree".into(),
            ));
        }
        for segment in self.in_flight() {
            super::execution_limits::validate_job_id(&segment.job_id)?;
            validate_text("work session", &segment.session_id, 128, false, false)?;
            if self.generation.as_deref() != Some(segment.generation.as_str()) {
                return Err(ActivityError::Invalid(
                    "work segment generation differs".into(),
                ));
            }
            if let Some(batch) = &self.parallel {
                validate_plan(&batch.tasks)?;
                super::execution_limits::validate_job_id(&batch.parent_job_id)?;
                let mut jobs = BTreeSet::from([batch.parent_job_id.as_str()]);
                if batch.running.len() > MAX_IN_FLIGHT_CHILDREN
                    || batch
                        .running
                        .keys()
                        .chain(batch.finished.keys())
                        .any(|id| !batch.tasks.iter().any(|task| task.id == *id))
                    || batch.running.iter().any(|(id, segment)| {
                        batch.finished.contains_key(id) || !jobs.insert(&segment.job_id)
                    })
                {
                    return Err(ActivityError::Invalid(
                        "invalid parallel work reservation".into(),
                    ));
                }
                for (id, record) in &batch.finished {
                    record.validate()?;
                    if !jobs.insert(&record.job_id)
                        || record.child.as_ref().is_none_or(|link| {
                            link.parent_job_id != batch.parent_job_id || link.task_id != *id
                        })
                    {
                        return Err(ActivityError::Invalid(
                            "invalid parallel work result identity".into(),
                        ));
                    }
                }
            }
            work_time(&segment.prepared_at)?;
            if let Some(checkpoint) = &segment.checkpoint {
                checkpoint.validate()?;
            }
        }
        if let Some(record) = &self.last_checkpoint {
            record.validate()?;
        }
        if let Some(message) = &self.message {
            validate_text("work message", message, 2048, false, true)?;
        }
        if self.stagnant_segments > MAX_STAGNANT_WORK_SEGMENTS {
            return Err(ActivityError::Invalid(
                "invalid work stagnation counter".into(),
            ));
        }
        Ok(())
    }
}

impl ActivityWorkRecord {
    pub(crate) fn child_reported_done(&self) -> bool {
        self.outcome == "reported"
            && self
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| matches!(checkpoint.next, ActivityWorkNext::Done))
    }

    pub fn validate(&self) -> Result<(), ActivityError> {
        super::execution_limits::validate_job_id(&self.job_id)?;
        validate_text("work session", &self.session_id, 128, false, false)?;
        work_time(&self.recorded_at)?;
        if !matches!(
            self.outcome.as_str(),
            "reported" | "failed" | "indeterminate" | "cancelled"
        ) {
            return Err(ActivityError::Invalid(
                "invalid work segment outcome".into(),
            ));
        }
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.validate()?;
            checkpoint.validate_role(self.child.is_some())?;
        }
        if let Some(child) = &self.child {
            super::execution_limits::validate_job_id(&child.parent_job_id)?;
            if child.parent_job_id == self.job_id || !valid_task_id(&child.task_id) {
                return Err(ActivityError::Invalid(
                    "invalid parent or subtask identity".into(),
                ));
            }
        }
        if let Some(message) = &self.message {
            validate_text("work message", message, 2048, false, true)?;
        }
        Ok(())
    }
}

/// Trusted consumer operations, deliberately not a wire-deserializable enum.
#[derive(Debug, Clone)]
pub enum ActivityWorkChange {
    Configure(ActivityWorkDraft),
    Enable {
        generation: String,
        next_task: Option<String>,
        review_note: Option<String>,
    },
    Disable,
    Reserve {
        generation: String,
        job_id: String,
        session_id: String,
    },
    ReserveChild {
        generation: String,
        job_id: String,
        session_id: String,
        task_id: String,
    },
    Stage {
        job_id: String,
        checkpoint: ActivityWorkCheckpoint,
    },
    Finish {
        job_id: String,
        outcome: ActivityWorkOutcome,
        event_after: u64,
    },
    Wake {
        generation: String,
        event_index: u64,
    },
    Hold(String),
}

#[derive(Debug, Clone)]
pub enum ActivityWorkOutcome {
    Reported,
    Failed(String),
    Indeterminate(String),
    Cancelled,
}

pub(crate) fn work_time(value: &str) -> Result<DateTime<Utc>, ActivityError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ActivityError::Invalid("work timestamp must be RFC3339".into()))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/activities/work.rs"
    ));
}

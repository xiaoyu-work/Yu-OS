//! Automatic Activity progression through the existing authenticated scheduler.

use super::*;
use crate::activities::{
    ActivityService, ActivityWork, ActivityWorkChange, ActivityWorkCheckpoint, ActivityWorkNext,
    ActivityWorkOutcome, ActivityWorkPhase, ActivityWorkRecord, ActivityWorkTask,
};
use crate::agent::service::{ExecutionPhase, Job, JobStatus, Store};

pub(crate) fn rule_id(activity_id: &str) -> String {
    format!("activity-work-{activity_id}")
}

pub(super) fn preflight_enable(
    owner: u32,
    activity_id: &str,
    args: &[String],
) -> Result<(), String> {
    let work = crate::activities::open_default()
        .map_err(|error| error.to_string())?
        .work(owner, activity_id)
        .map_err(|error| error.to_string())?
        .ok_or("configure Activity work first")?;
    let expected = flag(args, "work-revision")
        .ok_or("work revision is missing")?
        .parse::<u64>()
        .map_err(|_| "invalid work revision")?;
    if work.revision != expected || work.has_in_flight() {
        return Err("work changed or still has an in-flight Job; refresh before enabling".into());
    }
    if flag(args, "id").as_deref() != Some(rule_id(activity_id).as_str())
        || flag(args, "prompt").as_deref() != Some(work.config.task.as_str())
        || flag(args, "contains").is_some()
    {
        return Err("automatic work rule differs from its Activity configuration".into());
    }
    let (source, event_type) = match &work.config.wake_event {
        Some(event) => (event.source.as_str(), event.event_type.as_str()),
        None => ("activity-work-disabled-events", "none"),
    };
    if flag(args, "source").as_deref() != Some(source)
        || flag(args, "event-type").as_deref() != Some(event_type)
    {
        return Err("automatic work event filter differs from its Activity configuration".into());
    }
    if matches!(
        work.phase,
        ActivityWorkPhase::Review | ActivityWorkPhase::Held
    ) && (flag(args, "work-review-note").is_none() || flag(args, "work-next-task").is_none())
    {
        return Err(
            "inspect previous work and supply an explicit next task and review note".into(),
        );
    }
    if let Some(task) = flag(args, "work-next-task") {
        crate::activities::validate_work_task(&task).map_err(|error| error.to_string())?;
    }
    if let Some(note) = flag(args, "work-review-note") {
        if note.len() > 2048 || note.contains('\0') {
            return Err("work review note is invalid or exceeds 2048 bytes".into());
        }
        if let Some(task_id) = flag(args, "work-retry-task") {
            let task = flag(args, "work-next-task").ok_or("a retry requires a safe next task")?;
            let note = flag(args, "work-review-note").ok_or("a retry requires an inspection note")?;
            work.validate_retry(&task_id, &task, &note).map_err(|error| error.to_string())?;
        }
    }
    binding(owner, activity_id)?;
    Ok(())
}

pub(crate) fn binding(owner: u32, activity_id: &str) -> Result<Option<TriggerRule>, String> {
    let path = rule_path(&rule_id(activity_id));
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err("Activity work scheduler binding is not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect Activity work scheduler binding: {error}")),
    }
    let rule = load_rule(&rule_id(activity_id))?;
    if rule.owner_uid != Some(owner)
        || rule.activity_id.as_deref() != Some(activity_id)
        || !rule.activity_work
    {
        return Err("Activity work scheduler binding identity differs".into());
    }
    Ok(Some(rule))
}

pub(crate) fn admit_job(job: &Job) -> Result<(), String> {
    let generation = job
        .activity_work_generation
        .as_deref()
        .ok_or("Job is not automatic Activity work")?;
    let owner = job.owner_uid.ok_or("automatic work Job has no owner")?;
    let activity_id = job
        .activity_id
        .as_deref()
        .ok_or("automatic work Job has no Activity")?;
    let work = crate::activities::open_default()
        .map_err(|error| error.to_string())?
        .work(owner, activity_id)
        .map_err(|error| error.to_string())?
        .ok_or("automatic work state is missing; replay is refused")?;
    let segment = work.segment(&job.id).ok_or("automatic work Job is not reserved")?;
    if !work.enabled
        || work.generation.as_deref() != Some(generation)
        || job.session_id.as_deref() != Some(segment.session_id.as_str())
        || job.prompt != work.config.task
    {
        return Err("automatic work Job no longer matches its enabled Activity binding".into());
    }
    let rule =
        binding(owner, activity_id)?.ok_or("automatic work scheduler authority is missing")?;
    if !rule.enabled || rule.generation.as_deref() != Some(generation) {
        return Err("automatic work scheduler binding is disabled or replaced".into());
    }
    execution_owner(&rule)?;
    Ok(())
}

pub(crate) fn stage_checkpoint(
    job: &Job,
    checkpoint: ActivityWorkCheckpoint,
) -> Result<(), String> {
    let owner = job.owner_uid.ok_or("work report has no owner")?;
    let activity_id = job
        .activity_id
        .as_deref()
        .ok_or("work report has no Activity")?;
    let generation = job
        .activity_work_generation
        .as_deref()
        .ok_or("a non-automatic task cannot report an automatic work handoff")?;
    let service = crate::activities::open_default().map_err(|error| error.to_string())?;
    let work = service
        .work(owner, activity_id)
        .map_err(|error| error.to_string())?
        .ok_or("Activity work state is missing")?;
    if work.generation.as_deref() != Some(generation)
        || work.segment(&job.id).is_none_or(|current| {
            job.session_id.as_deref() != Some(current.session_id.as_str())
        })
    {
        return Err("handoff does not match the broker-owned work Job/session/generation".into());
    }
    checkpoint.validate().map_err(|error| error.to_string())?;
    checkpoint.validate_role(work.current.is_none()).map_err(|error| error.to_string())?;
    if let Some(sid)=job.session_id.as_deref(){
        let sid=sid.parse::<crate::session::SessionId>().map_err(|e|e.to_string())?;
        if crate::workspaces::context_access::has_temporary(&sid)?{
            return Err("temporary context cannot be automatically handed to another segment; inspect the result and explicitly supply a fresh next task".into());
        }
    }
    crate::yuosd::activity_receipts::attach_reported_artifacts(
        owner,
        activity_id,
        "agent",
        &checkpoint.artifacts,
    )?;
    let mut checkpoint = checkpoint;
    let redactor = crate::agent::safety::redact::Redactor::default_set();
    checkpoint.summary = redactor.redact(&checkpoint.summary);
    for step in &mut checkpoint.completed_steps {
        *step = redactor.redact(step);
    }
    match &mut checkpoint.next {
        ActivityWorkNext::Continue { task } => *task = redactor.redact(task),
        ActivityWorkNext::Wait { task, reason } => {
            *task = redactor.redact(task);
            *reason = redactor.redact(reason);
        }
        ActivityWorkNext::Review { reason } | ActivityWorkNext::Blocked { reason } => {
            *reason = redactor.redact(reason)
        }
        ActivityWorkNext::Parallel { tasks, then } => {
            *then = redactor.redact(then);
            for task in tasks {
                task.task = redactor.redact(&task.task);
            }
        }
        ActivityWorkNext::Done => {}
    }
    service
        .change_work(
            owner,
            activity_id,
            None,
            ActivityWorkChange::Stage {
                job_id: job.id.clone(),
                checkpoint,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn tick_all(event_after: u64) -> Result<(), String> {
    let path = crate::paths::data_dir().join("activities.db");
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("inspect Activity work storage: {error}")),
    }
    let service = crate::activities::open_default().map_err(|error| error.to_string())?;
    let entries = service.work_entries().map_err(|error| error.to_string())?;
    for (owner, activity_id) in entries {
        if let Err(error) = tick_one(&service, owner, &activity_id, event_after) {
            tracing::warn!(owner_uid=owner, activity_id, %error, "Activity work reconciliation failed");
        }
    }
    Ok(())
}

fn tick_one(
    service: &impl ActivityService,
    owner: u32,
    activity_id: &str,
    event_after: u64,
) -> Result<(), String> {
    let Some(mut work) = service
        .work(owner, activity_id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    if !work.enabled && !work.has_in_flight() {
        return Ok(());
    }
    let store = Store::open_default().map_err(|error| error.to_string())?;
    let segments = work.in_flight().cloned().collect::<Vec<_>>();
    for current in segments {
        let located = store
            .locate_for_owner(&current.job_id, Some(owner))
            .map_err(|error| error.to_string())?;
        let outcome = match located {
            Some((_, job)) if job.activity_id.as_deref() == Some(activity_id)
                && job.activity_work_generation.as_ref() == work.generation.as_ref()
                && job.session_id.as_deref() == Some(current.session_id.as_str())
                && job.prompt == work.config.task => {
                match job.status {
                    JobStatus::Pending | JobStatus::Running | JobStatus::WaitingApproval => continue,
                    JobStatus::Ok => ActivityWorkOutcome::Reported,
                    JobStatus::Cancelled => ActivityWorkOutcome::Cancelled,
                    JobStatus::Error if job.execution_phase == ExecutionPhase::Indeterminate =>
                        ActivityWorkOutcome::Indeterminate(bounded_message(job.error.as_deref()
                            .unwrap_or("The Job outcome is indeterminate."))),
                    JobStatus::Error => ActivityWorkOutcome::Failed(bounded_message(job.error.as_deref()
                        .unwrap_or("The work Job failed."))),
                }
            }
            _ => ActivityWorkOutcome::Indeterminate(
                "The reserved work Job is missing or its identity differs. No automatic publication or replay; inspect retained work before resuming.".into()),
        };
        work = service
            .change_work(
                owner,
                activity_id,
                Some(work.revision),
                ActivityWorkChange::Finish {
                    job_id: current.job_id.clone(),
                    outcome,
                    event_after,
                },
            )
            .map_err(|error| error.to_string())?;
        if matches!(
            work.phase,
            ActivityWorkPhase::Review | ActivityWorkPhase::Held
        ) {
            notify_review(&work);
        }
    }
    if !work.enabled || (work.next_child().is_none()
        && !work.due_at(chrono::Utc::now()).map_err(|error| error.to_string())?)
    {
        return Ok(());
    }
    let activity = service
        .get(owner, activity_id)
        .map_err(|error| error.to_string())?;
    if !activity.state.allows_work() {
        return Ok(());
    }
    let rule = binding(owner, activity_id)?;
    let Some(rule) = rule.filter(|rule| rule.enabled && rule.generation == work.generation) else {
        if !work.has_in_flight() {
            hold(service, &work, "The scheduler binding was removed, disabled or replaced; explicitly review and re-enable automatic work.")?;
        }
        return Ok(());
    };
    if let Err(error) = validate_activity_trigger(owner, activity_id)
        .and_then(|_| execution_owner(&rule).map(|_| ()))
    {
        if !work.has_in_flight() {
            hold(service, &work, &error)?;
        }
        return Ok(());
    }
    let active = store
        .list_for_activity(owner, activity_id, usize::MAX)
        .map_err(|error| error.to_string())?
        .into_iter()
        .any(|job| {
            work.segment(&job.id).is_none() && matches!(
                job.status,
                JobStatus::Pending | JobStatus::Running | JobStatus::WaitingApproval
            )
        });
    if active {
        return Ok(());
    }
    use crate::workspaces::WorkspaceService;
    let domain=crate::workspaces::open_default().and_then(|service|service.activity_workspace(owner,activity_id))
        .map_err(|e|e.to_string())?.map(|workspace|workspace.id);
    loop {
        let child = work.next_child();
        if child.is_none() && !work.due_at(chrono::Utc::now()).map_err(|error| error.to_string())? {
            break;
        }
        let task_id = child.map(|task| task.id.clone());
        let context = match scheduled_context(&work, child, domain.as_deref()) {
            Ok(context) => context,
            Err(error) => {
                if !work.has_in_flight() {
                    hold(service, &work, &error)?;
                }
                return Ok(());
            }
        };
        let generation = work
                .generation
                .clone()
                .ok_or("work generation is missing")?;
        let mut job = prepare_job(&rule, work.config.task.clone())?;
        job.activity_work_generation = Some(generation.clone());
        job.context = Some(context.render_fenced(crate::agent::trust::envelope::process_seal()));
        let session_id = job.session_id.clone().ok_or("work session is missing")?;
        let change = if let Some(task_id) = task_id {
            ActivityWorkChange::ReserveChild { generation, job_id: job.id.clone(), session_id, task_id }
        } else {
            ActivityWorkChange::Reserve { generation, job_id: job.id.clone(), session_id }
        };
        match service.change_work(owner, activity_id, Some(work.revision), change) {
            Ok(reserved) => work = reserved,
            Err(error) => {
                end_unpublished_session(&job);
                return Err(error.to_string());
            }
        }
        // A publish failure is reconciled by exact reservation, never by sending a replacement.
        store.publish(job).map_err(|error| format!("publish Activity work: {error}"))?;
    }
    Ok(())
}

fn scheduled_context(
    work: &ActivityWork,
    child: Option<&ActivityWorkTask>,
    domain: Option<&str>,
) -> Result<crate::agent::trust::LabeledSegment, String> {
    let visible = |record: &ActivityWorkRecord| -> Result<bool, String> {
        let meta = checkpoint_session_meta(record, work.owner_uid, &work.activity_id)?;
        Ok(meta.workspace_id.as_deref() == domain
            && !crate::workspaces::context_access::has_temporary(&meta.id)?)
    };
    let Some(batch) = &work.parallel else {
        let prior = match &work.last_checkpoint {
            Some(record) if visible(record)? => Some(record),
            _ => None,
        };
        return handoff_segment(&work.next_task, prior);
    };
    let mut reports = Vec::new();
    for (id, record) in &batch.finished {
        if child.is_some_and(|task| !task.after.contains(id)) || !visible(record)? {
            continue;
        }
        if let Some(checkpoint) = &record.checkpoint {
            let summary: String = checkpoint.summary.chars().take(512).collect();
            let artifacts = checkpoint.artifacts.iter().filter(|reference| reference.len() <= 1024)
                .take(2).collect::<Vec<_>>();
            reports.push(serde_json::json!({
                "task_id": id, "summary": summary, "artifact_count": checkpoint.artifacts.len(), "artifacts": artifacts,
            }));
        }
    }
    let payload = serde_json::json!({
        "role": if child.is_some() { "subtask" } else { "coordinator" },
        "next_task": child.map_or(work.next_task.as_str(), |task| batch.instruction(task)),
        "task_id": child.map(|task| &task.id),
        "parent_job_id": batch.parent_job_id,
        "reports": reports,
        "source": "caller_reported",
        "effects_verified": false,
        "goal_completed": false,
    });
    Ok(crate::agent::trust::LabeledSegment::of(
        crate::agent::trust::SourceKind::ActivityWorkCheckpoint,
        payload.to_string(),
    ))
}

fn checkpoint_session_meta(
    record: &crate::activities::ActivityWorkRecord,
    owner: u32,
    activity_id: &str,
) -> Result<crate::session::SessionMeta, String> {
    let sid = record
        .session_id
        .parse::<crate::session::SessionId>()
        .map_err(|error| error.to_string())?;
    let meta = crate::session::get_meta(&sid).map_err(|error| error.to_string())?;
    // Automatic segments belong to the scheduler, not resumable Agent conversations.
    if meta.id != sid
        || meta.owner_uid != Some(owner)
        || meta.activity_id.as_deref() != Some(activity_id)
        || meta.creator_runtime.as_deref() != Some("trigger")
        || meta.origin != Some(crate::session::SessionOrigin::TriggerDelegation)
        || meta.client != trigger_client()
    {
        return Err("automatic work checkpoint has a mismatched scheduler session".into());
    }
    Ok(meta)
}

pub(crate) fn handoff_segment(
    next_task: &str,
    last_handoff: Option<&crate::activities::ActivityWorkRecord>,
) -> Result<crate::agent::trust::LabeledSegment, String> {
    crate::activities::validate_work_task(next_task).map_err(|error| error.to_string())?;
    if let Some(record) = last_handoff {
        record.validate().map_err(|error| error.to_string())?;
    }
    #[derive(serde::Serialize)]
    struct Context<'a> {
        next_task: &'a str,
        source: &'static str,
        effects_verified: bool,
        goal_completed: bool,
        last_handoff: Option<&'a crate::activities::ActivityWorkRecord>,
    }
    let payload = serde_json::to_string(&Context {
        next_task,
        source: "caller_reported",
        effects_verified: false,
        goal_completed: false,
        last_handoff,
    })
    .map_err(|error| error.to_string())?;
    Ok(crate::agent::trust::LabeledSegment::of(
        crate::agent::trust::SourceKind::ActivityWorkCheckpoint,
        payload,
    ))
}

fn hold(service: &impl ActivityService, work: &ActivityWork, message: &str) -> Result<(), String> {
    let held = service
        .change_work(
            work.owner_uid,
            &work.activity_id,
            Some(work.revision),
            ActivityWorkChange::Hold(bounded_message(message)),
        )
        .map_err(|error| error.to_string())?;
    notify_review(&held);
    Ok(())
}

fn bounded_message(message: &str) -> String {
    let redacted = crate::agent::safety::redact::Redactor::default_set().redact(message);
    let message = redacted.as_str();
    let mut end = message.len().min(2048);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_string()
}

fn notify_review(work: &ActivityWork) {
    let mut notification = crate::notifications::NotificationDraft::new(
        "trigger",
        "activity.work.review",
        crate::notifications::Severity::Warning,
        "Activity work needs review",
        format!(
            "Activity {}: {}",
            work.activity_id,
            work.message
                .as_deref()
                .unwrap_or("Inspect the work before continuing.")
        ),
    )
    .dedupe(format!(
        "activity-work:{}:{}",
        work.activity_id, work.revision
    ));
    notification
        .actions
        .push(crate::notifications::NotificationAction {
            id: "open-agent".into(),
            label: "Open Agent".into(),
            uri: "yuos://agent".into(),
        });
    if let Some(record) = &work.last_checkpoint {
        notification.task_id = Some(record.job_id.clone());
        notification.session_id = Some(record.session_id.clone());
    }
    if let Err(error) = crate::yuosd::notifications::publish_for_owner(work.owner_uid, notification)
    {
        tracing::warn!(activity_id=work.activity_id, %error, "failed to publish Activity work review");
    }
}

pub(super) fn on_event(rule: &TriggerRule, event_index: u64) -> Result<(), String> {
    let owner = rule.owner_uid.ok_or("work rule has no owner")?;
    let id = rule
        .activity_id
        .as_deref()
        .ok_or("work rule has no Activity")?;
    let service = crate::activities::open_default().map_err(|error| error.to_string())?;
    let Some(work) = service.work(owner, id).map_err(|error| error.to_string())? else {
        return Err("work rule has no Activity work state".into());
    };
    if work.enabled
        && work.phase == ActivityWorkPhase::Waiting
        && event_index >= work.event_after
        && rule.generation == work.generation
        && work.config.wake_event.is_some()
    {
        match service.change_work(
            owner,
            id,
            Some(work.revision),
            ActivityWorkChange::Wake {
                generation: rule
                    .generation
                    .clone()
                    .ok_or("work rule has no generation")?,
                event_index,
            },
        ) {
            Ok(_) => {}
            Err(crate::activities::ActivityError::ExecutionBlocked(
                crate::activities::ExecutionBlockedReason::Inactive,
            )) => activity::record(
                rule,
                TriggerDeliveryStatus::Blocked,
                None,
                None,
                "Activity is inactive; this event was consumed without waking automatic work.",
            )?,
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/triggers/work.rs"
    ));
}

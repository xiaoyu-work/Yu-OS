//! Progress and grouped execution history over the existing broker projections.

use super::{
    Activities, ActivityDetailResponse, ActivityState, AppMessage, DetailTab, Element, Length,
    Message, control, fl, job_view, receipt_view, styles,
};
use cosmic::theme;
use cosmic::widget::{Column, Row, container, text};
use yuos_agent_protocol::{AutoWorkNext, AutoWorkPhase, AutoWorkRecord};

impl Activities {
    pub(super) fn detail_navigation_enabled(&self) -> bool {
        self.detail.is_some()
            && self.pending.is_none()
            && self.form.is_none()
            && self.object_form.is_none()
            && !self.continuity.is_editing()
            && self.object_state.form.is_none()
            && self.execution_limits.form.is_none()
            && self.monetary_budget.form.is_none()
            && self.scheduling_priority.form.is_none()
            && self.capability_policy.form.is_none()
            && !self.automatic_work.is_editing()
    }

    pub(super) fn detail_tabs(&self) -> Element<'_, AppMessage> {
        let mut tabs = Row::new().spacing(8);
        for (tab, label) in [
            (DetailTab::Progress, fl!("activity-progress-tab")),
            (DetailTab::History, fl!("activity-history-tab")),
            (DetailTab::Settings, fl!("activity-settings-tab")),
        ] {
            tabs = tabs.push(if tab == self.detail_tab {
                container(text(label).size(14.0)).padding(8).into()
            } else {
                control(
                    label,
                    Message::DetailTab(tab),
                    self.detail_navigation_enabled(),
                )
            });
        }
        tabs.into()
    }

    pub(super) fn progress_view<'a>(
        &'a self,
        detail: &'a ActivityDetailResponse,
        available: bool,
    ) -> Element<'a, AppMessage> {
        let mut content = Column::new()
            .spacing(8)
            .push(text(fl!("activity-progress-tab")).size(18.0));
        let Some(work) = self
            .automatic_work
            .response
            .as_ref()
            .and_then(|view| view.work.as_ref())
        else {
            return content
                .push(
                    text(if let Some(error) = &self.automatic_work.load_error {
                        error.clone()
                    } else if self.automatic_work.response.is_some() {
                        fl!("activity-auto-work-unconfigured")
                    } else {
                        fl!("activity-auto-work-load")
                    })
                    .size(13.0),
                )
                .into();
        };
        let status = if !work.has_in_flight()
            && !matches!(work.phase, AutoWorkPhase::Review | AutoWorkPhase::Held)
            && (!work.enabled || detail.activity.state != ActivityState::Active)
        {
            fl!("activity-progress-stopped")
        } else {
            match work.phase {
                AutoWorkPhase::Ready => fl!("activity-progress-ready"),
                AutoWorkPhase::Running => fl!("activity-auto-work-inflight"),
                AutoWorkPhase::Waiting => fl!("activity-progress-waiting"),
                AutoWorkPhase::Review | AutoWorkPhase::Held => fl!("activity-progress-review"),
            }
        };
        content = content.push(text(status).size(14.0));
        if let Some(message) = &work.message {
            content = content.push(text(message).size(13.0));
        }
        content = content.push(
            text(fl!(
                "activity-auto-work-next",
                task = work.next_task.clone()
            ))
            .size(13.0),
        );
        if let Some(wake) = &work.next_wake_at {
            content =
                content.push(text(fl!("activity-progress-wake", time = wake.clone())).size(12.0));
        }
        if let Some(segment) = &work.current {
            content = content.push(control(
                fl!("activity-history-inspect"),
                Message::ToggleRun(segment.job_id.clone()),
                self.detail_navigation_enabled(),
            ));
        }
        if let Some(batch) = &work.parallel {
            let mut tasks = Column::new().spacing(8).push(control(
                fl!(
                    "activity-progress-parent",
                    job = batch.parent_job_id.clone()
                ),
                Message::ToggleRun(batch.parent_job_id.clone()),
                self.detail_navigation_enabled(),
            ));
            for task in &batch.tasks {
                let state = if let Some(record) = batch.finished.get(&task.id) {
                    if record.reported_done() {
                        fl!("activity-progress-reported")
                    } else {
                        fl!("activity-progress-review")
                    }
                } else if let Some(segment) = batch.running.get(&task.id) {
                    detail
                        .jobs
                        .iter()
                        .find(|job| job.id == segment.job_id)
                        .map_or_else(
                            || fl!("activity-auto-work-inflight"),
                            |job| job.status.clone(),
                        )
                } else if !work.enabled || detail.activity.state != ActivityState::Active {
                    fl!("activity-progress-stopped")
                } else if task.after.iter().all(|id| {
                    batch
                        .finished
                        .get(id)
                        .is_some_and(AutoWorkRecord::reported_done)
                }) {
                    fl!("activity-progress-ready")
                } else {
                    fl!("activity-progress-dependencies")
                };
                let mut row = Column::new()
                    .spacing(3)
                    .push(text(batch.instruction(task)).size(13.0))
                    .push(text(format!("{} · {state}", task.id)).size(12.0));
                if batch.retry_task.as_deref() == Some(task.id.as_str()) {
                    row = row.push(text(fl!("activity-subtask-retry-scheduled")).size(12.0));
                }
                if let Some(record) = batch
                    .finished
                    .get(&task.id)
                    .filter(|record| !record.reported_done())
                {
                    let reason = record.message.as_deref().or_else(|| {
                        match &record.checkpoint.as_ref()?.next {
                            AutoWorkNext::Review { reason } | AutoWorkNext::Blocked { reason } => {
                                Some(reason.as_str())
                            }
                            _ => None,
                        }
                    });
                    if let Some(reason) = reason {
                        row = row.push(text(reason).size(12.0));
                    }
                }
                if !task.after.is_empty() {
                    row = row.push(
                        text(fl!(
                            "activity-progress-after",
                            tasks = task.after.join(", ")
                        ))
                        .size(12.0),
                    );
                }
                if let Some(id) = batch
                    .running
                    .get(&task.id)
                    .map(|segment| &segment.job_id)
                    .or_else(|| batch.finished.get(&task.id).map(|record| &record.job_id))
                {
                    row = row.push(control(
                        fl!("activity-history-inspect"),
                        Message::ToggleRun(id.clone()),
                        self.detail_navigation_enabled(),
                    ));
                }
                if self.detail_navigation_enabled() {
                    let action = if work.can_retry(&task.id)
                        && detail.activity.state == ActivityState::Active
                    {
                        Some(fl!("activity-subtask-retry"))
                    } else if batch.can_edit(&task.id) && super::editable(detail.activity.state) {
                        Some(fl!("activity-subtask-edit"))
                    } else {
                        None
                    };
                    if let Some(label) = action {
                        row = row.push(control(
                            label,
                            Message::AutomaticWork(super::automatic_work::Message::Subtask(
                                task.id.clone(),
                            )),
                            available,
                        ));
                    }
                }
                if self.automatic_work.is_editing_subtask(&task.id) {
                    row = row.push(self.automatic_work_view(detail, available));
                }
                tasks = tasks.push(
                    container(row)
                        .padding(9)
                        .width(Length::Fill)
                        .class(theme::Container::custom(styles::tool_card)),
                );
            }
            content = content.push(tasks);
        }
        if let Some(checkpoint) = work
            .last_checkpoint
            .as_ref()
            .and_then(|record| record.checkpoint.as_ref())
        {
            content = content.push(
                text(fl!(
                    "activity-auto-work-handoff",
                    summary = checkpoint.summary.clone()
                ))
                .size(13.0),
            );
        }
        content.into()
    }

    pub(super) fn history_view<'a>(
        &'a self,
        detail: &'a ActivityDetailResponse,
        available: bool,
    ) -> Element<'a, AppMessage> {
        let reports = self
            .automatic_work
            .response
            .as_ref()
            .map_or(&[][..], |view| view.history.as_slice());
        let mut content = Column::new()
            .spacing(10)
            .push(text(fl!("activity-jobs")).size(18.0));
        if detail.jobs.is_empty() && reports.is_empty() {
            content = content.push(text(fl!("activity-no-jobs")).size(12.0));
        }
        for id in detail.jobs.iter().map(|job| job.id.as_str()).chain(
            reports
                .iter()
                .filter(|record| !detail.jobs.iter().any(|job| job.id == record.job_id))
                .map(|record| record.job_id.as_str()),
        ) {
            let job = detail.jobs.iter().find(|job| job.id == id);
            let report = reports.iter().find(|record| record.job_id == id);
            let title = job
                .filter(|job| !job.title.is_empty())
                .map_or(id, |job| job.title.as_str());
            let expanded = self.expanded_run.as_deref() == Some(id);
            let mut row = Column::new().spacing(6).push(control(
                format!("{} {title}", if expanded { "-" } else { "+" }),
                Message::ToggleRun(id.into()),
                true,
            ));
            if let Some(child) = report.and_then(|record| record.child.as_ref()) {
                row = row.push(
                    text(fl!(
                        "activity-history-child",
                        task = child.task_id.clone(),
                        job = child.parent_job_id.clone()
                    ))
                    .size(11.0),
                );
            }
            if let Some(job) = job {
                row = row.push(text(format!("{} · {}", job.status, job.created_at)).size(11.0));
            }
            if expanded {
                if let Some(job) = job {
                    row = row.push(job_view(
                        job,
                        available,
                        report.is_none() && detail.activity.state == ActivityState::Active,
                    ));
                } else if let Some(report) = report {
                    row = row.push(
                        cosmic::widget::button::text(fl!("activity-open-session"))
                            .on_press(AppMessage::OpenActivitySession(report.session_id.clone())),
                    );
                }
                if let Some(checkpoint) = report.and_then(|record| record.checkpoint.as_ref()) {
                    row = row.push(
                        text(fl!(
                            "activity-auto-work-handoff",
                            summary = checkpoint.summary.clone()
                        ))
                        .size(13.0),
                    );
                    if let AutoWorkNext::Parallel { tasks, .. } = &checkpoint.next {
                        for task in tasks {
                            row = row.push(text(format!("{}: {}", task.id, task.task)).size(12.0));
                            if !task.after.is_empty() {
                                row = row.push(
                                    text(fl!(
                                        "activity-progress-after",
                                        tasks = task.after.join(", ")
                                    ))
                                    .size(12.0),
                                );
                            }
                        }
                    }
                    for step in &checkpoint.completed_steps {
                        row = row.push(text(step).size(12.0));
                    }
                    for reference in &checkpoint.artifacts {
                        row = row.push(text(reference).size(12.0));
                    }
                }
                if let Some(receipts) = &self.receipts {
                    for receipt in receipts
                        .receipts
                        .iter()
                        .filter(|receipt| receipt.job_id.as_deref() == Some(id))
                    {
                        row = row.push(receipt_view(receipt));
                    }
                }
            }
            content = content.push(
                container(row)
                    .padding(10)
                    .width(Length::Fill)
                    .class(theme::Container::custom(styles::tool_card)),
            );
        }
        content.push(self.receipts_view(detail, available)).into()
    }
}

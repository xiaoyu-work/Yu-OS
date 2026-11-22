use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::super::state::{App, EntryKind, RunStatus, ToolStatus};
use super::accent;

const BRAND_BLUE: Color = Color::Rgb(0, 92, 254);

pub(super) fn spinner(frame: u64) -> &'static str {
    ["|", "/", "-", "\\"][(frame as usize) % 4]
}

/// The Yu Agent speaking mark used by every Agent client: the logo's three
/// bars shrink to 54% and back (periods 0.9 s, 0.7 s and 1.1 s) beside the
/// still blue dot. `frame` advances every 100 ms.
fn agent_mark(frame: u64) -> Vec<Span<'static>> {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let seconds = frame as f64 / 10.0;
    let bars: String = [(8.0, 0.9, 0.0), (5.2, 0.7, 0.25), (3.2, 1.1, 0.5)]
        .into_iter()
        .map(|(height, period, lead): (f64, f64, f64)| {
            let phase = ((seconds + lead) / period).fract();
            let scale = 1.0 - 0.23 * (1.0 - (std::f64::consts::TAU * phase).cos());
            LEVELS[((height * scale).round() as usize).clamp(1, 8) - 1]
        })
        .collect();
    vec![
        Span::raw(format!(" {bars}")),
        Span::styled("• ", Style::default().fg(BRAND_BLUE)),
    ]
}

fn glyph(mark: &str, color: Color) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {mark} "), Style::default().fg(color))]
}

fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

pub(super) fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if area.is_empty() {
        return;
    }
    if let Some(label) = app.pending_request {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!(" {} {label}", spinner(app.frame)),
                    Style::default().fg(accent(app)).add_modifier(Modifier::BOLD),
                ),
                Line::styled(
                    "   Waiting for broker confirmation | Esc stop",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            area,
        );
        return;
    }
    if app.active_task.is_none() {
        return;
    }
    let tools = || {
        app.entries
            .iter()
            .rev()
            .filter_map(|entry| match entry.kind {
                EntryKind::Tool { status, .. } => Some((entry.text.as_str(), status)),
                _ => None,
            })
    };
    let active = tools()
        .find(|(_, status)| matches!(status, ToolStatus::Running { .. }))
        .or_else(|| tools().find(|(_, status)| *status == ToolStatus::Preparing));
    let task_time = app.task_elapsed().map(elapsed).unwrap_or_default();
    let (mark, color, label, detail) = match app.status {
        RunStatus::Working => {
            let (label, detail) = match active {
                Some((name, ToolStatus::Running { started_at })) => (
                    format!("Running {name}"),
                    format!(
                        "Tool {} | Esc stop | waiting for result",
                        elapsed(started_at.elapsed())
                    ),
                ),
                Some((name, _)) => (
                    format!("Preparing {name}"),
                    format!("Task {task_time} | Esc stop | execution not started"),
                ),
                None => (
                    if app.provider_had_text() {
                        "Receiving response".into()
                    } else {
                        "Waiting for agent response".into()
                    },
                    format!("Task {task_time} | Esc stop"),
                ),
            };
            (agent_mark(app.frame), accent(app), label, detail)
        }
        RunStatus::Reconnecting => (
            glyph(spinner(app.frame), Color::Yellow),
            Color::Yellow,
            match active {
                Some((name, _)) => format!("Reconnecting - last tool: {name}"),
                None => "Reconnecting to task".into(),
            },
            format!("Task {task_time} | Esc stop | execution status unconfirmed"),
        ),
        RunStatus::WaitingApproval => (
            glyph("?", Color::Magenta),
            Color::Magenta,
            "Waiting for approval".into(),
            format!("Task {task_time} | Left/Right + Enter | Esc stop"),
        ),
        RunStatus::Cancelling => (
            glyph(spinner(app.frame), Color::Yellow),
            Color::Yellow,
            match active {
                Some((name, _)) => format!("Stopping - last tool: {name}"),
                None => "Stopping task".into(),
            },
            format!("Task {task_time} | waiting for stop confirmation"),
        ),
        RunStatus::Ready => return,
    };
    let indent = " ".repeat(Line::from(mark.clone()).width());
    let mut title = mark;
    title.push(Span::styled(
        label,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(title),
            Line::styled(format!("{indent}{detail}"), Style::default().fg(Color::DarkGray)),
        ]),
        area,
    );
}

pub(super) fn tool_line(name: &str, status: ToolStatus, app: &App) -> Line<'static> {
    let (mark, color, detail) = match status {
        ToolStatus::Preparing => (".", Color::DarkGray, "preparing".into()),
        ToolStatus::Running { started_at } => match app.status {
            RunStatus::Working => (
                spinner(app.frame),
                Color::Cyan,
                format!("running {}", elapsed(started_at.elapsed())),
            ),
            RunStatus::Cancelling => (
                spinner(app.frame),
                Color::Yellow,
                "waiting for stop confirmation".into(),
            ),
            RunStatus::WaitingApproval => ("?", Color::Magenta, "waiting for approval".into()),
            RunStatus::Ready | RunStatus::Reconnecting => {
                ("?", Color::Yellow, "execution status unconfirmed".into())
            }
        },
        ToolStatus::Succeeded { duration_ms } => (
            "+",
            Color::Green,
            duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default(),
        ),
        ToolStatus::Failed { duration_ms } => (
            "!",
            Color::Red,
            duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default(),
        ),
        ToolStatus::Unfinished => ("?", Color::Yellow, "no result reported".into()),
    };
    Line::from(vec![
        Span::styled(format!("{mark} "), Style::default().fg(color)),
        Span::styled(name.to_string(), Style::default().fg(Color::Gray)),
        Span::styled(
            if detail.is_empty() {
                detail
            } else {
                format!("  {detail}")
            },
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{elapsed, spinner};

    #[test]
    fn elapsed_time_advances_and_formats_long_runs() {
        assert_eq!(elapsed(Duration::ZERO), "0s");
        assert_eq!(elapsed(Duration::from_secs(1)), "1s");
        assert_eq!(elapsed(Duration::from_secs(2)), "2s");
        assert_eq!(elapsed(Duration::from_secs(65)), "1m 05s");
    }

    #[test]
    fn spinner_advances_on_each_frame() {
        assert_ne!(spinner(0), spinner(1));
        assert_ne!(spinner(1), spinner(2));
        assert_eq!(spinner(0), spinner(4));
    }
}

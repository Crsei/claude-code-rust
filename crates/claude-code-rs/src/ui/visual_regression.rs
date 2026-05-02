//! Snapshot targets for the foundational Rust TUI surfaces.

use super::approval_overlay::{ApprovalChoice, ApprovalKind, ApprovalOverlay};
use super::bottom_pane::{BottomPane, BottomPaneView};
use super::capability_contract::{render_contract_table, surface_for_event, BackendEventKind};
use super::chat_composer::ChatComposerState;
use super::event_router::{render_route_trace, EngineEvent, RouteContext, TerminalEvent, UiEvent};
use super::feature_panels::{default_panels, render_panel_index, PanelState};
use super::frame_requester::{FrameReason, FrameRequester};
use super::history_cell::{render_history, HistoryCell, HistoryRenderMode};
use super::selection_surface::{SelectionItem, SelectionSurface};
use super::status_widget::StatusSnapshot;
use super::streaming_controller::{StreamingController, StreamingDelta};
use super::terminal_integration::{render_policy, TerminalEnvironment};
use super::tool_activity::{render_grouped_activity, ToolActivity, ToolState};

pub fn render_foundation_snapshot() -> String {
    let mut sections = Vec::new();

    let mut approval = ApprovalOverlay::new(ApprovalKind::Bash {
        command: "cargo test -p claude-code-rs".into(),
    })
    .with_choices(vec![
        ApprovalChoice::AllowOnce,
        ApprovalChoice::AllowAlways,
        ApprovalChoice::Deny,
    ]);
    approval.select(1);
    sections.push(section("approval", approval.render_lines(72).join("\n")));

    let mut composer = ChatComposerState::new();
    composer.set_input("/model opus");
    composer.set_busy(true);
    let effect = composer.submit_or_queue();
    sections.push(section(
        "composer",
        format!(
            "effect={effect:?}\n{}",
            composer.render_lines(72).join("\n")
        ),
    ));

    let mut pane = BottomPane::new(composer.clone());
    pane.push(BottomPaneView::Approval(approval));
    sections.push(section("bottom-pane", pane.render_lines(72).join("\n")));

    sections.push(section(
        "capability-contract",
        format!(
            "permission surface={:?}\n{}",
            surface_for_event(BackendEventKind::PermissionRequest),
            render_contract_table()
        ),
    ));

    sections.push(section(
        "event-router",
        render_route_trace(
            &[
                UiEvent::Terminal(TerminalEvent::Key("Enter".into())),
                UiEvent::Engine(EngineEvent::PermissionRequested("bash".into())),
                UiEvent::OverlayResponse("allow".into()),
            ],
            RouteContext {
                busy: true,
                overlay_active: true,
            },
        ),
    ));

    let mut panels = default_panels();
    panels[0].badge = 2;
    panels[3].state = PanelState::NeedsConfig;
    sections.push(section("feature-panels", render_panel_index(&panels)));

    let mut requester = FrameRequester::new();
    requester.request(FrameReason::Input);
    requester.request(FrameReason::Stream);
    let pending_before_take = requester.snapshot();
    let took = requester.take_pending();
    sections.push(section(
        "frame-requester",
        format!(
            "before={pending_before_take:?}\ntake={took}\nafter={:?}",
            requester.snapshot()
        ),
    ));

    let cells = vec![
        HistoryCell::User("show status".into()),
        HistoryCell::Assistant("All systems visible.".into()),
        HistoryCell::Tool {
            name: "bash".into(),
            summary: "cargo test passed".into(),
        },
        HistoryCell::Diff {
            path: "src/ui/mod.rs".into(),
            added: 14,
            removed: 0,
        },
    ];
    sections.push(section(
        "history-cells",
        render_history(&cells, HistoryRenderMode::Transcript),
    ));

    let mut picker = SelectionSurface::new(
        "Slash commands",
        vec![
            SelectionItem {
                description: "show status".into(),
                ..SelectionItem::new("status", "/status")
            },
            SelectionItem {
                description: "manage plugins".into(),
                ..SelectionItem::new("plugin", "/plugin")
            },
            SelectionItem {
                description: "start agent".into(),
                ..SelectionItem::new("agent", "/agent")
            },
        ],
    );
    picker.set_filter("p");
    picker.move_next();
    sections.push(section("selection", picker.render_lines(4).join("\n")));

    let status = StatusSnapshot {
        model: "claude-sonnet".into(),
        cwd: "F:/repo".into(),
        permission_mode: "ask".into(),
        sandbox: "workspace-write".into(),
        cost_usd: 0.1234,
        running_tools: 1,
        active_agents: 2,
        subsystems_ok: false,
    };
    sections.push(section(
        "status-widget",
        format!("{}\n{}", status.render_line(), status.render_details()),
    ));

    let mut stream = StreamingController::new();
    stream.apply(StreamingDelta::Thinking("checking".into()));
    stream.apply(StreamingDelta::ToolCall("bash".into()));
    stream.apply(StreamingDelta::Assistant("partial answer".into()));
    stream.commit_visible();
    sections.push(section("streaming", stream.render_lines().join("\n")));

    sections.push(section(
        "terminal-policy",
        render_policy(&TerminalEnvironment {
            term: "xterm-256color".into(),
            tmux: true,
            zellij: false,
            ssh: true,
            windows_terminal: false,
        }),
    ));

    let activities = vec![
        ToolActivity {
            name: "Read".into(),
            state: ToolState::Succeeded,
            summary: "loaded src/ui/app.rs".into(),
            elapsed_ms: 12,
            output_lines: 80,
        },
        ToolActivity {
            name: "Bash".into(),
            state: ToolState::Running,
            summary: "cargo test".into(),
            elapsed_ms: 240,
            output_lines: 14,
        },
    ];
    sections.push(section(
        "tool-activity",
        render_grouped_activity(&activities),
    ));

    sections.join("\n\n")
}

fn section(name: &str, body: impl AsRef<str>) -> String {
    format!("## {name}\n{}", body.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_foundational_ui_surfaces() {
        insta::assert_snapshot!("foundational_ui_surfaces", render_foundation_snapshot());
    }
}

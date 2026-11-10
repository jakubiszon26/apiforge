//! Who holds the project while this window cannot write it, and taking it
//! back once they are gone (MCP-07, ADR 0014 decisions 2 and 3).
//!
//! The bar at the bottom names the holder from the description it left beside
//! the lock. The watcher asks again with every check; when the lock is free
//! the window becomes the writer, never sooner and never by time, and runs the
//! checks of a start: an activated transaction opens recovery, an interrupted
//! live run its journal review.
//!
//! While an agent holds the project over the control channel (MCP-08b) the
//! bar ends in *Take back*; while it waits for the agent's run to finish it
//! says so; and a window that kept the project for unsaved work says why in
//! the same bar.
//!
//! While the agent runs a scenario or a component, the bar says so in place
//! of the last change, and then how the run ended (MCP-10, mock-up M4). The
//! status is never cut at the minimum window: the holder's words give way to
//! a shorter form first.
//!
//! While the window observes, *Follow* sits left of *Take back*, pressed
//! while the window follows the holder to every document it changes
//! (MCP-16, mock-up f1–f4).
use super::*;
use apiforge_channel::{RunMode, RunState};
use apiforge_storage::project::{Holder, HolderRole};

/// The bar's height in the approved mock-up M1.
const BAR_HEIGHT: f32 = 36.0;
/// What the window does while it observes; the same for every holder.
const PAUSED: &str = "editing is paused here; this window follows every change";
/// What the window does after *Take back* while the agent's run finishes.
const WAITING: &str = "waiting for the agent's run to finish";
/// Why the window kept the project when an agent asked for it (MCP-08b).
const REFUSED: &str = "An agent asked to edit this project; save your work first";
/// The bar's button while an agent holds the project (mock-up M1, round 4).
pub(super) const TAKE_BACK: &str = "Take back";
/// A client names itself; a longer name is cut so the bar keeps its line.
const MAX_CLIENT_CHARS: usize = 40;
/// A failed step's label in the status line is cut to this many characters.
/// With the file name's bound the longest status, about 115 characters
/// (some 650 px), leaves the bar at 960 × 540 room for *Follow* and *Take
/// back*, so the status itself is never cut; the holder's short form is
/// then cut too, whole on hover.
const MAX_LABEL_CHARS: usize = 30;
/// The file name in the status line is cut to this many characters.
const MAX_FILE_CHARS: usize = 40;

/// The holder and the bar's text, formatted once per description rather
/// than on every frame.
pub(super) struct Observed {
    holder: Holder,
    text: String,
    /// The same, while *Take back* waits for the agent's run.
    waiting: String,
    /// The short form beside a run status that leaves no room for the
    /// text: *Claude Code · since 10:42* (MCP-10).
    short: String,
    /// The short form while *Take back* waits for the agent's run.
    short_waiting: String,
}

impl Observed {
    pub(super) fn new(holder: Holder) -> Self {
        let text = holder_text(&holder, PAUSED);
        let waiting = holder_text(&holder, WAITING);
        let short = short_holder_text(&holder);
        let short_waiting = format!("{short} · {WAITING}");
        Self {
            holder,
            text,
            waiting,
            short,
            short_waiting,
        }
    }

    pub(super) fn holder(&self) -> &Holder {
        &self.holder
    }
}

/// The agent's run as the server last told it (MCP-10): its status line,
/// formatted once when told, with the time an ended run ended.
pub(super) struct AgentRun {
    running: bool,
    /// The run sends live requests (MCP-09 reads it).
    live: bool,
    text: String,
}

impl AgentRun {
    /// `failure` is the first step that did not pass and its label.
    pub(super) fn new(
        path: &str,
        mode: RunMode,
        state: RunState,
        step_count: Option<u64>,
        failure: Option<(u64, String)>,
    ) -> Self {
        let ended = chrono::Local::now().format("%H:%M").to_string();
        Self {
            running: state == RunState::Running,
            live: mode == RunMode::Live,
            text: run_text(path, mode, state, step_count, failure, &ended),
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.running
    }

    pub(super) fn is_live(&self) -> bool {
        self.live
    }
}

#[derive(Default)]
pub(super) struct TakeBack {
    /// A take-back is queued or running on the storage worker.
    pending: bool,
    /// The take-back met a transaction the holder left behind and opened
    /// recovery; it waits until recovery has been finished and reloaded.
    recovering: bool,
    /// Take-backs that failed in a row. A failure is most often passing (a
    /// reader or a scanner held a file for a moment), so the next check tries
    /// again; the notice is shown at the third (KTLO-33, decision 2 A).
    failures: u32,
}

/// Failed take-backs in a row after which the window says so; it keeps
/// trying after that.
const FAILURES_NOTICED: u32 = 3;

impl FolderApp {
    /// Whether this window observes a project another process holds, which
    /// is when the bar and the *Observing* badge show.
    pub(super) fn observing(&self) -> bool {
        self.holder.is_some()
            && self
                .session
                .as_ref()
                .is_some_and(ProjectSession::is_read_only)
    }

    /// Follows the watcher's answer to who holds the project.
    pub(super) fn follow_holder(&mut self, probe: Option<Holder>) {
        if !self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_read_only)
        {
            return;
        }
        if !self.watcher.recovery_required {
            self.take_back.recovering = false;
        }
        match probe {
            Some(holder) => {
                self.handover.holder_seen(&holder);
                if self
                    .holder
                    .as_ref()
                    .is_none_or(|observed| observed.holder != holder)
                {
                    self.holder = Some(Observed::new(holder));
                }
            }
            None => self.take_back(),
        }
    }

    /// The lock is free: become the writer, keeping what was typed here.
    fn take_back(&mut self) {
        // An agent connected to this window that has not let go may be
        // between the window's grant and its own lock (MCP-08b).
        if self.take_back.pending
            || self.take_back.recovering
            || self.persistence.closing
            || self.handover.holds_back()
        {
            return;
        }
        self.take_back.pending = true;
        let holder = self.window_holder();
        self.storage_project(
            "Taking the project back…",
            apiforge_project::StorageMode::Exclusive,
            false,
            move |session| session.take_writer(&holder),
            |app, result| {
                app.take_back.pending = false;
                match result {
                    Ok(true) => app.taken_back(),
                    // Another writer was quicker, or the window changed
                    // while the project was read; the next check tries again.
                    Ok(false) | Err(apiforge_project::Error::StaleStorage) => {}
                    Err(apiforge_project::Error::Storage(
                        apiforge_project::ProjectError::RecoveryRequired,
                    )) => {
                        // As at start: the transaction is reviewed and
                        // finished before the project is used.
                        app.take_back.recovering = true;
                        app.watcher.recovery_required = true;
                        app.review_transaction(app.root.clone());
                    }
                    // The next check tries again; only a failure that
                    // persists is worth the person's attention.
                    Err(error) => {
                        app.take_back.failures += 1;
                        if app.take_back.failures == FAILURES_NOTICED {
                            app.notify(
                                format!("This window could not take the project back: {error}"),
                                true,
                            );
                        }
                    }
                }
            },
        );
    }

    /// The writer's start, for a session that has just become one.
    fn taken_back(&mut self) {
        self.handover.reset_observing();
        self.holder = None;
        self.last_change = None;
        self.take_back = TakeBack::default();
        self.watcher.recovery_required = false;
        self.follow_external(ExternalOutcome {
            project_reloaded: true,
            ..Default::default()
        });
        self.open_run_journal();
        self.watcher.rescan();
    }

    /// The status bar at the bottom while another process holds the project:
    /// the holder at the left, in the warning colour, what its last followed
    /// commit changed at the right, and *Take back* at the right end while
    /// an agent holds it over the channel (mock-up M1, MCP-08a, MCP-08b).
    /// While the window holds the project the bar shows only to say why it
    /// kept it from an agent.
    pub(super) fn holder_bar(&mut self, ui: &mut egui::Ui) {
        self.track_observing();
        let observing = self.observing();
        let follow = self.follow.toggle().filter(|_| observing);
        let (text, short, last_change, status, take_back) = match self.holder.as_ref() {
            Some(observed) if observing => {
                let waits = self.handover.waits_for_run();
                // The agent's run, in place of the last change (MCP-10).
                let status = self.handover.run.as_ref().map(|run| run.text.clone());
                (
                    if waits {
                        observed.waiting.clone()
                    } else {
                        observed.text.clone()
                    },
                    Some(if waits {
                        observed.short_waiting.clone()
                    } else {
                        observed.short.clone()
                    }),
                    status.is_none().then(|| self.last_change.clone()).flatten(),
                    status,
                    self.handover
                        .offers_take_back()
                        .then_some(!self.handover.taking_back),
                )
            }
            _ if !observing && self.handover.refused => {
                (REFUSED.to_owned(), None, None, None, None)
            }
            _ => return,
        };
        let mut taken = false;
        let mut toggled = false;
        let response = egui::Panel::bottom("folder-holder")
            .exact_size(BAR_HEIGHT)
            .resizable(false)
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(theme::SPACE_LG as i8, 0)),
            )
            .show(ui, |ui| {
                // The button and the last change first, from the right, so
                // the holder's words give way to them on a narrow window.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(enabled) = take_back {
                        // Disabled, greyed, while the agent finishes.
                        taken = ui
                            .add_enabled(enabled, widgets::action(Icon::Undo, TAKE_BACK).compact())
                            .clicked();
                        // *Follow* sits close to it, the status further off.
                        ui.add_space(if follow.is_some() {
                            theme::SPACE_SM
                        } else {
                            theme::SPACE_LG
                        });
                    }
                    if let Some(pinned) = follow {
                        // Pressed while following (MCP-16 decision 5).
                        toggled = ui
                            .add(
                                widgets::action(Icon::Pin, follow::FOLLOW)
                                    .compact()
                                    .toggle(pinned)
                                    .named(follow::FOLLOW_NAME),
                            )
                            .on_hover_text(if pinned {
                                follow::PINNED_HINT
                            } else {
                                follow::RELEASED_HINT
                            })
                            .clicked();
                        ui.add_space(theme::SPACE_LG);
                    }
                    // The run's status whole while the bar has room for it;
                    // the holder gives way to it.
                    let status_shown = status.is_some();
                    if let Some(status) = status {
                        let room = ui.available_width();
                        ui.scope(|ui| {
                            ui.set_max_width(room);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(status)
                                        .size(theme::FONT_META)
                                        .color(theme::MUTED),
                                )
                                .truncate(),
                            );
                        });
                        ui.add_space(theme::SPACE_LG);
                    }
                    if let Some(change) = last_change {
                        // At most half the bar: a long file name is cut,
                        // whole on hover, and the holder keeps its half.
                        let room = ui.available_width() / 2.0;
                        ui.scope(|ui| {
                            ui.set_max_width(room);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&change)
                                        .size(theme::FONT_META)
                                        .color(theme::MUTED),
                                )
                                .truncate(),
                            );
                        });
                        ui.add_space(theme::SPACE_LG);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        // Beside a status the full text goes when it does
                        // not fit, for its short form, whole on hover.
                        let shortened = short.filter(|_| {
                            status_shown && text_width(ui, &text) > ui.available_width()
                        });
                        let response = ui.add(
                            egui::Label::new(
                                RichText::new(shortened.as_deref().unwrap_or(&text))
                                    .size(theme::FONT_META)
                                    .color(theme::WARNING),
                            )
                            .truncate(),
                        );
                        if shortened.is_some() {
                            response.on_hover_text(&text);
                        }
                    });
                });
            })
            .response;
        // The whole bar, named for assistive technology and for tests.
        let region = ui.interact(
            response.rect,
            egui::Id::new("folder-holder-region"),
            egui::Sense::hover(),
        );
        region.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Project holder")
        });
        if taken {
            self.request_take_back();
        }
        if toggled {
            self.toggle_follow();
        }
    }
}

/// The width `text` takes in the bar's font, uncut.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    let job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat::simple(egui::FontId::proportional(theme::FONT_META), theme::WARNING),
    );
    ui.ctx().fonts_mut(|fonts| fonts.layout_job(job)).size().x
}

/// The status line of the agent's run (MCP-10, mock-up M4): *Agent: running
/// promotion.afs (simulation)…*, then *Agent: promotion.afs passed · 18
/// steps · 12:58*, *… failed at step 7 · Promotional price · 12:58* or
/// *… cancelled · 12:58*, `ended` being the time it ended. An ended live
/// run says *(live)* after the file's name.
fn run_text(
    path: &str,
    mode: RunMode,
    state: RunState,
    step_count: Option<u64>,
    failure: Option<(u64, String)>,
    ended: &str,
) -> String {
    let file = bounded_file(file_name(path));
    let (mode, live) = match mode {
        RunMode::Simulation => ("simulation", ""),
        RunMode::Live => ("live", " (live)"),
    };
    match state {
        RunState::Running => format!("Agent: running {file} ({mode})…"),
        RunState::Passed => match step_count {
            Some(1) => format!("Agent: {file}{live} passed · 1 step · {ended}"),
            Some(count) => format!("Agent: {file}{live} passed · {count} steps · {ended}"),
            None => format!("Agent: {file}{live} passed · {ended}"),
        },
        RunState::Failed => match failure {
            Some((step, label)) => format!(
                "Agent: {file}{live} failed at step {step} · {} · {ended}",
                bounded(&label, MAX_LABEL_CHARS)
            ),
            None => format!("Agent: {file}{live} failed · {ended}"),
        },
        RunState::Cancelled => format!("Agent: {file}{live} cancelled · {ended}"),
    }
}

/// A file name bounded as `bounded` does, its extension kept: the stem is
/// cut, *a-very-long-…-name….afs*.
fn bounded_file(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && extension.chars().count() < 8
                && name.chars().count() > MAX_FILE_CHARS =>
        {
            let room = MAX_FILE_CHARS - extension.chars().count() - 1;
            format!("{}.{}", bounded(stem, room), bounded(extension, 8))
        }
        _ => bounded(name, MAX_FILE_CHARS),
    }
}

/// `text` on one line, cut to `limit` characters with an ellipsis.
fn bounded(text: &str, limit: usize) -> String {
    let mut shown: String = text
        .chars()
        .filter(|character| !character.is_control())
        .take(limit + 1)
        .collect();
    if shown.chars().count() > limit {
        shown = shown.chars().take(limit - 1).collect();
        shown.push('…');
    }
    shown
}

/// *Claude Code via apiforge mcp*, or *An agent via apiforge mcp* for a
/// client that gave no name. The client's name is its own choice: it is
/// shown without control characters and cut to a bounded length.
pub(super) fn agent_name(client: Option<&str>) -> String {
    match client.map(shown_client) {
        Some(client) => format!("{client} via apiforge mcp"),
        None => "An agent via apiforge mcp".to_owned(),
    }
}

/// *apiforge run --live*, or *The apiforge command* for one that gave no
/// name (MCP-15).
pub(super) fn command_name(client: Option<&str>) -> String {
    match client.map(shown_client) {
        Some(command) => format!("apiforge {command}"),
        None => "The apiforge command".to_owned(),
    }
}

fn shown_client(client: &str) -> String {
    client
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_CLIENT_CHARS)
        .collect()
}

/// *Claude Code via apiforge mcp · since 10:42 · editing is paused here; …*,
/// with `doing` saying what the window does.
pub(super) fn holder_text(holder: &Holder, doing: &str) -> String {
    let client = holder.client().map(shown_client);
    let who = match (holder.role(), client) {
        (None, _) => return format!("Another process · {doing}"),
        (Some(HolderRole::Window), _) => "Another API Forge window".to_owned(),
        (Some(HolderRole::Agent), client) => agent_name(client.as_deref()),
        (Some(HolderRole::Command), client) => command_name(client.as_deref()),
    };
    match holder
        .since()
        .and_then(|since| wall_clock(since, &chrono::Local))
    {
        Some(time) => format!("{who} · since {time} · {doing}"),
        None => format!("{who} · {doing}"),
    }
}

/// The holder in a few words, for a bar that also shows a run status:
/// *Claude Code · since 10:42*.
fn short_holder_text(holder: &Holder) -> String {
    let client = holder.client().map(shown_client);
    let who = match (holder.role(), client) {
        (None, _) => return "Another process".to_owned(),
        (Some(HolderRole::Window), _) => "Another API Forge window".to_owned(),
        (Some(HolderRole::Agent), Some(client)) => client,
        (Some(HolderRole::Agent), None) => "An agent".to_owned(),
        (Some(HolderRole::Command), Some(command)) => format!("apiforge {command}"),
        (Some(HolderRole::Command), None) => "The apiforge command".to_owned(),
    };
    match holder
        .since()
        .and_then(|since| wall_clock(since, &chrono::Local))
    {
        Some(time) => format!("{who} · since {time}"),
        None => who,
    }
}

/// `HH:MM` on the wall clock of `zone` for an RFC 3339 timestamp.
fn wall_clock<Zone: chrono::TimeZone>(since: &str, zone: &Zone) -> Option<String>
where
    Zone::Offset: std::fmt::Display,
{
    let at = chrono::DateTime::parse_from_rfc3339(since).ok()?;
    Some(at.with_timezone(zone).format("%H:%M").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_time_is_shown_on_the_local_wall_clock() {
        let warsaw = chrono::FixedOffset::east_opt(3600).unwrap();
        assert_eq!(
            wall_clock("2026-10-27T09:42:11Z", &warsaw).as_deref(),
            Some("10:42")
        );
        let utc = chrono::Utc;
        assert_eq!(
            wall_clock("2026-10-27T23:59:59Z", &utc).as_deref(),
            Some("23:59")
        );
        assert_eq!(wall_clock("yesterday", &utc), None);
    }

    /// The hand-over prompt names the agent as the bar does (MCP-08c).
    #[test]
    fn the_agents_name_is_bounded_and_on_one_line() {
        let long = format!("Claude\nCode{}", "x".repeat(200));
        let name = agent_name(Some(&long));
        assert!(!name.contains('\n'), "{name:?}");
        assert_eq!(name, format!("{} via apiforge mcp", shown_client(&long)),);
        assert_eq!(shown_client(&long).chars().count(), MAX_CLIENT_CHARS);
        assert_eq!(agent_name(None), "An agent via apiforge mcp");
    }

    /// The status line in each state of the agent's run (MCP-10, M4).
    #[test]
    fn the_run_status_names_the_file_the_mode_and_the_end() {
        let path = "scenarios/promotion.afs";
        let at = "12:58";
        let text = |mode, state, count, failure| run_text(path, mode, state, count, failure, at);
        assert_eq!(
            text(RunMode::Simulation, RunState::Running, None, None),
            "Agent: running promotion.afs (simulation)…"
        );
        assert_eq!(
            text(RunMode::Live, RunState::Running, None, None),
            "Agent: running promotion.afs (live)…"
        );
        assert_eq!(
            text(RunMode::Simulation, RunState::Passed, Some(18), None),
            "Agent: promotion.afs passed · 18 steps · 12:58"
        );
        assert_eq!(
            text(RunMode::Simulation, RunState::Passed, Some(1), None),
            "Agent: promotion.afs passed · 1 step · 12:58"
        );
        assert_eq!(
            text(
                RunMode::Simulation,
                RunState::Failed,
                Some(18),
                Some((7, "Promotional price".into()))
            ),
            "Agent: promotion.afs failed at step 7 · Promotional price · 12:58"
        );
        assert_eq!(
            text(RunMode::Live, RunState::Failed, None, None),
            "Agent: promotion.afs (live) failed · 12:58"
        );
        assert_eq!(
            text(RunMode::Live, RunState::Cancelled, Some(3), None),
            "Agent: promotion.afs (live) cancelled · 12:58"
        );
        let label = format!("Line\nbreak{}", "x".repeat(100));
        let failed = text(
            RunMode::Simulation,
            RunState::Failed,
            None,
            Some((2, label)),
        );
        assert!(!failed.contains('\n'), "{failed:?}");
        assert!(
            failed.contains(&format!(
                "· Linebreak{}… ·",
                "x".repeat(MAX_LABEL_CHARS - 10)
            )),
            "{failed:?}"
        );
    }

    #[test]
    fn a_long_file_name_keeps_its_extension() {
        let long = format!("scenarios/{}.afs", "a".repeat(80));
        let shown = bounded_file(file_name(&long));
        assert_eq!(shown.chars().count(), MAX_FILE_CHARS, "{shown:?}");
        assert!(shown.ends_with("….afs"), "{shown:?}");
        assert_eq!(bounded_file("promotion.afs"), "promotion.afs");
    }

    #[test]
    fn the_short_holder_text_keeps_who_and_since() {
        let holder = Holder::new(HolderRole::Agent, Some("Claude Code".into()))
            .with_since("2026-10-27T09:42:11Z");
        let since = wall_clock("2026-10-27T09:42:11Z", &chrono::Local).unwrap();
        assert_eq!(
            short_holder_text(&holder),
            format!("Claude Code · since {since}")
        );
        let observed = Observed::new(holder);
        assert_eq!(
            observed.short_waiting,
            format!("{} · {WAITING}", observed.short)
        );
        assert_eq!(
            short_holder_text(&Holder::new(HolderRole::Agent, None).with_since("-")),
            "An agent"
        );
        assert_eq!(short_holder_text(&Holder::unknown()), "Another process");
    }

    #[test]
    fn every_holder_is_named_and_a_client_cannot_break_the_line() {
        let text = holder_text(
            &Holder::new(HolderRole::Agent, None).with_since("not a time"),
            PAUSED,
        );
        assert_eq!(text, format!("An agent via apiforge mcp · {PAUSED}"));
        let text = holder_text(&Holder::command("run").with_since("not a time"), PAUSED);
        assert_eq!(text, format!("apiforge run · {PAUSED}"));
        let long = format!("Claude\nCode{}", "x".repeat(100));
        let text = holder_text(
            &Holder::new(HolderRole::Agent, Some(long)).with_since("-"),
            PAUSED,
        );
        assert!(!text.contains('\n'));
        assert!(text.starts_with(&format!("ClaudeCode{} via", "x".repeat(30))));
        assert_eq!(
            holder_text(&Holder::unknown(), PAUSED),
            format!("Another process · {PAUSED}")
        );
        let text = holder_text(
            &Holder::new(HolderRole::Agent, Some("Claude Code".into())).with_since("-"),
            WAITING,
        );
        assert_eq!(text, format!("Claude Code via apiforge mcp · {WAITING}"));
    }
}

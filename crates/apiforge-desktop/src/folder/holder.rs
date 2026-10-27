//! Who holds the project while this window cannot write it, and taking it
//! back once they are gone (MCP-07, ADR 0014 decisions 2 and 3).
//!
//! The bar at the bottom names the holder from the description it left beside
//! the lock. The watcher asks again with every check; when the lock is free
//! the window becomes the writer, never sooner and never by time, and runs the
//! checks of a start: an activated transaction opens recovery, an interrupted
//! live run its journal review.
use super::*;
use apiforge_storage::project::{Holder, HolderRole};

/// The bar's height in the approved mock-up M1.
const BAR_HEIGHT: f32 = 36.0;
/// What the window does while it observes; the same for every holder.
const PAUSED: &str = "editing is paused here; this window follows every change";
/// A client names itself; a longer name is cut so the bar keeps its line.
const MAX_CLIENT_CHARS: usize = 40;

/// The holder and the bar's text, formatted once per description rather
/// than on every frame.
pub(super) struct Observed {
    holder: Holder,
    text: String,
}

impl Observed {
    pub(super) fn new(holder: Holder) -> Self {
        let text = holder_text(&holder);
        Self { holder, text }
    }
}

#[derive(Default)]
pub(super) struct TakeBack {
    /// A take-back is queued or running on the storage worker.
    pending: bool,
    /// The take-back met a transaction the holder left behind and opened
    /// recovery; it waits until recovery has been finished and reloaded.
    recovering: bool,
    /// The take-back failed for a reason another attempt would repeat.
    stopped: bool,
}

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
        if self.take_back.pending
            || self.take_back.recovering
            || self.take_back.stopped
            || self.persistence.closing
        {
            return;
        }
        self.take_back.pending = true;
        self.storage_project(
            "Taking the project back…",
            apiforge_project::StorageMode::Exclusive,
            false,
            |session| session.take_writer(&Holder::window()),
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
                    Err(error) => {
                        app.take_back.stopped = true;
                        app.notify(
                            format!("This window could not take the project back: {error}"),
                            true,
                        );
                    }
                }
            },
        );
    }

    /// The writer's start, for a session that has just become one.
    fn taken_back(&mut self) {
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
    /// the holder at the left, in the warning colour, and what its last
    /// followed commit changed at the right (mock-up M1, MCP-08a).
    pub(super) fn holder_bar(&mut self, ui: &mut egui::Ui) {
        if !self.observing() {
            return;
        }
        let Some(observed) = self.holder.as_ref() else {
            return;
        };
        let text = observed.text.clone();
        let last_change = self.last_change.clone();
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
                // The last change first, from the right, so the holder's
                // words give way to it on a narrow window.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(change) = last_change {
                        ui.add(
                            egui::Label::new(
                                RichText::new(change)
                                    .size(theme::FONT_META)
                                    .color(theme::MUTED),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.add_space(theme::SPACE_LG);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(text)
                                    .size(theme::FONT_META)
                                    .color(theme::WARNING),
                            )
                            .truncate(),
                        );
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
    }
}

/// *Claude Code via apiforge mcp · since 10:42 · editing is paused here; …*
pub(super) fn holder_text(holder: &Holder) -> String {
    let client = holder.client().map(|client| {
        client
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_CLIENT_CHARS)
            .collect::<String>()
    });
    let who = match (holder.role(), client) {
        (None, _) => return format!("Another process · {PAUSED}"),
        (Some(HolderRole::Window), _) => "Another API Forge window".to_owned(),
        (Some(HolderRole::Agent), Some(client)) => format!("{client} via apiforge mcp"),
        (Some(HolderRole::Agent), None) => "An agent via apiforge mcp".to_owned(),
        (Some(HolderRole::Command), Some(command)) => format!("apiforge {command}"),
        (Some(HolderRole::Command), None) => "The apiforge command".to_owned(),
    };
    match holder
        .since()
        .and_then(|since| wall_clock(since, &chrono::Local))
    {
        Some(time) => format!("{who} · since {time} · {PAUSED}"),
        None => format!("{who} · {PAUSED}"),
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

    #[test]
    fn every_holder_is_named_and_a_client_cannot_break_the_line() {
        let text = holder_text(&Holder::new(HolderRole::Agent, None).with_since("not a time"));
        assert_eq!(text, format!("An agent via apiforge mcp · {PAUSED}"));
        let text = holder_text(&Holder::command("run").with_since("not a time"));
        assert_eq!(text, format!("apiforge run · {PAUSED}"));
        let long = format!("Claude\nCode{}", "x".repeat(100));
        let text = holder_text(&Holder::new(HolderRole::Agent, Some(long)).with_since("-"));
        assert!(!text.contains('\n'));
        assert!(text.starts_with(&format!("ClaudeCode{} via", "x".repeat(30))));
        assert_eq!(
            holder_text(&Holder::unknown()),
            format!("Another process · {PAUSED}")
        );
    }
}

//! Capture only this application's viewport for visual regression review.
use eframe::egui;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

/// How long a capture waits for the window to have a document, so that a
/// folder that cannot be opened ends the probe instead of keeping it open.
const READY_PATIENCE: Duration = Duration::from_secs(30);

pub struct Capture {
    path: PathBuf,
    requested: bool,
    frames: u8,
    finished: bool,
    saved: Option<Receiver<Result<(), String>>>,
    started: Instant,
    patience: Duration,
}
impl Capture {
    pub fn new(path: PathBuf) -> Self {
        Self::with_patience(path, READY_PATIENCE)
    }
    fn with_patience(path: PathBuf, patience: Duration) -> Self {
        Self {
            path,
            requested: false,
            frames: 0,
            finished: false,
            saved: None,
            started: Instant::now(),
            patience,
        }
    }
    pub fn update(&mut self, context: &egui::Context, ready: bool) -> Option<Result<(), String>> {
        if self.finished {
            return None;
        }
        if let Some(saved) = &self.saved {
            let result = match saved.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err("Screenshot encoder stopped".into())),
            };
            self.finished = result.is_some();
            return result;
        }
        if self.requested {
            let screenshot = context.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(screenshot) = screenshot {
                let (sender, receiver) = mpsc::sync_channel(1);
                let path = self.path.clone();
                let context = context.clone();
                self.saved = Some(receiver);
                let spawned = std::thread::Builder::new()
                    .name("viewport-capture".into())
                    .spawn(move || {
                        let result = image::save_buffer_with_format(
                            &path,
                            screenshot.as_raw(),
                            screenshot.width() as u32,
                            screenshot.height() as u32,
                            image::ColorType::Rgba8,
                            image::ImageFormat::Png,
                        )
                        .map_err(|_| "Cannot write screenshot PNG".to_string());
                        let _ = sender.send(result);
                        context.request_repaint();
                    });
                if spawned.is_err() {
                    return Some(Err("Cannot start screenshot encoder".into()));
                }
            }
        } else if ready {
            self.frames += 1;
            if self.frames >= 8 {
                context.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::UserData::default(),
                ));
                self.requested = true;
            }
        }
        context.request_repaint();
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successful_capture_is_terminal_even_if_egui_does_another_pass_before_closing() {
        let mut capture = Capture::new("unused.png".into());
        let (sender, receiver) = mpsc::sync_channel(1);
        capture.saved = Some(receiver);
        sender.send(Ok(())).expect("send");
        drop(sender);
        let context = egui::Context::default();
        assert_eq!(capture.update(&context, false), Some(Ok(())));
        assert_eq!(capture.update(&context, false), None);
    }

    #[test]
    fn a_window_that_never_has_a_document_ends_the_capture_with_an_error() {
        // KTLO-16: `--screenshot --folder` on a folder that cannot be opened
        // kept the window open until something killed it.
        let mut capture = Capture::with_patience("unused.png".into(), Duration::ZERO);
        let context = egui::Context::default();
        let result = capture.update(&context, false);
        assert!(
            matches!(&result, Some(Err(message)) if message.contains("no document")),
            "{result:?}"
        );
        assert_eq!(capture.update(&context, false), None);
    }
}

//! Capture only this application's viewport for visual regression review.
use eframe::egui;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};

pub struct Capture {
    path: PathBuf,
    requested: bool,
    frames: u8,
    finished: bool,
    saved: Option<Receiver<Result<(), String>>>,
}
impl Capture {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            requested: false,
            frames: 0,
            finished: false,
            saved: None,
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
}

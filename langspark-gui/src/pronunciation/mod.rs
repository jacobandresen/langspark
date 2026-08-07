//! Pronunciation practice tab: pick a word, play its reference pronunciation,
//! record an attempt, and see a score + feedback with waveform comparison.
//!
//! TTS/recording/scoring are injected as callbacks rather than called
//! directly, since the concrete backends (`VoicevoxTts`/`PiperTts`,
//! `AudioRecorder`, `PronunciationScorer`) need a running VOICEVOX Engine,
//! a microphone, and installed voice models respectively — resources this
//! widget shouldn't assume are present. `app.rs` wires the real backends in
//! once language/model setup (section 24) is in place.

use crate::task;
use crate::widgets::waveform::{Waveform, WaveformColor};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use langspark_core::PronunciationResult;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// A word available for pronunciation practice.
#[derive(Debug, Clone)]
pub struct PracticeWord {
    pub text: String,
    pub reading: Option<String>,
}

/// Callbacks the pronunciation tab needs from the host application, each
/// running on the background thread pool via `task::run_blocking` — so each
/// closure must be `Send + Sync` (the tab itself stays single-threaded; only
/// the work these callbacks do is offloaded).
pub struct PronunciationCallbacks {
    /// Synthesize + return WAV bytes for the reference pronunciation.
    pub synthesize: Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync>,
    /// Record from the microphone for a fixed duration, returning samples + rate.
    pub record: Box<dyn Fn() -> anyhow::Result<(Vec<f32>, u32)> + Send + Sync>,
    /// Play WAV bytes through the default output device.
    pub play: Box<dyn Fn(Vec<u8>) -> anyhow::Result<()> + Send + Sync>,
    /// Score a recognized transcript against the expected text.
    pub score: Box<dyn Fn(&str, &str) -> PronunciationResult + Send + Sync>,
    /// Transcribe recorded samples to text (speech recognition).
    pub transcribe: Box<dyn Fn(&[f32], u32) -> anyhow::Result<String> + Send + Sync>,
}

pub struct PronunciationTab {
    pub widget: gtk4::Widget,
}

impl PronunciationTab {
    pub fn new(words: Vec<PracticeWord>, callbacks: PronunciationCallbacks) -> Self {
        let root = GtkBox::new(Orientation::Vertical, 12);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let word_label = Label::builder().css_classes(["title-1"]).build();
        let nav_box = GtkBox::new(Orientation::Horizontal, 8);
        nav_box.set_halign(gtk4::Align::Center);
        let prev_btn = gtk4::Button::from_icon_name("go-previous-symbolic");
        let next_btn = gtk4::Button::from_icon_name("go-next-symbolic");
        let play_btn = gtk4::Button::builder().label("▶ Play").css_classes(["suggested-action"]).build();
        let record_btn = gtk4::Button::builder().label("● Record").css_classes(["destructive-action"]).build();
        nav_box.append(&prev_btn);
        nav_box.append(&word_label);
        nav_box.append(&next_btn);

        let action_box = GtkBox::new(Orientation::Horizontal, 8);
        action_box.set_halign(gtk4::Align::Center);
        action_box.append(&play_btn);
        action_box.append(&record_btn);

        let waveform = Waveform::new();
        let score_label = Label::builder().css_classes(["title-2"]).build();
        let feedback_label = Label::builder().wrap(true).justify(gtk4::Justification::Center).build();

        root.append(&nav_box);
        root.append(&action_box);
        root.append(&waveform.widget);
        root.append(&score_label);
        root.append(&feedback_label);

        let words = Rc::new(words);
        let index = Rc::new(Cell::new(0usize));
        let callbacks = Arc::new(callbacks);

        let refresh = {
            let words = words.clone();
            let index = index.clone();
            let word_label = word_label.clone();
            let score_label = score_label.clone();
            let feedback_label = feedback_label.clone();
            move || {
                let i = index.get();
                if let Some(word) = words.get(i) {
                    word_label.set_label(&word.text);
                } else {
                    word_label.set_label("No words available");
                }
                score_label.set_label("");
                feedback_label.set_label("");
            }
        };
        refresh();

        prev_btn.connect_clicked(glib::clone!(
            #[strong]
            index,
            #[strong]
            words,
            #[strong]
            refresh,
            move |_| {
                let i = index.get();
                if i > 0 {
                    index.set(i - 1);
                }
                let _ = &words;
                refresh();
            }
        ));

        next_btn.connect_clicked(glib::clone!(
            #[strong]
            index,
            #[strong]
            words,
            #[strong]
            refresh,
            move |_| {
                if index.get() + 1 < words.len() {
                    index.set(index.get() + 1);
                }
                refresh();
            }
        ));

        play_btn.connect_clicked(glib::clone!(
            #[strong]
            words,
            #[strong]
            index,
            #[strong]
            callbacks,
            #[strong]
            waveform,
            #[weak]
            feedback_label,
            move |_| {
                let Some(word) = words.get(index.get()).cloned() else { return };
                let callbacks = callbacks.clone();
                let waveform = waveform.clone();
                feedback_label.set_label("");
                task::spawn_on_main(async move {
                    let text = word.reading.clone().unwrap_or_else(|| word.text.clone());
                    let synth_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        move || (callbacks.synthesize)(&text)
                    })
                    .await;
                    match synth_result {
                        Ok(wav) => {
                            if let Ok((samples, _rate)) = langspark_core::audio::decode_wav(&wav) {
                                let downsampled = langspark_core::audio::extract_waveform(&samples, 200);
                                waveform.set_samples(downsampled, WaveformColor::REFERENCE);
                            }
                            let play_result = task::run_blocking(move || (callbacks.play)(wav)).await;
                            if let Err(e) = play_result {
                                feedback_label.set_label(&format!("Couldn't play reference audio: {e}"));
                            }
                        }
                        Err(e) => {
                            feedback_label.set_label(&format!("Couldn't generate reference audio: {e}"));
                        }
                    }
                });
            }
        ));

        record_btn.connect_clicked(glib::clone!(
            #[strong]
            words,
            #[strong]
            index,
            #[strong]
            callbacks,
            #[strong]
            waveform,
            #[weak]
            score_label,
            #[weak]
            feedback_label,
            move |_| {
                let Some(word) = words.get(index.get()).cloned() else { return };
                let callbacks = callbacks.clone();
                let waveform = waveform.clone();
                task::spawn_on_main(async move {
                    let record_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        move || (callbacks.record)()
                    })
                    .await;

                    let Ok((samples, rate)) = record_result else {
                        feedback_label.set_label("Recording failed — check your microphone.");
                        return;
                    };

                    let downsampled = langspark_core::audio::extract_waveform(&samples, 200);
                    waveform.set_samples(downsampled, WaveformColor::USER);

                    let expected = word.reading.clone().unwrap_or_else(|| word.text.clone());
                    let transcribe_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        let samples = samples.clone();
                        move || (callbacks.transcribe)(&samples, rate)
                    })
                    .await;

                    match transcribe_result {
                        Ok(recognized) => {
                            let result = (callbacks.score)(&recognized, &expected);
                            score_label.set_label(&format!("{:.0}%", result.score));
                            score_label.set_css_classes(&[if result.is_correct {
                                "langspark-score-good"
                            } else {
                                "langspark-score-bad"
                            }]);
                            feedback_label.set_label(&result.feedback);
                        }
                        Err(e) => {
                            feedback_label.set_label(&format!("Couldn't recognize speech: {e}"));
                        }
                    }
                });
            }
        ));

        let scroller = gtk4::ScrolledWindow::builder().child(&root).vexpand(true).build();
        Self { widget: scroller.upcast() }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn noop_callbacks() -> PronunciationCallbacks {
        PronunciationCallbacks {
            synthesize: Box::new(|_| Ok(Vec::new())),
            record: Box::new(|| Ok((Vec::new(), 44_100))),
            play: Box::new(|_| Ok(())),
            score: Box::new(|r, e| langspark_core::score_pronunciation(r, e, "ja")),
            transcribe: Box::new(|_, _| Ok(String::new())),
        }
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.
}

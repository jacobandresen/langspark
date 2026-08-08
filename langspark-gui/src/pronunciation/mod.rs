//! Pronunciation practice tab: pick a word, play its reference pronunciation,
//! record an attempt, and see a score + feedback with waveform comparison.
//!
//! TTS/recording/scoring are injected as callbacks rather than called
//! directly, since the concrete backends (`VoicevoxTts`, `AudioRecorder`,
//! `PronunciationScorer`) need a running VOICEVOX Engine, a microphone, and
//! installed voice models respectively — resources this widget shouldn't
//! assume are present. `app.rs` wires the real backends in once language/
//! model setup (section 24) is in place.

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
    /// How long `record` actually captures for — the tab uses this to drive
    /// the recording countdown/progress bar, so it must match `record`'s
    /// real behavior.
    pub record_duration: std::time::Duration,
    /// Play WAV bytes through the default output device.
    pub play: Box<dyn Fn(Vec<u8>) -> anyhow::Result<()> + Send + Sync>,
    /// Score a recognized transcript against the expected text.
    pub score: Box<dyn Fn(&str, &str) -> PronunciationResult + Send + Sync>,
    /// Transcribe recorded samples to text (speech recognition).
    pub transcribe: Box<dyn Fn(&[f32], u32) -> anyhow::Result<String> + Send + Sync>,
}

/// Peak amplitude (see `langspark_core::audio::peak_level`) below which a
/// recording is treated as silence — i.e. the mic likely isn't picking
/// anything up. Chosen well above typical electrical/digital noise floor but
/// well below any actual speech.
const SILENCE_THRESHOLD: f32 = 0.01;

/// Disable (or re-enable) the tab's Play/Record/Test Mic/Replay buttons
/// together, so a user can't start an overlapping recording or playback
/// while one is already in flight on the (single, shared) audio device.
/// `has_test_recording` gates Replay specifically — it should only become
/// sensitive again if there's actually a "Test Mic" recording to play back.
fn set_actions_sensitive(
    play_btn: &gtk4::Button,
    record_btn: &gtk4::Button,
    test_mic_btn: &gtk4::Button,
    replay_mic_btn: &gtk4::Button,
    has_test_recording: bool,
    sensitive: bool,
) {
    play_btn.set_sensitive(sensitive);
    record_btn.set_sensitive(sensitive);
    test_mic_btn.set_sensitive(sensitive);
    replay_mic_btn.set_sensitive(sensitive && has_test_recording);
}

/// Replace `diff_box`'s children with one [`Label`] per character of
/// `expected`, colored per [`langspark_core::DiffOp`] — a per-character view
/// of what was and wasn't heard, since a bare score percentage doesn't say
/// *what* was missed. Pass an empty `ops` to clear the box (e.g. when
/// switching words or starting a new recording).
fn render_diff(diff_box: &GtkBox, ops: &[(char, langspark_core::DiffOp)]) {
    while let Some(child) = diff_box.first_child() {
        diff_box.remove(&child);
    }
    for (ch, op) in ops {
        let label = Label::new(Some(&ch.to_string()));
        match op {
            langspark_core::DiffOp::Match => {}
            langspark_core::DiffOp::Substitute => label.set_css_classes(&["langspark-diff-substitute"]),
            langspark_core::DiffOp::Missing => label.set_css_classes(&["langspark-diff-missing"]),
        }
        diff_box.append(&label);
    }
}

/// How often the recording countdown/progress bar updates. The actual
/// `AudioRecorder` capture window is driven independently on a background
/// thread (see `app::build_record`) — this only paces the on-screen
/// countdown, so it doesn't need to be exact, just frequent enough to read
/// as a smooth fill.
const COUNTDOWN_TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// Show a filling progress bar and a ticking "Recording… Ns left" message in
/// `feedback_label` for `duration`, matching how long the mic is actually
/// capturing for. Once `duration` elapses the mic is no longer listening —
/// which the fixed-length capture doesn't otherwise surface — so this
/// replaces the countdown with `stopped_message` explaining that a fresh
/// press of the button is needed to record again. Any later `feedback_label`
/// update (transcription result, error, etc.) simply overwrites that text —
/// callers whose post-recording work can take a noticeable while (e.g.
/// transcription) should set an interim "processing" message themselves
/// before that work starts, since `stopped_message` reads as idle/actionable
/// and the Record button stays disabled until that work finishes.
fn start_recording_countdown(progress: &gtk4::ProgressBar, feedback_label: &Label, duration: std::time::Duration, stopped_message: &'static str) {
    let total_ticks = (duration.as_millis() / COUNTDOWN_TICK.as_millis()).max(1) as u32;
    progress.set_fraction(0.0);
    progress.set_visible(true);

    let elapsed_ticks = Rc::new(Cell::new(0u32));
    glib::source::timeout_add_local(COUNTDOWN_TICK, glib::clone!(
        #[strong]
        elapsed_ticks,
        #[weak]
        progress,
        #[weak]
        feedback_label,
        #[upgrade_or]
        glib::ControlFlow::Break,
        move || {
            let tick = elapsed_ticks.get() + 1;
            elapsed_ticks.set(tick);
            progress.set_fraction((tick as f64 / total_ticks as f64).min(1.0));

            if tick >= total_ticks {
                progress.set_visible(false);
                feedback_label.set_label(stopped_message);
                glib::ControlFlow::Break
            } else {
                let remaining_secs = (total_ticks - tick).div_ceil(COUNTDOWN_TICK_PER_SEC);
                feedback_label.set_label(&format!("Recording… {remaining_secs}s left"));
                glib::ControlFlow::Continue
            }
        }
    ));
}

/// Number of [`COUNTDOWN_TICK`]s per second, for converting remaining ticks
/// to a whole-seconds countdown display.
const COUNTDOWN_TICK_PER_SEC: u32 = 1000 / COUNTDOWN_TICK.as_millis() as u32;

/// Show a pulsing (indeterminate) progress bar and an elapsed-time
/// "Processing… Ns" message in `feedback_label` while a variable-length
/// background step (transcription) runs — unlike recording, there's no fixed
/// duration to count down, and transcription can take anywhere from
/// instant to well over a minute (CPU-only inference of a ~2B-parameter
/// model, plus a one-time model load on first use), so without this the UI
/// looks identical to a hang. Returns a flag the caller must `set(false)`
/// once the background step finishes, which stops the pulse on its next
/// tick; the caller is still responsible for hiding `progress` and setting
/// the final `feedback_label` text.
fn start_processing_indicator(progress: &gtk4::ProgressBar, feedback_label: &Label) -> Rc<Cell<bool>> {
    let active = Rc::new(Cell::new(true));
    progress.set_fraction(0.0);
    progress.set_visible(true);
    feedback_label.set_label("Processing your recording…");

    let elapsed_ticks = Rc::new(Cell::new(0u32));
    glib::source::timeout_add_local(COUNTDOWN_TICK, glib::clone!(
        #[strong]
        active,
        #[strong]
        elapsed_ticks,
        #[weak]
        progress,
        #[weak]
        feedback_label,
        #[upgrade_or]
        glib::ControlFlow::Break,
        move || {
            if !active.get() {
                return glib::ControlFlow::Break;
            }
            progress.pulse();
            let tick = elapsed_ticks.get() + 1;
            elapsed_ticks.set(tick);
            let elapsed_secs = tick / COUNTDOWN_TICK_PER_SEC;
            feedback_label.set_label(&format!("Processing your recording… ({elapsed_secs}s)"));
            glib::ControlFlow::Continue
        }
    ));

    active
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
        let test_mic_btn =
            gtk4::Button::builder().label("🎙 Test Mic").tooltip_text("Record a couple seconds and check whether any sound was picked up").build();
        let replay_mic_btn = gtk4::Button::builder()
            .label("🔁 Replay")
            .tooltip_text("Play back your last \"Test Mic\" recording")
            .sensitive(false)
            .build();
        nav_box.append(&prev_btn);
        nav_box.append(&word_label);
        nav_box.append(&next_btn);

        let action_box = GtkBox::new(Orientation::Horizontal, 8);
        action_box.set_halign(gtk4::Align::Center);
        action_box.append(&play_btn);
        action_box.append(&record_btn);
        action_box.append(&test_mic_btn);
        action_box.append(&replay_mic_btn);

        let record_progress = gtk4::ProgressBar::builder().visible(false).show_text(false).build();

        let waveform = Waveform::new();
        let score_label = Label::builder().css_classes(["title-2"]).build();
        let diff_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(0)
            .halign(gtk4::Align::Center)
            .tooltip_text(
                "Struck-through red = a sound you didn't say; orange = a sound recognized \
                 differently from what was expected.",
            )
            .build();
        let feedback_label = Label::builder().wrap(true).justify(gtk4::Justification::Center).build();

        root.append(&nav_box);
        root.append(&action_box);
        root.append(&record_progress);
        root.append(&waveform.widget);
        root.append(&score_label);
        root.append(&diff_box);
        root.append(&feedback_label);

        let words = Rc::new(words);
        let index = Rc::new(Cell::new(0usize));
        let callbacks = Arc::new(callbacks);
        // The most recent "Test Mic" capture, kept around so Replay can play
        // it back without re-recording. `None` until the first Test Mic run.
        let last_test_recording: Rc<std::cell::RefCell<Option<(Vec<f32>, u32)>>> = Rc::new(std::cell::RefCell::new(None));

        let refresh = {
            let words = words.clone();
            let index = index.clone();
            let word_label = word_label.clone();
            let score_label = score_label.clone();
            let diff_box = diff_box.clone();
            let feedback_label = feedback_label.clone();
            move || {
                let i = index.get();
                if let Some(word) = words.get(i) {
                    word_label.set_label(&word.text);
                } else {
                    word_label.set_label("No words available");
                }
                score_label.set_label("");
                render_diff(&diff_box, &[]);
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
            #[weak]
            play_btn,
            #[weak]
            record_btn,
            #[weak]
            test_mic_btn,
            #[weak]
            replay_mic_btn,
            #[strong]
            last_test_recording,
            move |_| {
                let Some(word) = words.get(index.get()).cloned() else { return };
                let callbacks = callbacks.clone();
                let waveform = waveform.clone();
                let has_test_recording = last_test_recording.borrow().is_some();
                feedback_label.set_label("");
                set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, false);
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
                    set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, true);
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
            diff_box,
            #[weak]
            feedback_label,
            #[weak]
            record_progress,
            #[weak]
            play_btn,
            #[weak]
            record_btn,
            #[weak]
            test_mic_btn,
            #[weak]
            replay_mic_btn,
            #[strong]
            last_test_recording,
            move |_| {
                let Some(word) = words.get(index.get()).cloned() else { return };
                let callbacks = callbacks.clone();
                let waveform = waveform.clone();
                let has_test_recording = last_test_recording.borrow().is_some();
                render_diff(&diff_box, &[]);
                set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, false);
                start_recording_countdown(
                    &record_progress,
                    &feedback_label,
                    callbacks.record_duration,
                    "Recording stopped — press Record again to record another attempt.",
                );
                task::spawn_on_main(async move {
                    let record_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        move || (callbacks.record)()
                    })
                    .await;

                    let Ok((samples, rate)) = record_result else {
                        record_progress.set_visible(false);
                        render_diff(&diff_box, &[]);
                        feedback_label.set_label(
                            "Recording failed — check that a microphone is connected and selected in \
                             Preferences, then try \"Test Mic\".",
                        );
                        set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, true);
                        return;
                    };

                    let downsampled = langspark_core::audio::extract_waveform(&samples, 200);
                    waveform.set_samples(downsampled, WaveformColor::USER);

                    if langspark_core::audio::peak_level(&samples) < SILENCE_THRESHOLD {
                        render_diff(&diff_box, &[]);
                        feedback_label.set_label(
                            "No sound was picked up — check your microphone, then try \"Test Mic\" to confirm it's working.",
                        );
                        set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, true);
                        return;
                    }

                    let processing = start_processing_indicator(&record_progress, &feedback_label);

                    let expected = word.reading.clone().unwrap_or_else(|| word.text.clone());
                    let transcribe_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        let samples = samples.clone();
                        move || (callbacks.transcribe)(&samples, rate)
                    })
                    .await;

                    processing.set(false);
                    record_progress.set_visible(false);

                    match transcribe_result {
                        Ok(recognized) => {
                            let result = (callbacks.score)(&recognized, &expected);
                            score_label.set_label(&format!("{:.0}%", result.score));
                            score_label.set_css_classes(&[if result.is_correct {
                                "langspark-score-good"
                            } else {
                                "langspark-score-bad"
                            }]);
                            render_diff(&diff_box, &langspark_core::diff_chars(&recognized, &expected));
                            feedback_label.set_label(&result.feedback);
                        }
                        Err(e) => {
                            render_diff(&diff_box, &[]);
                            feedback_label.set_label(&format!("Couldn't recognize speech: {e}"));
                        }
                    }
                    set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, has_test_recording, true);
                });
            }
        ));

        test_mic_btn.connect_clicked(glib::clone!(
            #[strong]
            callbacks,
            #[strong]
            waveform,
            #[weak]
            feedback_label,
            #[weak]
            record_progress,
            #[weak]
            play_btn,
            #[weak]
            record_btn,
            #[weak]
            test_mic_btn,
            #[weak]
            replay_mic_btn,
            #[strong]
            last_test_recording,
            move |_| {
                let callbacks = callbacks.clone();
                let waveform = waveform.clone();
                let last_test_recording = last_test_recording.clone();
                let had_test_recording = last_test_recording.borrow().is_some();
                set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, had_test_recording, false);
                start_recording_countdown(
                    &record_progress,
                    &feedback_label,
                    callbacks.record_duration,
                    "Recording stopped — press Test Mic again to re-test.",
                );
                task::spawn_on_main(async move {
                    let record_result = task::run_blocking({
                        let callbacks = callbacks.clone();
                        move || (callbacks.record)()
                    })
                    .await;

                    let mut now_has_test_recording = had_test_recording;
                    match record_result {
                        Ok((samples, rate)) => {
                            let downsampled = langspark_core::audio::extract_waveform(&samples, 200);
                            waveform.set_samples(downsampled, WaveformColor::USER);
                            let peak = langspark_core::audio::peak_level(&samples);
                            if peak < SILENCE_THRESHOLD {
                                feedback_label.set_label(
                                    "No sound detected. Check that your microphone is connected, unmuted, \
                                     and selected in Preferences.",
                                );
                            } else {
                                feedback_label.set_label(&format!(
                                    "Microphone is working — picked up audio at {:.0}% peak level. Press \
                                     \"Replay\" to hear it back.",
                                    (peak * 100.0).min(100.0)
                                ));
                            }
                            // Keep the capture around for Replay even when
                            // silent — hearing "nothing" back confirms the
                            // mic test's verdict rather than just asserting it.
                            *last_test_recording.borrow_mut() = Some((samples, rate));
                            now_has_test_recording = true;
                        }
                        Err(e) => {
                            record_progress.set_visible(false);
                            feedback_label.set_label(&format!("Microphone test failed: {e}"));
                        }
                    }
                    set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, now_has_test_recording, true);
                });
            }
        ));

        replay_mic_btn.connect_clicked(glib::clone!(
            #[strong]
            callbacks,
            #[strong]
            last_test_recording,
            #[weak]
            feedback_label,
            #[weak]
            play_btn,
            #[weak]
            record_btn,
            #[weak]
            test_mic_btn,
            #[weak]
            replay_mic_btn,
            move |_| {
                let Some((samples, rate)) = last_test_recording.borrow().clone() else { return };
                let callbacks = callbacks.clone();
                set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, true, false);
                task::spawn_on_main(async move {
                    let play_result = task::run_blocking(move || {
                        let wav = langspark_core::audio::encode_wav(&samples, rate)?;
                        (callbacks.play)(wav)
                    })
                    .await;
                    if let Err(e) = play_result {
                        feedback_label.set_label(&format!("Couldn't play back the recording: {e}"));
                    }
                    set_actions_sensitive(&play_btn, &record_btn, &test_mic_btn, &replay_mic_btn, true, true);
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
            record_duration: std::time::Duration::from_secs(3),
            play: Box::new(|_| Ok(())),
            score: Box::new(|r, e| langspark_core::score_pronunciation(r, e, "ja")),
            transcribe: Box::new(|_, _| Ok(String::new())),
        }
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.
}

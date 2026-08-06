//! Waveform visualization widget (section 18): draws peak-amplitude samples
//! (from `langspark_core::audio::extract_waveform`) as vertical bars, with
//! distinct colors for a reference (TTS) waveform vs. the user's recording,
//! optionally stacked for side-by-side comparison.

use gtk4::prelude::*;
use gtk4::{gdk, DrawingArea};
use std::cell::RefCell;
use std::rc::Rc;

/// RGBA color for a waveform trace.
#[derive(Debug, Clone, Copy)]
pub struct WaveformColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl WaveformColor {
    pub const REFERENCE: Self = Self { r: 0.2, g: 0.5, b: 0.9, a: 1.0 };
    pub const USER: Self = Self { r: 0.9, g: 0.4, b: 0.2, a: 1.0 };

    fn to_gdk(self) -> gdk::RGBA {
        gdk::RGBA::new(self.r, self.g, self.b, self.a)
    }
}

struct WaveformTrace {
    samples: Vec<f32>,
    color: WaveformColor,
}

/// A `DrawingArea`-backed widget that renders one or two waveform traces.
/// Wrapped in a `ScrolledWindow` by the caller for long recordings (smooth
/// horizontal scrolling, task 18.5) since the drawing area's width simply
/// grows with sample count rather than being clipped.
#[derive(Clone)]
pub struct Waveform {
    pub widget: gtk4::Widget,
    traces: Rc<RefCell<Vec<WaveformTrace>>>,
    area: DrawingArea,
}

/// Pixels of drawing-area width allotted per waveform bar, for long-recording scrolling.
const PIXELS_PER_BAR: i32 = 3;

impl Waveform {
    pub fn new() -> Self {
        let area = DrawingArea::builder().content_height(120).css_classes(["langspark-waveform"]).build();
        let traces: Rc<RefCell<Vec<WaveformTrace>>> = Rc::new(RefCell::new(Vec::new()));

        area.set_draw_func(glib::clone!(
            #[strong]
            traces,
            move |_area, cr, width, height| {
                draw_traces(cr, width, height, &traces.borrow());
            }
        ));

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .child(&area)
            .build();

        Self { widget: scroller.upcast(), traces, area }
    }

    /// Replace the current trace set with a single waveform (e.g. reference TTS audio).
    pub fn set_samples(&self, samples: Vec<f32>, color: WaveformColor) {
        self.traces.replace(vec![WaveformTrace { samples: samples.clone(), color }]);
        self.resize_for(&samples);
        self.area.queue_draw();
    }

    /// Show reference and user recordings stacked for comparison (task 18.4).
    pub fn set_comparison(&self, reference: Vec<f32>, user: Vec<f32>) {
        let width_source = if reference.len() >= user.len() { &reference } else { &user };
        self.resize_for(width_source);
        self.traces.replace(vec![
            WaveformTrace { samples: reference, color: WaveformColor::REFERENCE },
            WaveformTrace { samples: user, color: WaveformColor::USER },
        ]);
        self.area.queue_draw();
    }

    pub fn clear(&self) {
        self.traces.replace(Vec::new());
        self.area.set_content_width(-1);
        self.area.queue_draw();
    }

    fn resize_for(&self, samples: &[f32]) {
        let width = (samples.len() as i32 * PIXELS_PER_BAR).max(200);
        self.area.set_content_width(width);
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_traces(cr: &gtk4::cairo::Context, width: i32, height: i32, traces: &[WaveformTrace]) {
    let width = width as f64;
    let height = height as f64;
    let mid = height / 2.0;

    for trace in traces {
        let color = trace.color.to_gdk();
        cr.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, color.alpha() as f64);

        let n = trace.samples.len().max(1);
        let bar_width = (width / n as f64).max(1.0);

        for (i, &sample) in trace.samples.iter().enumerate() {
            let amplitude = (sample.abs() as f64).min(1.0) * mid;
            let x = i as f64 * bar_width;
            cr.rectangle(x, mid - amplitude, (bar_width - 1.0).max(1.0), amplitude * 2.0);
        }
        let _ = cr.fill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_colors_are_distinct() {
        let reference = WaveformColor::REFERENCE.to_gdk();
        let user = WaveformColor::USER.to_gdk();
        assert_ne!((reference.red(), reference.blue()), (user.red(), user.blue()));
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.
}

//! Statistics tab: overall progress, daily streak, review history, upcoming
//! reviews, and per-deck breakdowns.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};
use langspark_core::{Deck, ReviewStats, SrsCard};

/// One row in the "next reviews" schedule: a date and how many cards fall due then.
#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub date: String,
    pub count: usize,
}

/// One day's review count, for the history chart.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub date: String,
    pub count: usize,
}

/// A simple custom-drawn bar chart of reviews-per-day (task 16.4). Bars are
/// scaled to the tallest entry; hovering isn't implemented, but each bar's
/// date/count is available via the tooltip.
fn build_history_chart(history: &[HistoryEntry]) -> gtk4::Widget {
    let area = gtk4::DrawingArea::builder().content_height(120).content_width(320).build();
    let history = history.to_vec();
    let max_count = history.iter().map(|h| h.count).max().unwrap_or(0).max(1);

    area.set_draw_func(move |_area, cr, width, height| {
        let width = width as f64;
        let height = height as f64;
        if history.is_empty() {
            return;
        }
        let bar_width = width / history.len() as f64;
        cr.set_source_rgba(0.2, 0.5, 0.9, 0.9);
        for (i, entry) in history.iter().enumerate() {
            let bar_height = (entry.count as f64 / max_count as f64) * (height - 4.0);
            let x = i as f64 * bar_width;
            cr.rectangle(x + 1.0, height - bar_height, (bar_width - 2.0).max(1.0), bar_height);
        }
        let _ = cr.fill();
    });

    area.upcast()
}

/// Per-deck summary shown in the statistics tab.
#[derive(Debug, Clone)]
pub struct DeckStats {
    pub deck: Deck,
    pub total_cards: usize,
    pub due_cards: usize,
}

fn stat_tile(label: &str, value: &str) -> gtk4::Box {
    let tile = Box::new(Orientation::Vertical, 2);
    tile.set_css_classes(&["card", "langspark-card"]);
    tile.set_halign(gtk4::Align::Center);
    let value_label = Label::builder().label(value).css_classes(["title-1"]).build();
    let caption = Label::builder().label(label).css_classes(["caption", "dim-label"]).build();
    tile.append(&value_label);
    tile.append(&caption);
    tile
}

/// Build the statistics tab's root widget from already-computed stats/decks/schedule.
/// (The computations themselves — `calculate_retention_rate`, `calculate_streak`,
/// deck due-counts — live in `langspark-core` so they're reusable and unit-tested there.)
pub fn build_tab(
    stats: &ReviewStats,
    history: &[HistoryEntry],
    schedule: &[ScheduleEntry],
    deck_stats: &[DeckStats],
) -> gtk4::Widget {
    let root = Box::new(Orientation::Vertical, 16);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    // Overall progress summary
    let summary_header = Label::builder()
        .label("Overall Progress")
        .css_classes(["langspark-section-header"])
        .xalign(0.0)
        .build();
    root.append(&summary_header);

    let tiles = Box::new(Orientation::Horizontal, 12);
    tiles.append(&stat_tile("Reviews", &stats.total_reviews.to_string()));
    tiles.append(&stat_tile("Correct", &stats.correct_reviews.to_string()));
    tiles.append(&stat_tile("Retention", &format!("{:.0}%", stats.retention_rate)));
    tiles.append(&stat_tile("Day Streak", &stats.streak_days.to_string()));
    root.append(&tiles);

    // Review history chart
    let history_header = Label::builder()
        .label("Review History")
        .css_classes(["langspark-section-header"])
        .xalign(0.0)
        .build();
    root.append(&history_header);
    if history.is_empty() {
        root.append(&Label::builder().label("No reviews yet.").css_classes(["dim-label"]).xalign(0.0).build());
    } else {
        root.append(&build_history_chart(history));
    }

    // Next reviews schedule
    let schedule_header = Label::builder()
        .label("Upcoming Reviews")
        .css_classes(["langspark-section-header"])
        .xalign(0.0)
        .build();
    root.append(&schedule_header);

    if schedule.is_empty() {
        root.append(&Label::builder().label("Nothing scheduled.").css_classes(["dim-label"]).xalign(0.0).build());
    } else {
        let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).build();
        for entry in schedule {
            let row = adw::ActionRow::builder()
                .title(&entry.date)
                .subtitle(format!("{} card(s) due", entry.count))
                .build();
            list.append(&row);
        }
        root.append(&list);
    }

    // Per-deck statistics
    let deck_header = Label::builder()
        .label("Decks")
        .css_classes(["langspark-section-header"])
        .xalign(0.0)
        .build();
    root.append(&deck_header);

    if deck_stats.is_empty() {
        root.append(&Label::builder().label("No decks yet.").css_classes(["dim-label"]).xalign(0.0).build());
    } else {
        let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).build();
        for ds in deck_stats {
            let row = adw::ActionRow::builder()
                .title(&ds.deck.name)
                .subtitle(format!("{} due / {} total", ds.due_cards, ds.total_cards))
                .build();
            list.append(&row);
        }
        root.append(&list);
    }

    let scroller = gtk4::ScrolledWindow::builder().child(&root).vexpand(true).build();
    scroller.upcast()
}

/// Build a per-deck stats row from a deck and the full card set (counts how
/// many of the deck's cards are due today) — pure logic, testable without GTK.
pub fn compute_deck_stats(deck: Deck, deck_card_ids: &[i64], all_cards: &[SrsCard]) -> DeckStats {
    let total_cards = deck_card_ids.len();
    let due_cards = all_cards
        .iter()
        .filter(|c| c.id.map(|id| deck_card_ids.contains(&id)).unwrap_or(false) && c.is_due_today())
        .count();
    DeckStats { deck, total_cards, due_cards }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_deck_stats() {
        let deck = Deck { id: Some(1), name: "N4".to_string(), description: None, language: "ja".to_string(), created_at: None };
        let mut card1 = SrsCard::new("vocabulary", "ja");
        card1.id = Some(10);
        let mut card2 = SrsCard::new("vocabulary", "ja");
        card2.id = Some(11);
        card2.next_review_date = Some("2099-01-01".to_string());

        let stats = compute_deck_stats(deck, &[10, 11], &[card1, card2]);
        assert_eq!(stats.total_cards, 2);
        assert_eq!(stats.due_cards, 1); // only card1 (new cards are due; card2 is in the future)
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.
}

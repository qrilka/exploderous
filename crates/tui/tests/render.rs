use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_tui::{App, TICK_INTERVAL, render};
use ratatui::{Terminal, backend::TestBackend};
use unicode_width::UnicodeWidthStr;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
}

fn draw(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    terminal
}

fn screen(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

#[test]
fn centered_map_has_one_unpadded_glyph_per_cell_and_display_only_status() {
    let app = App::new(Instant::now(), 19, 11);
    let terminal = draw(&app, 19, 11);
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(5, 0)].symbol(), "█");
    assert_eq!(buffer[(6, 1)].symbol(), "@");
    assert_eq!(buffer[(7, 2)].symbol(), "█");
    assert_eq!(buffer[(7, 1)].symbol(), " ");
    for (column, row) in [
        (3, 1),
        (5, 1),
        (5, 3),
        (1, 5),
        (3, 5),
        (7, 5),
        (3, 7),
        (5, 7),
    ] {
        assert_eq!(
            buffer[(5 + column, row)].symbol(),
            "▒",
            "cell=({column},{row})"
        );
    }
    assert_eq!(screen(&terminal)[9], "Lives: 0 | Score: 0");
    assert_eq!(screen(&terminal)[10].trim(), "Offset: +000,+000");
    for y in 0..9 {
        for x in 5..14 {
            assert_eq!(UnicodeWidthStr::width(buffer[(x, y)].symbol()), 1);
        }
    }
}

#[test]
fn larger_terminal_centers_the_fixed_arena_and_status_below_it() {
    let app = App::new(Instant::now(), 30, 20);
    let terminal = draw(&app, 30, 20);
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(10, 4)].symbol(), "█");
    assert_eq!(buffer[(11, 5)].symbol(), "@");
    assert_eq!(screen(&terminal)[13].trim(), "Lives: 0 | Score: 0");
    assert_eq!(screen(&terminal)[14].trim(), "Offset: +000,+000");
}

#[test]
fn center_offset_tracks_movement_and_rebases_at_the_cell_boundary() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(key(KeyCode::Down), start);

    app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(screen(&draw(&app, 19, 11))[10].trim(), "Offset: +000,+016");

    for tick in 2..=8 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 11);
    assert_eq!(screen(&terminal)[10].trim(), "Offset: +000,-128");
    assert_eq!(terminal.backend().buffer()[(6, 2)].symbol(), "@");
    assert_eq!(terminal.backend().buffer()[(6, 1)].symbol(), " ");
}

#[test]
fn center_offset_shows_gradual_assistance_without_claiming_full_centering() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(key(KeyCode::Down), start);
    for tick in 1..=4 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    app.handle_key(key(KeyCode::Right), start + TICK_INTERVAL * 4);
    for (tick, offset) in [
        (5, "Offset: +016,+064"),
        (6, "Offset: +032,+064"),
        (7, "Offset: +032,+048"),
        (8, "Offset: +032,+032"),
        (9, "Offset: +048,+032"),
    ] {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
        assert_eq!(screen(&draw(&app, 19, 11))[10].trim(), offset);
    }
}

#[test]
fn help_documents_controls_live_and_reserved_glyphs_and_scrolls_wrapped_labels() {
    let now = Instant::now();
    let mut app = App::new(now, 80, 30);
    app.handle_key(key(KeyCode::Char('h')), now);
    let terminal = draw(&app, 80, 30);
    let text = screen(&terminal).join("\n");
    for entry in [
        "Arrow keys: move",
        "R: reset; Q or Escape: quit",
        "(blank space) Floor",
        "█ Wall",
        "▒ Block",
        "@ Player",
        "Offset: signed X,Y world-unit delta from the current Cell centre; 0,0 is centred",
        "a AI-controlled non-Enemy Character",
        "e Enemy Character (takes precedence over AI)",
        "● Bomb",
        "┼ Blast centre",
        "│ Blast north/south arms",
        "─ Blast east/west arms",
        "⬚ Exit",
        "▣ Bomb-capacity Item",
        "◈ Blast-reach Item",
    ] {
        assert!(text.contains(entry), "missing {entry:?} in:\n{text}");
    }

    let mut narrow = App::new(now, 19, 11);
    narrow.handle_key(key(KeyCode::Char('?')), now);
    for _ in 0..100 {
        narrow.handle_key(key(KeyCode::Down), now);
    }
    let terminal = draw(&narrow, 19, 11);
    assert!(screen(&terminal).join("\n").contains("◈ Blast-reach Item"));
}

#[test]
fn every_live_and_reserved_glyph_occupies_one_terminal_column() {
    for glyph in ["█", "▒", "@", "a", "e", "●", "┼", "│", "─", "⬚", "▣", "◈"] {
        assert_eq!(UnicodeWidthStr::width(glyph), 1, "glyph={glyph}");
    }
}

#[test]
fn undersized_terminal_shows_resize_instructions_instead_of_the_map() {
    for (width, height) in [(18, 10), (19, 10)] {
        let app = App::new(Instant::now(), width, height);
        let terminal = draw(&app, width, height);
        let text = screen(&terminal).join("\n");
        assert!(text.contains("Resize terminal"));
        assert!(!text.contains("Lives: 0 | Score: 0"));
    }
}

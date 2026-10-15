mod support;

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
fn game_over_modal_overlays_the_arena_at_minimum_size_and_is_removed_on_restart() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    let initial = screen(&draw(&app, 19, 11));
    app.handle_key(key(KeyCode::Char(' ')), start);
    for tick in 1..=121 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    for (width, height, left, top) in [(19, 11, 0, 2), (80, 30, 30, 11)] {
        let rows = screen(&draw(&app, width, height));
        // Literal placement is over the middle five Arena rows, not below it.
        // Interior spaces must erase underlying terrain, not let glyphs bleed through.
        for (line, expected) in [
            "┌─────────────────┐",
            "│    GAME OVER    │",
            "│   R: restart    │",
            "│   Q/Esc: quit   │",
            "└─────────────────┘",
        ]
        .into_iter()
        .enumerate()
        {
            let actual: String = rows[top + line].chars().skip(left).take(19).collect();
            assert_eq!(actual, expected);
        }
        let text = rows.join("\n");
        assert!(!text.contains("Lives:"));
        assert!(!text.contains("Offset:"));
    }
    let defeated = screen(&draw(&app, 19, 11));
    app.handle_key(key(KeyCode::Char('h')), start + TICK_INTERVAL * 121);
    app.advance(start + TICK_INTERVAL * 1000).unwrap();
    assert_eq!(screen(&draw(&app, 19, 11)), defeated);
    app.handle_key(key(KeyCode::Char('r')), start + TICK_INTERVAL * 1000);
    assert_eq!(screen(&draw(&app, 19, 11)), initial);
}

#[test]
fn centered_map_has_one_unpadded_glyph_per_cell_and_display_only_status() {
    let app = App::new(Instant::now(), 19, 11);
    let terminal = draw(&app, 19, 11);
    let buffer = terminal.backend().buffer();
    let exit = app.game().exit().expect("the Normal level has an Exit");
    assert_eq!((exit.column().value(), exit.row().value()), (5, 3));
    assert!(!screen(&terminal).join("\n").contains('⬚'));

    assert_eq!(buffer[(5, 0)].symbol(), "█");
    assert_eq!(buffer[(6, 1)].symbol(), "@");
    assert_eq!(buffer[(7, 2)].symbol(), "█");
    assert_eq!(buffer[(7, 1)].symbol(), " ");
    for (column, row) in [(3, 3), (5, 3), (7, 3), (3, 5), (7, 5), (5, 7)] {
        assert_eq!(
            buffer[(5 + column, row)].symbol(),
            "▒",
            "cell=({column},{row})"
        );
    }
    for (column, row) in [(3, 1), (5, 1), (1, 5), (5, 5), (3, 7)] {
        assert_eq!(
            buffer[(5 + column, row)].symbol(),
            " ",
            "cleared Cell=({column},{row})"
        );
    }
    assert_eq!(
        buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == "▒")
            .count(),
        6
    );
    assert_eq!(screen(&terminal)[9], "Lives: 0 | Score: 0");
    assert_eq!(screen(&terminal)[10].trim(), "Offset: +000,+000");
    for y in 0..9 {
        for x in 5..14 {
            assert_eq!(UnicodeWidthStr::width(buffer[(x, y)].symbol()), 1);
        }
    }
}

#[test]
fn blast_origins_and_both_arm_directions_use_live_glyphs_without_hiding_walls() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(key(KeyCode::Char(' ')), start);
    app.handle_key(key(KeyCode::Right), start);
    for tick in 1..=32 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    app.handle_key(
        KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Release),
        start + TICK_INTERVAL * 32,
    );
    for tick in 33..=121 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 11);
    let buffer = terminal.backend().buffer();
    for ((x, y), glyph) in [
        ((6, 1), "┼"),
        ((7, 1), "─"),
        ((6, 2), "│"),
        ((6, 0), "█"),
        ((5, 1), "█"),
        ((8, 1), "@"),
    ] {
        assert_eq!(buffer[(x, y)].symbol(), glyph);
    }
}

#[test]
fn a_bomb_clears_a_passage_and_its_blast_covers_the_destroyed_block() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    let mut tick = 0;
    // Reach (2,3), next to the new Block at (3,3), through ordinary input.
    for (code, count) in [(KeyCode::Down, 32), (KeyCode::Right, 16)] {
        app.handle_key(key(code), start + TICK_INTERVAL * tick);
        for _ in 0..count {
            tick += 1;
            app.advance(start + TICK_INTERVAL * tick).unwrap();
        }
        app.handle_key(
            KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Release),
            start + TICK_INTERVAL * tick,
        );
    }
    app.handle_key(key(KeyCode::Char(' ')), start + TICK_INTERVAL * tick);
    app.handle_key(key(KeyCode::Left), start + TICK_INTERVAL * tick);
    for _ in 0..16 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    app.handle_key(
        KeyEvent::new_with_kind(KeyCode::Left, KeyModifiers::NONE, KeyEventKind::Release),
        start + TICK_INTERVAL * tick,
    );
    let terminal = draw(&app, 19, 11);
    assert_eq!(terminal.backend().buffer()[(7, 3)].symbol(), "●");
    assert_eq!(terminal.backend().buffer()[(8, 3)].symbol(), "▒");
    // Get out of the left Blast arm, not just off the Bomb's origin Cell.
    app.handle_key(key(KeyCode::Up), start + TICK_INTERVAL * tick);
    for _ in 0..16 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    app.handle_key(
        KeyEvent::new_with_kind(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Release),
        start + TICK_INTERVAL * tick,
    );
    while tick < 169 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 11);
    assert_eq!(terminal.backend().buffer()[(7, 3)].symbol(), "┼");
    assert_eq!(terminal.backend().buffer()[(8, 3)].symbol(), "─");
    assert_eq!(terminal.backend().buffer()[(7, 2)].symbol(), "█");
    while tick < 199 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 11);
    assert_eq!(terminal.backend().buffer()[(7, 3)].symbol(), " ");
    assert_eq!(terminal.backend().buffer()[(8, 3)].symbol(), " ");
    app.handle_key(key(KeyCode::Char('r')), start + TICK_INTERVAL * tick);
    assert_eq!(
        screen(&draw(&app, 19, 11)),
        screen(&draw(&App::new(start, 19, 11), 19, 11))
    );
}

#[test]
fn the_exit_is_hidden_by_terrain_then_blasts_and_victory_has_a_distinct_modal() {
    let mut now = Instant::now();
    let mut app = App::new(now, 19, 11);
    support::reveal_exit(&mut app, &mut now);
    assert_eq!(app.game().outcome(), None);
    assert_eq!(app.game().tick(), 201);
    assert_eq!(draw(&app, 19, 11).backend().buffer()[(10, 3)].symbol(), "│");
    assert!(!screen(&draw(&app, 19, 11)).join("\n").contains('⬚'));
    support::ticks(&mut app, &mut now, 30);
    assert_eq!(draw(&app, 19, 11).backend().buffer()[(10, 3)].symbol(), "⬚");
    support::travel(&mut app, &mut now, KeyCode::Left, 16);
    support::travel(&mut app, &mut now, KeyCode::Down, 24);
    for (width, height, left, top) in [(19, 11, 0, 2), (80, 30, 30, 11)] {
        let rows = screen(&draw(&app, width, height));
        for (line, expected) in [
            "┌─────────────────┐",
            "│    VICTORY!     │",
            "│   R: restart    │",
            "│   Q/Esc: quit   │",
            "└─────────────────┘",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                rows[top + line]
                    .chars()
                    .skip(left)
                    .take(19)
                    .collect::<String>(),
                expected
            );
        }
        assert!(!rows.join("\n").contains("GAME OVER"));
    }
    let victory = screen(&draw(&app, 19, 11));
    app.handle_key(key(KeyCode::Char('h')), now);
    support::ticks(&mut app, &mut now, 100);
    assert_eq!(screen(&draw(&app, 19, 11)), victory);
    app.handle_key(key(KeyCode::Char('r')), now);
    assert_eq!(
        screen(&draw(&app, 19, 11)),
        screen(&draw(&App::new(now, 19, 11), 19, 11))
    );
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
    let mut app = App::new(now, 80, 40);
    app.handle_key(key(KeyCode::Char('h')), now);
    let terminal = draw(&app, 80, 40);
    let text = screen(&terminal).join("\n");
    for entry in [
        "Arrow keys: move",
        "Space: place one Bomb; place another after it detonates",
        "Blasts clear Blocks; entering a Blast Cell with your centre is lethal: Game Over",
        "Bombs reveal an Exit hidden beneath a Block; reach its Cell alive to win",
        "Victory or Game Over freezes play; only restart or quit acts",
        "R: restart during play or either outcome; Q or Escape: quit",
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

    let (live, reserved) = text.split_once("Reserved glyphs").unwrap();
    for entry in [
        "● Bomb",
        "┼ Blast centre",
        "│ Blast north/south arms",
        "─ Blast east/west arms",
        "⬚ Exit",
    ] {
        assert!(live.contains(entry));
        assert!(!reserved.contains(entry));
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

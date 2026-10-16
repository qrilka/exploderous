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
    let mut app = App::new(start, 19, 17);
    let initial = screen(&draw(&app, 19, 17));
    app.handle_key(key(KeyCode::Char(' ')), start);
    for tick in 1..=121 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    for (width, height, left, top) in [(19, 17, 0, 5), (80, 30, 30, 11)] {
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
        assert!(!text.contains("Blast reach:"));
        assert!(!text.contains("Offset:"));
    }
    let defeated = screen(&draw(&app, 19, 17));
    app.handle_key(key(KeyCode::Char('h')), start + TICK_INTERVAL * 121);
    app.advance(start + TICK_INTERVAL * 1000).unwrap();
    assert_eq!(screen(&draw(&app, 19, 17)), defeated);
    app.handle_key(key(KeyCode::Char('r')), start + TICK_INTERVAL * 1000);
    assert_eq!(screen(&draw(&app, 19, 17)), initial);
}

#[test]
fn centered_map_has_thirteen_playable_cells_inside_a_separate_frame() {
    for (width, height, left, top) in [(19, 17, 2, 0), (30, 20, 7, 1), (80, 30, 32, 6)] {
        let app = App::new(Instant::now(), width, height);
        let terminal = draw(&app, width, height);
        let buffer = terminal.backend().buffer();
        let exit = app.game().exit().expect("the Normal level has an Exit");
        assert_eq!((exit.column().value(), exit.row().value()), (4, 2));
        assert!(!screen(&terminal).join("\n").contains('⬚'));
        for offset in 0..15 {
            assert_eq!(buffer[(left + offset, top)].symbol(), "▓");
            assert_eq!(buffer[(left + offset, top + 14)].symbol(), "▓");
            assert_eq!(buffer[(left, top + offset)].symbol(), "▓");
            assert_eq!(buffer[(left + 14, top + offset)].symbol(), "▓");
        }
        // One glyph per authoritative Cell, with no terrain ring or hidden margins.
        for (cell, terrain) in app.game().arena().cells() {
            let x = left + 1 + cell.column().value() as u16;
            let y = top + 1 + cell.row().value() as u16;
            let expected = if cell.column().value() == 0 && cell.row().value() == 0 {
                "@"
            } else {
                match terrain {
                    exploderous_core::game::Terrain::Floor => " ",
                    exploderous_core::game::Terrain::Wall => "█",
                    exploderous_core::game::Terrain::Block => "▒",
                }
            };
            assert_eq!(buffer[(x, y)].symbol(), expected, "cell={cell:?}");
            assert_eq!(UnicodeWidthStr::width(buffer[(x, y)].symbol()), 1);
        }
        assert_eq!(
            buffer
                .content()
                .iter()
                .filter(|cell| cell.symbol() == "▒")
                .count(),
            6
        );
        let rows = screen(&terminal);
        assert_eq!(rows[usize::from(top + 15)].trim(), "Blast reach: 1");
        assert_eq!(rows[usize::from(top + 16)].trim(), "Offset: +000,+000");
    }
}

#[test]
fn blast_origins_and_both_arm_directions_use_live_glyphs_without_hiding_walls() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
    let terminal = draw(&app, 19, 17);
    let buffer = terminal.backend().buffer();
    for ((x, y), glyph) in [
        ((3, 1), "┼"),
        ((4, 1), "─"),
        ((3, 2), "│"),
        ((3, 0), "▓"), // exterior frame, not a Wall
        ((2, 1), "▓"),
        ((4, 2), "█"), // interior pillar
        ((5, 1), "@"),
    ] {
        assert_eq!(buffer[(x, y)].symbol(), glyph);
    }
}

#[test]
fn a_bomb_clears_a_passage_and_its_blast_covers_the_destroyed_block() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
    let mut tick = 0;
    // Reach (1,2), next to the Block at (2,2), through ordinary input.
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
    let terminal = draw(&app, 19, 17);
    assert_eq!(terminal.backend().buffer()[(4, 3)].symbol(), "●");
    assert_eq!(terminal.backend().buffer()[(5, 3)].symbol(), "▒");
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
    let terminal = draw(&app, 19, 17);
    assert_eq!(terminal.backend().buffer()[(4, 3)].symbol(), "┼");
    assert_eq!(terminal.backend().buffer()[(5, 3)].symbol(), "─");
    assert_eq!(terminal.backend().buffer()[(4, 2)].symbol(), "█");
    while tick < 199 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 17);
    assert_eq!(terminal.backend().buffer()[(4, 3)].symbol(), " ");
    assert_eq!(terminal.backend().buffer()[(5, 3)].symbol(), "◈");
    app.handle_key(key(KeyCode::Char('r')), start + TICK_INTERVAL * tick);
    assert_eq!(
        screen(&draw(&app, 19, 17)),
        screen(&draw(&App::new(start, 19, 17), 19, 17))
    );
}

#[test]
fn the_exit_is_hidden_by_terrain_then_blasts_and_victory_has_a_distinct_modal() {
    let mut now = Instant::now();
    let mut app = App::new(now, 19, 17);
    support::reveal_exit(&mut app, &mut now);
    assert_eq!(app.game().outcome(), None);
    assert_eq!(app.game().tick(), 201);
    assert_eq!(draw(&app, 19, 17).backend().buffer()[(7, 3)].symbol(), "│");
    assert!(!screen(&draw(&app, 19, 17)).join("\n").contains('⬚'));
    support::ticks(&mut app, &mut now, 30);
    assert_eq!(draw(&app, 19, 17).backend().buffer()[(7, 3)].symbol(), "⬚");
    support::travel(&mut app, &mut now, KeyCode::Left, 16);
    support::travel(&mut app, &mut now, KeyCode::Down, 24);
    for (width, height, left, top) in [(19, 17, 0, 5), (80, 30, 30, 11)] {
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
    let victory = screen(&draw(&app, 19, 17));
    app.handle_key(key(KeyCode::Char('h')), now);
    support::ticks(&mut app, &mut now, 100);
    assert_eq!(screen(&draw(&app, 19, 17)), victory);
    app.handle_key(key(KeyCode::Char('r')), now);
    assert_eq!(
        screen(&draw(&app, 19, 17)),
        screen(&draw(&App::new(now, 19, 17), 19, 17))
    );
}

#[test]
fn center_offset_tracks_movement_and_rebases_at_the_cell_boundary() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
    app.handle_key(key(KeyCode::Down), start);

    app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(screen(&draw(&app, 19, 17))[16].trim(), "Offset: +000,+016");

    for tick in 2..=8 {
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    let terminal = draw(&app, 19, 17);
    assert_eq!(screen(&terminal)[16].trim(), "Offset: +000,-128");
    assert_eq!(terminal.backend().buffer()[(3, 2)].symbol(), "@");
    assert_eq!(terminal.backend().buffer()[(3, 1)].symbol(), " ");
}

#[test]
fn center_offset_shows_gradual_assistance_without_claiming_full_centering() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
        assert_eq!(screen(&draw(&app, 19, 17))[16].trim(), offset);
    }
}

#[test]
fn help_documents_controls_live_and_reserved_glyphs_and_scrolls_wrapped_labels() {
    let now = Instant::now();
    let mut app = App::new(now, 100, 45);
    app.handle_key(key(KeyCode::Char('h')), now);
    let terminal = draw(&app, 100, 45);
    let text = screen(&terminal).join("\n");
    for entry in [
        "Arrow keys: move",
        "Space: place one Bomb; place another after it detonates",
        "While overlapping a new Bomb, move only away from its centre",
        "Once your whole Footprint clears the Bomb Cell, re-entry is blocked",
        "Blasts clear Blocks; entering a Blast Cell with your centre is lethal: Game Over",
        "Bombs reveal an Exit hidden beneath a Block; reach its Cell alive to win",
        "Victory or Game Over freezes play; only restart or quit acts",
        "R: restart during play or either outcome; Q or Escape: quit",
        "(blank space) Floor",
        "Live map: 13×13 playable Cells, plus a separate exterior frame",
        "▓ Exterior boundary (not terrain)",
        "█ Wall pillar",
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
        "◈ Fire-up",
        "Collect Fire-up with your centre to add one Cell of reach to later Bombs",
        "Blast reach shows your current ability; ticking Bombs keep their old reach",
        "A later Blast can destroy exposed Fire-up; collecting it is not required to win",
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
        "◈ Fire-up",
    ] {
        assert!(live.contains(entry));
        assert!(!reserved.contains(entry));
    }

    let mut narrow = App::new(now, 19, 17);
    narrow.handle_key(key(KeyCode::Char('?')), now);
    for _ in 0..100 {
        narrow.handle_key(key(KeyCode::Down), now);
    }
    let terminal = draw(&narrow, 19, 17);
    assert!(screen(&terminal).join("\n").contains("◈ Fire-up"));
}

#[test]
fn every_live_and_reserved_glyph_occupies_one_terminal_column() {
    for glyph in [
        "┌", "┐", "└", "┘", "▓", "█", "▒", "@", "a", "e", "●", "┼", "│", "─", "⬚", "▣", "◈",
    ] {
        assert_eq!(UnicodeWidthStr::width(glyph), 1, "glyph={glyph}");
    }
}

#[test]
fn undersized_terminal_shows_resize_instructions_instead_of_the_map() {
    for (width, height) in [(18, 17), (19, 16), (19, 11)] {
        let app = App::new(Instant::now(), width, height);
        let terminal = draw(&app, width, height);
        let text = screen(&terminal).join("\n");
        assert!(text.contains("Resize terminal"));
        assert!(!text.contains("Blast reach:"));
    }
}

#[test]
fn fire_up_is_hidden_then_revealed_collected_and_shows_the_longer_blast_at_supported_sizes() {
    for (width, height, left, top) in [(19, 17, 2, 0), (80, 30, 32, 6)] {
        let mut now = Instant::now();
        let mut app = App::new(now, width, height);
        let initial = screen(&draw(&app, width, height));
        assert!(!initial.join("\n").contains('◈'));
        assert!(initial.join("\n").contains("Blast reach: 1"));
        support::reveal_fire_up(&mut app, &mut now);
        assert_eq!(
            draw(&app, width, height).backend().buffer()[(left + 3, top + 3)].symbol(),
            "─"
        );
        support::ticks(&mut app, &mut now, 30);
        assert_eq!(
            draw(&app, width, height).backend().buffer()[(left + 3, top + 3)].symbol(),
            "◈"
        );
        support::travel(&mut app, &mut now, KeyCode::Down, 16);
        support::travel(&mut app, &mut now, KeyCode::Right, 32);
        let collected = screen(&draw(&app, width, height)).join("\n");
        assert!(!collected.contains('◈'));
        assert!(collected.contains("Blast reach: 2"));
        app.handle_key(key(KeyCode::Char(' ')), now);
        support::ticks(&mut app, &mut now, 1);
        support::travel(&mut app, &mut now, KeyCode::Left, 32);
        support::travel(&mut app, &mut now, KeyCode::Up, 16);
        support::ticks(&mut app, &mut now, 72);
        assert_eq!(
            draw(&app, width, height).backend().buffer()[(left + 3, top + 1)].symbol(),
            "│"
        );
        app.handle_key(key(KeyCode::Char('r')), now);
        assert_eq!(screen(&draw(&app, width, height)), initial);
    }
}

use super::*;

#[test]
fn centered_map_has_thirteen_playable_cells_inside_a_separate_frame() {
    for (width, height, left, top) in [(19, 17, 2, 0), (30, 20, 7, 1), (80, 30, 32, 6)] {
        let app = App::new_with_seed(Instant::now(), width, height, Default::default(), 42);
        let terminal = draw(&app, width, height);
        let buffer = terminal.backend().buffer();
        let text = screen(&terminal).join("\n");
        assert!(!text.contains('⬚'));
        assert!(!text.contains('▧'));
        assert!(!text.contains('◈'));
        assert_eq!(
            text.lines()
                .take(usize::from(top + 15))
                .flat_map(str::chars)
                .filter(|&glyph| glyph == 'e')
                .count(),
            3
        );
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
            } else if app
                .game()
                .enemies()
                .any(|enemy| app.game().arena().cell_containing(enemy.position) == Some(cell))
            {
                "e"
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
            app.game()
                .arena()
                .cells()
                .filter(|(_, terrain)| *terrain == exploderous_core::game::Terrain::Block)
                .count()
        );
        let rows = screen(&terminal);
        assert_eq!(rows[usize::from(top + 15)].trim(), "Blast reach: 1");
        assert_eq!(rows[usize::from(top + 16)].trim(), "Offset: +000,+000");
    }
}

#[test]
fn an_explicit_larger_scenario_pauses_until_its_own_frame_fits() {
    use exploderous_core::game::{
        CharacterId, GameRules, HumanBomberStart, Position, Scenario, Terrain, WorldX, WorldY,
    };
    let now = Instant::now();
    let scenario = Scenario::new(
        21,
        18,
        vec![Terrain::Floor; 21 * 18],
        HumanBomberStart {
            id: CharacterId::new(0),
            position: Position::new(WorldX::new(128), WorldY::new(128)),
        },
        GameRules::default(),
    )
    .unwrap();
    let mut app = App::from_scenario(now, 19, 17, scenario, 0);
    assert!(
        screen(&draw(&app, 19, 17))
            .join("\n")
            .contains("Resize terminal")
    );
    assert!(
        app.advance(now + TICK_INTERVAL * 100)
            .unwrap()
            .ticks
            .is_empty()
    );
    let resized = now + TICK_INTERVAL * 100;
    app.set_terminal_size(23, 22, resized);
    assert!(
        screen(&draw(&app, 23, 22))
            .join("\n")
            .contains("Blast reach: 1")
    );
    assert!(app.advance(resized).unwrap().ticks.is_empty());
    assert_eq!(app.advance(resized + TICK_INTERVAL).unwrap().ticks.len(), 1);
}

#[test]
fn center_offset_tracks_movement_and_rebases_at_the_cell_boundary() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
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
    let mut app = support::app(start, 19, 17);
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
    let mut app = support::app(now, 100, 45);
    app.handle_key(key(KeyCode::Char('h')), now);
    let terminal = draw(&app, 100, 45);
    let text = screen(&terminal).join("\n");
    for entry in [
        "Arrow keys: move",
        "Space: place one Bomb; place another after it detonates",
        "While overlapping a new Bomb, you can move toward or across it",
        "Once your whole Footprint clears the Bomb Cell, re-entry is blocked",
        "Blasts clear Blocks; entering a Blast Cell with your centre is lethal: Game Over",
        "Bombs reveal an Exit hidden beneath a Block; defeat all three Buroru, then reach it alive",
        "Buroru wander at half player speed; one Blast hit kills each",
        "Sharing a centre Cell with a Buroru is lethal; overlapping bodies in adjacent Cells are safe",
        "Victory or Game Over freezes play; only restart or quit acts",
        "R: restart with a freshly generated level; Q or Escape: quit",
        "(blank space) Floor",
        "Live map: 13×13 playable Cells, plus a separate exterior frame",
        "▓ Exterior boundary (not terrain)",
        "█ Wall pillar",
        "▒ Block",
        "@ Player",
        "Offset: signed X,Y world-unit delta from the current Cell centre; 0,0 is centred",
        "a AI-controlled non-Enemy Character",
        "e Buroru (wandering enemy)",
        "● Bomb",
        "┼ Blast centre",
        "│ Blast north/south arms",
        "─ Blast east/west arms",
        "▧ Exit locked (enemies survive)",
        "⬚ Exit ready",
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
        "▧ Exit locked",
        "⬚ Exit ready",
        "e Buroru",
        "◈ Fire-up",
    ] {
        assert!(live.contains(entry));
        assert!(!reserved.contains(entry));
    }

    let mut narrow = support::app(now, 19, 17);
    narrow.handle_key(key(KeyCode::Char('?')), now);
    for _ in 0..100 {
        narrow.handle_key(key(KeyCode::Down), now);
    }
    let terminal = draw(&narrow, 19, 17);
    assert!(screen(&terminal).join("\n").contains("◈ Fire-up"));
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

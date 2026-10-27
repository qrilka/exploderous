use super::*;

#[test]
fn blast_origins_and_both_arm_directions_use_live_glyphs_without_hiding_walls() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
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
    let mut app = support::app(start, 19, 17);
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
        screen(&draw(
            &support::generated_app(start, 19, 17, Default::default()),
            19,
            17
        ))
    );
}

#[test]
fn fire_up_is_hidden_then_revealed_collected_and_shows_the_longer_blast_at_supported_sizes() {
    for (width, height, left, top) in [(19, 17, 2, 0), (80, 30, 32, 6)] {
        let mut now = Instant::now();
        let mut app = support::app(now, width, height);
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
        assert_eq!(
            screen(&draw(&app, width, height)),
            screen(&draw(
                &support::generated_app(now, width, height, Default::default()),
                width,
                height,
            ))
        );
    }
}

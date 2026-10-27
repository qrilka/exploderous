use super::*;

#[test]
fn one_space_press_is_consumed_once_even_across_detonation_in_a_catch_up_batch() {
    let start = Instant::now();
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 1)
        .unwrap();
    let mut app = support::app_with_rules(start, 19, 17, rules);
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        app.handle_key(key(KeyCode::Char(' '), kind), start);
    }
    let report = app.advance(start + TICK_INTERVAL).unwrap();
    assert!(!report.ticks[0].place_bomb);
    assert!(app.game().bomb().is_none());
    let now = start + TICK_INTERVAL;
    app.handle_key(press(KeyCode::Right), now); // leave the Bomb's reach before detonation
    app.handle_key(press(KeyCode::Char(' ')), now);
    app.handle_key(release(KeyCode::Char(' ')), now); // releasing does not cancel a press
    let report = app.advance(start + TICK_INTERVAL * 6).unwrap();
    assert_eq!(
        report
            .ticks
            .iter()
            .map(|tick| tick.place_bomb)
            .collect::<Vec<_>>(),
        [true, false, false, false, false]
    );
    assert!(app.game().bomb().is_none());
    assert!(app.game().blasts().is_empty());
    app.handle_key(
        key(KeyCode::Char(' '), KeyEventKind::Repeat),
        start + TICK_INTERVAL * 6,
    );
    app.advance(start + TICK_INTERVAL * 7).unwrap();
    assert!(app.game().bomb().is_none());
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL * 7);
    app.advance(start + TICK_INTERVAL * 8).unwrap();
    assert_eq!(app.game().bomb().unwrap().detonate_at(), 10);
}

#[test]
fn failed_placement_is_consumed_not_deferred_until_capacity_is_free() {
    let start = Instant::now();
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 1)
        .unwrap();
    let mut app = support::app_with_rules(start, 19, 17, rules);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL);
    app.handle_key(press(KeyCode::Right), start + TICK_INTERVAL);
    let report = app.advance(start + TICK_INTERVAL * 6).unwrap();
    assert_eq!(
        report
            .ticks
            .iter()
            .map(|tick| tick.place_bomb)
            .collect::<Vec<_>>(),
        [true, false, false, false, false]
    );
    assert!(app.game().bomb().is_none());
    assert!(app.game().blasts().is_empty());
}

#[test]
fn space_places_a_bomb_after_movement_on_the_next_tick() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(app.game().character().position, point(144, 128));
    let bomb = app.game().bomb().expect("Space placed a Bomb");
    assert_eq!(bomb.cell().column().value(), 0);
    assert_eq!(bomb.cell().row().value(), 0);
    assert_eq!(bomb.detonate_at(), 121);
}

#[test]
fn the_most_recent_held_direction_wins_and_release_restores_the_previous_one() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Down));

    app.handle_key(release(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.advance(start + TICK_INTERVAL).unwrap().ticks.len(), 1);
    assert_eq!(app.game().character().position, point(144, 128));
}

#[test]
fn every_arrow_key_maps_to_its_cardinal_movement() {
    let start = Instant::now();
    for (code, expected) in [
        (KeyCode::Up, point(128, 112)),
        (KeyCode::Right, point(144, 128)),
        (KeyCode::Down, point(128, 144)),
        (KeyCode::Left, point(112, 128)),
    ] {
        let mut app = support::app(start, 19, 17);
        app.handle_key(press(code), start);
        app.advance(start + TICK_INTERVAL).unwrap();
        assert_eq!(app.game().character().position, expected, "key={code:?}");
    }
}

#[test]
fn a_release_stops_future_tick_records_after_a_short_direction_press() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Down), start);
    let first_tick = app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(
        first_tick.ticks,
        [TickRecord {
            tick: 1,
            direction: Some(Direction::Down),
            place_bomb: false,
            position: point(128, 144),
        }]
    );

    app.handle_key(release(KeyCode::Down), start + TICK_INTERVAL);
    let after_release = app.advance(start + TICK_INTERVAL * 20).unwrap();
    assert_eq!(after_release.ticks.len(), 5);
    assert_eq!(after_release.discarded_ticks, 14);
    assert!(
        after_release
            .ticks
            .iter()
            .all(|tick| { tick.direction.is_none() && tick.position == point(128, 144) })
    );
}

#[test]
fn repeat_events_do_not_change_held_precedence_or_trigger_actions() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Up, KeyEventKind::Repeat), start);
    app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert!(!app.quit_requested());

    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Char('r'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.game().tick(), 0);
}

#[test]
fn catch_up_processes_five_ticks_and_discards_the_remaining_backlog() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);

    let report = app.advance(start + TICK_INTERVAL * 10).unwrap();
    assert_eq!(report.ticks.len(), 5);
    assert_eq!(report.discarded_ticks, 5);
    assert_eq!(app.game().tick(), 5);
    assert_eq!(app.game().character().position, point(208, 128));
    assert_eq!(
        app.advance(start + TICK_INTERVAL * 10).unwrap().ticks.len(),
        0
    );
    assert_eq!(app.next_tick(), Some(start + TICK_INTERVAL * 11));
}

#[test]
fn unknown_startup_options_are_rejected_before_terminal_initialization() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_exploderous-tui"))
        .arg("--unknown")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("usage: exploderous-tui [--strict-movement]")
    );
}

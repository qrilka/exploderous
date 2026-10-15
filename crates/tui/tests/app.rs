use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_core::game::{Direction, GameRules, Position, WorldX, WorldY};
use exploderous_tui::{App, TICK_INTERVAL, TickRecord};

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

fn key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
    KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
}

fn press(code: KeyCode) -> KeyEvent {
    key(code, KeyEventKind::Press)
}

fn release(code: KeyCode) -> KeyEvent {
    key(code, KeyEventKind::Release)
}

#[test]
fn one_space_press_is_consumed_once_even_across_detonation_in_a_catch_up_batch() {
    let start = Instant::now();
    let rules = GameRules::default().with_bomb_parameters(2, 1, 1).unwrap();
    let mut app = App::new_with_rules(start, 19, 11, rules);
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        app.handle_key(key(KeyCode::Char(' '), kind), start);
    }
    let report = app.advance(start + TICK_INTERVAL).unwrap();
    assert!(!report.ticks[0].place_bomb);
    assert!(app.game().bomb().is_none());
    let now = start + TICK_INTERVAL;
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
    let rules = GameRules::default().with_bomb_parameters(2, 1, 1).unwrap();
    let mut app = App::new_with_rules(start, 19, 11, rules);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL);
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
fn every_pause_discards_placement_and_freezes_bomb_and_blast_deadlines() {
    let start = Instant::now();
    let rules = GameRules::default().with_bomb_parameters(2, 1, 4).unwrap();
    // Pause either with a live Bomb or a live Blast. Neither timers nor input leak.
    for initial_ticks in [1, 3] {
        for pause in 0..3 {
            let mut app = App::new_with_rules(start, 19, 11, rules);
            app.handle_key(press(KeyCode::Char(' ')), start);
            for tick in 1..=initial_ticks {
                app.advance(start + TICK_INTERVAL * tick).unwrap();
            }
            let snapshot = app.game().clone();
            let now = start + TICK_INTERVAL * initial_ticks;
            app.handle_key(press(KeyCode::Char(' ')), now);
            match pause {
                0 => app.handle_key(press(KeyCode::Char('h')), now),
                1 => app.set_focused(false, now),
                _ => app.set_terminal_size(18, 10, now),
            }
            app.handle_key(press(KeyCode::Char(' ')), now);
            let resumed = start + TICK_INTERVAL * 1000;
            assert!(app.advance(resumed).unwrap().ticks.is_empty());
            assert_eq!(app.game(), &snapshot);
            match pause {
                0 => app.handle_key(press(KeyCode::Esc), resumed),
                1 => app.set_focused(true, resumed),
                _ => app.set_terminal_size(19, 11, resumed),
            }
            assert!(app.advance(resumed).unwrap().ticks.is_empty());
            let report = app.advance(resumed + TICK_INTERVAL).unwrap();
            assert_eq!(report.ticks.len(), 1);
            assert!(!report.ticks[0].place_bomb);
            let mut expected = snapshot;
            expected.step(&[]).unwrap();
            assert_eq!(app.game(), &expected);
        }
    }
}

#[test]
fn reset_and_quit_discard_pending_actions_with_live_bombs() {
    let start = Instant::now();
    let rules = GameRules::default()
        .with_turn_assistance(false)
        .with_bomb_parameters(2, 2, 4)
        .unwrap();
    let mut app = App::new_with_rules(start, 19, 11, rules);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL);
    app.handle_key(press(KeyCode::Char('r')), start + TICK_INTERVAL * 100);
    let fresh = App::new_with_rules(start, 19, 11, rules);
    assert_eq!(app.game(), fresh.game());
    let report = app.advance(start + TICK_INTERVAL * 101).unwrap();
    assert_eq!(report.ticks.len(), 1);
    assert!(!report.ticks[0].place_bomb);
    assert!(app.game().bomb().is_none());
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL * 101);
    app.advance(start + TICK_INTERVAL * 102).unwrap();
    assert!(app.game().bomb().is_some());
    let snapshot = app.game().clone();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL * 102);
    app.handle_key(press(KeyCode::Char('q')), start + TICK_INTERVAL * 102);
    assert!(app.quit_requested());
    assert!(
        app.advance(start + TICK_INTERVAL * 1000)
            .unwrap()
            .ticks
            .is_empty()
    );
    assert_eq!(app.game(), &snapshot);
    assert_eq!(app.held_direction(), None);
}

#[test]
fn space_places_a_bomb_after_movement_on_the_next_tick() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(app.game().character().position, point(400, 384));
    let bomb = app.game().bomb().expect("Space placed a Bomb");
    assert_eq!(bomb.cell().column().value(), 1);
    assert_eq!(bomb.cell().row().value(), 1);
    assert_eq!(bomb.detonate_at(), 121);
}

#[test]
fn the_most_recent_held_direction_wins_and_release_restores_the_previous_one() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Down));

    app.handle_key(release(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.advance(start + TICK_INTERVAL).unwrap().ticks.len(), 1);
    assert_eq!(app.game().character().position, point(400, 384));
}

#[test]
fn every_arrow_key_maps_to_its_cardinal_movement() {
    let start = Instant::now();
    for (code, expected) in [
        (KeyCode::Up, point(384, 368)),
        (KeyCode::Right, point(400, 384)),
        (KeyCode::Down, point(384, 400)),
        (KeyCode::Left, point(368, 384)),
    ] {
        let mut app = App::new(start, 19, 11);
        app.handle_key(press(code), start);
        app.advance(start + TICK_INTERVAL).unwrap();
        assert_eq!(app.game().character().position, expected, "key={code:?}");
    }
}

#[test]
fn a_release_stops_future_tick_records_after_a_short_direction_press() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Down), start);
    let first_tick = app.advance(start + TICK_INTERVAL).unwrap();
    assert_eq!(
        first_tick.ticks,
        [TickRecord {
            tick: 1,
            direction: Some(Direction::Down),
            place_bomb: false,
            position: point(384, 400),
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
            .all(|tick| { tick.direction.is_none() && tick.position == point(384, 400) })
    );
}

#[test]
fn repeat_events_do_not_change_held_precedence_or_trigger_actions() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Up, KeyEventKind::Repeat), start);
    app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert!(!app.quit_requested());

    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Char('r'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.game().tick(), 0);
}

#[test]
fn reset_recreates_the_game_and_discards_held_input_and_tick_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    let reset_at = start + TICK_INTERVAL * 10;
    app.handle_key(press(KeyCode::Char('R')), reset_at);

    assert_eq!(app.game().tick(), 0);
    assert_eq!(app.game().character().position, point(384, 384));
    assert_eq!(app.held_direction(), None);
    assert_eq!(app.advance(reset_at).unwrap().ticks.len(), 0);
    assert_eq!(
        app.advance(reset_at + TICK_INTERVAL).unwrap().ticks.len(),
        1
    );
    assert_eq!(app.game().character().position, point(384, 384));
}

#[test]
fn catch_up_processes_five_ticks_and_discards_the_remaining_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);

    let report = app.advance(start + TICK_INTERVAL * 10).unwrap();
    assert_eq!(report.ticks.len(), 5);
    assert_eq!(report.discarded_ticks, 5);
    assert_eq!(app.game().tick(), 5);
    assert_eq!(app.game().character().position, point(464, 384));
    assert_eq!(
        app.advance(start + TICK_INTERVAL * 10).unwrap().ticks.len(),
        0
    );
    assert_eq!(app.next_tick(), Some(start + TICK_INTERVAL * 11));
}

#[test]
fn resize_pause_clears_input_and_resume_starts_without_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    let paused_at = start + TICK_INTERVAL;
    app.set_terminal_size(18, 10, paused_at);
    assert_eq!(app.held_direction(), None);
    assert_eq!(
        app.advance(start + TICK_INTERVAL * 100)
            .unwrap()
            .ticks
            .len(),
        0
    );

    let resumed_at = start + TICK_INTERVAL * 100;
    app.set_terminal_size(19, 11, resumed_at);
    assert_eq!(app.advance(resumed_at).unwrap().ticks.len(), 0);
    assert_eq!(
        app.advance(resumed_at + TICK_INTERVAL).unwrap().ticks.len(),
        1
    );
    assert_eq!(app.game().character().position, point(384, 384));
}

#[test]
fn help_pauses_movement_and_ignores_reset_and_quit_until_closed() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Char('?')), start);
    assert!(app.help_open());
    assert_eq!(app.held_direction(), None);
    app.handle_key(press(KeyCode::Char('r')), start);
    app.handle_key(press(KeyCode::Char('q')), start);
    assert_eq!(app.game().tick(), 0);
    assert!(!app.quit_requested());

    app.handle_key(press(KeyCode::Esc), start);
    assert!(!app.help_open());
    assert_eq!(app.advance(start).unwrap().ticks.len(), 0);
    assert_eq!(app.advance(start + TICK_INTERVAL).unwrap().ticks.len(), 1);
}

#[test]
fn focus_pause_clears_input_and_resumes_idle() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    app.handle_key(press(KeyCode::Right), start);
    app.set_focused(false, start + TICK_INTERVAL);
    assert_eq!(app.held_direction(), None);
    assert_eq!(
        app.advance(start + TICK_INTERVAL * 20).unwrap().ticks.len(),
        0
    );

    let resumed_at = start + TICK_INTERVAL * 20;
    app.set_focused(true, resumed_at);
    assert_eq!(app.advance(resumed_at).unwrap().ticks.len(), 0);
    assert_eq!(
        app.advance(resumed_at + TICK_INTERVAL).unwrap().ticks.len(),
        1
    );
    assert_eq!(app.game().character().position, point(384, 384));
}

#[test]
fn assisted_and_strict_rules_drive_turns_and_survive_reset() {
    let start = Instant::now();
    for (assisted, expected) in [(true, point(432, 416)), (false, point(416, 448))] {
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut app = App::new_with_rules(start, 19, 11, rules);
        app.handle_key(press(KeyCode::Down), start);
        for tick in 1..=4 {
            app.advance(start + TICK_INTERVAL * tick).unwrap();
        }
        let turn_at = start + TICK_INTERVAL * 4;
        app.handle_key(release(KeyCode::Down), turn_at);
        app.handle_key(press(KeyCode::Right), turn_at);
        for tick in 5..=9 {
            app.advance(start + TICK_INTERVAL * tick).unwrap();
        }
        assert_eq!(app.game().character().position, expected);
        app.handle_key(release(KeyCode::Right), start + TICK_INTERVAL * 9);
        app.advance(start + TICK_INTERVAL * 10).unwrap();
        assert_eq!(app.game().character().position, expected);
        app.handle_key(press(KeyCode::Char('r')), start + TICK_INTERVAL * 10);
        assert_eq!(app.game().character().position, point(384, 384));
        assert_eq!(app.game().rules(), &rules);
    }
}

#[test]
fn a_turn_past_a_cell_boundary_corrects_safely_and_release_stops_correction() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 11);
    let mut tick = 0;
    // Approach the upper pillar: the old lower route now has an authored Block.
    for (code, ticks) in [(KeyCode::Right, 30), (KeyCode::Down, 9)] {
        app.handle_key(press(code), start + TICK_INTERVAL * tick);
        for _ in 0..ticks {
            tick += 1;
            app.advance(start + TICK_INTERVAL * tick).unwrap();
        }
        app.handle_key(release(code), start + TICK_INTERVAL * tick);
    }
    assert_eq!(app.game().character().position, point(864, 528));
    app.handle_key(press(KeyCode::Left), start + TICK_INTERVAL * tick);
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(864, 512));
    app.handle_key(release(KeyCode::Left), start + TICK_INTERVAL * tick);
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(864, 512));
    app.handle_key(press(KeyCode::Left), start + TICK_INTERVAL * tick);
    for _ in 0..6 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    assert_eq!(app.game().character().position, point(864, 416));
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(848, 416));
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

#[test]
fn tick_interval_is_the_nominal_sixtieth_of_a_second() {
    assert_eq!(TICK_INTERVAL, Duration::from_nanos(16_666_667));
}

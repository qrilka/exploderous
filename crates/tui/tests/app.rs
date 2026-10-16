mod support;

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_core::game::{
    CharacterCommand, Direction, GameRules, Outcome, Position, WorldX, WorldY,
};
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
fn normal_level_has_thirteen_playable_cells_per_axis_and_a_protected_opening() {
    use exploderous_core::game::{CellCoord, Column, Row, Terrain};

    let app = App::new(Instant::now(), 19, 17);
    let arena = app.game().arena();
    assert_eq!((arena.width(), arena.height()), (13, 13));
    assert_eq!(arena.cells().count(), 169);
    assert_eq!(app.game().character().position, point(128, 128));
    for (cell, terrain) in arena.cells() {
        let pillar = cell.column().value() % 2 == 1 && cell.row().value() % 2 == 1;
        assert_eq!(terrain == Terrain::Wall, pillar, "cell={cell:?}");
    }
    for (column, row) in [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)] {
        assert_eq!(
            arena.terrain_at(CellCoord::new(Column::new(column), Row::new(row))),
            Some(Terrain::Floor)
        );
    }
    assert_eq!(
        arena.terrain_at(app.game().exit().expect("authored Exit is retained")),
        Some(Terrain::Block)
    );
}

#[test]
fn ordinary_input_can_traverse_the_full_perimeter_without_crossing_the_exterior_frame() {
    for (width, height) in [(19, 17), (80, 30)] {
        let mut now = Instant::now();
        let mut app = App::new(now, width, height);
        // 256 ticks exceeds the full thirteen-Cell extent. A ring of Walls, an
        // eleven-Cell interior, or centre-only bounds would stop at different positions.
        for (direction, expected) in [
            (KeyCode::Right, point(3232, 128)),
            (KeyCode::Down, point(3232, 3232)),
            (KeyCode::Left, point(96, 3232)),
            (KeyCode::Up, point(96, 96)),
        ] {
            support::travel(&mut app, &mut now, direction, 256);
            assert_eq!(app.game().character().position, expected);
            assert_eq!(app.game().outcome(), None);
        }
        assert_eq!(app.game().tick(), 1024);
    }
}

#[test]
fn reachable_outcomes_stop_catch_up_freeze_input_and_restart_cleanly() {
    let start = Instant::now();
    for (assisted, outcome) in [
        (true, Outcome::Defeat),
        (false, Outcome::Defeat),
        (true, Outcome::Victory),
        (false, Outcome::Victory),
    ] {
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut app = App::new_with_rules(start, 19, 17, rules);
        let mut now = start;
        let (direction, terminal_tick) = match outcome {
            Outcome::Defeat => {
                app.handle_key(press(KeyCode::Char(' ')), now);
                support::ticks(&mut app, &mut now, 119);
                (KeyCode::Right, 121)
            }
            Outcome::Victory => {
                support::reveal_exit(&mut app, &mut now);
                support::ticks(&mut app, &mut now, 30);
                support::travel(&mut app, &mut now, KeyCode::Left, 16);
                support::travel(&mut app, &mut now, KeyCode::Down, 22);
                (KeyCode::Down, 271)
            }
        };
        let mut continuation = app.game().clone();
        app.handle_key(press(direction), now);
        app.handle_key(press(KeyCode::Char(' ')), now);
        let ended_at = now + TICK_INTERVAL * 11;
        let report = app.advance(ended_at).unwrap();
        assert_eq!(report.ticks.len(), 2); // don't finish the batch after an outcome
        assert_eq!(report.discarded_ticks, 9);
        assert_eq!(app.game().outcome(), Some(outcome));
        assert_eq!(app.game().tick(), terminal_tick);
        for record in &report.ticks {
            continuation
                .step(&[CharacterCommand {
                    character: continuation.character().id,
                    movement: record.direction,
                    place_bomb: record.place_bomb,
                }])
                .unwrap();
        }
        assert_eq!(app.game(), &continuation); // whole-Game clone across the outcome
        for _ in 0..100 {
            continuation.step(&[]).unwrap();
        }
        assert_eq!(app.game(), &continuation); // timers, terrain and live Bomb all freeze
        assert_eq!(app.held_direction(), None);
        assert_eq!(app.next_tick(), None);
        let defeated = app.game().clone();
        for code in [
            KeyCode::Left,
            KeyCode::Down,
            KeyCode::Char(' '),
            KeyCode::Char('?'),
            KeyCode::Char('h'),
            KeyCode::Char('H'),
            KeyCode::Char('x'),
        ] {
            for kind in [
                KeyEventKind::Press,
                KeyEventKind::Repeat,
                KeyEventKind::Release,
            ] {
                app.handle_key(key(code, kind), ended_at);
            }
        }
        for code in [
            KeyCode::Char('r'),
            KeyCode::Char('R'),
            KeyCode::Char('q'),
            KeyCode::Char('Q'),
            KeyCode::Esc,
        ] {
            for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
                app.handle_key(key(code, kind), ended_at);
            }
        }
        assert!(!app.help_open());
        assert!(!app.quit_requested());
        app.set_focused(false, ended_at);
        app.set_terminal_size(19, 16, ended_at);
        let resumed = start + TICK_INTERVAL * 1000;
        app.set_focused(true, resumed);
        app.set_terminal_size(19, 17, resumed);
        assert_eq!(app.next_tick(), None);
        assert!(app.advance(resumed).unwrap().ticks.is_empty());
        assert_eq!(app.game(), &defeated);
        app.handle_key(press(KeyCode::Char('R')), resumed);
        assert_eq!(app.game(), App::new_with_rules(start, 19, 17, rules).game());
        assert!(app.advance(resumed).unwrap().ticks.is_empty());
        let report = app.advance(resumed + TICK_INTERVAL).unwrap();
        assert_eq!(report.ticks.len(), 1);
        assert!(!report.ticks[0].place_bomb);
        assert_eq!(report.ticks[0].direction, None);
        assert!(app.game().bomb().is_none());
        // Available capacity, no permission inherited from the previous run.
        app.handle_key(press(KeyCode::Char(' ')), resumed + TICK_INTERVAL);
        app.advance(resumed + TICK_INTERVAL * 2).unwrap();
        assert!(app.game().bomb().is_some());
        if outcome == Outcome::Victory {
            // A second successful run after restart, using the identical authored Level.
            let mut now = resumed + TICK_INTERVAL * 2;
            app.handle_key(press(KeyCode::Char('r')), now);
            support::reveal_exit(&mut app, &mut now);
            support::ticks(&mut app, &mut now, 30);
            support::travel(&mut app, &mut now, KeyCode::Left, 16);
            support::travel(&mut app, &mut now, KeyCode::Down, 24);
            assert_eq!(app.game().outcome(), Some(Outcome::Victory));
            assert_eq!(app.game().tick(), 271);
        }
    }
}

#[test]
fn the_reported_neighbouring_cell_escape_survives_but_remaining_in_danger_loses() {
    let start = Instant::now();
    // Original supported-terminal trace translated one Cell up/left: Bomb at
    // (4,1), player apparently beside its upper arm at (4,0).
    for (escape_ticks, expected_x, outcome) in [
        (8, 1264, Some(Outcome::Defeat)),
        (9, 1280, None),
        (11, 1312, None),
    ] {
        let mut app = App::new(start, 19, 17);
        let mut tick = 0;
        for (code, count) in [(KeyCode::Right, 63), (KeyCode::Down, 16)] {
            app.handle_key(press(code), start + TICK_INTERVAL * tick);
            for _ in 0..count {
                tick += 1;
                app.advance(start + TICK_INTERVAL * tick).unwrap();
            }
            app.handle_key(release(code), start + TICK_INTERVAL * tick);
        }
        app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL * tick);
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
        assert_eq!(app.game().bomb().unwrap().detonate_at(), 200);
        for (code, count) in [(KeyCode::Up, 18), (KeyCode::Right, escape_ticks)] {
            app.handle_key(press(code), start + TICK_INTERVAL * tick);
            for _ in 0..count {
                tick += 1;
                app.advance(start + TICK_INTERVAL * tick).unwrap();
            }
            app.handle_key(release(code), start + TICK_INTERVAL * tick);
        }
        assert_eq!(app.game().character().position, point(expected_x, 96));
        while tick < 200 {
            tick += 1;
            app.advance(start + TICK_INTERVAL * tick).unwrap();
        }
        assert_eq!(app.game().tick(), 200);
        assert_eq!(app.game().outcome(), outcome);
        // Blast Cell (4,0) ends at x=1280, exclusive. Centre 1264 is
        // still in danger; 1280 and the translated 1312 draw in safe (5,0),
        // despite positive-area body overlap with the Blast in both cases.
        assert_eq!(app.game().character().position, point(expected_x, 96));
        if outcome.is_none() {
            // Remain beside the existing danger for its entire lifetime,
            // not just safely outside its Cell on the creation tick.
            for _ in 0..30 {
                tick += 1;
                app.advance(start + TICK_INTERVAL * tick).unwrap();
                assert_eq!(app.game().outcome(), None);
                assert_eq!(app.game().character().position, point(expected_x, 96));
            }
            assert!(app.game().blasts().is_empty());
        }
    }
}

#[test]
fn outcome_quit_commands_act_only_on_press() {
    let start = Instant::now();
    let rules = GameRules::default().with_bomb_parameters(1, 1, 30).unwrap();
    for outcome in [Outcome::Defeat, Outcome::Victory] {
        for code in [KeyCode::Char('q'), KeyCode::Char('Q'), KeyCode::Esc] {
            let mut now = start;
            let mut app = if outcome == Outcome::Defeat {
                let mut app = App::new_with_rules(now, 19, 17, rules);
                app.handle_key(press(KeyCode::Char(' ')), now);
                support::ticks(&mut app, &mut now, 2);
                app
            } else {
                let mut app = App::new(now, 19, 17);
                support::reveal_exit(&mut app, &mut now);
                support::ticks(&mut app, &mut now, 30);
                support::travel(&mut app, &mut now, KeyCode::Left, 16);
                support::travel(&mut app, &mut now, KeyCode::Down, 24);
                app
            };
            assert_eq!(app.game().outcome(), Some(outcome));
            app.handle_key(key(code, KeyEventKind::Repeat), now);
            assert!(!app.quit_requested());
            app.handle_key(press(code), now);
            assert!(app.quit_requested());
        }
    }
}

#[test]
fn one_space_press_is_consumed_once_even_across_detonation_in_a_catch_up_batch() {
    let start = Instant::now();
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 1)
        .unwrap();
    let mut app = App::new_with_rules(start, 19, 17, rules);
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
    let mut app = App::new_with_rules(start, 19, 17, rules);
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
fn every_pause_discards_placement_and_freezes_bomb_and_blast_deadlines() {
    let start = Instant::now();
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 4)
        .unwrap();
    // Pause either with a live Bomb or a live Blast. Neither timers nor input leak.
    for initial_ticks in [1, 3] {
        for pause in 0..3 {
            let mut app = App::new_with_rules(start, 19, 17, rules);
            app.handle_key(press(KeyCode::Char(' ')), start);
            for tick in 1..=initial_ticks {
                if tick == 2 {
                    app.handle_key(press(KeyCode::Right), start + TICK_INTERVAL);
                }
                app.advance(start + TICK_INTERVAL * tick).unwrap();
            }
            assert_eq!(app.game().outcome(), None);
            let snapshot = app.game().clone();
            let now = start + TICK_INTERVAL * initial_ticks;
            app.handle_key(press(KeyCode::Char(' ')), now);
            match pause {
                0 => app.handle_key(press(KeyCode::Char('h')), now),
                1 => app.set_focused(false, now),
                _ => app.set_terminal_size(18, 17, now),
            }
            app.handle_key(press(KeyCode::Char(' ')), now);
            let resumed = start + TICK_INTERVAL * 1000;
            assert!(app.advance(resumed).unwrap().ticks.is_empty());
            assert_eq!(app.game(), &snapshot);
            match pause {
                0 => app.handle_key(press(KeyCode::Esc), resumed),
                1 => app.set_focused(true, resumed),
                _ => app.set_terminal_size(19, 17, resumed),
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
    let mut app = App::new_with_rules(start, 19, 17, rules);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL);
    app.handle_key(press(KeyCode::Char('r')), start + TICK_INTERVAL * 100);
    let fresh = App::new_with_rules(start, 19, 17, rules);
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
    let mut app = App::new(start, 19, 17);
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
fn ordinary_input_escapes_away_from_a_bomb_but_cannot_approach_or_reenter_it() {
    for assisted in [false, true] {
        let mut now = Instant::now();
        let mut app = App::new_with_rules(
            now,
            19,
            17,
            GameRules::default().with_turn_assistance(assisted),
        );
        support::travel(&mut app, &mut now, KeyCode::Right, 1);
        app.handle_key(press(KeyCode::Char(' ')), now);
        support::ticks(&mut app, &mut now, 1);
        assert_eq!(app.game().character().position, point(144, 128));
        assert_eq!(app.game().bomb().unwrap().cell().column().value(), 0);
        for (direction, count, expected) in [
            (KeyCode::Left, 1, point(144, 128)), // approach blocked, not a bad command
            (KeyCode::Down, 1, point(144, 144)), // perpendicular escape is legal
            (KeyCode::Up, 1, point(144, 144)),   // cannot undo that travel
            (KeyCode::Right, 12, point(336, 144)), // body still overlaps by 16
            (KeyCode::Left, 1, point(336, 144)),
            (KeyCode::Right, 1, point(352, 144)), // exact edge: fully clear
            (KeyCode::Left, 1, point(352, 144)),  // re-entry blocked
        ] {
            let tick = app.game().tick();
            support::travel(&mut app, &mut now, direction, count);
            assert_eq!(app.game().tick(), tick + u64::from(count));
            assert_eq!(app.game().character().position, expected);
            assert_eq!(app.game().outcome(), None);
        }
        assert_eq!(app.game().bomb().unwrap().detonate_at(), 122);
    }
}

#[test]
fn the_most_recent_held_direction_wins_and_release_restores_the_previous_one() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
        let mut app = App::new(start, 19, 17);
        app.handle_key(press(code), start);
        app.advance(start + TICK_INTERVAL).unwrap();
        assert_eq!(app.game().character().position, expected, "key={code:?}");
    }
}

#[test]
fn a_release_stops_future_tick_records_after_a_short_direction_press() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
    let mut app = App::new(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Up, KeyEventKind::Repeat), start);
    app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert!(!app.quit_requested());

    let mut app = App::new(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Char('r'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.game().tick(), 0);
}

#[test]
fn reset_recreates_the_game_and_discards_held_input_and_tick_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    let reset_at = start + TICK_INTERVAL * 10;
    app.handle_key(press(KeyCode::Char('R')), reset_at);

    assert_eq!(app.game().tick(), 0);
    assert_eq!(app.game().character().position, point(128, 128));
    assert_eq!(app.held_direction(), None);
    assert_eq!(app.advance(reset_at).unwrap().ticks.len(), 0);
    assert_eq!(
        app.advance(reset_at + TICK_INTERVAL).unwrap().ticks.len(),
        1
    );
    assert_eq!(app.game().character().position, point(128, 128));
}

#[test]
fn catch_up_processes_five_ticks_and_discards_the_remaining_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
fn resize_pause_clears_input_and_resume_starts_without_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
    app.handle_key(press(KeyCode::Right), start);
    let paused_at = start + TICK_INTERVAL;
    app.set_terminal_size(19, 16, paused_at);
    assert_eq!(app.held_direction(), None);
    assert_eq!(
        app.advance(start + TICK_INTERVAL * 100)
            .unwrap()
            .ticks
            .len(),
        0
    );

    let resumed_at = start + TICK_INTERVAL * 100;
    app.set_terminal_size(19, 17, resumed_at);
    assert_eq!(app.advance(resumed_at).unwrap().ticks.len(), 0);
    assert_eq!(
        app.advance(resumed_at + TICK_INTERVAL).unwrap().ticks.len(),
        1
    );
    assert_eq!(app.game().character().position, point(128, 128));
}

#[test]
fn help_pauses_movement_and_ignores_reset_and_quit_until_closed() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
    let mut app = App::new(start, 19, 17);
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
    assert_eq!(app.game().character().position, point(128, 128));
}

#[test]
fn assisted_and_strict_rules_drive_turns_and_survive_reset() {
    let start = Instant::now();
    for (assisted, expected) in [(true, point(176, 160)), (false, point(160, 192))] {
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut app = App::new_with_rules(start, 19, 17, rules);
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
        assert_eq!(app.game().character().position, point(128, 128));
        assert_eq!(app.game().rules(), &rules);
    }
}

#[test]
fn a_turn_past_a_cell_boundary_corrects_safely_and_release_stops_correction() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 17);
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
    assert_eq!(app.game().character().position, point(608, 272));
    app.handle_key(press(KeyCode::Left), start + TICK_INTERVAL * tick);
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(608, 256));
    app.handle_key(release(KeyCode::Left), start + TICK_INTERVAL * tick);
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(608, 256));
    app.handle_key(press(KeyCode::Left), start + TICK_INTERVAL * tick);
    for _ in 0..6 {
        tick += 1;
        app.advance(start + TICK_INTERVAL * tick).unwrap();
    }
    assert_eq!(app.game().character().position, point(608, 160));
    tick += 1;
    app.advance(start + TICK_INTERVAL * tick).unwrap();
    assert_eq!(app.game().character().position, point(592, 160));
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

#[test]
fn ordinary_input_reveals_collects_and_uses_fire_up_and_restart_restores_the_covered_item() {
    use exploderous_core::game::Terrain;
    let mut now = Instant::now();
    let mut app = App::new(now, 19, 17);
    let fresh = app.game().clone();
    let item = app.game().fire_up().expect("one authored Fire-up");
    assert_ne!(Some(item), app.game().exit());
    assert_eq!(app.game().arena().terrain_at(item), Some(Terrain::Block));
    support::reveal_fire_up(&mut app, &mut now);
    assert_eq!(app.game().tick(), 169);
    assert_eq!(app.game().outcome(), None);
    assert_eq!(app.game().arena().terrain_at(item), Some(Terrain::Floor));
    assert_eq!(app.game().fire_up(), Some(item));
    support::ticks(&mut app, &mut now, 30);
    support::travel(&mut app, &mut now, KeyCode::Down, 16);
    support::travel(&mut app, &mut now, KeyCode::Right, 32);
    assert_eq!(app.game().character().position, point(640, 640));
    assert_eq!(app.game().fire_up(), None);
    assert_eq!(app.game().current_blast_reach(), 2);
    app.handle_key(press(KeyCode::Char(' ')), now);
    support::ticks(&mut app, &mut now, 1);
    assert_eq!(app.game().bomb().unwrap().blast_reach(), 2);
    support::travel(&mut app, &mut now, KeyCode::Left, 32);
    support::travel(&mut app, &mut now, KeyCode::Up, 16);
    support::ticks(&mut app, &mut now, 72);
    assert_eq!(app.game().outcome(), None);
    assert!(
        app.game().blasts()[0]
            .cells()
            .iter()
            .any(|cell| cell.column().value() == 2 && cell.row().value() == 0)
    );
    // Reset while upgraded, with live effects, held input, pending placement and elapsed backlog.
    app.handle_key(press(KeyCode::Right), now);
    app.handle_key(press(KeyCode::Char(' ')), now);
    now += TICK_INTERVAL * 100;
    app.handle_key(press(KeyCode::Char('r')), now);
    assert_eq!(app.game(), &fresh);
    assert_eq!(app.held_direction(), None);
    assert!(app.advance(now).unwrap().ticks.is_empty());
    let report = app.advance(now + TICK_INTERVAL).unwrap();
    assert_eq!(report.ticks.len(), 1);
    assert_eq!(report.ticks[0].direction, None);
    assert!(!report.ticks[0].place_bomb);
    app.handle_key(press(KeyCode::Char(' ')), now + TICK_INTERVAL);
    app.advance(now + TICK_INTERVAL * 2).unwrap();
    assert_eq!(app.game().bomb().unwrap().blast_reach(), 1);
}

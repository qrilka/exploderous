use super::*;

#[test]
fn reachable_outcomes_stop_catch_up_freeze_input_and_restart_cleanly() {
    let start = Instant::now();
    for (assisted, outcome) in [
        (true, Outcome::Defeat),
        (false, Outcome::Defeat),
        (true, Outcome::Victory),
        (false, Outcome::Victory),
    ] {
        let rules = if outcome == Outcome::Victory {
            support::objective_rules(assisted)
        } else {
            GameRules::default().with_turn_assistance(assisted)
        };
        let mut app = support::app_with_rules(start, 19, 17, rules);
        let mut now = start;
        let (direction, terminal_tick) = match outcome {
            Outcome::Defeat => {
                app.handle_key(press(KeyCode::Char(' ')), now);
                support::ticks(&mut app, &mut now, 119);
                (KeyCode::Right, 121)
            }
            Outcome::Victory => {
                support::prepare_ready_exit(&mut app, &mut now);
                support::travel(&mut app, &mut now, KeyCode::Left, 2);
                (KeyCode::Down, app.game().tick() + 2)
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
        assert_eq!(
            app.game(),
            support::generated_app(start, 19, 17, rules).game()
        );
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
                let mut app = support::app_with_rules(now, 19, 17, rules);
                app.handle_key(press(KeyCode::Char(' ')), now);
                support::ticks(&mut app, &mut now, 2);
                app
            } else {
                let mut app = support::app_with_rules(now, 19, 17, support::objective_rules(true));
                support::prepare_ready_exit(&mut app, &mut now);
                support::travel(&mut app, &mut now, KeyCode::Left, 2);
                support::travel(&mut app, &mut now, KeyCode::Down, 2);
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
fn every_pause_discards_placement_and_freezes_bomb_and_blast_deadlines() {
    let start = Instant::now();
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 4)
        .unwrap();
    // Pause either with a live Bomb or a live Blast. Neither timers nor input leak.
    for initial_ticks in [1, 3] {
        for pause in 0..3 {
            let mut app = support::app_with_rules(start, 19, 17, rules);
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
    let mut app = support::app_with_rules(start, 19, 17, rules);
    app.handle_key(press(KeyCode::Char(' ')), start);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    app.handle_key(press(KeyCode::Char(' ')), start + TICK_INTERVAL);
    app.handle_key(press(KeyCode::Char('r')), start + TICK_INTERVAL * 100);
    let fresh = support::generated_app(start, 19, 17, rules);
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
fn reset_recreates_the_game_and_discards_held_input_and_tick_backlog() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
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
fn resize_pause_clears_input_and_resume_starts_without_backlog() {
    let start = Instant::now();
    let mut app = exploderous_tui::App::new_with_seed(start, 19, 17, GameRules::default(), 42);
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
    let mut app = support::app(start, 19, 17);
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
    let mut app = support::app(start, 19, 17);
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

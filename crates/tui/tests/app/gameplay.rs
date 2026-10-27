use super::*;

#[test]
fn normal_level_has_thirteen_playable_cells_per_axis_and_a_protected_opening() {
    use exploderous_core::game::{CellCoord, Column, Row, Terrain};

    let app = exploderous_tui::App::new_with_seed(Instant::now(), 19, 17, GameRules::default(), 42);
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
        arena.terrain_at(app.game().exit().expect("generated Scenario has an Exit")),
        Some(Terrain::Block)
    );
}

#[test]
fn authored_buroru_wander_on_idle_ticks_pause_completely_and_restart_generates() {
    let mut now = Instant::now();
    let mut app = support::app(now, 19, 17);
    let enemies = app.game().enemies().copied().collect::<Vec<_>>();
    assert_eq!(enemies.len(), 3);
    for (index, enemy) in enemies.iter().enumerate() {
        assert_ne!(enemy.id, app.game().character().id);
        assert!(enemies[..index].iter().all(|other| other.id != enemy.id));
    }
    let mut continuation = app.game().clone();
    support::ticks(&mut app, &mut now, 200);
    for _ in 0..200 {
        continuation.step(&[]).unwrap();
    }
    assert_eq!(app.game(), &continuation);
    assert_ne!(app.game().enemies().copied().collect::<Vec<_>>(), enemies);
    app.handle_key(press(KeyCode::Char('h')), now);
    let paused = app.game().clone();
    support::ticks(&mut app, &mut now, 100);
    assert_eq!(app.game(), &paused);
    app.handle_key(press(KeyCode::Char('h')), now);
    app.handle_key(press(KeyCode::Char('r')), now);
    assert_eq!(
        app.game(),
        support::generated_app(now, 19, 17, GameRules::default()).game()
    );
}

#[test]
fn a_revealed_locked_exit_is_traversable_and_clearing_enemies_away_from_it_does_not_win() {
    let mut now = Instant::now();
    let mut app = support::app(now, 19, 17);
    support::reveal_exit(&mut app, &mut now);
    support::ticks(&mut app, &mut now, 30);
    support::travel(&mut app, &mut now, KeyCode::Left, 16);
    support::travel(&mut app, &mut now, KeyCode::Down, 32);
    assert_eq!(app.game().character().position, point(1152, 640));
    assert!(!app.game().exit_ready());
    assert_eq!(app.game().outcome(), None);
    // Exercise the cleared objective independently with faster valid player/Bomb tuning.
    let mut app = support::app_with_rules(now, 19, 17, support::objective_rules(true));
    support::prepare_ready_exit(&mut app, &mut now);
    assert!(app.game().exit_ready());
    assert_eq!(app.game().outcome(), None);
    assert_eq!(app.game().current_blast_reach(), 13); // No Fire-up required.
    app.handle_key(press(KeyCode::Char('r')), now);
    assert_eq!(
        app.game(),
        support::generated_app(now, 19, 17, support::objective_rules(true)).game()
    );
    assert_eq!(app.game().enemies().len(), 3);
    assert!(app.game().bomb().is_none());
    assert!(app.game().blasts().is_empty());
}

#[test]
fn ordinary_input_can_traverse_the_full_perimeter_without_crossing_the_exterior_frame() {
    for (width, height) in [(19, 17), (80, 30)] {
        let mut now = Instant::now();
        let mut app = support::app_with_rules(now, width, height, support::objective_rules(true));
        support::prepare_ready_exit(&mut app, &mut now);
        support::travel(&mut app, &mut now, KeyCode::Up, 1);
        let initial_tick = app.game().tick().value();
        // 256 ticks exceeds the full thirteen-Cell extent. A ring of Walls, an
        // eleven-Cell interior, or centre-only bounds would stop at different positions.
        for (direction, expected) in [
            (KeyCode::Right, point(3232, 96)),
            (KeyCode::Down, point(3232, 3232)),
            (KeyCode::Left, point(96, 3232)),
            (KeyCode::Up, point(96, 96)),
        ] {
            support::travel(&mut app, &mut now, direction, 256);
            assert_eq!(app.game().character().position, expected);
            assert_eq!(app.game().outcome(), None);
        }
        assert_eq!(app.game().tick().value(), initial_tick + 1024);
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
        let mut app = support::app(start, 19, 17);
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
        assert_eq!(app.game().bomb().unwrap().detonate_at().value(), 200);
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
        assert_eq!(app.game().tick().value(), 200);
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
fn ordinary_input_can_reverse_while_overlapping_a_bomb_but_cannot_reenter_after_clearance() {
    for assisted in [false, true] {
        let mut now = Instant::now();
        let mut app = support::app_with_rules(
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
            (KeyCode::Left, 1, point(128, 128)),   // approach the Bomb centre
            (KeyCode::Down, 1, point(128, 144)),   // perpendicular travel is legal
            (KeyCode::Up, 1, point(128, 128)),     // reverse before clearance
            (KeyCode::Right, 13, point(336, 128)), // body still overlaps by 16
            (KeyCode::Left, 1, point(320, 128)),   // centre has left, permission has not
            (KeyCode::Right, 2, point(352, 128)),  // exact edge: fully clear
            (KeyCode::Left, 1, point(352, 128)),   // re-entry blocked
        ] {
            let tick = app.game().tick().value();
            support::travel(&mut app, &mut now, direction, count);
            assert_eq!(app.game().tick().value(), tick + u64::from(count));
            assert_eq!(app.game().character().position, expected);
            assert_eq!(app.game().outcome(), None);
        }
        assert_eq!(app.game().bomb().unwrap().detonate_at().value(), 122);
    }
}

#[test]
fn off_centre_dead_end_placement_allows_retreat_and_survival_through_the_blast() {
    for assisted in [false, true] {
        let mut now = Instant::now();
        let mut app = support::app_with_rules(
            now,
            19,
            17,
            GameRules::default().with_turn_assistance(assisted),
        );
        // Reach the reported offset (+32,+32) in Cell (4,1), at contact with
        // the Wall to the right and covered-Exit Block below, using only input.
        support::travel(&mut app, &mut now, KeyCode::Right, 66);
        support::travel(&mut app, &mut now, KeyCode::Down, 18);
        assert_eq!(app.game().character().position, point(1184, 416));
        app.handle_key(press(KeyCode::Char(' ')), now);
        support::ticks(&mut app, &mut now, 1);
        let bomb = app.game().bomb().unwrap();
        assert_eq!(
            app.game().arena().cell_center(bomb.cell()),
            Some(point(1152, 384))
        );
        assert_eq!(bomb.detonate_at().value(), 205);
        assert_eq!(app.game().character().position, point(1184, 416));
        // Waiting does not revoke permission. UP initially approaches the Bomb.
        support::ticks(&mut app, &mut now, 16);
        support::travel(&mut app, &mut now, KeyCode::Up, 16);
        assert_eq!(app.game().character().position, point(1184, 160));
        support::travel(&mut app, &mut now, KeyCode::Right, 16);
        assert_eq!(app.game().character().position, point(1440, 160));
        support::ticks(&mut app, &mut now, 72);
        assert_eq!(app.game().tick().value(), 205);
        assert!(app.game().bomb().is_none());
        assert_eq!(app.game().blasts().len(), 1);
        assert_eq!(app.game().outcome(), None);
        for _ in 0..30 {
            support::ticks(&mut app, &mut now, 1);
            assert_eq!(app.game().character().position, point(1440, 160));
            assert_eq!(app.game().outcome(), None);
        }
        assert_eq!(app.game().tick().value(), 235);
        assert!(app.game().blasts().is_empty());
    }
}

#[test]
fn assisted_and_strict_rules_drive_turns_and_survive_reset() {
    let start = Instant::now();
    for (assisted, expected) in [(true, point(176, 160)), (false, point(160, 192))] {
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut app = support::app_with_rules(start, 19, 17, rules);
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
    let mut app = support::app(start, 19, 17);
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
fn ordinary_input_reveals_collects_and_uses_fire_up_and_restart_restores_the_covered_item() {
    use exploderous_core::game::Terrain;
    let mut now = Instant::now();
    let mut app = support::app(now, 19, 17);
    let fresh = support::generated_app(now, 19, 17, GameRules::default())
        .game()
        .clone();
    let item = app.game().fire_up().expect("one authored Fire-up");
    assert_ne!(Some(item), app.game().exit());
    assert_eq!(app.game().arena().terrain_at(item), Some(Terrain::Block));
    support::reveal_fire_up(&mut app, &mut now);
    assert_eq!(app.game().tick().value(), 169);
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

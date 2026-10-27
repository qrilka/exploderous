use super::*;

#[test]
fn game_over_modal_overlays_the_arena_at_minimum_size_and_is_removed_on_restart() {
    let start = Instant::now();
    let mut app = support::app(start, 19, 17);
    let initial = screen(&draw(
        &support::generated_app(start, 19, 17, Default::default()),
        19,
        17,
    ));
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
fn the_exit_is_hidden_by_terrain_then_blasts_and_victory_has_a_distinct_modal() {
    let mut now = Instant::now();
    let mut app = support::app(now, 19, 17);
    support::reveal_exit(&mut app, &mut now);
    assert_eq!(app.game().outcome(), None);
    assert_eq!(app.game().tick().value(), 201);
    assert_eq!(draw(&app, 19, 17).backend().buffer()[(7, 3)].symbol(), "│");
    assert!(!screen(&draw(&app, 19, 17)).join("\n").contains('⬚'));
    support::ticks(&mut app, &mut now, 30);
    assert_eq!(draw(&app, 19, 17).backend().buffer()[(7, 3)].symbol(), "▧");
    let enemy_cells: Vec<_> = app
        .game()
        .enemies()
        .map(|enemy| app.game().arena().cell_containing(enemy.position).unwrap())
        .collect();
    for cell in enemy_cells {
        assert_eq!(
            draw(&app, 19, 17).backend().buffer()[(
                3 + cell.column().value() as u16,
                1 + cell.row().value() as u16
            )]
                .symbol(),
            "e"
        );
    }
    let rules = support::objective_rules(true);
    let mut app = support::app_with_rules(now, 19, 17, rules);
    support::prepare_ready_exit(&mut app, &mut now);
    assert_eq!(draw(&app, 19, 17).backend().buffer()[(7, 3)].symbol(), "⬚");
    for (width, height, left, top) in [(19, 17, 2, 0), (80, 30, 32, 6)] {
        let terminal = draw(&app, width, height);
        for row in [2, 4, 6] {
            assert_eq!(
                terminal.backend().buffer()[(left + 9, top + row + 1)].symbol(),
                " "
            );
        }
    }
    support::travel(&mut app, &mut now, KeyCode::Left, 2);
    support::travel(&mut app, &mut now, KeyCode::Down, 2);
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
        screen(&draw(&support::generated_app(now, 19, 17, rules), 19, 17))
    );
}

#[test]
fn small_scenario_outcomes_are_centered_without_bypassing_terminal_size_checks() {
    use exploderous_core::game::{
        CellCoord, CharacterId, Column, GameRules, HumanBomberStart, Outcome, Position, Row,
        Scenario, Terrain, WorldX, WorldY,
    };
    for arena_height in [1, 2] {
        for outcome in [Outcome::Defeat, Outcome::Victory] {
            let mut now = Instant::now();
            let rules = GameRules::default().with_bomb_parameters(1, 1, 30).unwrap();
            let mut scenario = Scenario::new(
                1,
                arena_height,
                vec![Terrain::Floor; arena_height as usize],
                HumanBomberStart {
                    id: CharacterId::new(0),
                    position: Position::new(WorldX::new(128), WorldY::new(128)),
                },
                rules,
            )
            .unwrap();
            if outcome == Outcome::Victory {
                scenario = scenario
                    .with_exit(CellCoord::new(Column::new(0), Row::new(0)))
                    .unwrap();
            }
            let mut app = App::from_scenario(now, 19, 16, scenario, 0);
            assert!(
                screen(&draw(&app, 19, 16))
                    .join("\n")
                    .contains("Resize terminal")
            );
            support::ticks(&mut app, &mut now, 10);
            assert_eq!(app.game().tick().value(), 0);
            app.set_terminal_size(19, 17, now);
            if outcome == Outcome::Defeat {
                app.handle_key(key(KeyCode::Char(' ')), now);
            }
            support::ticks(&mut app, &mut now, 2);
            assert_eq!(app.game().outcome(), Some(outcome));
            let title = match outcome {
                Outcome::Defeat => "│    GAME OVER    │",
                Outcome::Victory => "│    VICTORY!     │",
            };
            // Literal positions at both supported sizes, including even-height
            // framed Arenas where the central row is the lower of the two middles.
            let large_top = if arena_height == 1 { 11 } else { 12 };
            for (width, height, left, top) in [(19, 17, 0, 5), (80, 30, 30, large_top)] {
                let rows = screen(&draw(&app, width, height));
                for (line, expected) in [
                    "┌─────────────────┐",
                    title,
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
            }
        }
    }
}

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use exploderous_core::game::{Game, GameRules, Outcome, Scenario, Terrain};
use exploderous_tui::{App, TICK_INTERVAL};

#[test]
fn generated_startup_outcome_and_restarts_follow_session_random_state_not_game_state() {
    for assisted in [false, true] {
        let start = Instant::now();
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut expected_state = 42;
        let mut app = App::new_with_seed(start, 19, 17, rules, 42);
        assert_eq!(
            app.game(),
            &Game::new(
                Scenario::generate(rules, 40, &mut expected_state).unwrap(),
                0
            )
        );
        for run in 0..3 {
            let now = start + TICK_INTERVAL * (run * 1000);
            app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);
            for tick in 1..=120 {
                app.advance(now + TICK_INTERVAL * tick).unwrap();
            }
            let report = app.advance(now + TICK_INTERVAL * 131).unwrap();
            assert_eq!(report.ticks.len(), 1);
            assert_eq!(report.discarded_ticks, 10);
            assert_eq!(app.game().outcome(), Some(Outcome::Defeat));
            assert_eq!(app.game().tick(), 121);
            let frozen = app.game().clone();
            app.handle_key(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                now + TICK_INTERVAL * 132,
            );
            app.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                now + TICK_INTERVAL * 132,
            );
            assert!(
                app.advance(now + TICK_INTERVAL * 500)
                    .unwrap()
                    .ticks
                    .is_empty()
            );
            assert_eq!(app.game(), &frozen);
            let restarted = now + TICK_INTERVAL * 900;
            app.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                restarted,
            );
            // Compare each request to production generation with advancing random
            // state, not an assertion that every layout differs from the old one.
            assert_eq!(
                app.game(),
                &Game::new(
                    Scenario::generate(rules, 40, &mut expected_state).unwrap(),
                    0
                )
            );
            assert_eq!(app.game().enemies().len(), 3);
            assert_eq!(app.game().current_blast_reach(), 1);
            assert!(app.game().bomb().is_none());
            assert!(app.game().blasts().is_empty());
            assert_eq!(app.game().outcome(), None);
            for covered in [app.game().exit().unwrap(), app.game().fire_up().unwrap()] {
                assert_eq!(app.game().arena().terrain_at(covered), Some(Terrain::Block));
            }
            assert_eq!(app.held_direction(), None);
            assert!(app.advance(restarted).unwrap().ticks.is_empty());
            let report = app.advance(restarted + TICK_INTERVAL).unwrap();
            assert_eq!(report.ticks.len(), 1);
            assert_eq!(report.ticks[0].direction, None);
            assert!(!report.ticks[0].place_bomb);
            // Capacity is available in the new run, with only new escape permission.
            app.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                restarted + TICK_INTERVAL,
            );
            app.advance(restarted + TICK_INTERVAL * 2).unwrap();
            assert!(
                app.game()
                    .bomb()
                    .unwrap()
                    .escape_allowed_for(app.game().character().id)
            );
            // Restart during active play clears pending input and the elapsed backlog too.
            app.handle_key(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                restarted + TICK_INTERVAL * 2,
            );
            app.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                restarted + TICK_INTERVAL * 2,
            );
            app.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                now + TICK_INTERVAL * 1000,
            );
            assert_eq!(
                app.game(),
                &Game::new(
                    Scenario::generate(rules, 40, &mut expected_state).unwrap(),
                    0
                )
            );
        }
    }
}

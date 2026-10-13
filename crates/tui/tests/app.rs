use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_core::game::{Direction, Position};
use exploderous_tui::{App, TICK_INTERVAL};

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
fn the_most_recent_held_direction_wins_and_release_restores_the_previous_one() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(press(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Down));

    app.handle_key(release(KeyCode::Down), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.advance(start + TICK_INTERVAL).unwrap(), 1);
    assert_eq!(app.game().character().position, Position { x: 400, y: 384 });
}

#[test]
fn every_arrow_key_maps_to_its_cardinal_movement() {
    let start = Instant::now();
    for (code, expected) in [
        (KeyCode::Up, Position { x: 384, y: 368 }),
        (KeyCode::Right, Position { x: 400, y: 384 }),
        (KeyCode::Down, Position { x: 384, y: 400 }),
        (KeyCode::Left, Position { x: 368, y: 384 }),
    ] {
        let mut app = App::new(start, 19, 10);
        app.handle_key(press(code), start);
        app.advance(start + TICK_INTERVAL).unwrap();
        assert_eq!(app.game().character().position, expected, "key={code:?}");
    }
}

#[test]
fn repeat_events_do_not_change_held_precedence_or_trigger_actions() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Up, KeyEventKind::Repeat), start);
    app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert!(!app.quit_requested());

    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    app.handle_key(key(KeyCode::Char('r'), KeyEventKind::Repeat), start);
    assert_eq!(app.held_direction(), Some(Direction::Right));
    assert_eq!(app.game().tick(), 0);
}

#[test]
fn reset_recreates_the_game_and_discards_held_input_and_tick_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    app.advance(start + TICK_INTERVAL).unwrap();
    let reset_at = start + TICK_INTERVAL * 10;
    app.handle_key(press(KeyCode::Char('R')), reset_at);

    assert_eq!(app.game().tick(), 0);
    assert_eq!(app.game().character().position, Position { x: 384, y: 384 });
    assert_eq!(app.held_direction(), None);
    assert_eq!(app.advance(reset_at).unwrap(), 0);
    assert_eq!(app.advance(reset_at + TICK_INTERVAL).unwrap(), 1);
    assert_eq!(app.game().character().position, Position { x: 384, y: 384 });
}

#[test]
fn catch_up_processes_five_ticks_and_discards_the_remaining_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);

    assert_eq!(app.advance(start + TICK_INTERVAL * 10).unwrap(), 5);
    assert_eq!(app.game().tick(), 5);
    assert_eq!(app.game().character().position, Position { x: 464, y: 384 });
    assert_eq!(app.advance(start + TICK_INTERVAL * 10).unwrap(), 0);
    assert_eq!(app.next_tick(), Some(start + TICK_INTERVAL * 11));
}

#[test]
fn resize_pause_clears_input_and_resume_starts_without_backlog() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    let paused_at = start + TICK_INTERVAL;
    app.set_terminal_size(18, 10, paused_at);
    assert_eq!(app.held_direction(), None);
    assert_eq!(app.advance(start + TICK_INTERVAL * 100).unwrap(), 0);

    let resumed_at = start + TICK_INTERVAL * 100;
    app.set_terminal_size(19, 10, resumed_at);
    assert_eq!(app.advance(resumed_at).unwrap(), 0);
    assert_eq!(app.advance(resumed_at + TICK_INTERVAL).unwrap(), 1);
    assert_eq!(app.game().character().position, Position { x: 384, y: 384 });
}

#[test]
fn help_pauses_movement_and_ignores_reset_and_quit_until_closed() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
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
    assert_eq!(app.advance(start).unwrap(), 0);
    assert_eq!(app.advance(start + TICK_INTERVAL).unwrap(), 1);
}

#[test]
fn focus_pause_clears_input_and_resumes_idle() {
    let start = Instant::now();
    let mut app = App::new(start, 19, 10);
    app.handle_key(press(KeyCode::Right), start);
    app.set_focused(false, start + TICK_INTERVAL);
    assert_eq!(app.held_direction(), None);
    assert_eq!(app.advance(start + TICK_INTERVAL * 20).unwrap(), 0);

    let resumed_at = start + TICK_INTERVAL * 20;
    app.set_focused(true, resumed_at);
    assert_eq!(app.advance(resumed_at).unwrap(), 0);
    assert_eq!(app.advance(resumed_at + TICK_INTERVAL).unwrap(), 1);
    assert_eq!(app.game().character().position, Position { x: 384, y: 384 });
}

#[test]
fn tick_interval_is_the_nominal_sixtieth_of_a_second() {
    assert_eq!(TICK_INTERVAL, Duration::from_nanos(16_666_667));
}

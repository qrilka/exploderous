use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_tui::{App, TICK_INTERVAL};

pub fn ticks(app: &mut App, now: &mut Instant, count: u32) {
    for _ in 0..count {
        *now += TICK_INTERVAL;
        app.advance(*now).unwrap();
    }
}

pub fn travel(app: &mut App, now: &mut Instant, direction: KeyCode, count: u32) {
    app.handle_key(KeyEvent::new(direction, KeyModifiers::NONE), *now);
    ticks(app, now, count);
    app.handle_key(
        KeyEvent::new_with_kind(direction, KeyModifiers::NONE, KeyEventKind::Release),
        *now,
    );
}

// Actual six-Block Level, default movement and Bomb timing, no privileged state changes.
// Place at (5,2), escape through (5,1) to (6,1), wait for detonation on tick 201.
pub fn reveal_exit(app: &mut App, now: &mut Instant) {
    travel(app, now, KeyCode::Right, 64);
    travel(app, now, KeyCode::Down, 16);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), *now);
    ticks(app, now, 1);
    travel(app, now, KeyCode::Up, 16);
    travel(app, now, KeyCode::Right, 16);
    ticks(app, now, 88);
}

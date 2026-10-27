mod support;

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_core::game::{
    CharacterCommand, Direction, GameRules, Outcome, Position, WorldX, WorldY,
};
use exploderous_tui::{TICK_INTERVAL, TickRecord};

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

#[path = "app/gameplay.rs"]
mod gameplay;
#[path = "app/input.rs"]
mod input;
#[path = "app/lifecycle.rs"]
mod lifecycle;

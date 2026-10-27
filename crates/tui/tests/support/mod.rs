use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use exploderous_core::game::{
    BuroruStart, CellCoord, CharacterId, Column, GameRules, HumanBomberStart, Position, Row,
    Scenario, Terrain, WorldX, WorldY,
};
use exploderous_tui::{App, TICK_INTERVAL};

// Precise interaction fixture, not the generated level for Normal mode. Public
// construction and keyboard/time input keep these replays independent of generation.
pub fn app(now: Instant, width: u16, height: u16) -> App {
    app_with_rules(now, width, height, GameRules::default())
}

pub fn app_with_rules(now: Instant, width: u16, height: u16, rules: GameRules) -> App {
    let cell = |x, y| CellCoord::new(Column::new(x), Row::new(y));
    let blocks = [(2, 2), (4, 2), (6, 2), (2, 4), (6, 4), (4, 6)];
    let terrain = (0..13)
        .flat_map(|y| {
            (0..13).map(move |x| {
                if x % 2 == 1 && y % 2 == 1 {
                    Terrain::Wall
                } else if blocks.contains(&(x, y)) {
                    Terrain::Block
                } else {
                    Terrain::Floor
                }
            })
        })
        .collect();
    let scenario = Scenario::new(
        13,
        13,
        terrain,
        HumanBomberStart {
            id: CharacterId::new(0),
            position: Position::new(WorldX::new(128), WorldY::new(128)),
        },
        rules,
    )
    .unwrap()
    .with_enemies(
        [2, 4, 6]
            .into_iter()
            .enumerate()
            .map(|(index, row)| BuroruStart {
                id: CharacterId::new(index as u32 + 1),
                cell: cell(8, row),
            })
            .collect(),
    )
    .unwrap()
    .with_exit(cell(4, 2))
    .unwrap()
    .with_fire_up(cell(2, 2))
    .unwrap();
    App::from_scenario(now, width, height, scenario, 0)
}

pub fn generated_app(now: Instant, width: u16, height: u16, rules: GameRules) -> App {
    App::new_with_seed(now, width, height, rules, 0)
}

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

// Reveal the Fire-up at (2,2), from (1,2), escaping via (0,2) to (0,1).
pub fn reveal_fire_up(app: &mut App, now: &mut Instant) {
    travel(app, now, KeyCode::Down, 32);
    travel(app, now, KeyCode::Right, 16);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), *now);
    ticks(app, now, 1);
    travel(app, now, KeyCode::Left, 16);
    travel(app, now, KeyCode::Up, 16);
    ticks(app, now, 88);
}

// Outcome/UI fixture: one-Cell player steps and a short, long-reaching Bomb.
// Enemies still use ordinary eight-unit wandering. Kill before they leave column 8,
// then reveal the Exit. This avoids scripts that assume stationary enemies.
pub fn objective_rules(assisted: bool) -> exploderous_core::game::GameRules {
    exploderous_core::game::GameRules::new(192, 256)
        .unwrap()
        .with_turn_assistance(assisted)
        .with_bomb_parameters(4, 13, 2)
        .unwrap()
}

pub fn prepare_ready_exit(app: &mut App, now: &mut Instant) {
    travel(app, now, KeyCode::Right, 8);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), *now);
    ticks(app, now, 1);
    travel(app, now, KeyCode::Left, 2);
    travel(app, now, KeyCode::Down, 1);
    ticks(app, now, 3); // detonation and expiry
    assert!(app.game().enemies().is_empty());
    assert_eq!(app.game().outcome(), None);
    travel(app, now, KeyCode::Up, 1);
    travel(app, now, KeyCode::Left, 2);
    travel(app, now, KeyCode::Down, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), *now);
    ticks(app, now, 1);
    travel(app, now, KeyCode::Up, 1);
    travel(app, now, KeyCode::Right, 2);
    ticks(app, now, 3);
    assert!(app.game().exit_ready());
    assert_eq!(app.game().outcome(), None);
}

// Authored six-Block fixture, default movement and Bomb timing, no privileged state changes.
// Place at (4,1), escape through (4,0) to (5,0), wait for detonation on tick 201.
pub fn reveal_exit(app: &mut App, now: &mut Instant) {
    travel(app, now, KeyCode::Right, 64);
    travel(app, now, KeyCode::Down, 16);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), *now);
    ticks(app, now, 1);
    travel(app, now, KeyCode::Up, 16);
    travel(app, now, KeyCode::Right, 16);
    ticks(app, now, 88);
}

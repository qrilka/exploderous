// Each integration-test binary uses a different subset of these helpers.
#![allow(dead_code)]

use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, CharacterKind, Column, ControlSource,
    Direction, GameRules, Position, Row, Scenario, Terrain, WorldX, WorldY,
};

pub fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

pub fn cell(column: u32, row: u32) -> CellCoord {
    CellCoord::new(Column::new(column), Row::new(row))
}

pub fn bomber(position: Position) -> Character {
    Character {
        id: CharacterId::new(0),
        position,
        kind: CharacterKind::Bomber,
        control: ControlSource::Human,
    }
}

pub fn enemy(id: u32, position: Position) -> Character {
    Character {
        id: CharacterId::new(id),
        position,
        kind: CharacterKind::Buroru,
        control: ControlSource::Npc,
    }
}

pub fn action(movement: Option<Direction>, place_bomb: bool) -> CharacterCommand {
    CharacterCommand {
        character: CharacterId::new(0),
        movement,
        place_bomb,
    }
}

pub fn scenario(layout: &str, position: Position, rules: GameRules) -> Scenario {
    let rows: Vec<_> = layout.lines().collect();
    let width = rows.first().expect("empty terrain layout").len();
    assert!(rows.iter().all(|row| row.len() == width));
    let terrain = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|glyph| match glyph {
            '.' => Terrain::Floor,
            '#' => Terrain::Wall,
            '+' => Terrain::Block,
            _ => panic!("unknown terrain"),
        })
        .collect();
    Scenario::new(
        width as u32,
        rows.len() as u32,
        terrain,
        bomber(position),
        rules,
    )
    .unwrap()
}

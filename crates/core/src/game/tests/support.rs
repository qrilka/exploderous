use crate::game::{
    BuroruStart, CellCoord, CharacterCommand, CharacterId, Column, Direction, GameRules,
    HumanBomberStart, Position, Row, Scenario, Terrain, WorldX, WorldY,
};

pub fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

pub fn cell(column: u32, row: u32) -> CellCoord {
    CellCoord::new(Column::new(column), Row::new(row))
}

pub fn bomber(position: Position) -> HumanBomberStart {
    HumanBomberStart {
        id: CharacterId::new(0),
        position,
    }
}

pub fn enemy(id: u32, position: Position) -> BuroruStart {
    let x = position.x().value();
    let y = position.y().value();
    assert_eq!((x.rem_euclid(256), y.rem_euclid(256)), (128, 128));
    BuroruStart {
        id: CharacterId::new(id),
        cell: cell(
            u32::try_from(x / 256).unwrap(),
            u32::try_from(y / 256).unwrap(),
        ),
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

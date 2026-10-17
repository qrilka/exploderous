use exploderous_core::game::{
    Character, CharacterCommand, CharacterId, CharacterKind, ControlSource, Direction, Game,
    GameRules, Outcome, Position, Scenario, Terrain, WorldX, WorldY,
};

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

fn character(id: u32, x: i32, y: i32) -> Character {
    Character {
        id: CharacterId::new(id),
        position: point(x, y),
        kind: if id == 0 {
            CharacterKind::Bomber
        } else {
            CharacterKind::Buroru
        },
        control: if id == 0 {
            ControlSource::Human
        } else {
            ControlSource::Npc
        },
    }
}

fn action(movement: Option<Direction>, place_bomb: bool) -> CharacterCommand {
    CharacterCommand {
        character: CharacterId::new(0),
        movement,
        place_bomb,
    }
}

#[test]
fn moving_contact_requires_simultaneous_cells_not_the_union_of_visited_cells() {
    // Enemy moves Right along the top corridor. After 15 ticks its x is 248.
    // The tiny bomber then traverses x=257..1281: it leaves Cell 1 before the
    // enemy enters it at t=1. Both visit Cell 1, but never simultaneously.
    let mut terrain = vec![Terrain::Floor; 6 * 2];
    terrain[6..].fill(Terrain::Wall);
    let initial = Scenario::new(
        6,
        2,
        terrain,
        character(0, 257, 128),
        GameRules::new(2, 1024).unwrap(),
    )
    .unwrap()
    .with_enemies(vec![character(7, 128, 128)])
    .unwrap();
    let mut game = Game::new(initial, 0);
    for _ in 0..15 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.enemies()[0].position, point(248, 128));
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.enemies()[0].position, point(256, 128));
    assert_eq!(game.outcome(), None);

    // With no player movement that same enemy entry is lethal at the exact boundary.
    let initial = Scenario::new(
        6,
        1,
        vec![Terrain::Floor; 6],
        character(0, 384, 128),
        GameRules::default(),
    )
    .unwrap()
    .with_enemies(vec![character(7, 128, 128)])
    .unwrap();
    let mut game = Game::new(initial, 0);
    for _ in 0..16 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
    let frozen = game.clone();
    for _ in 0..130 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game, frozen);
}

#[test]
fn assisted_forward_contact_uses_the_time_spent_on_correction_first() {
    for (prefix, outcome) in [(15, None), (16, Some(Outcome::Defeat))] {
        let mut terrain = vec![Terrain::Floor; 8 * 4];
        terrain[4] = Terrain::Wall; // (4,0)
        terrain[8 + 3] = Terrain::Block; // Forces the bomber's downward correction.
        terrain[8 + 5] = Terrain::Wall; // Enemy can initially choose only Down.
        let initial = Scenario::new(
            8,
            4,
            terrain,
            character(0, 767, 497),
            GameRules::new(2, 1024).unwrap(),
        )
        .unwrap()
        .with_enemies(vec![character(7, 1152, 384)])
        .unwrap();
        let mut game = Game::new(initial, 0);
        for _ in 0..prefix {
            game.step(&[]).unwrap();
        }
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
        // (767,497) -> (767,513) -> (1775,513). Cell (4,2) is visited
        // before t=0.52. For prefix=15 the enemy enters that Cell only at t=1.
        assert_eq!(game.character().position, point(1775, 513));
        assert_eq!(game.outcome(), outcome, "prefix={prefix}");
    }
}

#[test]
fn empty_input_advances_a_persistent_half_speed_centred_run() {
    // A one-way start forces Right; the distant player is isolated below the corridor.
    let mut terrain = vec![Terrain::Floor; 13 * 3];
    terrain[13..26].fill(Terrain::Wall);
    let scenario = Scenario::new(13, 3, terrain, character(0, 128, 640), GameRules::default())
        .unwrap()
        .with_enemies(vec![character(7, 128, 128)])
        .unwrap();
    let mut game = Game::new(scenario, 0);
    for tick in 1..=32 {
        game.step(&[]).unwrap();
        assert_eq!(game.enemies()[0].position, point(128 + tick * 8, 128));
        assert_eq!(game.character().position, point(128, 640));
        assert_eq!(game.outcome(), None);
    }
}

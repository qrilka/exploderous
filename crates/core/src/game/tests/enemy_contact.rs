use super::support::{action, bomber, cell, enemy, point};
use crate::game::{Direction, Game, GameRules, Outcome, Scenario, Terrain};

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
        bomber(point(257, 128)),
        GameRules::new(2, 1024).unwrap(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(128, 128))])
    .unwrap();
    let mut game = Game::new(initial, 0);
    for _ in 0..15 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.enemies().next().unwrap().position, point(248, 128));
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.enemies().next().unwrap().position, point(256, 128));
    assert_eq!(game.outcome(), None);

    // With no player movement that same enemy entry is lethal at the exact boundary.
    let initial = Scenario::new(
        6,
        1,
        vec![Terrain::Floor; 6],
        bomber(point(384, 128)),
        GameRules::default(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(128, 128))])
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
fn passing_a_buroru_between_tick_endpoints_causes_defeat() {
    let initial = Scenario::new(
        4,
        1,
        vec![
            Terrain::Floor,
            Terrain::Floor,
            Terrain::Floor,
            Terrain::Wall,
        ],
        bomber(point(511, 128)),
        GameRules::default(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(640, 128))])
    .unwrap();
    let mut game = Game::new(initial, 0);
    // The Wall forces Left. Reach the Cell boundary through ordinary movement.
    for _ in 0..16 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.character().position, point(511, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(512, 128));
    assert_eq!(game.outcome(), None);

    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    // Their endpoint Cells differ, but both centres occupy Cell 1 for 0 < t < 1/16.
    assert_eq!(game.character().position, point(527, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(504, 128));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
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
            bomber(point(767, 497)),
            GameRules::new(2, 1024).unwrap(),
        )
        .unwrap()
        .with_enemies(vec![enemy(7, point(1152, 384))])
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
fn late_contact_between_boundary_instants_causes_defeat() {
    let initial = Scenario::new(
        6,
        1,
        vec![Terrain::Floor; 6],
        bomber(point(1024, 128)),
        GameRules::new(2, 1024).unwrap(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(128, 128))])
    .unwrap();
    let mut game = Game::new(initial, 0);
    // The arena edge forces Right. Reach x=248 through ordinary movement.
    for _ in 0..15 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.character().position, point(1024, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(248, 128));
    assert_eq!(game.outcome(), None);

    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    // At t=3/4 the player is x=256 (Cell 1); at t=1 the enemy enters
    // Cell 1. Both centres occupy Cell 0 only for 3/4 < t < 1.
    assert_eq!(game.character().position, point(1, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(256, 128));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn contact_around_an_assisted_bend_uses_only_the_actual_path() {
    for (blocked, position, outcome) in [
        (true, point(511, 1432), Some(Outcome::Defeat)),
        (false, point(600, 1521), None),
    ] {
        let mut terrain = vec![Terrain::Floor; 4 * 6];
        if blocked {
            terrain[2 * 4 + 2] = Terrain::Block;
        }
        let initial = Scenario::new(
            4,
            6,
            terrain,
            bomber(point(600, 497)),
            GameRules::new(2, 1024).unwrap(),
        )
        .unwrap()
        .with_enemies(vec![enemy(7, point(384, 384))])
        .unwrap();
        let mut game = Game::new(initial, 0);
        game.step(&[action(Some(Direction::Down), false)]).unwrap();
        // The Block requires 89 Left, then 935 Down. Cell (1,1) is shared
        // only for 88/1024 < t < 104/1024: x=512 and y=512 exclude its edges.
        // Without the Block the actual path is straight Down, never through it.
        assert_eq!(game.character().position, position, "blocked={blocked}");
        // Any initial enemy heading stays in the same Cell for this tick.
        assert_eq!(
            game.arena()
                .cell_containing(game.enemies().next().unwrap().position),
            Some(cell(1, 1)),
            "blocked={blocked}"
        );
        assert_eq!(game.outcome(), outcome, "blocked={blocked}");
    }
}

#[test]
fn early_contact_before_another_cell_crossing_causes_defeat() {
    let initial = Scenario::new(
        4,
        1,
        vec![Terrain::Floor; 4],
        bomber(point(320, 128)),
        GameRules::new(192, 512).unwrap(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(896, 128))])
    .unwrap();
    let mut game = Game::new(initial, 0);
    // The right arena edge forces Left; the clear corridor preserves that heading.
    for _ in 0..48 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.character().position, point(320, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(512, 128));
    assert_eq!(game.outcome(), None);

    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    // Both centres occupy Cell 1 only for 0 < t < 3/8. The player then
    // enters Cell 2 at t=3/8 and Cell 3 at t=7/8; later travel cannot undo harm.
    assert_eq!(game.character().position, point(832, 128));
    assert_eq!(game.enemies().next().unwrap().position, point(504, 128));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

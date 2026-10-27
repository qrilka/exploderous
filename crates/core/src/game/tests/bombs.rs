use super::support::{action, bomber, cell, point, scenario};
use crate::game::{
    CharacterCommand, CharacterId, ConstructionError, Direction, Game, GameRules, Outcome,
    Position, Scenario, StepError, Terrain,
};

mod movement;
mod propagation;

fn game(layout: &str, position: Position, rules: GameRules, tick: u64) -> Game {
    Game::new(
        scenario(layout, position, rules),
        crate::game::Tick::new(tick),
    )
}

mod timers;

#[test]
fn a_new_blast_kills_in_the_final_centre_cell_and_freezes_the_complete_run() {
    let rules = GameRules::default().with_bomb_parameters(1, 1, 30).unwrap();
    let mut game = game(".+.", point(128, 128), rules, 0);
    game.step(&[action(None, true)]).unwrap();
    let mut sibling = game.clone();
    game.step(&[]).unwrap();
    sibling.step(&[]).unwrap();
    assert_eq!(game, sibling);
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
    assert_eq!(game.tick().value(), 2);
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Floor));
    assert_eq!(game.blasts().len(), 1);
    let defeated = game.clone();
    for _ in 0..100 {
        game.step(&[action(Some(Direction::Right), true)]).unwrap();
        sibling.step(&[]).unwrap();
        assert_eq!(game, defeated);
        assert_eq!(sibling, defeated);
    }
}

#[test]
fn new_blast_damage_uses_the_final_centre_cell_with_half_open_boundaries() {
    // The vertical Blast spans y=256..1024. The body may overlap safely;
    // centre membership includes y=256 but excludes y=1024.
    for (direction, distance, expected_y, expected_row, outcome) in [
        (Direction::Up, 385, 255, 0, None),
        (Direction::Up, 384, 256, 1, Some(Outcome::Defeat)),
        (Direction::Up, 383, 257, 1, Some(Outcome::Defeat)),
        (Direction::Down, 383, 1023, 3, Some(Outcome::Defeat)),
        (Direction::Down, 384, 1024, 4, None),
    ] {
        let rules = GameRules::new(192, distance)
            .unwrap()
            .with_bomb_parameters(1, 1, 30)
            .unwrap();
        let mut game = game(
            ".....\n.....\n.....\n.....\n.....",
            point(640, 640),
            rules,
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(direction), false)]).unwrap();
        assert_eq!(game.character().position, point(640, expected_y));
        assert_eq!(
            game.arena().cell_containing(game.character().position),
            Some(cell(2, expected_row))
        );
        assert_eq!(game.outcome(), outcome);
    }
}

#[test]
fn existing_blasts_kill_a_crossing_with_a_safe_endpoint_but_expire_before_movement() {
    for (lifetime, outcome) in [(30, Some(Outcome::Defeat)), (1, None)] {
        let rules = GameRules::new(192, 1024)
            .unwrap()
            .with_bomb_parameters(1, 1, lifetime)
            .unwrap();
        let mut game = game(
            ".....\n.....\n.....\n.....\n.....\n.....",
            point(640, 640),
            rules,
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(Direction::Up), false)]).unwrap();
        assert_eq!(game.character().position, point(640, 96));
        assert_eq!(game.outcome(), None); // clear before detonation
        let mut sibling = game.clone();
        game.step(&[action(Some(Direction::Down), true)]).unwrap();
        sibling
            .step(&[action(Some(Direction::Down), true)])
            .unwrap();
        assert_eq!(game, sibling);
        // Blast ends at y=1024; the endpoint's top is exactly 1024: safe.
        assert_eq!(game.character().position, point(640, 1120));
        assert_eq!(game.outcome(), outcome);
        assert_eq!(game.blasts().is_empty(), lifetime == 1);
        assert!(game.bomb().is_some()); // placement precedes harm, even on the losing tick
        if outcome.is_some() {
            let defeated = game.clone();
            for _ in 0..100 {
                game.step(&[action(Some(Direction::Up), true)]).unwrap();
                assert_eq!(game, defeated); // live Bomb, permission and effects all freeze
            }
        }
    }
}

#[test]
fn existing_blast_centre_paths_include_lower_edges_but_exclude_upper_edges_in_both_axes() {
    let rules = GameRules::new(192, 128)
        .unwrap()
        .with_bomb_parameters(4, 1, 30)
        .unwrap();
    for (escape, returning, lower_edge) in [
        (Direction::Up, Direction::Down, true),
        (Direction::Left, Direction::Right, true),
        (Direction::Down, Direction::Up, false),
        (Direction::Right, Direction::Left, false),
    ] {
        let mut game = game(
            ".....\n.....\n.....\n.....\n.....",
            point(640, 640),
            rules,
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        for _ in 0..4 {
            game.step(&[action(Some(escape), false)]).unwrap();
        }
        assert_eq!(game.outcome(), None); // centre clear before the tick-5 explosion
        // Each Blast arm spans 256..1024 on its movement axis. Moving back
        // one step ends exactly at 256 or 1024. Only the lower edge is inside.
        game.step(&[action(Some(returning), false)]).unwrap();
        assert_eq!(
            game.outcome(),
            if lower_edge {
                Some(Outcome::Defeat)
            } else {
                None
            }
        );
        if !lower_edge {
            let at_edge = game.character().position;
            game.step(&[]).unwrap();
            assert_eq!(game.character().position, at_edge);
            assert_eq!(game.outcome(), None); // stationary body overlap is safe
            game.step(&[action(Some(returning), false)]).unwrap();
            assert_eq!(game.outcome(), Some(Outcome::Defeat));
        }
    }
}

#[test]
fn a_diagonal_centre_cell_is_safe_despite_nearby_blast_arms() {
    let rules = GameRules::new(192, 224)
        .unwrap()
        .with_bomb_parameters(2, 1, 30)
        .unwrap();
    let mut game = game("...\n...\n...", point(384, 384), rules, 0);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[action(Some(Direction::Up), false)]).unwrap();
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(160, 160));
    // Centre Cell (0,0) is diagonal to the origin, outside both adjacent arms.
    assert_eq!(game.outcome(), None);
    assert_eq!(game.blasts().len(), 1);
}

#[test]
fn assistance_checks_its_correction_before_forward_travel_not_the_endpoint_chord() {
    let rules = GameRules::new(2, 64)
        .unwrap()
        .with_bomb_parameters(7, 1, 30)
        .unwrap();
    let mut game = game(".....\n.#...\n.....", point(833, 142), rules, 0);
    game.step(&[action(None, true)]).unwrap(); // Bomb at (3,0)
    for _ in 0..5 {
        game.step(&[action(Some(Direction::Left), false)]).unwrap();
    }
    for _ in 0..2 {
        game.step(&[action(Some(Direction::Down), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(513, 270));
    assert_eq!(game.outcome(), None);
    // The Blast includes (2,0): [512,768] x [0,256], not (2,1).
    // Left is blocked by (1,1); correction goes UP 15, then LEFT 49:
    // (513,270) -> (513,255) -> (464,255). The bend's centre enters danger.
    // Both tick endpoints are safe. The endpoint chord reaches y<256 only
    // after x<468, already left of danger, so a diagonal check misses it.
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(464, 255));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn walls_blocks_and_arena_edges_bound_even_a_very_large_reach() {
    let rules = GameRules::default()
        .with_bomb_parameters(1, u32::MAX, 1)
        .unwrap();
    let mut game = game(".#.\n+.+\n.+.", point(384, 384), rules, 0);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[]).unwrap();
    assert_eq!(
        game.blasts()[0].cells().collect::<Vec<_>>(),
        [cell(1, 1), cell(2, 1), cell(1, 2), cell(0, 1)]
    );
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Wall));
    for location in [cell(2, 1), cell(1, 2), cell(0, 1)] {
        assert_eq!(game.arena().terrain_at(location), Some(Terrain::Floor));
    }
    let mut corner = Game::new(
        Scenario::new(1, 1, vec![Terrain::Floor], bomber(point(128, 128)), rules).unwrap(),
        crate::game::Tick::new(0),
    );
    corner.step(&[action(None, true)]).unwrap();
    corner.step(&[]).unwrap();
    assert_eq!(corner.blasts()[0].cells().collect::<Vec<_>>(), [cell(0, 0)]);
}

#[test]
fn clones_replay_bombs_permissions_terrain_and_effects_independently() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(3, 1, 2)
        .unwrap();
    let mut game = game(
        ".....\n.....\n...+.\n.....\n.....",
        point(640, 640),
        rules,
        0,
    );
    game.step(&[action(None, true)]).unwrap();
    let initial_clone = game.clone();
    let mut sibling = initial_clone.clone();
    for frame in [
        vec![action(Some(Direction::Up), false)],
        vec![action(Some(Direction::Right), true)],
        vec![],
        vec![],
        vec![action(Some(Direction::Down), true)],
    ] {
        game.step(&frame).unwrap();
        sibling.step(&frame).unwrap();
        assert_eq!(game, sibling);
    }
    assert!(initial_clone.bomb().is_some());
    assert!(initial_clone.blasts().is_empty());
    assert_eq!(
        initial_clone.arena().terrain_at(cell(3, 2)),
        Some(Terrain::Block)
    );
    assert_eq!(game.arena().terrain_at(cell(3, 2)), Some(Terrain::Floor));
    assert!(game.bomb().is_some());
    let mut branch = game.clone();
    branch
        .step(&[action(Some(Direction::Down), false)])
        .unwrap(); // the later Bomb grants its own escape
    assert_ne!(branch.character().position, game.character().position);
    assert_eq!(game, sibling);
    assert_ne!(game, initial_clone);
}

#[test]
fn lingering_blasts_trigger_new_bombs_without_expanding_old_coverage() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(2, 2, 4)
        .unwrap();
    let mut game = game(".+...\n.....\n.....", point(128, 128), rules, 0);
    for command in [
        action(None, true),
        action(Some(Direction::Down), false),
        action(Some(Direction::Right), false),
    ] {
        game.step(&[command]).unwrap();
    }
    assert_eq!(game.outcome(), None);
    assert!(game.bomb().is_none());
    let old = game.blasts()[0].clone();
    assert_eq!(
        old.cells().collect::<Vec<_>>(),
        [cell(0, 0), cell(1, 0), cell(0, 1), cell(0, 2)]
    );
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Floor));
    game.step(&[action(Some(Direction::Up), true)]).unwrap();
    assert!(game.bomb().is_none()); // old Blast triggers the newly placed Bomb
    assert_eq!(game.blasts().len(), 2);
    assert_eq!(game.blasts()[0], old); // destroyed Block never extends old coverage
    assert!(game.blasts()[1].contains(cell(3, 0)));
    assert_eq!(game.blasts()[1].expires_at().value(), 8);
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn overlapping_effects_expire_independently_while_the_player_survives() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 4)
        .unwrap();
    let mut game = game(
        ".....\n.....\n.....\n.....\n.....",
        point(640, 640),
        rules,
        0,
    );
    for command in [
        action(None, true),
        action(Some(Direction::Up), false),
        action(None, true),
        action(Some(Direction::Right), false),
    ] {
        game.step(&[command]).unwrap();
    }
    assert_eq!(game.blasts().len(), 2);
    assert!(game.blasts().iter().all(|blast| blast.contains(cell(2, 1))));
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert_eq!(game.tick().value(), 6);
    assert_eq!(game.blasts().len(), 1);
    assert_eq!(game.blasts()[0].expires_at().value(), 8);
    assert!(game.blasts()[0].contains(cell(2, 1)));
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.outcome(), None);
    game.step(&[action(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at().value(), 10);
}

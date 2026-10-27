use super::support::{action, bomber, cell, point};
use crate::game::{
    ConstructionError, Direction, Game, GameRules, Outcome, Position, Scenario, Terrain,
};

fn scenario(terrain: Vec<Terrain>, position: Position, rules: GameRules) -> Scenario {
    Scenario::new(5, 3, terrain, bomber(position), rules).unwrap()
}

#[test]
fn exit_construction_accepts_floor_or_block_but_rejects_walls_and_out_of_bounds() {
    let mut terrain = vec![Terrain::Floor; 15];
    terrain[1] = Terrain::Wall;
    terrain[2] = Terrain::Block;
    let scenario = scenario(terrain, point(128, 384), GameRules::default());
    assert_eq!(
        Game::new(scenario.clone(), crate::game::Tick::new(0)).exit(),
        None
    );
    for exit in [cell(0, 0), cell(2, 0)] {
        let authored = scenario.clone().with_exit(exit).unwrap();
        assert_eq!(
            Game::new(authored, crate::game::Tick::new(0)).exit(),
            Some(exit)
        );
    }
    for exit in [cell(1, 0), cell(5, 0), cell(0, 3), cell(u32::MAX, u32::MAX)] {
        assert_eq!(
            scenario.clone().with_exit(exit),
            Err(ConstructionError::InvalidExitLocation)
        );
    }
}

#[test]
fn victory_requires_final_centre_entry_not_footprint_overlap_or_a_missing_objective() {
    for (start_x, direction, steps_to_win) in [
        (240, Direction::Right, 1), // body already overlaps; x=256 enters the lower edge
        (528, Direction::Left, 2),  // x=512 is still outside the upper edge
    ] {
        let initial = scenario(
            vec![Terrain::Floor; 15],
            point(start_x, 384),
            GameRules::default(),
        );
        let mut no_objective = Game::new(initial.clone(), crate::game::Tick::new(0));
        let initial = initial.with_exit(cell(1, 1)).unwrap();
        let mut game = Game::new(initial.clone(), crate::game::Tick::new(0));
        let fresh = game.clone();
        game.step(&[]).unwrap();
        assert_eq!(game.outcome(), None); // overlap without centre entry cannot win
        for step in 1..=steps_to_win {
            game.step(&[action(Some(direction), step == steps_to_win)])
                .unwrap();
            no_objective
                .step(&[action(Some(direction), false)])
                .unwrap();
            assert_eq!(no_objective.outcome(), None);
            assert_eq!(
                game.outcome(),
                (step == steps_to_win).then_some(Outcome::Victory)
            );
        }
        let won = game.clone();
        let mut sibling = won.clone();
        for _ in 0..40 {
            game.step(&[action(Some(Direction::Down), true)]).unwrap();
            sibling.step(&[]).unwrap();
        }
        assert_eq!(game, won);
        assert_eq!(sibling, won);
        assert!(game.bomb().is_some()); // even this live fuse is frozen
        assert_eq!(Game::new(initial, crate::game::Tick::new(0)), fresh);
    }
}

#[test]
fn a_covered_exit_is_blocking_and_survives_reveal_and_later_blasts() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 3)
        .unwrap();
    let mut terrain = vec![Terrain::Floor; 15];
    terrain[8] = Terrain::Block; // (3,1), beside the Bomb at (2,1)
    let initial = scenario(terrain, point(640, 384), rules)
        .with_exit(cell(3, 1))
        .unwrap();
    let mut game = Game::new(initial.clone(), crate::game::Tick::new(7));
    let fresh = game.clone();
    let mut blocked = game.clone();
    blocked
        .step(&[action(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(blocked.character().position, point(672, 384));
    assert_eq!(blocked.outcome(), None);
    assert_eq!(blocked.arena().terrain_at(cell(3, 1)), Some(Terrain::Block));

    for _ in 0..2 {
        game.step(&[action(None, true)]).unwrap();
        let mut sibling = game.clone(); // continuation across reveal, with permission and live fuse
        for run in [&mut game, &mut sibling] {
            run.step(&[action(Some(Direction::Left), false)]).unwrap();
            assert_eq!(run.outcome(), None);
            assert_eq!(run.exit(), Some(cell(3, 1)));
            assert_eq!(run.arena().terrain_at(cell(3, 1)), Some(Terrain::Floor));
            assert!(run.blasts()[0].contains(cell(3, 1)));
            for _ in 0..3 {
                run.step(&[]).unwrap();
            }
            assert!(run.blasts().is_empty());
            assert_eq!(run.exit(), Some(cell(3, 1)));
        }
        assert_eq!(game, sibling);
        sibling
            .step(&[action(Some(Direction::Down), false)])
            .unwrap();
        assert_ne!(game, sibling); // independent continuation, no shared mutable state
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
    }
    assert_eq!(Game::new(initial, crate::game::Tick::new(7)), fresh); // restores covered terrain and initial clock
}

#[test]
fn harm_wins_over_exit_entry_from_new_blasts_and_crossed_existing_blasts() {
    for existing in [false, true] {
        let rules = GameRules::new(192, if existing { 1024 } else { 256 })
            .unwrap()
            .with_bomb_parameters(1, 1, 30)
            .unwrap();
        let exit = cell(if existing { 4 } else { 1 }, 1);
        let mut game = Game::new(
            scenario(vec![Terrain::Floor; 15], point(640, 384), rules)
                .with_exit(exit)
                .unwrap(),
            crate::game::Tick::new(0),
        );
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(Direction::Left), false)]).unwrap();
        if existing {
            assert_eq!(game.outcome(), None); // outside the newly created Blast
            let mut sibling = game.clone();
            game.step(&[action(Some(Direction::Right), false)]).unwrap();
            sibling
                .step(&[action(Some(Direction::Right), false)])
                .unwrap();
            assert_eq!(game, sibling);
            assert!(!game.blasts()[0].contains(exit)); // safe endpoint, lethal path
        }
        assert_eq!(
            game.arena().cell_containing(game.character().position),
            Some(exit)
        );
        assert_eq!(game.outcome(), Some(Outcome::Defeat));
    }
}

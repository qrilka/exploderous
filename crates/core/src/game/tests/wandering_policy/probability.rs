use super::super::support::{enemy, point};
use super::{junction, junction_scenario, vector};
use crate::game::{Direction, Game, Position};

fn delta(a: Position, b: Position) -> (i32, i32) {
    (b.x().value() - a.x().value(), b.y().value() - a.y().value())
}

#[test]
fn open_junctions_prefer_straight_and_only_make_legal_perpendicular_turns() {
    // 512 seeds × 4 rotations × 1/2 side exits. Rotations exercise geometry, not
    // independent samples. Each sample reaches one decision after a forced 32-tick
    // approach. Broad, preselected count bands reject no-turn/uniform/reverse AI;
    // they are not a proof of exact probabilities or a private PRNG sequence oracle.
    for direction in [
        Direction::Up,
        Direction::Right,
        Direction::Down,
        Direction::Left,
    ] {
        let (dx, dy) = vector(direction);
        for side_count in [1, 2] {
            let mut straight = 0;
            let mut sides = [0; 2];
            for seed in 0..512 {
                let mut game = junction(direction, true, side_count, seed);
                game.step(&[]).unwrap();
                let movement = delta(point(640, 640), game.enemies().next().unwrap().position);
                match movement {
                    m if m == (dx * 8, dy * 8) => straight += 1,
                    m if m == (-dy * 8, dx * 8) => sides[0] += 1,
                    m if side_count == 2 && m == (dy * 8, -dx * 8) => sides[1] += 1,
                    _ => panic!(
                        "illegal/reversed move: seed={seed}, direction={direction:?}, movement={movement:?}"
                    ),
                }
                // No off-centre reselection during the following Cell of travel.
                for tick in 34..=64 {
                    let previous = game.enemies().next().unwrap().position;
                    game.step(&[]).unwrap();
                    assert_eq!(
                        delta(previous, game.enemies().next().unwrap().position),
                        movement,
                        "seed={seed}, direction={direction:?}, tick={tick}"
                    );
                }
            }
            assert!(
                (437..=485).contains(&straight),
                "direction={direction:?}, straight={straight}"
            );
            if side_count == 2 {
                for count in sides {
                    assert!((8..=45).contains(&count), "side counts={sides:?}");
                }
            } else {
                assert_eq!(sides[1], 0);
            }
        }
    }
}

#[test]
fn blocked_routes_prefer_reversal_and_split_the_remaining_choices_among_sides() {
    // Same forced approach as the open-junction check, but the forward neighbour
    // is solid. 512 draws distinguish 75% reversal from uniform or always-reverse;
    // one/two exits check category weighting rather than a weight per side exit.
    for direction in [
        Direction::Up,
        Direction::Right,
        Direction::Down,
        Direction::Left,
    ] {
        let (dx, dy) = vector(direction);
        for side_count in [1, 2] {
            let mut reversals = 0;
            let mut sides = [0; 2];
            for seed in 0..512 {
                let mut game = junction(direction, false, side_count, seed);
                game.step(&[]).unwrap();
                let movement = delta(point(640, 640), game.enemies().next().unwrap().position);
                match movement {
                    m if m == (-dx * 8, -dy * 8) => reversals += 1,
                    m if m == (-dy * 8, dx * 8) => sides[0] += 1,
                    m if side_count == 2 && m == (dy * 8, -dx * 8) => sides[1] += 1,
                    _ => panic!(
                        "illegal move: seed={seed}, direction={direction:?}, movement={movement:?}"
                    ),
                }
            }
            assert!(
                (345..=425).contains(&reversals),
                "direction={direction:?}, reversals={reversals}"
            );
            if side_count == 2 {
                for count in sides {
                    assert!((35..=95).contains(&count), "side counts={sides:?}");
                }
            } else {
                assert_eq!(sides[1], 0);
            }
        }
    }
}

#[test]
fn initial_heading_is_chosen_from_all_legal_cardinal_directions() {
    // One decision per fixed seed. Bands are roughly four binomial standard
    // deviations around 512 / legal_count, not proof of exact probabilities.
    for (legal_count, minimum, maximum) in [(2, 212, 300), (3, 129, 213), (4, 90, 166)] {
        let mut counts = [0; 4];
        for seed in 0..512 {
            // Right/Left are always open; Down then Up add the remaining exits.
            let initial = junction_scenario(Direction::Right, true, legal_count - 2, seed)
                .with_enemies(vec![enemy(7, point(640, 640))])
                .unwrap();
            let mut game = Game::new(initial, crate::game::Tick::new(0));
            game.step(&[]).unwrap();
            let index = match delta(point(640, 640), game.enemies().next().unwrap().position) {
                (8, 0) => 0,
                (-8, 0) => 1,
                (0, 8) if legal_count >= 3 => 2,
                (0, -8) if legal_count == 4 => 3,
                movement => panic!(
                    "seed={seed}, exits={legal_count}, unexpected initial movement={movement:?}"
                ),
            };
            counts[index] += 1;
        }
        eprintln!(
            "initial headings: exits={legal_count}, counts={:?}",
            &counts[..legal_count]
        );
        for &count in &counts[..legal_count] {
            assert!(
                (minimum..=maximum).contains(&count),
                "exits={legal_count}, counts={counts:?}"
            );
        }
    }
}

#[test]
fn open_junction_side_turns_are_balanced_conditionally_on_turning() {
    let mut sides = [0; 2];
    // About 410 turns across 4,096 fixed seeds give a roughly four-standard-
    // deviation 40–60% band for each side. Existing tests check the 90/10 split.
    for seed in 0..4096 {
        let mut game = junction(Direction::Right, true, 2, seed);
        game.step(&[]).unwrap();
        match delta(point(640, 640), game.enemies().next().unwrap().position) {
            (8, 0) => {}
            (0, -8) => sides[0] += 1,
            (0, 8) => sides[1] += 1,
            movement => panic!("seed={seed}, illegal junction movement={movement:?}"),
        }
    }
    let turns: u32 = sides.iter().sum();
    eprintln!("open-junction side turns: sides={sides:?}, total={turns}");
    assert!(turns >= 300, "too few turns to check balance: {sides:?}");
    for count in sides {
        assert!(
            100 * count >= 40 * turns && 100 * count <= 60 * turns,
            "side counts={sides:?}, total turns={turns}"
        );
    }
}

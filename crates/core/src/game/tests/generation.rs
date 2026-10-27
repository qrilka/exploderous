use super::*;
use std::collections::VecDeque;

// Independent cardinal graph, not the generator's candidate/placement helpers.
fn distances(game: &Game) -> [[Option<u32>; 13]; 13] {
    let mut result = [[None; 13]; 13];
    result[0][0] = Some(0);
    let mut queue = VecDeque::from([(0usize, 0usize)]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) else {
                continue;
            };
            if nx < 13
                && ny < 13
                && result[ny][nx].is_none()
                && game.arena().terrain_at(cell(nx as u32, ny as u32)) != Some(Terrain::Wall)
            {
                result[ny][nx] = Some(result[y][x].unwrap() + 1);
                queue.push_back((nx, ny));
            }
        }
    }
    result
}

#[test]
fn generated_normal_scenarios_obey_geometry_opening_and_object_contracts() {
    let started = std::time::Instant::now();
    // Seeds vary placements; density endpoints stress mandatory Floor/Block reservations;
    // footprint extremes stress the same validated Scenario boundary as authored rules.
    for percent in [0, 40, 100] {
        for footprint in [2, 192, 254] {
            for seed in 0..64 {
                let context = format!("seed={seed}, percent={percent}, footprint={footprint}");
                let rules = GameRules::new(footprint, 16).unwrap();
                let mut state = seed;
                let scenario = Scenario::generate(rules, percent, &mut state)
                    .unwrap_or_else(|error| panic!("{context}: {error:?}"));
                let game = Game::new(scenario, 0);
                let arena = game.arena();
                assert_eq!((arena.width(), arena.height()), (13, 13), "{context}");
                assert_eq!(arena.cells().count(), 169, "{context}");
                assert_eq!(
                    Some(game.character().position),
                    arena.cell_center(cell(0, 0)),
                    "{context}"
                );
                let distances = distances(&game);
                for (cell, terrain) in arena.cells() {
                    let (x, y) = (cell.column().value(), cell.row().value());
                    assert_eq!(
                        terrain == Terrain::Wall,
                        x % 2 == 1 && y % 2 == 1,
                        "{context}: {cell:?}"
                    );
                    assert_eq!(
                        distances[y as usize][x as usize].is_some(),
                        terrain != Terrain::Wall,
                        "{context}: {cell:?}"
                    );
                }
                for opening in [cell(0, 0), cell(1, 0), cell(2, 0), cell(0, 1), cell(0, 2)] {
                    assert_eq!(arena.terrain_at(opening), Some(Terrain::Floor), "{context}");
                }
                assert_eq!(game.enemies().len(), 3, "{context}");
                let mut starts = Vec::new();
                for enemy in game.enemies() {
                    let start = arena.cell_containing(enemy.position).unwrap();
                    assert!(!starts.contains(&start), "{context}");
                    starts.push(start);
                    assert_eq!(Some(enemy.position), arena.cell_center(start), "{context}");
                    assert_eq!(arena.terrain_at(start), Some(Terrain::Floor), "{context}");
                    let (x, y) = (
                        start.column().value() as usize,
                        start.row().value() as usize,
                    );
                    assert!(distances[y][x].unwrap() >= 6, "{context}: {start:?}");
                    assert!(
                        [(0, -1), (1, 0), (0, 1), (-1, 0)]
                            .into_iter()
                            .any(|(dx, dy)| {
                                let (Some(nx), Some(ny)) =
                                    (x.checked_add_signed(dx), y.checked_add_signed(dy))
                                else {
                                    return false;
                                };
                                arena.terrain_at(cell(nx as u32, ny as u32)) == Some(Terrain::Floor)
                            }),
                        "{context}: {start:?}"
                    );
                }
                let (item, exit) = (game.fire_up().unwrap(), game.exit().unwrap());
                assert_ne!(item, exit, "{context}");
                for covered in [item, exit] {
                    assert_eq!(arena.terrain_at(covered), Some(Terrain::Block), "{context}");
                }
                // Density tuning must affect terrain, not merely preserve required objects.
                if percent == 0 {
                    assert_eq!(
                        arena
                            .cells()
                            .filter(|(_, terrain)| *terrain == Terrain::Block)
                            .count(),
                        2, // Only the two covered objectives override zero density.
                        "{context}"
                    );
                } else if percent == 100 {
                    let floors = arena
                        .cells()
                        .filter(|(_, terrain)| *terrain == Terrain::Floor)
                        .count();
                    // Five opening Cells, three distinct starts, and up to three
                    // extra adjacent Floors; enemy pairs can share reservations.
                    assert!((8..=11).contains(&floors), "{context}: floors={floors}");
                }
            }
        }
    }
    eprintln!(
        "576 generated geometry cases: {:?} debug runtime",
        started.elapsed()
    );
}

#[test]
fn density_is_a_target_and_invalid_requests_fail_explicitly() {
    for (initial_state, percent) in [(0, 101), (u64::MAX, 255)] {
        let mut state = initial_state;
        assert_eq!(
            Scenario::generate(GameRules::default(), percent, &mut state),
            Err(ConstructionError::InvalidBlockDensity)
        );
        assert_eq!(state, initial_state);
    }
    let mut blocks = 0;
    for seed in 0..128 {
        let mut state = seed;
        let game = Game::new(
            Scenario::generate(GameRules::default(), 40, &mut state).unwrap(),
            0,
        );
        blocks += game
            .arena()
            .cells()
            .filter(|(_, terrain)| *terrain == Terrain::Block)
            .count();
    }
    // 169 interior - 36 pillars - 5 protected Cells = 128 eligible Cells.
    // This rejects a wrong default density or filling ~40% of the whole rectangle;
    // required enemy Floors/object Blocks may shift the target modestly.
    let density = blocks as f64 / (128.0 * 128.0);
    assert!(
        (0.35..0.45).contains(&density),
        "observed density={density}"
    );
}

#[test]
fn generation_progresses_separately_from_game_clones_and_replays() {
    for seed in [0, 1, u64::MAX] {
        let rules = GameRules::default();
        let mut generation_state = seed;
        let scenario = Scenario::generate(rules, 40, &mut generation_state).unwrap();
        assert_ne!(generation_state, seed, "seed={seed}");
        let mut replay_state = seed;
        assert_eq!(
            Scenario::generate(rules, 40, &mut replay_state).unwrap(),
            scenario,
            "seed={seed}"
        );
        let after_first = generation_state;
        let mut game = Game::new(scenario.clone(), 0);
        game.step(&[CharacterCommand {
            character: game.character().id,
            movement: None,
            place_bomb: true,
        }])
        .unwrap();
        let mut clone = game.clone();
        for tick in 0..160 {
            let command = CharacterCommand {
                character: game.character().id,
                movement: (tick < 32).then_some(Direction::Right),
                place_bomb: tick == 150,
            };
            game.step(&[command]).unwrap();
            clone.step(&[command]).unwrap();
            assert_eq!(game, clone, "seed={seed}, suffix tick={tick}");
        }
        assert_eq!(generation_state, after_first); // Active AI/objects never advance session generation.
        let next = Scenario::generate(rules, 40, &mut generation_state).unwrap();
        assert_ne!(generation_state, after_first, "seed={seed}");
        let mut replay_state = after_first;
        assert_eq!(
            next,
            Scenario::generate(rules, 40, &mut replay_state).unwrap()
        );
        assert_eq!(Game::new(scenario.clone(), 0), Game::new(scenario, 0));
        let before = game.clone();
        let npc = CharacterCommand {
            character: game.enemies().next().unwrap().id,
            movement: None,
            place_bomb: false,
        };
        assert_eq!(game.step(&[npc]), Err(StepError::NpcCommand));
        assert_eq!(game, before);
    }
}

#[test]
fn the_protected_opening_permits_escape_from_a_first_reach_one_bomb() {
    for assisted in [false, true] {
        let rules = GameRules::default().with_turn_assistance(assisted);
        let mut game = Game::new(Scenario::generate(rules, 40, &mut 0).unwrap(), 0);
        game.step(&[CharacterCommand {
            character: game.character().id,
            movement: None,
            place_bomb: true,
        }])
        .unwrap();
        for tick in 2..=151 {
            game.step(&[CharacterCommand {
                character: game.character().id,
                movement: (tick <= 33).then_some(Direction::Right),
                place_bomb: false,
            }])
            .unwrap();
            assert_eq!(game.outcome(), None, "assisted={assisted}, tick={tick}");
        }
        assert_eq!(
            game.arena().cell_containing(game.character().position),
            Some(cell(2, 0))
        );
        assert!(game.bomb().is_none());
        assert!(game.blasts().is_empty());
    }
}

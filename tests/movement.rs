use exploderous::game::{
    Character, CharacterId, ConstructionError, Direction, Game, MovementCommand, MovementRules,
    Position, Scenario, StepError, Terrain,
};

const ID: CharacterId = CharacterId::new(42);

fn command(direction: Direction) -> MovementCommand {
    MovementCommand {
        character: ID,
        direction,
    }
}

fn assert_step(game: &mut Game, frame: &[MovementCommand], expected: Position) {
    let mut authored = scenario(
        game.arena().width(),
        game.arena().height(),
        game.arena().terrain().to_vec(),
        expected.x,
        expected.y,
    );
    authored.character.id = game.character().id;
    authored.initial_tick = game.tick() + 1;
    let expected_game = Game::new(authored, *game.rules()).unwrap();
    game.step(frame).unwrap();
    assert_eq!(*game, expected_game, "frame={frame:?}");
}

fn rows(rows: &[&str], x: i32, y: i32) -> Scenario {
    assert!(rows.iter().all(|row| row.len() == rows[0].len()));
    scenario(
        u32::try_from(rows[0].len()).unwrap(),
        u32::try_from(rows.len()).unwrap(),
        rows.iter()
            .flat_map(|row| row.chars())
            .map(|cell| match cell {
                '.' => Terrain::Floor,
                '#' => Terrain::Wall,
                '+' => Terrain::Block,
                _ => panic!("invalid fixture cell"),
            })
            .collect(),
        x,
        y,
    )
}

fn scenario(width: u32, height: u32, terrain: Vec<Terrain>, x: i32, y: i32) -> Scenario {
    Scenario {
        width,
        height,
        terrain,
        character: Character {
            id: ID,
            position: Position { x, y },
        },
        initial_tick: 0,
    }
}

#[test]
fn construction_observes_the_complete_authored_state() {
    let authored = scenario(2, 1, vec![Terrain::Floor; 2], 256, 128);
    let game = Game::new(authored.clone(), MovementRules::default()).unwrap();
    assert_eq!(game.tick(), 0);
    assert_eq!(game.arena().width(), 2);
    assert_eq!(game.arena().height(), 1);
    assert_eq!(game.arena().terrain(), authored.terrain);
    assert_eq!(game.character(), &authored.character);
    assert_eq!(
        game.rules(),
        &MovementRules {
            footprint_side: 192,
            movement_distance: 16
        }
    );
    assert_eq!(game.clone(), game);
}

#[test]
fn construction_rejects_invalid_data_in_contract_order_and_accepts_limits() {
    use ConstructionError::*;
    let base = scenario(1, 1, vec![Terrain::Floor], 128, 128);
    let defaults = MovementRules::default();
    for (width, height) in [(0, 1), (1, 0), (257, 1), (1, 257), (u32::MAX, u32::MAX)] {
        let mut bad = base.clone();
        bad.width = width;
        bad.height = height;
        bad.terrain.clear();
        assert_eq!(
            Game::new(
                bad,
                MovementRules {
                    footprint_side: 0,
                    movement_distance: 0
                }
            ),
            Err(InvalidDimensions)
        );
    }
    for terrain in [vec![], vec![Terrain::Floor; 2]] {
        let mut bad = base.clone();
        bad.terrain = terrain;
        assert_eq!(
            Game::new(
                bad,
                MovementRules {
                    footprint_side: 0,
                    movement_distance: 0
                }
            ),
            Err(TerrainLengthMismatch)
        );
    }
    for side in [0, 1, 3, 191, 255, 256, 258, u32::MAX] {
        assert_eq!(
            Game::new(
                base.clone(),
                MovementRules {
                    footprint_side: side,
                    movement_distance: 0
                }
            ),
            Err(InvalidFootprintSide)
        );
    }
    for distance in [0, 1025, u32::MAX] {
        let mut bad = base.clone();
        bad.character.position.x = i32::MIN;
        assert_eq!(
            Game::new(
                bad,
                MovementRules {
                    footprint_side: 192,
                    movement_distance: distance
                }
            ),
            Err(InvalidMovementDistance)
        );
    }
    for position in [
        Position { x: -1, y: 128 },
        Position { x: 128, y: -1 },
        Position { x: 95, y: 128 },
        Position { x: 161, y: 128 },
        Position { x: 128, y: 95 },
        Position { x: 128, y: 161 },
        Position {
            x: i32::MIN,
            y: 128,
        },
        Position {
            x: i32::MAX,
            y: 128,
        },
        Position {
            x: 128,
            y: i32::MIN,
        },
        Position {
            x: 128,
            y: i32::MAX,
        },
    ] {
        let mut bad = base.clone();
        bad.character.position = position;
        assert_eq!(
            Game::new(bad, defaults),
            Err(InvalidStartingPosition),
            "{position:?}"
        );
    }
    for terrain in [Terrain::Wall, Terrain::Block] {
        assert_eq!(
            Game::new(
                scenario(2, 1, vec![Terrain::Floor, terrain], 161, 128),
                defaults
            ),
            Err(InvalidStartingPosition)
        );
        assert!(
            Game::new(
                scenario(2, 1, vec![Terrain::Floor, terrain], 160, 128),
                defaults
            )
            .is_ok()
        );
    }
    for x in [96, 160] {
        for y in [96, 160] {
            assert!(Game::new(scenario(1, 1, vec![Terrain::Floor], x, y), defaults).is_ok());
        }
    }
    for side in [2, 254] {
        for distance in [1, 1024] {
            let mut authored = scenario(
                256,
                256,
                vec![Terrain::Floor; 65536],
                65536 - (side / 2) as i32,
                (side / 2) as i32,
            );
            authored.character.id = CharacterId::new(u32::MAX);
            authored.initial_tick = u64::MAX;
            let game = Game::new(
                authored,
                MovementRules {
                    footprint_side: side,
                    movement_distance: distance,
                },
            )
            .unwrap();
            assert_eq!(game.tick(), u64::MAX);
            assert_eq!(game.character().id.value(), u32::MAX);
        }
    }
}

#[test]
fn cardinal_commands_apply_only_to_their_tick() {
    let base = Game::new(
        scenario(3, 3, vec![Terrain::Floor; 9], 384, 384),
        MovementRules::default(),
    )
    .unwrap();
    for (direction, expected) in [
        (Direction::Up, Position { x: 384, y: 368 }),
        (Direction::Right, Position { x: 400, y: 384 }),
        (Direction::Down, Position { x: 384, y: 400 }),
        (Direction::Left, Position { x: 368, y: 384 }),
    ] {
        let mut game = base.clone();
        assert_step(&mut game, &[command(direction)], expected);
    }
    let mut game = base;
    assert_step(&mut game, &[], Position { x: 384, y: 384 });
    assert_step(
        &mut game,
        &[command(Direction::Right)],
        Position { x: 400, y: 384 },
    );
    assert_step(
        &mut game,
        &[command(Direction::Right)],
        Position { x: 416, y: 384 },
    );
    assert_step(&mut game, &[], Position { x: 416, y: 384 });
    assert_step(
        &mut game,
        &[command(Direction::Left)],
        Position { x: 400, y: 384 },
    );
}

#[test]
fn swept_footprints_stop_at_the_first_obstruction_without_a_gap_or_sideways_motion() {
    use Direction::*;
    let fast = MovementRules {
        footprint_side: 192,
        movement_distance: 512,
    };
    let cases = [
        (
            rows(&[".#.."], 128, 128),
            fast,
            Right,
            Position { x: 160, y: 128 },
        ),
        (
            rows(&[".+.."], 128, 128),
            fast,
            Right,
            Position { x: 160, y: 128 },
        ),
        (
            rows(&["..#."], 896, 128),
            fast,
            Left,
            Position { x: 864, y: 128 },
        ),
        (
            rows(&["..+."], 896, 128),
            fast,
            Left,
            Position { x: 864, y: 128 },
        ),
        (
            rows(&[".", "#", ".", "."], 128, 128),
            fast,
            Down,
            Position { x: 128, y: 160 },
        ),
        (
            rows(&[".", "+", ".", "."], 128, 128),
            fast,
            Down,
            Position { x: 128, y: 160 },
        ),
        (
            rows(&[".", ".", "#", "."], 128, 896),
            fast,
            Up,
            Position { x: 128, y: 864 },
        ),
        (
            rows(&[".", ".", "+", "."], 128, 896),
            fast,
            Up,
            Position { x: 128, y: 864 },
        ),
        (
            rows(&["..#..+.."], 128, 128),
            MovementRules {
                movement_distance: 1024,
                ..fast
            },
            Right,
            Position { x: 416, y: 128 },
        ),
        (
            rows(&["..#.+..."], 1920, 128),
            MovementRules {
                movement_distance: 1024,
                ..fast
            },
            Left,
            Position { x: 1376, y: 128 },
        ),
    ];
    for (authored, rules, direction, expected) in cases {
        let mut game = Game::new(authored.clone(), rules).unwrap();
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[], expected);
    }
    let mut approaching = Game::new(rows(&[".#.."], 128, 128), MovementRules::default()).unwrap();
    for x in [144, 160, 160] {
        assert_step(&mut approaching, &[command(Right)], Position { x, y: 128 });
    }
}

#[test]
fn floor_edges_are_solid_and_exact_contact_is_safe() {
    for (direction, expected) in [
        (Direction::Up, Position { x: 128, y: 96 }),
        (Direction::Right, Position { x: 160, y: 128 }),
        (Direction::Down, Position { x: 128, y: 160 }),
        (Direction::Left, Position { x: 96, y: 128 }),
    ] {
        let mut game = Game::new(
            rows(&["."], 128, 128),
            MovementRules {
                footprint_side: 192,
                movement_distance: 1024,
            },
        )
        .unwrap();
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[command(direction)], expected);
    }
    for side in [2, 254] {
        let radius = (side / 2) as i32;
        let mut game = Game::new(
            scenario(
                256,
                256,
                vec![Terrain::Floor; 65536],
                65536 - radius - 1,
                radius + 1,
            ),
            MovementRules {
                footprint_side: side,
                movement_distance: 1024,
            },
        )
        .unwrap();
        assert_step(
            &mut game,
            &[command(Direction::Right)],
            Position {
                x: 65536 - radius,
                y: radius + 1,
            },
        );
        assert_step(
            &mut game,
            &[command(Direction::Up)],
            Position {
                x: 65536 - radius,
                y: radius,
            },
        );
        assert_step(
            &mut game,
            &[command(Direction::Left)],
            Position {
                x: 64512 - radius,
                y: radius,
            },
        );
        assert_step(
            &mut game,
            &[command(Direction::Down)],
            Position {
                x: 64512 - radius,
                y: radius + 1024,
            },
        );
    }
}

#[test]
fn off_centre_corridors_and_corner_contact_do_not_align_or_slide_the_character() {
    let classic = [
        "#######", "#.....#", "#.#.#.#", "#.....#", "#.#.#.#", "#.....#", "#######",
    ];
    for x in [352, 416] {
        let mut game = Game::new(rows(&classic, x, 640), MovementRules::default()).unwrap();
        assert_step(&mut game, &[command(Direction::Up)], Position { x, y: 624 });
    }
    for x in [351, 417] {
        assert_eq!(
            Game::new(rows(&classic, x, 640), MovementRules::default()),
            Err(ConstructionError::InvalidStartingPosition)
        );
    }
    let mut turn = Game::new(rows(&classic, 400, 417), MovementRules::default()).unwrap();
    assert_step(
        &mut turn,
        &[command(Direction::Right)],
        Position { x: 416, y: 417 },
    );
    assert_step(
        &mut turn,
        &[command(Direction::Right)],
        Position { x: 416, y: 417 },
    );
    assert_step(
        &mut turn,
        &[command(Direction::Up)],
        Position { x: 416, y: 401 },
    );
    assert_step(
        &mut turn,
        &[command(Direction::Right)],
        Position { x: 432, y: 401 },
    );

    let mut corner = Game::new(rows(&["..", ".#"], 160, 160), MovementRules::default()).unwrap();
    assert_step(
        &mut corner,
        &[command(Direction::Right)],
        Position { x: 176, y: 160 },
    );
    let mut blocked_corner =
        Game::new(rows(&["..", ".#"], 160, 161), MovementRules::default()).unwrap();
    assert_step(
        &mut blocked_corner,
        &[command(Direction::Right)],
        Position { x: 160, y: 161 },
    );
    let mut spanning = Game::new(rows(&["..", ".."], 256, 256), MovementRules::default()).unwrap();
    assert_step(
        &mut spanning,
        &[command(Direction::Down)],
        Position { x: 256, y: 272 },
    );
    let mut perpendicular =
        Game::new(rows(&[".#.", "..."], 384, 352), MovementRules::default()).unwrap();
    assert_step(
        &mut perpendicular,
        &[command(Direction::Right)],
        Position { x: 400, y: 352 },
    );
}

fn assert_rejected(game: &mut Game, frame: &[MovementCommand], error: StepError) {
    let before = game.clone();
    assert_eq!(
        game.step(frame),
        Err(error),
        "tick={}, frame={frame:?}",
        before.tick()
    );
    assert_eq!(*game, before, "rejected frame={frame:?}");
}

#[test]
fn malformed_frames_are_atomic_and_error_priority_is_order_independent() {
    let known = command(Direction::Right);
    let unknown = MovementCommand {
        character: CharacterId::new(43),
        direction: Direction::Left,
    };
    let other_unknown = MovementCommand {
        character: CharacterId::new(u32::MAX),
        direction: Direction::Down,
    };
    let faults = [
        (vec![known, known], StepError::DuplicateCommand),
        (vec![unknown, unknown], StepError::DuplicateCommand),
        (vec![unknown], StepError::UnknownCharacter),
        (vec![known, unknown], StepError::UnknownCharacter),
        (vec![unknown, other_unknown], StepError::UnknownCharacter),
        (vec![known, unknown, unknown], StepError::DuplicateCommand),
        (
            vec![known, known, unknown, other_unknown],
            StepError::DuplicateCommand,
        ),
        (
            vec![unknown, other_unknown, other_unknown],
            StepError::DuplicateCommand,
        ),
    ];
    for tick in [0, u64::MAX] {
        let mut authored = rows(&[".."], 128, 128);
        authored.initial_tick = tick;
        let mut game = Game::new(authored, MovementRules::default()).unwrap();
        for (frame, error) in &faults {
            for shift in 0..frame.len() {
                let mut reordered = frame.clone();
                reordered.rotate_left(shift);
                assert_rejected(&mut game, &reordered, *error);
                reordered.reverse();
                assert_rejected(&mut game, &reordered, *error);
            }
        }
        if tick == u64::MAX {
            assert_rejected(&mut game, &[], StepError::TickExhausted);
            assert_rejected(&mut game, &[known], StepError::TickExhausted);
        }
    }
}

#[test]
fn the_last_tick_succeeds_then_exhaustion_preserves_complete_state() {
    for (start, frame, expected) in [
        (128, vec![], Position { x: 128, y: 128 }),
        (
            128,
            vec![command(Direction::Right)],
            Position { x: 144, y: 128 },
        ),
        (
            160,
            vec![command(Direction::Right)],
            Position { x: 160, y: 128 },
        ),
    ] {
        let mut authored = rows(&[".#"], start, 128);
        authored.initial_tick = u64::MAX - 1;
        let mut game = Game::new(authored, MovementRules::default()).unwrap();
        assert_step(&mut game, &frame, expected);
        assert_eq!(game.tick(), u64::MAX);
        assert_rejected(&mut game, &frame, StepError::TickExhausted);
    }
    let mut authored = rows(&[".."], 128, 128);
    authored.character.id = CharacterId::new(0);
    let mut game = Game::new(authored, MovementRules::default()).unwrap();
    assert_step(
        &mut game,
        &[MovementCommand {
            character: CharacterId::new(0),
            direction: Direction::Right,
        }],
        Position { x: 144, y: 128 },
    );
}

// Independent reference: enumerate ALL terrain rectangles and measure positive-area
// overlap directly. No production geometry, cell query, or advancement helper is used.
fn reference_legal(authored: &Scenario, rules: MovementRules, position: Position) -> bool {
    let radius = i64::from(rules.footprint_side / 2);
    let (left, right) = (
        i64::from(position.x) - radius,
        i64::from(position.x) + radius,
    );
    let (top, bottom) = (
        i64::from(position.y) - radius,
        i64::from(position.y) + radius,
    );
    if left < 0
        || top < 0
        || right > i64::from(authored.width) * 256
        || bottom > i64::from(authored.height) * 256
    {
        return false;
    }
    let width = usize::try_from(authored.width).unwrap();
    for (index, terrain) in authored.terrain.iter().enumerate() {
        if *terrain == Terrain::Floor {
            continue;
        }
        let cell_left = i64::try_from(index % width).unwrap() * 256;
        let cell_top = i64::try_from(index / width).unwrap() * 256;
        let overlap_x = right.min(cell_left + 256) - left.max(cell_left);
        let overlap_y = bottom.min(cell_top + 256) - top.max(cell_top);
        if overlap_x > 0 && overlap_y > 0 {
            return false;
        }
    }
    true
}

fn reference_move(authored: &Scenario, rules: MovementRules, direction: Direction) -> Position {
    let (dx, dy) = match direction {
        Direction::Up => (0, -1),
        Direction::Right => (1, 0),
        Direction::Down => (0, 1),
        Direction::Left => (-1, 0),
    };
    let mut position = authored.character.position;
    for _ in 0..rules.movement_distance {
        let next = Position {
            x: position.x + dx,
            y: position.y + dy,
        };
        if !reference_legal(authored, rules, next) {
            break;
        }
        position = next;
    }
    position
}

#[test]
fn exhaustive_two_by_two_terrain_matches_the_independent_unit_step_reference() {
    for assignment in 0..81 {
        let mut digits = assignment;
        let terrain: Vec<_> = (0..4)
            .map(|_| {
                let value = [Terrain::Floor, Terrain::Wall, Terrain::Block][digits % 3];
                digits /= 3;
                value
            })
            .collect();
        for side in [2, 128, 192, 254] {
            let r = (side / 2) as i32;
            let mut coordinates = vec![
                r,
                128,
                256 - r,
                256 - r + 1,
                256,
                256 + r - 1,
                256 + r,
                384,
                512 - r,
            ];
            coordinates.sort_unstable();
            coordinates.dedup();
            for &x in &coordinates {
                for &y in &coordinates {
                    let authored = scenario(2, 2, terrain.clone(), x, y);
                    for distance in [1, 16, 255, 256, 257, 1024] {
                        let rules = MovementRules {
                            footprint_side: side,
                            movement_distance: distance,
                        };
                        let constructed = Game::new(authored.clone(), rules);
                        if !reference_legal(&authored, rules, authored.character.position) {
                            assert_eq!(
                                constructed,
                                Err(ConstructionError::InvalidStartingPosition),
                                "scenario={authored:?}, rules={rules:?}, step=0"
                            );
                            continue;
                        }
                        let base = constructed.unwrap_or_else(|error| {
                            panic!("scenario={authored:?}, rules={rules:?}, step=0: {error:?}")
                        });
                        for direction in [
                            Direction::Up,
                            Direction::Right,
                            Direction::Down,
                            Direction::Left,
                        ] {
                            let mut expected = authored.clone();
                            expected.character.position =
                                reference_move(&authored, rules, direction);
                            expected.initial_tick += 1;
                            let expected_game = Game::new(expected, rules).unwrap();
                            let mut game = base.clone();
                            let result = game.step(&[command(direction)]);
                            assert_eq!(
                                result,
                                Ok(()),
                                "scenario={authored:?}, rules={rules:?}, direction={direction:?}, step=1"
                            );
                            assert_eq!(
                                game, expected_game,
                                "scenario={authored:?}, rules={rules:?}, direction={direction:?}, step=1"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn deterministic_sequences_preserve_invariants_and_clones_continue_independently() {
    use Direction::*;
    let authored = rows(
        &[
            "#######", "#.....#", "#.#.#.#", "#..+..#", "#.#.#.#", "#.....#", "#######",
        ],
        384,
        384,
    );
    let mut frames = Vec::new();
    for direction in [
        Some(Right),
        Some(Right),
        Some(Right),
        Some(Right),
        Some(Right),
        None,
        Some(Down),
        Some(Down),
        Some(Down),
        Some(Up),
        Some(Left),
        Some(Left),
        None,
        Some(Down),
        Some(Down),
        Some(Down),
        Some(Down),
        Some(Right),
        Some(Up),
        Some(Left),
        Some(Left),
        Some(Left),
        Some(Left),
        None,
    ] {
        frames.push((
            direction.map(command).into_iter().collect::<Vec<_>>(),
            Ok(()),
        ));
        if frames.len() == 3 {
            frames.push((
                vec![command(Down), command(Up)],
                Err(StepError::DuplicateCommand),
            ));
        }
        if frames.len() == 11 {
            frames.push((
                vec![MovementCommand {
                    character: CharacterId::new(43),
                    direction: Right,
                }],
                Err(StepError::UnknownCharacter),
            ));
        }
    }
    for rules in [
        MovementRules::default(),
        MovementRules {
            footprint_side: 2,
            movement_distance: 1,
        },
        MovementRules {
            footprint_side: 128,
            movement_distance: 257,
        },
        MovementRules {
            footprint_side: 192,
            movement_distance: 512,
        },
        MovementRules {
            footprint_side: 254,
            movement_distance: 1024,
        },
    ] {
        let mut game = Game::new(authored.clone(), rules).unwrap();
        let mut replay = Game::new(authored.clone(), rules).unwrap();
        let mut reference = authored.clone();
        let mut continuation = None;
        for (step, (frame, outcome)) in frames.iter().enumerate() {
            if outcome.is_ok() {
                if let Some(input) = frame.first() {
                    reference.character.position =
                        reference_move(&reference, rules, input.direction);
                }
                reference.initial_tick += 1;
            }
            let before = game.clone();
            assert_eq!(
                game.step(frame),
                *outcome,
                "scenario={authored:?}, rules={rules:?}, frame={frame:?}, step={step}"
            );
            assert_eq!(replay.step(frame), *outcome);
            if let Some(clone) = &mut continuation {
                assert_eq!(Game::step(clone, frame), *outcome);
                assert_eq!(
                    *clone, game,
                    "clone suffix, scenario={authored:?}, rules={rules:?}, step={step}"
                );
            }
            if outcome.is_err() {
                assert_eq!(
                    game, before,
                    "rejection, scenario={authored:?}, rules={rules:?}, step={step}"
                );
            }
            assert!(
                reference_legal(&reference, rules, game.character().position),
                "scenario={authored:?}, rules={rules:?}, step={step}"
            );
            assert_eq!(
                game,
                Game::new(reference.clone(), rules).unwrap(),
                "reference sequence, scenario={authored:?}, rules={rules:?}, frame={frame:?}, step={step}"
            );
            assert_eq!(
                game, replay,
                "replay, scenario={authored:?}, rules={rules:?}, step={step}"
            );
            if step == 7 {
                continuation = Some(game.clone());
                let frozen_original = game.clone();
                let frozen_sibling = continuation.clone();
                let mut divergent = game.clone();
                divergent.step(&[command(Left)]).unwrap();
                assert_ne!(divergent, game);
                assert_eq!(game, frozen_original);
                assert_eq!(continuation, frozen_sibling);
            }
        }
    }
}

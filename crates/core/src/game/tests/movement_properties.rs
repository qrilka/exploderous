use super::*;

// Independent reference: enumerate ALL terrain rectangles and measure positive-area
// overlap directly. No production geometry, cell query, or advancement helper is used.
fn reference_legal(authored: &ScenarioInput, rules: GameRules, position: Position) -> bool {
    let radius = i64::from(rules.footprint_side() / 2);
    let (left, right) = (
        i64::from(position.x().value()) - radius,
        i64::from(position.x().value()) + radius,
    );
    let (top, bottom) = (
        i64::from(position.y().value()) - radius,
        i64::from(position.y().value()) + radius,
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

fn reference_direction_delta(direction: Direction) -> (i32, i32) {
    match direction {
        Direction::Up => (0, -1),
        Direction::Right => (1, 0),
        Direction::Down => (0, 1),
        Direction::Left => (-1, 0),
    }
}

fn reference_cardinal(
    authored: &ScenarioInput,
    rules: GameRules,
    mut position: Position,
    direction: Direction,
    distance: u32,
) -> Position {
    let (dx, dy) = reference_direction_delta(direction);
    for _ in 0..distance {
        let next = point(position.x().value() + dx, position.y().value() + dy);
        if !reference_legal(authored, rules, next) {
            break;
        }
        position = next;
    }
    position
}

fn reference_move(authored: &ScenarioInput, rules: GameRules, direction: Direction) -> Position {
    let start = authored.character.position;
    let distance = rules.movement_distance();
    let (dx, dy) = reference_direction_delta(direction);
    let ordinary = reference_cardinal(authored, rules, start, direction, distance);
    let desired = point(
        start.x().value() + dx * distance as i32,
        start.y().value() + dy * distance as i32,
    );
    if !rules.turn_assistance() || ordinary == desired {
        return ordinary;
    }
    // A sideways correction cannot repair a forward arena-boundary violation.
    let radius = (rules.footprint_side() / 2) as i32;
    let (forward, extent) = if dx == 0 {
        (desired.y().value(), authored.height)
    } else {
        (desired.x().value(), authored.width)
    };
    if !(radius..=extent as i32 * 256 - radius).contains(&forward) {
        return ordinary;
    }
    let perpendicular = if dx == 0 {
        [(-1, 0), (1, 0)]
    } else {
        [(0, -1), (0, 1)]
    };
    let mut best: Option<(u32, i32, i32)> = None;
    // Independently walk each complete sideways route one unit at a time,
    // then compare its nearest safe entry with the other route's entry.
    for (sx, sy) in perpendicular {
        for correction in 1..=128 {
            let candidate = point(
                start.x().value() + sx * correction as i32,
                start.y().value() + sy * correction as i32,
            );
            if !reference_legal(authored, rules, candidate) {
                break;
            }
            let desired = point(
                candidate.x().value() + dx * distance as i32,
                candidate.y().value() + dy * distance as i32,
            );
            if reference_cardinal(authored, rules, candidate, direction, distance) == desired {
                let entry = (correction, sx, sy);
                best = Some(best.map_or(entry, |other| other.min(entry)));
                break;
            }
        }
    }
    if let Some((correction, sx, sy)) = best {
        let spent = correction.min(distance);
        let partial = point(
            start.x().value() + sx * spent as i32,
            start.y().value() + sy * spent as i32,
        );
        return reference_cardinal(authored, rules, partial, direction, distance - spent);
    }
    ordinary
}

#[test]
fn off_centre_corridor_entries_make_safe_progress_within_fifteen_ticks() {
    use Direction::*;
    let rules = GameRules::default();
    // Downward entrance at y=768, with legal corridor centres x=608..=672.
    // At contact, y=672 is 96 units before the mouth. Even a 128-unit
    // correction plus 97 forward units fits in 15 ticks at 16 units/tick.
    let entrance = rows("#####\n#...#\n#...#\n##.##\n#####", 640, 672);
    for (quarter_turns, direction) in [(0, Down), (1, Left), (2, Up), (3, Right)] {
        let rotate = |mut x, mut y| {
            for _ in 0..quarter_turns {
                (x, y) = (1280 - y, x);
            }
            point(x, y)
        };
        let mut terrain = entrance.terrain.clone();
        for _ in 0..quarter_turns {
            terrain = (0..25)
                .map(|index| terrain[(4 - index % 5) * 5 + index / 5])
                .collect();
        }
        let mouth = rotate(640, 768);
        let (dx, dy) = reference_direction_delta(direction);
        for side in [-1, 1] {
            for correction in 1..=128 {
                let start = rotate(640 + side * (32 + correction), 672);
                let authored =
                    scenario(5, 5, terrain.clone(), start.x().value(), start.y().value());
                let mut game = Game::new(
                    authored.clone().parse(rules).unwrap(),
                    crate::game::Tick::new(0),
                );
                let case = (direction, side, correction);
                let mut entered = false;
                for tick in 1..=15 {
                    let before = game.character().position;
                    game.step(&[command(direction)]).unwrap();
                    let after = game.character().position;
                    assert_eq!(game.tick().value(), tick, "case={case:?}");
                    assert!(
                        reference_legal(&authored, rules, after),
                        "case={case:?}, tick={tick}"
                    );
                    let travel = before.x().value().abs_diff(after.x().value())
                        + before.y().value().abs_diff(after.y().value());
                    assert!(travel <= 16, "case={case:?}, tick={tick}, travel={travel}");

                    // Check only the observed path: sideways, then commanded
                    // forward travel. No oracle predicts the correction chosen.
                    let (bend, sideways_direction) = if dx == 0 {
                        (
                            point(after.x().value(), before.y().value()),
                            if after.x().value() < before.x().value() {
                                Left
                            } else {
                                Right
                            },
                        )
                    } else {
                        (
                            point(before.x().value(), after.y().value()),
                            if after.y().value() < before.y().value() {
                                Up
                            } else {
                                Down
                            },
                        )
                    };
                    let sideways = before.x().value().abs_diff(bend.x().value())
                        + before.y().value().abs_diff(bend.y().value());
                    let forward = (after.x().value() - bend.x().value()) * dx
                        + (after.y().value() - bend.y().value()) * dy;
                    assert!(forward >= 0, "case={case:?}, tick={tick}");
                    assert_eq!(
                        reference_cardinal(&authored, rules, before, sideways_direction, sideways),
                        bend,
                        "unsafe correction: case={case:?}, tick={tick}"
                    );
                    assert_eq!(
                        reference_cardinal(&authored, rules, bend, direction, forward as u32),
                        after,
                        "unsafe forward path: case={case:?}, tick={tick}"
                    );
                    if (after.x().value() - mouth.x().value()) * dx
                        + (after.y().value() - mouth.y().value()) * dy
                        > 0
                    {
                        entered = true;
                        break;
                    }
                }
                assert!(
                    entered,
                    "did not enter within 15 ticks: case={case:?}, position={:?}",
                    game.character().position
                );
            }
        }
    }
}

#[test]
fn deterministic_sequences_preserve_invariants_and_clones_continue_independently() {
    use Direction::*;
    let authored = rows(
        r#"
#######
#.....#
#.#.#.#
#..+..#
#.#.#.#
#.....#
#######
"#,
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
                vec![CharacterCommand {
                    character: CharacterId::new(43),
                    movement: Some(Right),
                    place_bomb: false,
                }],
                Err(StepError::UnknownCharacter),
            ));
        }
    }
    for rules in [
        GameRules::default(),
        GameRules::new(2, 1).unwrap(),
        GameRules::new(128, 257).unwrap(),
        GameRules::new(192, 512).unwrap(),
        GameRules::new(254, 1024).unwrap(),
    ]
    .into_iter()
    .flat_map(|rules| [rules.with_turn_assistance(false), rules])
    {
        let initial_tick = 0;
        let mut game = Game::new(
            authored.clone().parse(rules).unwrap(),
            crate::game::Tick::new(initial_tick),
        );
        let mut replay = Game::new(
            authored.clone().parse(rules).unwrap(),
            crate::game::Tick::new(initial_tick),
        );
        let mut reference = authored.clone();
        let mut reference_tick = initial_tick;
        let mut continuation = None;
        for (step, (frame, outcome)) in frames.iter().enumerate() {
            if outcome.is_ok() {
                if let Some(input) = frame.first() {
                    reference.character.position =
                        reference_move(&reference, rules, input.movement.unwrap());
                }
                reference_tick += 1;
            }
            let before = game.clone();
            assert_eq!(
                game.step(frame),
                *outcome,
                "scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, frame={frame:?}, step={step}"
            );
            assert_eq!(replay.step(frame), *outcome);
            if let Some(clone) = &mut continuation {
                assert_eq!(Game::step(clone, frame), *outcome);
                assert_eq!(
                    *clone, game,
                    "clone suffix, scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, step={step}"
                );
            }
            if outcome.is_err() {
                assert_eq!(
                    game, before,
                    "rejection, scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, step={step}"
                );
            }
            assert!(
                reference_legal(&reference, rules, game.character().position),
                "scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, step={step}"
            );
            assert_eq!(
                game,
                Game::new(
                    reference.clone().parse(rules).unwrap(),
                    crate::game::Tick::new(reference_tick)
                ),
                "reference sequence, scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, frame={frame:?}, step={step}"
            );
            assert_eq!(
                game, replay,
                "replay, scenario={authored:?}, initial_tick={initial_tick}, rules={rules:?}, step={step}"
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

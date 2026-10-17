use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, CharacterKind, Column, ControlSource,
    Direction, Game, GameRules, Position, Row, Scenario, StepError, Terrain, WorldX, WorldY,
};

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}
fn cell(x: u32, y: u32) -> CellCoord {
    CellCoord::new(Column::new(x), Row::new(y))
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
fn action(direction: Option<Direction>, place_bomb: bool) -> CharacterCommand {
    CharacterCommand {
        character: CharacterId::new(0),
        movement: direction,
        place_bomb,
    }
}
fn corridor(seed: u64, length: u32, enemies: Vec<Character>, rules: GameRules) -> Scenario {
    let mut terrain = vec![Terrain::Floor; length as usize * 3];
    terrain[length as usize..length as usize * 2].fill(Terrain::Wall);
    Scenario::new(length, 3, terrain, character(0, 128, 640), rules)
        .unwrap()
        .with_enemies(enemies)
        .unwrap()
        .with_random_seed(seed)
}
fn delta(a: Position, b: Position) -> (i32, i32) {
    (b.x().value() - a.x().value(), b.y().value() - a.y().value())
}

#[test]
fn a_clear_corridor_keeps_its_heading_beyond_the_old_maximum_run_duration() {
    // Ten Cells of unobstructed travel exceed the old four-Cell timer ceiling.
    // The one-way start forces Right; no random layout or heading oracle is needed.
    for seed in 0..16 {
        let mut game = Game::new(
            corridor(seed, 13, vec![character(7, 128, 128)], GameRules::default()),
            0,
        );
        for tick in 1..=320 {
            game.step(&[]).unwrap();
            assert_eq!(
                game.enemies()[0].position,
                point(128 + tick * 8, 128),
                "seed={seed}, tick={tick}"
            );
            assert_eq!(game.outcome(), None);
        }
    }
}

fn vector(direction: Direction) -> (i32, i32) {
    match direction {
        Direction::Up => (0, -1),
        Direction::Right => (1, 0),
        Direction::Down => (0, 1),
        Direction::Left => (-1, 0),
    }
}

fn junction_scenario(
    direction: Direction,
    forward_open: bool,
    side_count: usize,
    seed: u64,
) -> Scenario {
    let (dx, dy) = vector(direction);
    let mut terrain = vec![Terrain::Wall; 25];
    let mut floor = |x: i32, y: i32| terrain[(y * 5 + x) as usize] = Terrain::Floor;
    floor(4, 4); // Isolated player; contact cannot freeze the policy evidence.
    floor(2 - dx, 2 - dy); // One-way starting pocket forces the approach heading.
    floor(2, 2);
    if forward_open {
        floor(2 + dx, 2 + dy);
    }
    for (sx, sy) in [(-dy, dx), (dy, -dx)].into_iter().take(side_count) {
        floor(2 + sx, 2 + sy);
    }
    Scenario::new(
        5,
        5,
        terrain,
        character(0, 1152, 1152),
        GameRules::default(),
    )
    .unwrap()
    .with_enemies(vec![character(7, 640 - dx * 256, 640 - dy * 256)])
    .unwrap()
    .with_random_seed(seed)
}

fn junction(direction: Direction, forward_open: bool, side_count: usize, seed: u64) -> Game {
    let mut game = Game::new(
        junction_scenario(direction, forward_open, side_count, seed),
        0,
    );
    for _ in 0..32 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.enemies()[0].position, point(640, 640));
    game
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
                let movement = delta(point(640, 640), game.enemies()[0].position);
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
                    let previous = game.enemies()[0].position;
                    game.step(&[]).unwrap();
                    assert_eq!(
                        delta(previous, game.enemies()[0].position),
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
                let movement = delta(point(640, 640), game.enemies()[0].position);
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
    let mut counts = [0; 4];
    for seed in 0..512 {
        let initial = junction_scenario(Direction::Right, true, 2, seed)
            .with_enemies(vec![character(7, 640, 640)])
            .unwrap();
        let mut game = Game::new(initial, 0);
        game.step(&[]).unwrap();
        let index = match delta(point(640, 640), game.enemies()[0].position) {
            (0, -8) => 0,
            (8, 0) => 1,
            (0, 8) => 2,
            (-8, 0) => 3,
            movement => panic!("seed={seed}, unexpected initial movement={movement:?}"),
        };
        counts[index] += 1;
    }
    // One initial decision per seed; broad bands reject a fixed or biased heading.
    for count in counts {
        assert!((90..=166).contains(&count), "counts={counts:?}");
    }
}

#[test]
fn blocked_forward_and_reverse_leave_only_the_legal_side_category() {
    for side_count in [1, 2] {
        let mut sides = [0; 2];
        for seed in 0..64 {
            let mut terrain = vec![Terrain::Wall; 25];
            for (x, y) in [(0, 2), (1, 2), (2, 2), (2, 1)] {
                terrain[y * 5 + x] = Terrain::Floor;
            }
            if side_count == 2 {
                terrain[17] = Terrain::Floor;
            } // (2,3)
            let initial = Scenario::new(
                5,
                5,
                terrain,
                character(0, 128, 640),
                GameRules::new(2, 256).unwrap(),
            )
            .unwrap()
            .with_enemies(vec![character(7, 384, 640)])
            .unwrap()
            .with_random_seed(seed);
            let mut game = Game::new(initial, 0);
            game.step(&[]).unwrap();
            // The extra player access gives two initial choices. Observe the
            // Right approaches; do not assert a particular seed-to-heading map.
            if game.enemies()[0].position != point(392, 640) {
                continue;
            }
            for _ in 2..=30 {
                game.step(&[]).unwrap();
            }
            game.step(&[action(Some(Direction::Right), true)]).unwrap();
            assert!(!game.bomb().unwrap().escape_allowed_for(CharacterId::new(7)));
            game.step(&[action(Some(Direction::Left), false)]).unwrap();
            assert_eq!(game.enemies()[0].position, point(640, 640));
            game.step(&[]).unwrap();
            match game.enemies()[0].position {
                position if position == point(640, 632) => sides[0] += 1,
                position if side_count == 2 && position == point(640, 648) => sides[1] += 1,
                position => {
                    panic!("seed={seed}, blocked route or missing side progress: {position:?}")
                }
            }
            assert_eq!(game.outcome(), None);
        }
        assert!(sides[0] > 0, "sides={sides:?}");
        assert_eq!(sides[1] > 0, side_count == 2, "sides={sides:?}");
    }
}

#[test]
fn forced_travel_reversals_and_trapped_waiting_do_not_spend_other_enemies_random_choices() {
    // A lower-ID NPC oscillates in an isolated two-Cell corridor; another is
    // trapped. Neither may perturb the main NPC's random junction decisions.
    // 32 streams × 200 ticks include initial, between-centre, and forced reversals.
    for seed in 0..32 {
        let mut terrain = vec![Terrain::Wall; 5 * 7];
        for (x, y) in [
            (1, 2),
            (2, 2),
            (3, 2),
            (2, 1),
            (2, 3),
            (0, 6),
            (1, 6),
            (4, 0),
            (4, 6),
        ] {
            terrain[y * 5 + x] = Terrain::Floor;
        }
        let initial = Scenario::new(
            5,
            7,
            terrain,
            character(0, 1152, 1664),
            GameRules::default(),
        )
        .unwrap()
        .with_random_seed(seed);
        let main = character(7, 384, 640);
        let mut alone = Game::new(initial.clone().with_enemies(vec![main]).unwrap(), 0);
        let mut together = Game::new(
            initial
                .with_enemies(vec![main, character(1, 128, 1664), character(2, 1152, 128)])
                .unwrap(),
            0,
        );
        for tick in 1..=200 {
            alone.step(&[]).unwrap();
            together.step(&[]).unwrap();
            let observed = together
                .enemies()
                .iter()
                .find(|enemy| enemy.id == main.id)
                .unwrap();
            assert_eq!(
                alone.enemies()[0].position,
                observed.position,
                "seed={seed}, tick={tick}"
            );
            assert_eq!(together.outcome(), None);
        }
    }
}

#[test]
fn obstructions_interrupt_at_centres_trapped_enemies_wait_and_bodies_do_not_block() {
    for obstacle in [Terrain::Wall, Terrain::Block] {
        let mut terrain = vec![Terrain::Wall; 7 * 3];
        terrain[0..2].fill(Terrain::Floor);
        terrain[2] = obstacle;
        terrain[6] = Terrain::Floor; // Isolated trapped enemy.
        terrain[14] = Terrain::Floor; // Isolated player.
        let initial = Scenario::new(7, 3, terrain, character(0, 128, 640), GameRules::default())
            .unwrap()
            .with_enemies(vec![
                character(9, 1664, 128),
                character(7, 128, 128),
                character(8, 128, 128),
            ])
            .unwrap();
        let mut game = Game::new(initial, 0);
        for tick in 1..=128 {
            game.step(&[]).unwrap();
            let a = game.enemies()[0].position;
            let b = game.enemies()[1].position;
            assert!((128..=384).contains(&a.x().value()), "tick={tick}");
            assert!((128..=384).contains(&b.x().value()), "tick={tick}");
            assert_eq!((a.y().value(), b.y().value()), (128, 128));
            if tick <= 32 {
                assert_eq!(a, b);
            }
            if tick == 33 {
                assert_eq!(a, point(376, 128));
            }
            assert_eq!(game.enemies()[2].position, point(1664, 128));
            assert_eq!(game.outcome(), None);
        }
    }
}

#[test]
fn identity_order_clone_continuation_and_rejected_frames_preserve_all_policy_state() {
    let enemies = vec![character(99, 384, 640), character(7, 384, 640)];
    let mut reordered = enemies.clone();
    reordered.reverse();
    let initial = junction_scenario(Direction::Right, true, 2, 91);
    let a = initial.clone().with_enemies(enemies).unwrap();
    let b = initial.with_enemies(reordered).unwrap();
    let mut game = Game::new(a, 0);
    let mut reordered = Game::new(b, 0);
    // Pass a random junction decision, then clone between centres.
    for _ in 0..49 {
        game.step(&[]).unwrap();
        reordered.step(&[]).unwrap();
    }
    assert_eq!(game, reordered);
    let mut sibling = game.clone();
    let npc = CharacterCommand {
        character: CharacterId::new(7),
        movement: Some(Direction::Down),
        place_bomb: true,
    };
    let unknown = CharacterCommand {
        character: CharacterId::new(123),
        ..npc
    };
    for (frame, error) in [
        (vec![npc], StepError::NpcCommand),
        (vec![npc, npc], StepError::DuplicateCommand),
        (vec![unknown, npc], StepError::UnknownCharacter),
        (vec![npc, unknown], StepError::UnknownCharacter),
    ] {
        assert_eq!(game.step(&frame), Err(error));
        assert_eq!(game, sibling);
    }
    // 512 ticks cross repeated junction decisions and forced endpoint reversals.
    for tick in 0..512 {
        let frame = [action(None, tick == 0)];
        game.step(&frame).unwrap();
        sibling.step(&frame).unwrap();
        assert_eq!(game, sibling, "seed=91, suffix tick={tick}");
    }
    // Fail placement on a tick that would otherwise consume junction randomness.
    let mut near_limit = Game::new(
        junction_scenario(Direction::Right, true, 2, 91),
        u64::MAX - 33,
    );
    for _ in 0..32 {
        near_limit.step(&[]).unwrap();
    }
    let before = near_limit.clone();
    assert_eq!(
        near_limit.step(&[action(None, true)]),
        Err(StepError::TickExhausted)
    );
    assert_eq!(near_limit, before);

    for (tick, fuse, preplace) in [(u64::MAX - 1, 120, false), (u64::MAX - 2, 1, true)] {
        let rules = GameRules::default()
            .with_bomb_parameters(fuse, 1, 30)
            .unwrap();
        let mut game = Game::new(corridor(91, 13, vec![character(7, 128, 128)], rules), tick);
        if preplace {
            game.step(&[action(None, true)]).unwrap();
        }
        let before = game.clone();
        assert_eq!(
            game.step(&[action(None, true)]),
            Err(StepError::TickExhausted)
        );
        assert_eq!(game, before);
    }
}

#[test]
fn ordinary_wandering_crosses_and_reverses_over_a_new_bomb_until_full_clearance() {
    let mut crossed = false;
    let mut reversed_while_overlapping = false;
    let mut turned_while_overlapping = false;
    let mut blocked_reentry = false;
    // Open continuation demonstrates passage across the Bomb. A blocked forward
    // neighbour makes ordinary obstruction policy reverse while still overlapping.
    // The 150-tick horizon covers clearance and trapped re-entry without detonation;
    // a distant escape corridor keeps the player out of possible enemy paths.
    for forward_open in [false, true] {
        for seed in 0..32 {
            let mut terrain = vec![Terrain::Wall; 7 * 7];
            terrain[..if forward_open { 7 } else { 2 }].fill(Terrain::Floor);
            for row in 1..7 {
                terrain[row * 7 + 1] = Terrain::Floor;
            }
            terrain[42..].fill(Terrain::Floor);
            let initial = Scenario::new(
                7,
                7,
                terrain,
                character(0, 384, 128),
                GameRules::new(2, 512)
                    .unwrap()
                    .with_bomb_parameters(200, 1, 2)
                    .unwrap(),
            )
            .unwrap()
            .with_enemies(vec![character(7, 128, 128)])
            .unwrap()
            .with_random_seed(seed);
            let mut game = Game::new(initial, 0);
            for _ in 0..8 {
                game.step(&[]).unwrap();
            }
            game.step(&[action(None, true)]).unwrap();
            assert_eq!(game.enemies()[0].position, point(200, 128));
            assert!(game.bomb().unwrap().escape_allowed_for(CharacterId::new(7)));
            for direction in [
                Direction::Down,
                Direction::Down,
                Direction::Down,
                Direction::Right,
                Direction::Right,
            ] {
                game.step(&[action(Some(direction), false)]).unwrap();
            }
            let mut previous = game.enemies()[0].position;
            let mut cleared = false;
            let mut sibling = game.clone();
            for tick in 15..=150 {
                game.step(&[]).unwrap();
                sibling.step(&[]).unwrap();
                assert_eq!(
                    game, sibling,
                    "seed={seed}, open={forward_open}, tick={tick}"
                );
                let position = game.enemies()[0].position;
                let (x, y) = (position.x().value(), position.y().value());
                let overlaps = x > 160 && x < 608 && y < 352;
                let permission = game.bomb().unwrap().escape_allowed_for(CharacterId::new(7));
                assert_eq!(game.outcome(), None, "seed={seed}, tick={tick}");
                if !cleared {
                    assert_eq!(
                        permission, overlaps,
                        "seed={seed}, open={forward_open}, tick={tick}"
                    );
                    crossed |= previous.x().value() <= 384 && x > 384 && permission;
                    reversed_while_overlapping |= x < previous.x().value() && permission;
                    turned_while_overlapping |= y > previous.y().value() && permission;
                    cleared = !permission;
                } else {
                    assert!(
                        !permission && !overlaps,
                        "seed={seed}, open={forward_open}, tick={tick}"
                    );
                    blocked_reentry |= position == previous && position == point(128, 128);
                }
                previous = position;
            }
        }
    }
    assert!(crossed && reversed_while_overlapping && turned_while_overlapping && blocked_reentry);
}

#[test]
fn a_bomb_appearing_ahead_stops_an_in_flight_route_without_off_centre_turns() {
    let mut terrain = vec![Terrain::Floor; 7 * 2];
    terrain[7..].fill(Terrain::Wall);
    let initial = Scenario::new(
        7,
        2,
        terrain,
        character(0, 384, 128),
        GameRules::new(2, 512)
            .unwrap()
            .with_bomb_parameters(10, 1, 2)
            .unwrap(),
    )
    .unwrap()
    .with_enemies(vec![character(7, 128, 128)])
    .unwrap();
    let mut game = Game::new(initial, 0);
    game.step(&[action(None, true)]).unwrap();
    assert!(!game.bomb().unwrap().escape_allowed_for(CharacterId::new(7)));
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    for tick in 3..=10 {
        game.step(&[]).unwrap();
        assert_eq!(
            game.enemies()[0].position,
            point((128 + tick * 8).min(160), 128)
        );
    }
    game.step(&[]).unwrap();
    assert!(game.enemies().is_empty());
    assert_eq!(game.outcome(), None);
}

#[test]
fn clones_cross_random_decisions_after_one_moving_enemy_is_removed() {
    let rules = GameRules::new(2, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 2)
        .unwrap();
    let mut terrain = vec![Terrain::Wall; 13 * 7];
    terrain[..13].fill(Terrain::Floor);
    // The survivor reaches a real branching junction after the other NPC dies.
    for (x, y) in [(1, 4), (2, 4), (3, 4), (2, 3), (2, 5)] {
        terrain[y * 13 + x] = Terrain::Floor;
    }
    let initial = Scenario::new(13, 7, terrain, character(0, 640, 128), rules)
        .unwrap()
        .with_enemies(vec![character(7, 128, 128), character(99, 384, 1152)])
        .unwrap()
        .with_random_seed(91);
    let mut game = Game::new(initial, 0);
    for _ in 0..17 {
        game.step(&[]).unwrap();
    }
    let mut sibling = game.clone();
    for frame in [
        [action(None, true)],
        [action(Some(Direction::Right), false)],
    ] {
        game.step(&frame).unwrap();
        sibling.step(&frame).unwrap();
        assert_eq!(game, sibling);
    }
    assert_eq!(game.enemies().len(), 1);
    assert_eq!(game.enemies()[0].id, CharacterId::new(99));
    for tick in 0..300 {
        game.step(&[]).unwrap();
        sibling.step(&[]).unwrap();
        assert_eq!(game, sibling, "seed=91, suffix tick={tick}");
        assert_eq!(game.outcome(), None);
    }
}

#[test]
fn wandering_does_not_consume_fire_up() {
    let initial = corridor(0, 5, vec![character(7, 128, 128)], GameRules::default())
        .with_fire_up(cell(1, 0))
        .unwrap();
    let mut game = Game::new(initial, 0);
    for _ in 0..32 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.enemies()[0].position, point(384, 128));
    assert_eq!(game.fire_up(), Some(cell(1, 0)));
    assert_eq!(game.current_blast_reach(), 1);
    assert!(game.bomb().is_none());
}

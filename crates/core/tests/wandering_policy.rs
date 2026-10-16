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
fn runs_expire_at_one_through_four_cells_and_can_continue_or_reverse() {
    // 64 reproducible streams × 129 ticks: enough to cross every possible first
    // deadline. A 13×13 open room leaves four Cells clear in every direction.
    // The isolated player cannot freeze the evidence via accidental contact.
    let mut durations = std::collections::BTreeSet::new();
    let mut choices = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let mut terrain = vec![Terrain::Floor; 13 * 15];
        terrain[13 * 13..13 * 14].fill(Terrain::Wall);
        let scenario = Scenario::new(
            13,
            15,
            terrain,
            character(0, 128, 3712),
            GameRules::default(),
        )
        .unwrap()
        .with_enemies(vec![character(7, 1664, 1664)])
        .unwrap()
        .with_random_seed(seed);
        let mut game = Game::new(scenario, 0);
        let mut previous = game.enemies()[0].position;
        game.step(&[]).unwrap();
        let first = delta(previous, game.enemies()[0].position);
        let policy = game.buroru_wandering(CharacterId::new(7)).unwrap();
        let duration = policy.remaining_ticks() + 1;
        let heading = policy.heading();
        assert!([32, 64, 96, 128].contains(&duration), "seed={seed}");
        durations.insert(duration);
        assert!(
            matches!(first, (0, -8) | (8, 0) | (0, 8) | (-8, 0)),
            "seed={seed}"
        );
        previous = game.enemies()[0].position;
        let mut changed = false;
        for tick in 2..=129 {
            game.step(&[]).unwrap();
            let next = game.enemies()[0].position;
            let movement = delta(previous, next);
            let policy = game.buroru_wandering(CharacterId::new(7)).unwrap();
            if tick <= duration {
                assert_eq!(
                    policy.remaining_ticks(),
                    duration - tick,
                    "seed={seed}, tick={tick}"
                );
                assert_eq!(policy.heading(), heading, "seed={seed}, tick={tick}");
                assert_eq!(movement, first, "seed={seed}, tick={tick}");
            } else if tick == duration + 1 {
                assert!(
                    [31, 63, 95, 127].contains(&policy.remaining_ticks()),
                    "seed={seed}"
                );
            }
            if !changed && movement != first {
                assert!(
                    [33, 65, 97, 129].contains(&tick),
                    "seed={seed}, tick={tick}"
                );
                if tick == 33 {
                    choices.insert(movement);
                    choices.insert(first);
                }
                changed = true;
            }
            previous = next;
            assert_eq!(game.outcome(), None, "seed={seed}, tick={tick}");
        }
    }
    assert_eq!(durations, [32, 64, 96, 128].into_iter().collect());
    assert_eq!(choices.len(), 4); // All four cardinal choices, not a fixed heading.

    // In a straight corridor both reversal and continuation must be possible at
    // the first deadline. The endpoint is distant, so reversal cannot be blockage.
    let mut reversed = false;
    let mut continued = false;
    for seed in 0..64 {
        let mut game = Game::new(
            corridor(seed, 13, vec![character(7, 128, 128)], GameRules::default()),
            0,
        );
        for _ in 0..32 {
            game.step(&[]).unwrap();
        }
        assert_eq!(game.enemies()[0].position, point(384, 128));
        game.step(&[]).unwrap();
        let x = game.enemies()[0].position.x().value();
        reversed |= x == 376;
        continued |= x == 392;
    }
    assert!(reversed && continued);
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
    let enemies = vec![character(99, 128, 128), character(7, 128, 128)];
    let mut reordered = enemies.clone();
    reordered.reverse();
    let a = corridor(91, 13, enemies, GameRules::default());
    let b = corridor(91, 13, reordered, GameRules::default());
    let mut game = Game::new(a, 0);
    let mut reordered = Game::new(b, 0);
    for _ in 0..17 {
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
    // 512 ticks crosses at least four max-length runs and several end obstructions.
    for tick in 0..512 {
        let frame = [action(None, tick == 0)];
        game.step(&frame).unwrap();
        sibling.step(&frame).unwrap();
        assert_eq!(game, sibling, "seed=91, suffix tick={tick}");
    }
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
    let mut blocked_reentry = false;
    for seed in 0..64 {
        let mut terrain = vec![Terrain::Floor; 7 * 2];
        terrain[7..].fill(Terrain::Wall);
        let initial = Scenario::new(
            7,
            2,
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
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
        let mut previous = 224;
        let mut cleared = false;
        let mut sibling = game.clone();
        for tick in 13..=150 {
            game.step(&[]).unwrap();
            sibling.step(&[]).unwrap();
            assert_eq!(game, sibling, "seed={seed}, tick={tick}");
            let x = game.enemies()[0].position.x().value();
            let permission = game.bomb().unwrap().escape_allowed_for(CharacterId::new(7));
            assert_eq!(game.outcome(), None, "seed={seed}, tick={tick}");
            if !cleared {
                assert_eq!(permission, x > 160 && x < 608, "seed={seed}, tick={tick}");
                crossed |= previous <= 384 && x > 384;
                reversed_while_overlapping |= x < previous && permission;
                cleared = !permission;
            } else {
                assert!(!permission, "seed={seed}, tick={tick}");
                assert!(x <= 160 || x >= 608, "seed={seed}, tick={tick}, x={x}");
                blocked_reentry |= x == previous;
            }
            previous = x;
        }
    }
    assert!(crossed && reversed_while_overlapping && blocked_reentry);
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
    let mut terrain = vec![Terrain::Floor; 13 * 3];
    terrain[13..26].fill(Terrain::Wall);
    let initial = Scenario::new(13, 3, terrain, character(0, 640, 128), rules)
        .unwrap()
        .with_enemies(vec![character(7, 128, 128), character(99, 128, 640)])
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

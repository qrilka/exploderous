use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, Column, ConstructionError, Direction,
    Game, GameRules, Position, Row, Scenario, StepError, Terrain, WorldX, WorldY,
};

const PLAYER: CharacterId = CharacterId::new(0);

fn cell(column: u32, row: u32) -> CellCoord {
    CellCoord::new(Column::new(column), Row::new(row))
}

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

fn game(layout: &str, position: Position, rules: GameRules, tick: u64) -> Game {
    let rows: Vec<_> = layout.lines().collect();
    let width = rows[0].len();
    assert!(rows.iter().all(|row| row.len() == width));
    let terrain = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|glyph| match glyph {
            '.' => Terrain::Floor,
            '#' => Terrain::Wall,
            '+' => Terrain::Block,
            _ => panic!("unknown terrain"),
        })
        .collect();
    Game::new(
        Scenario::new(
            width as u32,
            rows.len() as u32,
            terrain,
            Character {
                id: PLAYER,
                position,
            },
            rules,
        )
        .unwrap(),
        tick,
    )
}

fn actions(movement: Option<Direction>, place_bomb: bool) -> CharacterCommand {
    CharacterCommand {
        character: PLAYER,
        movement,
        place_bomb,
    }
}

#[test]
fn placement_uses_the_post_movement_cell_without_snapping() {
    let mut game = game("...", point(248, 128), GameRules::default(), 0);
    game.step(&[actions(Some(Direction::Right), true)]).unwrap();
    assert_eq!(game.character().position, point(264, 128));
    assert_eq!(game.bomb().unwrap().cell(), cell(1, 0));
    let bomb = *game.bomb().unwrap();
    game.step(&[actions(Some(Direction::Right), true)]).unwrap();
    assert_eq!(game.character().position, point(280, 128));
    assert_eq!(game.bomb(), Some(&bomb));
}

#[test]
fn expired_blasts_cannot_trigger_a_bomb_placed_on_the_expiry_tick() {
    let rules = GameRules::default().with_bomb_parameters(2, 1, 4).unwrap();
    let mut game = game("..", point(128, 128), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    for _ in 2..=6 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.blasts()[0].expires_at(), 7);
    game.step(&[actions(None, true)]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.bomb().unwrap().detonate_at(), 9);
}

#[test]
fn bomb_obstacles_stop_whole_paths_from_every_direction() {
    for (escape, return_direction, far, stopped) in [
        (
            Direction::Up,
            Direction::Down,
            point(640, 128),
            point(640, 416),
        ),
        (
            Direction::Right,
            Direction::Left,
            point(1152, 640),
            point(864, 640),
        ),
        (
            Direction::Down,
            Direction::Up,
            point(640, 1152),
            point(640, 864),
        ),
        (
            Direction::Left,
            Direction::Right,
            point(128, 640),
            point(416, 640),
        ),
    ] {
        let mut game = game(
            ".....\n.....\n.....\n.....\n.....",
            point(640, 640),
            GameRules::new(192, 512).unwrap(),
            0,
        );
        game.step(&[actions(None, true)]).unwrap();
        game.step(&[actions(Some(escape), false)]).unwrap();
        assert_eq!(game.character().position, far);
        game.step(&[actions(Some(return_direction), false)])
            .unwrap();
        assert_eq!(
            game.character().position,
            stopped,
            "returning {return_direction:?}"
        );
    }
}

#[test]
fn turn_assistance_treats_an_escaped_bomb_like_a_full_cell_obstacle() {
    for assisted in [true, false] {
        let mut game = game(
            "...\n...\n...",
            point(384, 384),
            GameRules::default().with_turn_assistance(assisted),
            0,
        );
        game.step(&[actions(None, true)]).unwrap();
        for _ in 0..14 {
            game.step(&[actions(Some(Direction::Up), false)]).unwrap();
        }
        for _ in 0..12 {
            game.step(&[actions(Some(Direction::Left), false)]).unwrap();
        }
        assert_eq!(game.character().position, point(192, 160));
        for expected in [point(176, 160), point(160, 160), point(160, 176)] {
            game.step(&[actions(Some(Direction::Down), false)]).unwrap();
            assert_eq!(
                game.character().position,
                if assisted { expected } else { point(192, 160) }
            );
        }
    }
}

#[test]
fn walls_blocks_and_arena_edges_bound_even_a_very_large_reach() {
    let rules = GameRules::default()
        .with_bomb_parameters(1, u32::MAX, 1)
        .unwrap();
    let mut game = game(".#.\n+.+\n.+.", point(384, 384), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    game.step(&[]).unwrap();
    assert_eq!(
        game.blasts()[0].cells(),
        &[cell(1, 1), cell(2, 1), cell(1, 2), cell(0, 1)]
    );
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Wall));
    for location in [cell(2, 1), cell(1, 2), cell(0, 1)] {
        assert_eq!(game.arena().terrain_at(location), Some(Terrain::Floor));
    }
    let mut corner = Game::new(
        Scenario::new(
            1,
            1,
            vec![Terrain::Floor],
            Character {
                id: PLAYER,
                position: point(128, 128),
            },
            rules,
        )
        .unwrap(),
        0,
    );
    corner.step(&[actions(None, true)]).unwrap();
    corner.step(&[]).unwrap();
    assert_eq!(corner.blasts()[0].cells(), &[cell(0, 0)]);
}

#[test]
fn malformed_frames_and_deadline_overflow_preserve_complete_state() {
    let mut placement = game("...", point(128, 128), GameRules::default(), u64::MAX - 2);
    let before = placement.clone();
    assert_eq!(
        placement.step(&[actions(Some(Direction::Right), true)]),
        Err(StepError::TickExhausted)
    );
    assert_eq!(placement, before);

    // This failed step would move, expire the oldest Blast, place a Bomb, and
    // immediately detonate it. Its new expiry cannot be represented: roll back all.
    let rules = GameRules::default().with_bomb_parameters(1, 1, 3).unwrap();
    let mut effects = game("...", point(128, 128), rules, u64::MAX - 7);
    effects.step(&[actions(None, true)]).unwrap();
    effects.step(&[]).unwrap();
    effects.step(&[actions(None, true)]).unwrap();
    effects.step(&[actions(None, true)]).unwrap();
    assert_eq!(effects.tick(), u64::MAX - 3);
    assert_eq!(effects.blasts().len(), 3);
    let before = effects.clone();
    assert_eq!(
        effects.step(&[actions(Some(Direction::Right), true)]),
        Err(StepError::TickExhausted)
    );
    assert_eq!(effects, before);
    let duplicate = [actions(None, true), actions(Some(Direction::Right), false)];
    assert_eq!(effects.step(&duplicate), Err(StepError::DuplicateCommand));
    assert_eq!(effects, before);
    let unknown = CharacterCommand {
        character: CharacterId::new(7),
        ..actions(None, true)
    };
    assert_eq!(effects.step(&[unknown]), Err(StepError::UnknownCharacter));
    assert_eq!(effects, before);

    // At capacity there is no new fuse deadline to check.
    let rules = GameRules::default()
        .with_bomb_parameters(u64::MAX - 1, 1, 1)
        .unwrap();
    let mut full = game("...", point(128, 128), rules, 0);
    full.step(&[actions(None, true)]).unwrap();
    full.step(&[actions(Some(Direction::Right), true)]).unwrap();
    assert_eq!(full.tick(), 2);
    assert_eq!(full.character().position, point(144, 128));
    assert_eq!(full.bomb().unwrap().detonate_at(), u64::MAX);
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
    game.step(&[actions(None, true)]).unwrap();
    let initial_clone = game.clone();
    let mut sibling = initial_clone.clone();
    for frame in [
        vec![actions(Some(Direction::Up), false)],
        vec![actions(Some(Direction::Down), true)],
        vec![],
        vec![],
        vec![actions(Some(Direction::Down), true)],
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
        .step(&[actions(Some(Direction::Down), false)])
        .unwrap(); // the later Bomb grants its own escape
    assert_ne!(branch.character().position, game.character().position);
    assert_eq!(game, sibling);
    assert_ne!(game, initial_clone);
}

#[test]
fn lingering_blasts_trigger_new_bombs_and_expire_independently() {
    let rules = GameRules::default().with_bomb_parameters(2, 2, 4).unwrap();
    let mut game = game(".+.", point(128, 128), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    for _ in 2..=3 {
        game.step(&[]).unwrap();
    }
    assert!(game.bomb().is_none());
    assert_eq!(game.blasts()[0].cells(), &[cell(0, 0), cell(1, 0)]);
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Floor));
    game.step(&[actions(None, true)]).unwrap(); // tick 4: capacity freed, old Blast triggers this Bomb
    assert!(game.bomb().is_none());
    assert_eq!(game.blasts().len(), 2);
    assert_eq!(game.blasts()[0].cells(), &[cell(0, 0), cell(1, 0)]); // coverage never expands
    assert_eq!(
        game.blasts()[1].cells(),
        &[cell(0, 0), cell(1, 0), cell(2, 0)]
    );
    assert_eq!(game.blasts()[1].expires_at(), 8);
    for _ in 5..=6 {
        game.step(&[]).unwrap();
    }
    game.step(&[]).unwrap();
    assert_eq!(game.tick(), 7);
    assert_eq!(game.blasts().len(), 1);
    assert_eq!(game.blasts()[0].expires_at(), 8);
    game.step(&[]).unwrap();
    assert!(game.blasts().is_empty());
    game.step(&[actions(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at(), 11); // a later Bomb gets a fresh fuse/permission
}

#[test]
fn permission_ends_only_when_the_whole_footprint_leaves_the_bomb_cell() {
    let mut game = game("....", point(384, 128), GameRules::default(), 0);
    game.step(&[actions(None, true)]).unwrap();
    for _ in 0..13 {
        game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(176, 128)); // body still overlaps by 16
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(game.character().position, point(192, 128)); // permission has not ended
    for _ in 0..2 {
        game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(160, 128)); // exact edge: fully clear
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(game.character().position, point(160, 128)); // re-entry is blocked
    let bomb = *game.bomb().unwrap();
    game.step(&[actions(Some(Direction::Left), true)]).unwrap(); // full capacity is ordinary gameplay
    assert_eq!(game.character().position, point(144, 128));
    assert_eq!(game.bomb(), Some(&bomb));
    assert_eq!(game.tick(), 19);
}

#[test]
fn bomb_parameters_are_validated_and_drive_propagation_and_deadlines() {
    let defaults = GameRules::default();
    assert_eq!(
        (
            defaults.bomb_fuse_ticks(),
            defaults.blast_reach(),
            defaults.blast_lifetime_ticks()
        ),
        (120, 1, 30)
    );
    assert_eq!(
        defaults.with_bomb_parameters(0, 1, 30),
        Err(ConstructionError::InvalidBombFuse)
    );
    assert_eq!(
        defaults.with_bomb_parameters(1, 0, 30),
        Err(ConstructionError::InvalidBlastReach)
    );
    assert_eq!(
        defaults.with_bomb_parameters(1, 1, 0),
        Err(ConstructionError::InvalidBlastLifetime)
    );
    let rules = defaults.with_bomb_parameters(2, 3, 4).unwrap();
    let mut game = game(
        "...#...\n.......\n...+...\n.......\n.......",
        point(896, 896),
        rules,
        9,
    );
    game.step(&[actions(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at(), 12);
    game.step(&[]).unwrap();
    assert!(game.bomb().is_some());
    game.step(&[]).unwrap();
    assert_eq!(game.blasts()[0].expires_at(), 16);
    let mut cells = game.blasts()[0].cells().to_vec();
    cells.sort_by_key(|cell| (cell.row().value(), cell.column().value()));
    assert_eq!(
        cells,
        [
            cell(3, 2),
            cell(0, 3),
            cell(1, 3),
            cell(2, 3),
            cell(3, 3),
            cell(4, 3),
            cell(5, 3),
            cell(6, 3),
            cell(3, 4)
        ]
    );
    assert_eq!(game.arena().terrain_at(cell(3, 2)), Some(Terrain::Floor));
    assert_eq!(game.arena().terrain_at(cell(3, 0)), Some(Terrain::Wall));
    for _ in 13..=15 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.blasts().len(), 1);
    game.step(&[]).unwrap();
    assert!(game.blasts().is_empty());
}

#[test]
fn detonation_and_expiry_follow_absolute_tick_deadlines() {
    let mut game = game("...\n...\n...", point(384, 384), GameRules::default(), 9);
    game.step(&[actions(None, true)]).unwrap();
    assert_eq!(game.tick(), 10);
    assert_eq!(game.bomb().unwrap().detonate_at(), 130);
    for _ in 11..=129 {
        game.step(&[]).unwrap();
        assert!(game.bomb().is_some());
        assert!(game.blasts().is_empty());
    }
    game.step(&[]).unwrap();
    assert_eq!(game.tick(), 130);
    assert!(game.bomb().is_none());
    assert_eq!(game.blasts().len(), 1);
    assert_eq!(game.blasts()[0].origin(), cell(1, 1));
    assert_eq!(game.blasts()[0].expires_at(), 160);
    let mut coverage = game.blasts()[0].cells().to_vec();
    coverage.sort_by_key(|cell| (cell.row().value(), cell.column().value()));
    assert_eq!(
        coverage,
        [cell(1, 0), cell(0, 1), cell(1, 1), cell(2, 1), cell(1, 2)]
    );
    for _ in 131..=159 {
        game.step(&[]).unwrap();
        assert_eq!(game.blasts().len(), 1);
    }
    game.step(&[]).unwrap();
    assert_eq!(game.tick(), 160);
    assert!(game.blasts().is_empty());
}

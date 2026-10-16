use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, Column, ConstructionError, Direction,
    Game, GameRules, Outcome, Position, Row, Scenario, StepError, Terrain, WorldX, WorldY,
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
fn a_new_blast_kills_in_the_final_centre_cell_and_freezes_the_complete_run() {
    let rules = GameRules::default().with_bomb_parameters(1, 1, 30).unwrap();
    let mut game = game(".+.", point(128, 128), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    let mut sibling = game.clone();
    game.step(&[]).unwrap();
    sibling.step(&[]).unwrap();
    assert_eq!(game, sibling);
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
    assert_eq!(game.tick(), 2);
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Floor));
    assert_eq!(game.blasts().len(), 1);
    let defeated = game.clone();
    for _ in 0..100 {
        game.step(&[actions(Some(Direction::Right), true)]).unwrap();
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
        game.step(&[actions(None, true)]).unwrap();
        game.step(&[actions(Some(direction), false)]).unwrap();
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
        game.step(&[actions(None, true)]).unwrap();
        game.step(&[actions(Some(Direction::Up), false)]).unwrap();
        assert_eq!(game.character().position, point(640, 96));
        assert_eq!(game.outcome(), None); // clear before detonation
        let mut sibling = game.clone();
        game.step(&[actions(Some(Direction::Down), true)]).unwrap();
        sibling
            .step(&[actions(Some(Direction::Down), true)])
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
                game.step(&[actions(Some(Direction::Up), true)]).unwrap();
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
        game.step(&[actions(None, true)]).unwrap();
        for _ in 0..4 {
            game.step(&[actions(Some(escape), false)]).unwrap();
        }
        assert_eq!(game.outcome(), None); // centre clear before the tick-5 explosion
        // Each Blast arm spans 256..1024 on its movement axis. Moving back
        // one step ends exactly at 256 or 1024. Only the lower edge is inside.
        game.step(&[actions(Some(returning), false)]).unwrap();
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
            game.step(&[actions(Some(returning), false)]).unwrap();
            assert_eq!(game.outcome(), Some(Outcome::Defeat));
        }
    }
}

#[test]
fn a_tick_130_blast_is_dangerous_on_159_but_not_during_movement_on_160() {
    let rules = GameRules::new(192, 480).unwrap();
    let mut game = game(
        ".....\n.....\n.....\n.....\n.....",
        point(640, 640),
        rules,
        9,
    );
    game.step(&[actions(None, true)]).unwrap(); // placed at 10, detonates at 130
    game.step(&[actions(Some(Direction::Up), false)]).unwrap();
    while game.tick() < 158 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.outcome(), None);
    assert_eq!(game.blasts()[0].expires_at(), 160);
    assert_eq!(game.character().position, point(640, 160)); // touches the upper arm's edge
    let mut on_159 = game.clone();
    on_159
        .step(&[actions(Some(Direction::Down), false)])
        .unwrap();
    assert_eq!(on_159.tick(), 159);
    assert_eq!(on_159.outcome(), Some(Outcome::Defeat));
    game.step(&[]).unwrap(); // stationary exact-edge contact remains safe on tick 159
    game.step(&[actions(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.tick(), 160);
    assert_eq!(game.character().position, point(640, 640));
    assert_eq!(game.outcome(), None);
    assert!(game.blasts().is_empty());
}

#[test]
fn a_diagonal_centre_cell_is_safe_despite_nearby_blast_arms() {
    let rules = GameRules::new(192, 224)
        .unwrap()
        .with_bomb_parameters(2, 1, 30)
        .unwrap();
    let mut game = game("...\n...\n...", point(384, 384), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    game.step(&[actions(Some(Direction::Up), false)]).unwrap();
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
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
    game.step(&[actions(None, true)]).unwrap(); // Bomb at (3,0)
    for _ in 0..5 {
        game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    }
    for _ in 0..2 {
        game.step(&[actions(Some(Direction::Down), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(513, 270));
    assert_eq!(game.outcome(), None);
    // The Blast includes (2,0): [512,768] x [0,256], not (2,1).
    // Left is blocked by (1,1); correction goes UP 15, then LEFT 49:
    // (513,270) -> (513,255) -> (464,255). The bend's centre enters danger.
    // Both tick endpoints are safe. The endpoint chord reaches y<256 only
    // after x<468, already left of danger, so a diagonal check misses it.
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(464, 255));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn placement_uses_the_post_movement_cell_without_snapping() {
    let mut game = game("...", point(248, 128), GameRules::default(), 0);
    game.step(&[actions(Some(Direction::Right), true)]).unwrap();
    assert_eq!(game.character().position, point(264, 128));
    assert_eq!(game.bomb().unwrap().cell(), cell(1, 0));
    let bomb = *game.bomb().unwrap();
    game.step(&[actions(Some(Direction::Left), true)]).unwrap();
    assert_eq!(game.character().position, point(248, 128));
    assert_eq!(game.bomb(), Some(&bomb));
}

#[test]
fn escape_never_approaches_the_bomb_even_if_the_endpoint_would_be_farther_away() {
    for assisted in [false, true] {
        // Bomb centre is (384,128), so crossing it to x=144 would finish
        // farther away than the starting x=400, but initially approaches it.
        let rules = GameRules::new(192, 256)
            .unwrap()
            .with_turn_assistance(assisted);
        let mut game = game("....", point(400, 128), rules, 0);
        game.step(&[actions(None, true)]).unwrap();
        let bomb = *game.bomb().unwrap();
        game.step(&[actions(Some(Direction::Left), false)]).unwrap();
        assert_eq!(game.character().position, point(400, 128));
        assert_eq!(game.bomb(), Some(&bomb));
        assert_eq!(game.tick(), 2); // blocked movement is still an ordinary tick
        assert_eq!(game.outcome(), None);
    }
}

#[test]
fn overlapping_escape_allows_perpendicular_travel_but_not_a_turn_toward_the_centre() {
    // Each start is 16 units beside (640,640). Perpendicular travel increases
    // distance without needing a positive initial component in that direction.
    for (start, toward, perpendicular, returning, expected) in [
        (
            point(656, 640),
            Direction::Left,
            Direction::Up,
            Direction::Down,
            point(656, 624),
        ),
        (
            point(640, 656),
            Direction::Up,
            Direction::Right,
            Direction::Left,
            point(656, 656),
        ),
        (
            point(624, 640),
            Direction::Right,
            Direction::Down,
            Direction::Up,
            point(624, 656),
        ),
        (
            point(640, 624),
            Direction::Down,
            Direction::Left,
            Direction::Right,
            point(624, 624),
        ),
    ] {
        for assisted in [false, true] {
            let mut game = game(
                ".....\n.....\n.....\n.....\n.....",
                start,
                GameRules::default().with_turn_assistance(assisted),
                0,
            );
            game.step(&[actions(None, true)]).unwrap();
            let mut sibling = game.clone(); // clone while permission still applies
            for (direction, position) in [
                (toward, start),
                (perpendicular, expected),
                (returning, expected),
            ] {
                let frame = [actions(Some(direction), false)];
                game.step(&frame).unwrap();
                sibling.step(&frame).unwrap();
                assert_eq!(game, sibling);
                assert_eq!(game.character().position, position);
                assert_eq!(game.bomb().unwrap().cell(), cell(2, 2));
            }
        }
    }
}

#[test]
fn assistance_cannot_approach_during_its_correction_even_with_a_farther_endpoint() {
    let mut game = game(
        "....\n.#..\n....\n....",
        point(608, 528),
        GameRules::new(192, 256).unwrap(),
        0,
    );
    game.step(&[actions(None, true)]).unwrap(); // centre (640,640)
    // Left is blocked by the Wall. Without the escape constraint, assistance
    // would go DOWN 80 then LEFT 176 to (432,608), farther from the Bomb,
    // but its first segment approaches. The upward route is beyond 128 units.
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(608, 528));
    assert_eq!(game.tick(), 2);
}

#[test]
fn assistance_can_turn_toward_the_bomb_only_after_its_correction_fully_clears_it() {
    let mut game = game(
        "....\n....\n....",
        point(767, 270),
        GameRules::new(2, 64).unwrap(),
        0,
    );
    game.step(&[actions(None, true)]).unwrap(); // centre (640,384)
    // LEFT initially approaches, but UP 15 reaches y=255, with the body's
    // bottom exactly at the Bomb's top. Permission ends at that bend, so the
    // remaining LEFT 49 is legal along the edge, although distance decreases.
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(718, 255));
    assert_eq!(game.outcome(), None);
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(654, 255));
}

#[test]
fn removal_discards_old_escape_constraints_and_a_replacement_gets_its_own_permission() {
    let rules = GameRules::default()
        .with_turn_assistance(false)
        .with_bomb_parameters(4, 1, 1)
        .unwrap();
    let mut game = game("....\n....\n....\n....", point(520, 520), rules, 0);
    game.step(&[actions(None, true)]).unwrap(); // Bomb at (2,2)
    game.step(&[actions(Some(Direction::Up), false)]).unwrap();
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(504, 504));
    // The Footprint still overlaps the Bomb, but the diagonal centre Cell is safe.
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert!(game.bomb().is_none());
    assert_eq!(game.outcome(), None);
    game.step(&[actions(Some(Direction::Down), true)]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.character().position, point(504, 520));
    assert_eq!(game.bomb().unwrap().cell(), cell(1, 2));
    game.step(&[actions(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.character().position, point(504, 520)); // toward new centre: blocked
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(game.character().position, point(520, 520)); // old Bomb Cell is no obstacle
    assert_eq!(game.outcome(), None);
}

#[test]
fn expired_blasts_cannot_trigger_a_bomb_placed_on_the_expiry_tick() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 4)
        .unwrap();
    let mut game = game(".....", point(128, 128), rules, 0);
    game.step(&[actions(None, true)]).unwrap();
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    for _ in 3..=6 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.blasts()[0].expires_at(), 7);
    game.step(&[actions(Some(Direction::Left), true)]).unwrap();
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

    // Escape each explosion. The rejected final step would move, expire the
    // remaining Blast, revoke escape permission and detonate a Bomb, but its
    // new expiry cannot be represented. All of those changes must roll back.
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 3)
        .unwrap();
    let mut effects = game(
        ".....\n.....\n.....\n.....\n.....",
        point(640, 640),
        rules,
        u64::MAX - 7,
    );
    for command in [
        actions(None, true),
        actions(Some(Direction::Up), false),
        actions(None, true),
        actions(Some(Direction::Right), false),
        actions(None, false),
        actions(None, true),
    ] {
        effects.step(&[command]).unwrap();
    }
    assert_eq!(effects.outcome(), None);
    assert_eq!(effects.tick(), u64::MAX - 1);
    assert_eq!(effects.blasts().len(), 1);
    assert_eq!(effects.blasts()[0].expires_at(), u64::MAX);
    let before = effects.clone();
    assert_eq!(
        effects.step(&[actions(Some(Direction::Left), true)]),
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
        vec![actions(Some(Direction::Right), true)],
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
fn lingering_blasts_trigger_new_bombs_without_expanding_old_coverage() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(2, 2, 4)
        .unwrap();
    let mut game = game(".+...\n.....\n.....", point(128, 128), rules, 0);
    for command in [
        actions(None, true),
        actions(Some(Direction::Down), false),
        actions(Some(Direction::Right), false),
    ] {
        game.step(&[command]).unwrap();
    }
    assert_eq!(game.outcome(), None);
    assert!(game.bomb().is_none());
    let old = game.blasts()[0].clone();
    assert_eq!(
        old.cells(),
        &[cell(0, 0), cell(1, 0), cell(0, 1), cell(0, 2)]
    );
    assert_eq!(game.arena().terrain_at(cell(1, 0)), Some(Terrain::Floor));
    game.step(&[actions(Some(Direction::Up), true)]).unwrap();
    assert!(game.bomb().is_none()); // old Blast triggers the newly placed Bomb
    assert_eq!(game.blasts().len(), 2);
    assert_eq!(game.blasts()[0], old); // destroyed Block never extends old coverage
    assert!(game.blasts()[1].cells().contains(&cell(3, 0)));
    assert_eq!(game.blasts()[1].expires_at(), 8);
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
        actions(None, true),
        actions(Some(Direction::Up), false),
        actions(None, true),
        actions(Some(Direction::Right), false),
    ] {
        game.step(&[command]).unwrap();
    }
    assert_eq!(game.blasts().len(), 2);
    assert!(
        game.blasts()
            .iter()
            .all(|blast| blast.cells().contains(&cell(2, 1)))
    );
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert_eq!(game.tick(), 6);
    assert_eq!(game.blasts().len(), 1);
    assert_eq!(game.blasts()[0].expires_at(), 8);
    assert!(game.blasts()[0].cells().contains(&cell(2, 1)));
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.outcome(), None);
    game.step(&[actions(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at(), 10);
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
    assert_eq!(game.character().position, point(176, 128)); // approach blocked despite permission
    game.step(&[actions(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(160, 128)); // exact edge: fully clear
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(game.character().position, point(160, 128)); // re-entry is blocked
    let bomb = *game.bomb().unwrap();
    game.step(&[actions(Some(Direction::Left), true)]).unwrap(); // full capacity is ordinary gameplay
    assert_eq!(game.character().position, point(144, 128));
    assert_eq!(game.bomb(), Some(&bomb));
    assert_eq!(game.tick(), 18);
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
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 3, 4)
        .unwrap();
    let mut game = game(
        "...#...\n.......\n...+...\n.......\n.......",
        point(896, 896),
        rules,
        9,
    );
    game.step(&[actions(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at(), 12);
    game.step(&[actions(Some(Direction::Right), false)])
        .unwrap();
    assert!(game.bomb().is_some());
    game.step(&[actions(Some(Direction::Up), false)]).unwrap();
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
    for tick in 11..=129 {
        let movement = match tick {
            11..=26 => Some(Direction::Right),
            27..=42 => Some(Direction::Up),
            _ => None,
        };
        game.step(&[actions(movement, false)]).unwrap();
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

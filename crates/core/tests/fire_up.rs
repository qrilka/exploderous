use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, CharacterKind, Column, ConstructionError,
    ControlSource, Direction, Game, GameRules, Outcome, Position, Row, Scenario, StepError,
    Terrain, WorldX, WorldY,
};

fn cell(column: u32, row: u32) -> CellCoord {
    CellCoord::new(Column::new(column), Row::new(row))
}

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

fn scenario(layout: &str, position: Position, rules: GameRules) -> Scenario {
    let rows: Vec<_> = layout.lines().collect();
    assert!(rows.iter().all(|row| row.len() == rows[0].len()));
    Scenario::new(
        rows[0].len() as u32,
        rows.len() as u32,
        rows.iter()
            .flat_map(|row| row.chars())
            .map(|glyph| match glyph {
                '.' => Terrain::Floor,
                '#' => Terrain::Wall,
                '+' => Terrain::Block,
                _ => panic!("unknown terrain"),
            })
            .collect(),
        Character {
            id: CharacterId::new(0),
            position,
            kind: CharacterKind::Bomber,
            control: ControlSource::Human,
        },
        rules,
    )
    .unwrap()
}

fn action(movement: Option<Direction>, place_bomb: bool) -> CharacterCommand {
    CharacterCommand {
        character: CharacterId::new(0),
        movement,
        place_bomb,
    }
}

#[test]
fn fire_up_construction_accepts_floor_or_block_and_rejects_walls_and_outside_cells() {
    let initial = scenario(".#+\n...", point(128, 384), GameRules::default());
    assert_eq!(Game::new(initial.clone(), 0).fire_up(), None);
    for location in [cell(0, 0), cell(2, 0)] {
        let game = Game::new(initial.clone().with_fire_up(location).unwrap(), 0);
        assert_eq!(game.fire_up(), Some(location));
        assert_eq!(game.current_blast_reach(), 1);
    }
    for location in [cell(1, 0), cell(3, 0), cell(0, 2), cell(u32::MAX, u32::MAX)] {
        assert_eq!(
            initial.clone().with_fire_up(location),
            Err(ConstructionError::InvalidFireUpLocation)
        );
    }
}

#[test]
fn collection_uses_the_centre_cell_and_precedes_same_tick_placement_and_cardinal_coverage() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(3, 1, 4)
        .unwrap();
    let initial = scenario(
        ".......\n.......\n...#...\n....+..\n.......\n.......\n.......",
        point(640, 896),
        rules,
    )
    .with_fire_up(cell(3, 3))
    .unwrap();
    let mut game = Game::new(initial, 0);
    game.step(&[action(Some(Direction::Right), true)]).unwrap();
    assert_eq!(game.fire_up(), None);
    assert_eq!(game.current_blast_reach(), 2);
    assert_eq!(game.rules().blast_reach(), 1);
    assert_eq!(game.bomb().unwrap().blast_reach(), 2);
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    game.step(&[]).unwrap();
    assert_eq!(game.outcome(), None);
    let cells = game.blasts()[0].cells();
    assert_eq!(cells.len(), 6);
    for expected in [
        cell(3, 3),
        cell(4, 3),
        cell(3, 4),
        cell(3, 5),
        cell(2, 3),
        cell(1, 3),
    ] {
        assert!(cells.contains(&expected), "missing {expected:?}");
    }
    assert_eq!(game.arena().terrain_at(cell(4, 3)), Some(Terrain::Floor));
    assert_eq!(game.arena().terrain_at(cell(3, 2)), Some(Terrain::Wall));
}

#[test]
fn revealing_effect_preserves_fire_up_but_a_distinct_effect_destroys_it_while_both_linger() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(2, 1, 10)
        .unwrap();
    let initial = scenario(".....\n...+.\n.....\n.....\n.....", point(640, 384), rules)
        .with_fire_up(cell(3, 1))
        .unwrap();
    let mut game = Game::new(initial.clone(), 0);
    let fresh = game.clone();
    let mut blocked = game.clone();
    blocked
        .step(&[action(Some(Direction::Right), false)])
        .unwrap();
    assert_eq!(blocked.character().position, point(672, 384));
    assert_eq!(blocked.fire_up(), Some(cell(3, 1)));
    assert_eq!(blocked.current_blast_reach(), 1);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.outcome(), None);
    assert_eq!(game.arena().terrain_at(cell(3, 1)), Some(Terrain::Floor));
    assert_eq!(game.fire_up(), Some(cell(3, 1)));
    assert_eq!(game.blasts()[0].expires_at(), 13);
    let mut untouched = game.clone();
    for _ in 0..10 {
        untouched.step(&[]).unwrap();
        assert_eq!(untouched.fire_up(), Some(cell(3, 1)));
    }
    assert!(untouched.blasts().is_empty());
    // Go around the first effect to place a second Bomb below the exposed Item.
    for direction in [
        Direction::Down,
        Direction::Right,
        Direction::Right,
        Direction::Up,
    ] {
        game.step(&[action(Some(direction), direction == Direction::Up)])
            .unwrap();
        assert_eq!(game.outcome(), None);
    }
    let mut harmed = game.clone();
    harmed.step(&[action(Some(Direction::Up), false)]).unwrap();
    assert_eq!(
        harmed.arena().cell_containing(harmed.character().position),
        Some(cell(3, 1))
    );
    assert_eq!(harmed.outcome(), Some(Outcome::Defeat)); // Item protection is not player protection
    let mut sibling = game.clone();
    for run in [&mut game, &mut sibling] {
        run.step(&[action(Some(Direction::Right), false)]).unwrap();
        run.step(&[action(Some(Direction::Down), false)]).unwrap();
        assert_eq!(run.tick(), 9);
        assert_eq!(run.outcome(), None);
        assert_eq!(run.blasts().len(), 2);
        assert!(
            run.blasts()
                .iter()
                .all(|blast| blast.cells().contains(&cell(3, 1)))
        );
        assert_eq!(run.fire_up(), None);
        assert_eq!(run.current_blast_reach(), 1);
    }
    assert_eq!(game, sibling);
    assert_eq!(Game::new(initial, 0), fresh);
}

#[test]
fn footprint_overlap_does_not_collect_and_half_open_centre_entry_does() {
    for (x, direction, steps) in [(240, Direction::Right, 1), (528, Direction::Left, 2)] {
        let initial = scenario("...\n...\n...", point(x, 384), GameRules::default())
            .with_fire_up(cell(1, 1))
            .unwrap();
        let mut game = Game::new(initial, 0);
        game.step(&[]).unwrap();
        assert_eq!(game.fire_up(), Some(cell(1, 1)));
        for step in 1..=steps {
            game.step(&[action(Some(direction), false)]).unwrap();
            assert_eq!(game.fire_up().is_none(), step == steps);
            assert_eq!(
                game.current_blast_reach(),
                if step == steps { 2 } else { 1 }
            );
        }
    }
}

#[test]
fn ticking_bombs_keep_their_reach_but_later_bombs_gain_two_cells_in_all_four_directions() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(4, 1, 2)
        .unwrap();
    let initial = scenario(
        ".........\n.........\n.........\n.........\n.........\n.........\n.........",
        point(1152, 896),
        rules,
    )
    .with_fire_up(cell(5, 3))
    .unwrap();
    let mut game = Game::new(initial.clone(), 0);
    let fresh = game.clone();
    game.step(&[action(None, true)]).unwrap();
    let mut sibling = game.clone();
    for run in [&mut game, &mut sibling] {
        run.step(&[action(Some(Direction::Right), false)]).unwrap();
        assert_eq!(run.fire_up(), None);
        assert_eq!(run.current_blast_reach(), 2);
        assert_eq!(run.bomb().unwrap().blast_reach(), 1);
        run.step(&[action(Some(Direction::Down), false)]).unwrap();
        run.step(&[action(Some(Direction::Right), false)]).unwrap();
        run.step(&[]).unwrap();
        let cells = run.blasts()[0].cells();
        assert_eq!(cells.len(), 5);
        for expected in [cell(4, 3), cell(4, 2), cell(5, 3), cell(4, 4), cell(3, 3)] {
            assert!(cells.contains(&expected));
        }
        run.step(&[action(None, true)]).unwrap();
        assert_eq!(run.bomb().unwrap().blast_reach(), 2);
        for direction in [
            Direction::Down,
            Direction::Left,
            Direction::Left,
            Direction::Down,
        ] {
            run.step(&[action(Some(direction), false)]).unwrap();
        }
        assert_eq!(run.outcome(), None);
        let cells = run.blasts()[0].cells();
        assert_eq!(cells.len(), 9);
        for expected in [
            cell(6, 4),
            cell(6, 3),
            cell(6, 2),
            cell(7, 4),
            cell(8, 4),
            cell(6, 5),
            cell(6, 6),
            cell(5, 4),
            cell(4, 4),
        ] {
            assert!(cells.contains(&expected));
        }
    }
    assert_eq!(game, sibling);
    sibling
        .step(&[action(Some(Direction::Left), false)])
        .unwrap();
    assert_ne!(game, sibling);
    assert_eq!(Game::new(initial, 0), fresh);
}

#[test]
fn malformed_frames_and_both_deadline_overflows_cannot_partially_collect_or_upgrade() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(2, 1, 3)
        .unwrap();
    let initial = scenario(".....\n.....\n.....", point(384, 384), rules)
        .with_fire_up(cell(2, 1))
        .unwrap();
    let mut game = Game::new(initial.clone(), 0);
    let collect = action(Some(Direction::Right), true);
    let unknown = CharacterCommand {
        character: CharacterId::new(1),
        ..collect
    };
    for (frame, error) in [
        (vec![collect, collect], StepError::DuplicateCommand),
        (vec![unknown], StepError::UnknownCharacter),
    ] {
        let before = game.clone();
        assert_eq!(game.step(&frame), Err(error));
        assert_eq!(game, before);
    }
    let mut game = Game::new(initial.clone(), u64::MAX - 1);
    let before = game.clone();
    assert_eq!(game.step(&[collect]), Err(StepError::TickExhausted));
    assert_eq!(game, before); // collection and placement would both happen without fuse overflow
    let mut game = Game::new(initial, u64::MAX - 3);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[]).unwrap();
    let before = game.clone();
    assert_eq!(
        game.step(&[action(Some(Direction::Right), false)]),
        Err(StepError::TickExhausted)
    );
    assert_eq!(game, before); // includes Item, old Bomb, permission and ability
}

#[test]
fn the_maximum_base_reach_can_be_upgraded_without_wrapping_or_mutating_rules() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, u32::MAX, 1)
        .unwrap();
    let mut game = Game::new(
        scenario(".....\n.....\n.....", point(128, 128), rules)
            .with_fire_up(cell(0, 0))
            .unwrap(),
        0,
    );
    game.step(&[action(None, true)]).unwrap();
    assert_eq!(game.current_blast_reach(), u64::from(u32::MAX) + 1);
    assert_eq!(game.bomb().unwrap().blast_reach(), u64::from(u32::MAX) + 1);
    assert_eq!(game.rules().blast_reach(), u32::MAX);
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.blasts()[0].cells().len(), 7); // whole first row and column, bounded by Arena
}

#[test]
fn the_exit_does_not_require_collecting_or_preserving_fire_up() {
    let rules = GameRules::new(192, 256)
        .unwrap()
        .with_bomb_parameters(2, 1, 1)
        .unwrap();
    let initial = scenario(".....\n.....\n.....\n.....\n.....", point(640, 384), rules)
        .with_fire_up(cell(3, 1))
        .unwrap()
        .with_exit(cell(1, 2))
        .unwrap();
    let mut game = Game::new(initial, 0);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.fire_up(), None); // exposed Item destroyed, Exit still works
    assert_eq!(game.outcome(), Some(Outcome::Victory));
    let mut game = Game::new(
        scenario(
            "...\n...",
            point(128, 384),
            GameRules::new(192, 256).unwrap(),
        )
        .with_fire_up(cell(2, 0))
        .unwrap()
        .with_exit(cell(1, 1))
        .unwrap(),
        0,
    );
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.outcome(), Some(Outcome::Victory));
    assert_eq!(game.fire_up(), Some(cell(2, 0))); // uncollected Item is optional too
}

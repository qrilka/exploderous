use super::*;

#[test]
fn malformed_frames_and_deadline_overflow_preserve_complete_state() {
    let mut placement = game("...", point(128, 128), GameRules::default(), u64::MAX - 2);
    let before = placement.clone();
    assert_eq!(
        placement.step(&[action(Some(Direction::Right), true)]),
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
        action(None, true),
        action(Some(Direction::Up), false),
        action(None, true),
        action(Some(Direction::Right), false),
        action(None, false),
        action(None, true),
    ] {
        effects.step(&[command]).unwrap();
    }
    assert_eq!(effects.outcome(), None);
    assert_eq!(effects.tick().value(), u64::MAX - 1);
    assert_eq!(effects.blasts().len(), 1);
    assert_eq!(effects.blasts()[0].expires_at().value(), u64::MAX);
    let before = effects.clone();
    assert_eq!(
        effects.step(&[action(Some(Direction::Left), true)]),
        Err(StepError::TickExhausted)
    );
    assert_eq!(effects, before);
    let duplicate = [action(None, true), action(Some(Direction::Right), false)];
    assert_eq!(effects.step(&duplicate), Err(StepError::DuplicateCommand));
    assert_eq!(effects, before);
    let unknown = CharacterCommand {
        character: CharacterId::new(7),
        ..action(None, true)
    };
    assert_eq!(effects.step(&[unknown]), Err(StepError::UnknownCharacter));
    assert_eq!(effects, before);

    // At capacity there is no new fuse deadline to check.
    let rules = GameRules::default()
        .with_bomb_parameters(u64::MAX - 1, 1, 1)
        .unwrap();
    let mut full = game("...", point(128, 128), rules, 0);
    full.step(&[action(None, true)]).unwrap();
    full.step(&[action(Some(Direction::Right), true)]).unwrap();
    assert_eq!(full.tick().value(), 2);
    assert_eq!(full.character().position, point(144, 128));
    assert_eq!(full.bomb().unwrap().detonate_at().value(), u64::MAX);
}

#[test]
fn bomb_parameters_are_validated_and_drive_propagation_and_deadlines() {
    let defaults = GameRules::default();
    assert_eq!(
        (
            defaults.bomb_fuse_ticks().value(),
            defaults.blast_reach(),
            defaults.blast_lifetime_ticks().value()
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
    game.step(&[action(None, true)]).unwrap();
    assert_eq!(game.bomb().unwrap().detonate_at().value(), 12);
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert!(game.bomb().is_some());
    game.step(&[action(Some(Direction::Up), false)]).unwrap();
    assert_eq!(game.blasts()[0].expires_at().value(), 16);
    let mut cells = game.blasts()[0].cells().collect::<Vec<_>>();
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
    game.step(&[action(None, true)]).unwrap();
    assert_eq!(game.tick(), crate::game::Tick::new(10));
    assert_eq!(
        game.bomb().unwrap().detonate_at(),
        crate::game::Tick::new(130)
    );
    for tick in 11..=129 {
        let movement = match tick {
            11..=26 => Some(Direction::Right),
            27..=42 => Some(Direction::Up),
            _ => None,
        };
        game.step(&[action(movement, false)]).unwrap();
        assert!(game.bomb().is_some());
        assert!(game.blasts().is_empty());
    }
    game.step(&[]).unwrap();
    assert_eq!(game.tick().value(), 130);
    assert!(game.bomb().is_none());
    assert_eq!(game.blasts().len(), 1);
    assert_eq!(game.blasts()[0].origin(), cell(1, 1));
    assert_eq!(game.blasts()[0].expires_at().value(), 160);
    let mut coverage = game.blasts()[0].cells().collect::<Vec<_>>();
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
    assert_eq!(game.tick().value(), 160);
    assert!(game.blasts().is_empty());
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
    game.step(&[action(None, true)]).unwrap(); // placed at 10, detonates at 130
    game.step(&[action(Some(Direction::Up), false)]).unwrap();
    // Stage both runs before the last dangerous tick to compare crossings before/after expiry.
    for tick in 12..=158 {
        game.step(&[]).unwrap();
        assert_eq!(game.tick().value(), tick);
    }
    assert_eq!(game.outcome(), None);
    assert_eq!(game.blasts()[0].expires_at().value(), 160);
    assert_eq!(game.character().position, point(640, 160)); // touches the upper arm's edge
    let mut on_159 = game.clone();
    on_159
        .step(&[action(Some(Direction::Down), false)])
        .unwrap();
    assert_eq!(on_159.tick().value(), 159);
    assert_eq!(on_159.outcome(), Some(Outcome::Defeat));
    game.step(&[]).unwrap(); // stationary exact-edge contact remains safe on tick 159
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.tick().value(), 160);
    assert_eq!(game.character().position, point(640, 640));
    assert_eq!(game.outcome(), None);
    assert!(game.blasts().is_empty());
}

#[test]
fn expired_blasts_cannot_trigger_a_bomb_placed_on_the_expiry_tick() {
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 4)
        .unwrap();
    let mut game = game(".....", point(128, 128), rules, 0);
    game.step(&[action(None, true)]).unwrap();
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    for _ in 3..=6 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.blasts()[0].expires_at().value(), 7);
    game.step(&[action(Some(Direction::Left), true)]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.bomb().unwrap().detonate_at().value(), 9);
}

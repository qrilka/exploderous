use super::support::{action, bomber, cell, enemy, point};
use crate::game::{
    BuroruStart, Character, CharacterCommand, CharacterId, Game, GameRules, Outcome, Position,
    Scenario, Terrain,
};

fn scenario(position: Position, rules: GameRules, enemies: Vec<BuroruStart>) -> Scenario {
    Scenario::new(5, 3, vec![Terrain::Floor; 15], bomber(position), rules)
        .unwrap()
        .with_enemies(enemies)
        .unwrap()
}

#[test]
fn external_npc_commands_are_rejected_atomically_with_deterministic_priority() {
    use crate::game::{Direction, StepError};
    let mut game = Game::new(
        scenario(
            point(128, 128),
            GameRules::default(),
            vec![enemy(7, point(640, 128))],
        ),
        crate::game::Tick::new(0),
    );
    game.step(&[action(None, true)]).unwrap();
    let npc = CharacterCommand {
        character: CharacterId::new(7),
        movement: Some(Direction::Left),
        place_bomb: true,
    };
    let unknown = CharacterCommand {
        character: CharacterId::new(99),
        ..npc
    };
    for (frame, error) in [
        (vec![npc], StepError::NpcCommand),
        (vec![action(None, false), npc], StepError::NpcCommand),
        (vec![npc, npc], StepError::DuplicateCommand),
        (vec![npc, unknown], StepError::UnknownCharacter),
        (vec![unknown, npc], StepError::UnknownCharacter),
    ] {
        let before = game.clone();
        assert_eq!(game.step(&frame), Err(error));
        assert_eq!(game, before);
    }
}

#[test]
fn contact_uses_half_open_centre_cells_and_the_whole_travelled_path() {
    use crate::game::Direction::{Down, Left, Right, Up};
    for (start, target, direction, expected, outcome) in [
        (
            point(240, 128),
            point(384, 128),
            Right,
            point(256, 128),
            Some(Outcome::Defeat),
        ),
        (
            point(528, 128),
            point(384, 128),
            Left,
            point(512, 128),
            None,
        ),
        (
            point(512, 128),
            point(384, 128),
            Left,
            point(496, 128),
            Some(Outcome::Defeat),
        ),
        (
            point(128, 240),
            point(128, 384),
            Down,
            point(128, 256),
            Some(Outcome::Defeat),
        ),
        (point(128, 528), point(128, 384), Up, point(128, 512), None),
        (
            point(128, 512),
            point(128, 384),
            Up,
            point(128, 496),
            Some(Outcome::Defeat),
        ),
    ] {
        let mut game = Game::new(
            scenario(start, GameRules::default(), vec![enemy(7, target)]),
            crate::game::Tick::new(0),
        );
        game.step(&[action(Some(direction), false)]).unwrap();
        assert_eq!(game.character().position, expected);
        assert_eq!(game.outcome(), outcome);
    }
    // Bodies overlap, but adjacent centre Cells do not cause harm while idle.
    let mut adjacent = Game::new(
        scenario(
            point(255, 128),
            GameRules::default(),
            vec![enemy(7, point(384, 128))],
        ),
        crate::game::Tick::new(0),
    );
    adjacent.step(&[]).unwrap();
    assert_eq!(adjacent.outcome(), None);
    let mut crossing = Game::new(
        scenario(
            point(128, 128),
            GameRules::new(192, 1024).unwrap(),
            vec![enemy(7, point(640, 128))],
        ),
        crate::game::Tick::new(0),
    );
    crossing.step(&[action(Some(Right), true)]).unwrap();
    assert_eq!(crossing.character().position, point(1152, 128));
    assert_eq!(crossing.outcome(), Some(Outcome::Defeat));
    let frozen = crossing.clone();
    assert!(frozen.bomb().is_some());
    assert_eq!(frozen.enemies().len(), 1);
    for _ in 0..40 {
        crossing.step(&[action(Some(Left), true)]).unwrap();
    }
    assert_eq!(crossing, frozen);
}

#[test]
fn turn_assistance_contact_checks_the_bend_not_the_endpoint_chord() {
    use crate::game::Direction::Left;
    let mut terrain = vec![Terrain::Floor; 15];
    terrain[6] = Terrain::Block; // (1,1) forces an upward correction.
    let initial = Scenario::new(
        5,
        3,
        terrain,
        bomber(point(513, 270)),
        GameRules::new(2, 64).unwrap(),
    )
    .unwrap()
    .with_enemies(vec![enemy(7, point(640, 128))])
    .unwrap();
    let mut game = Game::new(initial, crate::game::Tick::new(0));
    game.step(&[action(Some(Left), false)]).unwrap();
    // (513,270) -> (513,255) -> (464,255); only the actual bend is in (2,0).
    assert_eq!(game.character().position, point(464, 255));
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn one_hit_kills_preserve_unrelated_identities_and_clone_independence() {
    use crate::game::Direction::Left;
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 3)
        .unwrap();
    let initial = scenario(
        point(640, 384),
        rules,
        vec![
            enemy(7, point(896, 384)),
            enemy(42, point(896, 384)),
            enemy(99, point(1152, 640)),
        ],
    )
    .with_exit(cell(0, 1))
    .unwrap()
    .with_fire_up(cell(4, 2))
    .unwrap();
    let mut game = Game::new(initial.clone(), crate::game::Tick::new(0));
    game.step(&[action(None, true)]).unwrap();
    let mut sibling = game.clone();
    game.step(&[action(Some(Left), false)]).unwrap();
    assert_eq!(
        game.enemies().map(|enemy| enemy.id).collect::<Vec<_>>(),
        vec![CharacterId::new(99)]
    );
    assert_eq!(game.fire_up(), Some(cell(4, 2))); // Buroru cannot collect an exposed Item.
    assert_eq!(game.outcome(), None); // Exit still locked by the unrelated survivor.
    assert!(!game.exit_ready());
    assert_eq!(sibling.enemies().len(), 3);
    sibling.step(&[action(Some(Left), false)]).unwrap();
    assert_eq!(game, sibling);
    for _ in 0..3 {
        game.step(&[]).unwrap();
    }
    assert!(game.blasts().is_empty());
    assert_eq!(
        game.enemies().map(|enemy| enemy.id).collect::<Vec<_>>(),
        vec![CharacterId::new(99)]
    );
    assert_eq!(
        Game::new(initial, crate::game::Tick::new(0))
            .enemies()
            .len(),
        3
    );
}

#[test]
fn last_kill_wins_on_an_uncovered_exit_without_reentry_but_harm_takes_precedence() {
    use crate::game::Direction::Left;
    for (distance, exit, outcome) in [
        (512, Some(cell(0, 1)), Some(Outcome::Victory)),
        (512, Some(cell(0, 0)), None), // Clearing enemies elsewhere does not win.
        (512, None, None),
        (256, Some(cell(1, 1)), Some(Outcome::Defeat)),
    ] {
        let rules = GameRules::new(192, distance)
            .unwrap()
            .with_bomb_parameters(2, 1, 3)
            .unwrap();
        let mut initial = scenario(point(640, 384), rules, vec![enemy(7, point(896, 384))]);
        if let Some(exit) = exit {
            initial = initial.with_exit(exit).unwrap();
        }
        let mut game = Game::new(initial, crate::game::Tick::new(0));
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(Left), false)]).unwrap();
        assert_eq!(game.outcome(), None); // On the Exit already, but the enemy is alive.
        assert_eq!(game.enemies().len(), 1);
        let mut sibling = game.clone();
        game.step(&[]).unwrap(); // Stationary on the Exit as the last enemy dies.
        sibling.step(&[]).unwrap();
        assert_eq!(game, sibling);
        assert!(game.enemies().next().is_none());
        assert_eq!(game.outcome(), outcome);
        if outcome.is_some() {
            let frozen = game.clone();
            for _ in 0..40 {
                game.step(&[action(Some(Left), true)]).unwrap();
            }
            assert_eq!(game, frozen);
        }
    }
}

#[test]
fn contact_cannot_be_undone_by_killing_the_last_enemy_later_in_the_tick() {
    use crate::game::Direction::Right;
    let rules = GameRules::new(192, 512)
        .unwrap()
        .with_bomb_parameters(1, 1, 3)
        .unwrap();
    let initial = scenario(point(640, 384), rules, vec![enemy(7, point(896, 384))])
        .with_exit(cell(4, 1))
        .unwrap();
    let mut game = Game::new(initial, crate::game::Tick::new(0));
    game.step(&[action(None, true)]).unwrap();
    game.step(&[action(Some(Right), false)]).unwrap();
    assert_eq!(game.character().position, point(1152, 384));
    assert!(game.enemies().next().is_none());
    assert!(game.exit_ready());
    assert_eq!(game.outcome(), Some(Outcome::Defeat));
}

#[test]
fn new_blast_hits_use_moving_enemy_centres_not_overlapping_footprints() {
    use crate::game::Direction::Right;
    for (fuse, lifetime, dies) in [(14, 1, false), (14, 2, false), (15, 1, true)] {
        let rules = GameRules::new(192, 512)
            .unwrap()
            .with_bomb_parameters(fuse, 1, lifetime)
            .unwrap();
        let mut terrain = vec![Terrain::Wall; 15];
        terrain[5..10].fill(Terrain::Floor);
        let initial = Scenario::new(5, 3, terrain, bomber(point(640, 384)), rules)
            .unwrap()
            .with_enemies(vec![enemy(7, point(128, 384))])
            .unwrap();
        let mut game = Game::new(initial, crate::game::Tick::new(0));
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(Right), false)]).unwrap();
        for _ in 2..=fuse {
            game.step(&[]).unwrap();
        }
        assert_eq!(game.outcome(), None);
        assert_eq!(game.enemies().next().is_none(), dies);
        if !dies {
            assert_eq!(game.enemies().next().unwrap().position, point(248, 384));
            // Expiry precedes movement; a still-live effect instead kills at entry.
            game.step(&[]).unwrap();
            if lifetime == 1 {
                assert_eq!(game.enemies().next().unwrap().position, point(256, 384));
            } else {
                assert!(game.enemies().next().is_none());
            }
        }
    }
}

#[test]
fn bomb_passage_is_per_character_and_belongs_only_to_the_current_bomb() {
    use crate::game::Direction::Right;
    let rules = GameRules::new(2, 512)
        .unwrap()
        .with_bomb_parameters(2, 1, 1)
        .unwrap();
    let mut terrain = vec![Terrain::Floor; 7 * 3];
    terrain[7..14].fill(Terrain::Wall);
    let initial = Scenario::new(7, 3, terrain, bomber(point(384, 128)), rules)
        .unwrap()
        .with_enemies(vec![enemy(7, point(128, 128)), enemy(99, point(128, 640))])
        .unwrap();
    let mut game = Game::new(initial, crate::game::Tick::new(0));
    for _ in 0..8 {
        game.step(&[]).unwrap();
    }
    game.step(&[action(None, true)]).unwrap();
    let bomb = game.bomb().unwrap();
    assert!(bomb.escape_allowed_for(CharacterId::new(0)));
    assert!(bomb.escape_allowed_for(CharacterId::new(7))); // Centre in Cell 0, body in Cell 1.
    assert!(!bomb.escape_allowed_for(CharacterId::new(99)));
    let mut sibling = game.clone();
    for _ in 0..2 {
        game.step(&[action(Some(Right), false)]).unwrap();
        sibling.step(&[action(Some(Right), false)]).unwrap();
        assert_eq!(game, sibling);
    }
    assert!(game.bomb().is_none());
    assert!(!game.enemies().any(|enemy| enemy.id == CharacterId::new(7)));
    game.step(&[action(None, true)]).unwrap();
    let replacement = game.bomb().unwrap();
    assert!(replacement.escape_allowed_for(CharacterId::new(0)));
    assert!(!replacement.escape_allowed_for(CharacterId::new(7)));
    assert!(!replacement.escape_allowed_for(CharacterId::new(99)));
}

#[test]
fn scenario_validation_owns_identity_cells_and_canonical_enemy_footprints() {
    use crate::game::ConstructionError;
    let rules = GameRules::new(2, 16).unwrap();
    let mut terrain = vec![Terrain::Floor; 15];
    terrain[1] = Terrain::Wall;
    terrain[2] = Terrain::Block;
    let initial = Scenario::new(5, 3, terrain, bomber(point(128, 384)), rules).unwrap();
    for enemies in [
        vec![enemy(0, point(128, 384))],
        vec![enemy(7, point(128, 384)); 2],
    ] {
        assert_eq!(
            initial.clone().with_enemies(enemies),
            Err(ConstructionError::DuplicateCharacterId)
        );
    }
    for cell in [
        cell(5, 1),
        cell(0, 3),
        cell(u32::MAX, 1),
        cell(1, 0),
        cell(2, 0),
    ] {
        assert_eq!(
            initial.clone().with_enemies(vec![BuroruStart {
                id: CharacterId::new(7),
                cell,
            }]),
            Err(ConstructionError::InvalidStartingPosition)
        );
    }
    // Canonical enemy Footprint remains independent of the tiny bomber Footprint.
    let legal = initial
        .with_enemies(vec![enemy(u32::MAX, point(128, 384))])
        .unwrap();
    assert_eq!(
        Game::new(legal, crate::game::Tick::new(0))
            .enemies()
            .next()
            .unwrap()
            .id,
        CharacterId::new(u32::MAX)
    );
}

#[test]
fn deadline_failures_preserve_enemies_items_and_per_character_permissions() {
    use crate::game::StepError;
    for (tick, rules, preplace) in [
        (u64::MAX, GameRules::default(), false),
        (u64::MAX - 1, GameRules::default(), false),
        (
            u64::MAX - 2,
            GameRules::default().with_bomb_parameters(1, 1, 30).unwrap(),
            true,
        ),
    ] {
        let initial = scenario(point(384, 384), rules, vec![enemy(7, point(640, 384))])
            .with_fire_up(cell(1, 1))
            .unwrap();
        let mut game = Game::new(initial, crate::game::Tick::new(tick));
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
fn wandering_enemies_lock_the_exit_and_can_initially_share_cells() {
    let enemies = vec![enemy(7, point(640, 128)), enemy(99, point(640, 128))];
    let scenario = Scenario::new(
        4,
        2,
        vec![Terrain::Floor; 8],
        bomber(point(128, 128)),
        GameRules::default(),
    )
    .unwrap()
    .with_enemies(enemies.clone())
    .unwrap()
    .with_exit(cell(0, 0))
    .unwrap();
    let mut game = Game::new(scenario, crate::game::Tick::new(0));
    let enemies = game.enemies().copied().collect::<Vec<_>>();
    assert!(!game.exit_ready());
    for _ in 0..10 {
        game.step(&[]).unwrap();
    }
    assert_eq!(game.enemies().len(), 2);
    assert!(
        game.enemies()
            .zip(&enemies)
            .all(|(after, before)| after.id == before.id)
    );
    assert_ne!(game.enemies().copied().collect::<Vec<_>>(), enemies);
    assert_eq!(game.character(), &Character::from(bomber(point(128, 128))));
    assert_eq!(game.outcome(), None);
    assert_eq!(game.tick().value(), 10);
    game.step(&[CharacterCommand {
        character: CharacterId::new(0),
        movement: None,
        place_bomb: false,
    }])
    .unwrap();
    assert_eq!(game.outcome(), None);
}

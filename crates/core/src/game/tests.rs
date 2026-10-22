mod arena;
mod bombs;
mod buroru;
mod enemy_contact;
mod exit;
mod fire_up;
mod generation;
mod movement;
mod movement_properties;
mod support;
mod wandering_policy;

use super::{
    Arena, CellCoord, Character, CharacterCommand, CharacterId, CharacterKind, Column,
    ConstructionError, ControlSource, Direction, Game, GameRules, Position, Row, Scenario,
    StepError, Terrain, WorldX, WorldY,
};

const ID: CharacterId = CharacterId::new(42);

// Raw test inputs for rejected construction cases and the independent reference.
#[derive(Clone, Debug)]
struct ScenarioInput {
    width: u32,
    height: u32,
    terrain: Vec<Terrain>,
    character: Character,
}

impl ScenarioInput {
    fn parse(self, rules: GameRules) -> Result<Scenario, ConstructionError> {
        Scenario::new(self.width, self.height, self.terrain, self.character, rules)
    }
}

fn command(direction: Direction) -> CharacterCommand {
    CharacterCommand {
        character: ID,
        movement: Some(direction),
        place_bomb: false,
    }
}

fn point(x: i32, y: i32) -> Position {
    Position::new(WorldX::new(x), WorldY::new(y))
}

fn cell(column: u32, row: u32) -> CellCoord {
    CellCoord::new(Column::new(column), Row::new(row))
}

fn assert_step(game: &mut Game, frame: &[CharacterCommand], expected: Position) {
    let mut authored = scenario(
        game.arena().width(),
        game.arena().height(),
        game.arena().terrain().to_vec(),
        expected.x().value(),
        expected.y().value(),
    );
    authored.character.id = game.character().id;
    let expected_game = Game::new(authored.parse(*game.rules()).unwrap(), game.tick() + 1);
    game.step(frame).unwrap();
    assert_eq!(*game, expected_game, "frame={frame:?}");
}

fn rows(input: &str, x: i32, y: i32) -> ScenarioInput {
    let rows: Vec<_> = input.trim().lines().collect();
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

fn scenario(width: u32, height: u32, terrain: Vec<Terrain>, x: i32, y: i32) -> ScenarioInput {
    ScenarioInput {
        width,
        height,
        terrain,
        character: Character {
            id: ID,
            position: point(x, y),
            kind: CharacterKind::Bomber,
            control: ControlSource::Human,
        },
    }
}

#[test]
fn arena_owns_typed_cell_conversion_and_row_major_terrain_access() {
    let arena = Arena::new(
        3,
        2,
        vec![
            Terrain::Floor,
            Terrain::Wall,
            Terrain::Block,
            Terrain::Block,
            Terrain::Floor,
            Terrain::Wall,
        ],
    )
    .unwrap();

    assert_eq!(arena.cell_containing(point(255, 255)), Some(cell(0, 0)));
    assert_eq!(arena.cell_containing(point(256, 255)), Some(cell(1, 0)));
    assert_eq!(arena.cell_containing(point(255, 256)), Some(cell(0, 1)));
    assert_eq!(arena.cell_containing(point(256, 256)), Some(cell(1, 1)));
    assert_eq!(arena.cell_containing(point(767, 511)), Some(cell(2, 1)));
    assert_eq!(arena.cell_containing(point(-1, 0)), None);
    assert_eq!(arena.cell_containing(point(768, 0)), None);
    assert_eq!(arena.cell_containing(point(0, 512)), None);
    assert_eq!(arena.cell_center(cell(2, 1)), Some(point(640, 384)));
    assert_eq!(
        arena.cell_containing(arena.cell_center(cell(2, 1)).unwrap()),
        Some(cell(2, 1))
    );
    assert_eq!(arena.cell_center(cell(3, 0)), None);
    assert_eq!(arena.terrain_at(cell(1, 0)), Some(Terrain::Wall));
    assert_eq!(arena.terrain_at(cell(2, 0)), Some(Terrain::Block));
    assert_eq!(arena.terrain_at(cell(3, 0)), None);
    assert_eq!(
        arena.cells().collect::<Vec<_>>(),
        [
            (cell(0, 0), Terrain::Floor),
            (cell(1, 0), Terrain::Wall),
            (cell(2, 0), Terrain::Block),
            (cell(0, 1), Terrain::Block),
            (cell(1, 1), Terrain::Floor),
            (cell(2, 1), Terrain::Wall),
        ]
    );
}

#[test]
fn construction_observes_the_complete_authored_state() {
    let authored = scenario(2, 1, vec![Terrain::Floor; 2], 256, 128);
    let game = Game::new(authored.clone().parse(GameRules::default()).unwrap(), 0);
    assert_eq!(game.tick(), 0);
    assert_eq!(game.arena().width(), 2);
    assert_eq!(game.arena().height(), 1);
    assert_eq!(game.arena().terrain(), authored.terrain);
    assert_eq!(game.character(), &authored.character);
    assert_eq!(game.rules(), &GameRules::new(192, 16).unwrap());
    assert_eq!(game.clone(), game);
}

#[test]
fn scenario_keeps_the_rules_that_make_its_starting_footprint_legal() {
    let character = Character {
        id: ID,
        position: point(64, 128),
        kind: CharacterKind::Bomber,
        control: ControlSource::Human,
    };
    assert_eq!(
        Scenario::new(
            1,
            1,
            vec![Terrain::Floor],
            character,
            GameRules::new(254, 1024).unwrap()
        ),
        Err(ConstructionError::InvalidStartingPosition)
    );
    let scenario = Scenario::new(
        1,
        1,
        vec![Terrain::Floor],
        character,
        GameRules::new(128, 1024).unwrap(),
    )
    .unwrap();
    let mut game = Game::new(scenario, 0);
    assert_step(&mut game, &[command(Direction::Right)], point(192, 128));
}

#[test]
fn movement_rules_reject_invalid_values_with_footprint_errors_first() {
    for side in [0, 1, 3, 191, 255, 256, 258, u32::MAX] {
        for distance in [0, 16] {
            assert_eq!(
                GameRules::new(side, distance),
                Err(ConstructionError::InvalidFootprintSide)
            );
        }
    }
    for distance in [0, 1025, u32::MAX] {
        assert_eq!(
            GameRules::new(192, distance),
            Err(ConstructionError::InvalidMovementDistance)
        );
    }
}

#[test]
fn scenario_construction_rejects_invalid_data_and_accepts_limits() {
    use ConstructionError::*;
    let base = scenario(1, 1, vec![Terrain::Floor], 128, 128);
    let defaults = GameRules::default();
    for (width, height) in [(0, 1), (1, 0), (257, 1), (1, 257), (u32::MAX, u32::MAX)] {
        let mut bad = base.clone();
        bad.width = width;
        bad.height = height;
        bad.terrain.clear();
        bad.character.position = point(i32::MIN, 128);
        assert_eq!(bad.parse(defaults), Err(InvalidDimensions));
    }
    for terrain in [vec![], vec![Terrain::Floor; 2]] {
        let mut bad = base.clone();
        bad.terrain = terrain;
        bad.character.position = point(i32::MIN, 128);
        assert_eq!(bad.parse(defaults), Err(TerrainLengthMismatch));
    }
    for position in [
        point(-1, 128),
        point(128, -1),
        point(95, 128),
        point(161, 128),
        point(128, 95),
        point(128, 161),
        point(i32::MIN, 128),
        point(i32::MAX, 128),
        point(128, i32::MIN),
        point(128, i32::MAX),
    ] {
        let mut bad = base.clone();
        bad.character.position = position;
        assert_eq!(
            bad.parse(defaults),
            Err(InvalidStartingPosition),
            "{position:?}"
        );
    }
    for terrain in [Terrain::Wall, Terrain::Block] {
        assert_eq!(
            scenario(2, 1, vec![Terrain::Floor, terrain], 161, 128).parse(defaults),
            Err(InvalidStartingPosition)
        );
        assert!(
            scenario(2, 1, vec![Terrain::Floor, terrain], 160, 128)
                .parse(defaults)
                .is_ok()
        );
    }
    for x in [96, 160] {
        for y in [96, 160] {
            assert!(
                scenario(1, 1, vec![Terrain::Floor], x, y)
                    .parse(defaults)
                    .is_ok()
            );
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
            let game = Game::new(
                authored
                    .parse(GameRules::new(side, distance).unwrap())
                    .unwrap(),
                u64::MAX,
            );
            assert_eq!(game.tick(), u64::MAX);
            assert_eq!(game.character().id.value(), u32::MAX);
        }
    }
}

fn assert_rejected(game: &mut Game, frame: &[CharacterCommand], error: StepError) {
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
    let unknown = CharacterCommand {
        character: CharacterId::new(43),
        movement: Some(Direction::Left),
        place_bomb: false,
    };
    let other_unknown = CharacterCommand {
        character: CharacterId::new(u32::MAX),
        movement: Some(Direction::Down),
        place_bomb: false,
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
    let scenario = rows("..", 128, 128).parse(GameRules::default()).unwrap();
    for tick in [0, u64::MAX] {
        let mut game = Game::new(scenario.clone(), tick);
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
        (128, vec![], point(128, 128)),
        (128, vec![command(Direction::Right)], point(144, 128)),
        (160, vec![command(Direction::Right)], point(160, 128)),
    ] {
        let authored = rows(".#", start, 128);
        let mut game = Game::new(authored.parse(GameRules::default()).unwrap(), u64::MAX - 1);
        assert_step(&mut game, &frame, expected);
        assert_eq!(game.tick(), u64::MAX);
        assert_rejected(&mut game, &frame, StepError::TickExhausted);
    }
    let mut authored = rows("..", 128, 128);
    authored.character.id = CharacterId::new(0);
    let mut game = Game::new(authored.parse(GameRules::default()).unwrap(), 0);
    assert_step(
        &mut game,
        &[CharacterCommand {
            character: CharacterId::new(0),
            movement: Some(Direction::Right),
            place_bomb: false,
        }],
        point(144, 128),
    );
}

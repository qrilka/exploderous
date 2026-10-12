//! The movement slice: validated construction, tick input, read-only state, and cloning.

use std::collections::BTreeSet;

/// Simulation units, not pixels. Cell intervals are lower-inclusive, upper-exclusive.
pub const WORLD_UNITS_PER_CELL: i32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Floor,
    Wall,
    Block,
}

/// Stable identity, independent of position or storage index; every u32 is allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CharacterId(u32);

impl CharacterId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Integer centre coordinates, with the origin at the arena's outer top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub position: Position,
}

/// Validated starting arena and character placement, bound to its movement rules.
/// A scenario defines no simulation time; that belongs to the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    arena: Arena,
    character: Character,
    rules: MovementRules,
}

impl Scenario {
    /// Parse authored data using valid rules. Rejection priority is dimensions,
    /// terrain length, then the starting footprint's placement.
    pub fn new(
        width: u32,
        height: u32,
        terrain: Vec<Terrain>,
        character: Character,
        rules: MovementRules,
    ) -> Result<Self, ConstructionError> {
        let arena = Arena::new(width, height, terrain)?;
        let footprint = Rect::footprint(character.position, rules);
        if footprint.left < 0
            || footprint.top < 0
            || footprint.right > i64::from(arena.width) * i64::from(WORLD_UNITS_PER_CELL)
            || footprint.bottom > i64::from(arena.height) * i64::from(WORLD_UNITS_PER_CELL)
            || arena.obstructions(footprint).next().is_some()
        {
            return Err(ConstructionError::InvalidStartingPosition);
        }
        Ok(Self {
            arena,
            character,
            rules,
        })
    }
}

/// Validated rules, fixed for a game's lifetime. Distances are integer world units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementRules {
    footprint_side: u32,
    movement_distance: u32,
}

impl MovementRules {
    /// Parse an even footprint side in 2..=254 and a movement distance in 1..=1024.
    /// If both are invalid, the footprint error takes priority.
    pub fn new(footprint_side: u32, movement_distance: u32) -> Result<Self, ConstructionError> {
        if !(2..=254).contains(&footprint_side) || !footprint_side.is_multiple_of(2) {
            return Err(ConstructionError::InvalidFootprintSide);
        }
        if !(1..=1024).contains(&movement_distance) {
            return Err(ConstructionError::InvalidMovementDistance);
        }
        Ok(Self {
            footprint_side,
            movement_distance,
        })
    }

    pub fn footprint_side(&self) -> u32 {
        self.footprint_side
    }

    pub fn movement_distance(&self) -> u32 {
        self.movement_distance
    }
}

impl Default for MovementRules {
    fn default() -> Self {
        Self::new(192, 16).expect("default movement rules are valid")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionError {
    InvalidDimensions,
    TerrainLengthMismatch,
    InvalidFootprintSide,
    InvalidMovementDistance,
    InvalidStartingPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

/// One cardinal command for one tick. Omission means no movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementCommand {
    pub character: CharacterId,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepError {
    DuplicateCommand,
    UnknownCharacter,
    TickExhausted,
}

/// Row-major terrain with validated dimensions and private live storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arena {
    width: u32,
    height: u32,
    terrain: Vec<Terrain>,
}

impl Arena {
    fn new(width: u32, height: u32, terrain: Vec<Terrain>) -> Result<Self, ConstructionError> {
        if !(1..=256).contains(&width) || !(1..=256).contains(&height) {
            return Err(ConstructionError::InvalidDimensions);
        }
        let cell_count = width
            .checked_mul(height)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(ConstructionError::InvalidDimensions)?;
        if terrain.len() != cell_count {
            return Err(ConstructionError::TerrainLengthMismatch);
        }
        Ok(Self {
            width,
            height,
            terrain,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn terrain(&self) -> &[Terrain] {
        &self.terrain
    }

    // Callers supply a positive-area rectangle wholly within the validated arena.
    fn obstructions(&self, rect: Rect) -> impl Iterator<Item = (i64, i64)> + '_ {
        let cell = i64::from(WORLD_UNITS_PER_CELL);
        let columns = rect.left / cell..=(rect.right - 1) / cell;
        let rows = rect.top / cell..=(rect.bottom - 1) / cell;
        rows.flat_map(move |row| {
            columns.clone().filter_map(move |column| {
                let index = u32::try_from(row)
                    .ok()
                    .and_then(|r| r.checked_mul(self.width))
                    .and_then(|offset| u32::try_from(column).ok()?.checked_add(offset))
                    .and_then(|index| usize::try_from(index).ok())
                    .expect("cell index fits validated arena dimensions");
                (self.terrain[index] != Terrain::Floor).then_some((column * cell, row * cell))
            })
        })
    }
}

#[derive(Clone, Copy)]
struct Rect {
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
}

impl Rect {
    fn footprint(position: Position, rules: MovementRules) -> Self {
        let half_side = i64::from(rules.footprint_side / 2);
        Self {
            left: i64::from(position.x) - half_side,
            top: i64::from(position.y) - half_side,
            right: i64::from(position.x) + half_side,
            bottom: i64::from(position.y) + half_side,
        }
    }
}

/// Complete movement state. Equality and cloning include every authoritative field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    arena: Arena,
    character: Character,
    tick: u64,
    rules: MovementRules,
}

impl Game {
    /// Start from a validated scenario without replacing its rules or revalidating it.
    /// The game's clock is separate from the scenario; ordinary play starts at zero.
    pub fn new(scenario: Scenario, initial_tick: u64) -> Self {
        Self {
            arena: scenario.arena,
            character: scenario.character,
            tick: initial_tick,
            rules: scenario.rules,
        }
    }

    /// Advance exactly one tick; the core has no clock or persistent held input.
    /// Errors preserve complete state, with priority duplicate, unknown, exhausted.
    pub fn step(&mut self, frame: &[MovementCommand]) -> Result<(), StepError> {
        let mut identities = BTreeSet::new();
        for command in frame {
            if !identities.insert(command.character) {
                return Err(StepError::DuplicateCommand);
            }
        }
        if frame
            .iter()
            .any(|command| command.character != self.character.id)
        {
            return Err(StepError::UnknownCharacter);
        }
        let next_tick = self.tick.checked_add(1).ok_or(StepError::TickExhausted)?;
        if let Some(command) = frame.first() {
            self.character.position = self.moved_position(command.direction);
        }
        self.tick = next_tick;
        Ok(())
    }

    fn moved_position(&self, direction: Direction) -> Position {
        let half = i64::from(self.rules.footprint_side / 2);
        let distance = i64::from(self.rules.movement_distance);
        let cell = i64::from(WORLD_UNITS_PER_CELL);
        let mut x = i64::from(self.character.position.x);
        let mut y = i64::from(self.character.position.y);
        let mut swept = Rect::footprint(self.character.position, self.rules);
        // Clip to the arena, then inspect every cell with positive-area swept overlap.
        // Cardinal motion leaves the perpendicular interval unchanged, so each such
        // obstacle limits the centre to its near face plus/minus the footprint radius.
        match direction {
            Direction::Up => {
                y = (y - distance).max(half);
                swept.top = y - half;
            }
            Direction::Right => {
                x = (x + distance).min(i64::from(self.arena.width) * cell - half);
                swept.right = x + half;
            }
            Direction::Down => {
                y = (y + distance).min(i64::from(self.arena.height) * cell - half);
                swept.bottom = y + half;
            }
            Direction::Left => {
                x = (x - distance).max(half);
                swept.left = x - half;
            }
        }
        for (column_edge, row_edge) in self.arena.obstructions(swept) {
            match direction {
                Direction::Up => y = y.max(row_edge + cell + half),
                Direction::Right => x = x.min(column_edge - half),
                Direction::Down => y = y.min(row_edge - half),
                Direction::Left => x = x.max(column_edge + cell + half),
            }
        }
        Position {
            x: i32::try_from(x).expect("movement stays within validated arena extents"),
            y: i32::try_from(y).expect("movement stays within validated arena extents"),
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    pub fn character(&self) -> &Character {
        &self.character
    }

    pub fn rules(&self) -> &MovementRules {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Character, CharacterId, ConstructionError, Direction, Game, MovementCommand, MovementRules,
        Position, Scenario, StepError, Terrain,
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
        fn parse(self, rules: MovementRules) -> Result<Scenario, ConstructionError> {
            Scenario::new(self.width, self.height, self.terrain, self.character, rules)
        }
    }

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
        let expected_game = Game::new(authored.parse(*game.rules()).unwrap(), game.tick() + 1);
        game.step(frame).unwrap();
        assert_eq!(*game, expected_game, "frame={frame:?}");
    }

    fn rows(rows: &[&str], x: i32, y: i32) -> ScenarioInput {
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
                position: Position { x, y },
            },
        }
    }

    #[test]
    fn construction_observes_the_complete_authored_state() {
        let authored = scenario(2, 1, vec![Terrain::Floor; 2], 256, 128);
        let game = Game::new(authored.clone().parse(MovementRules::default()).unwrap(), 0);
        assert_eq!(game.tick(), 0);
        assert_eq!(game.arena().width(), 2);
        assert_eq!(game.arena().height(), 1);
        assert_eq!(game.arena().terrain(), authored.terrain);
        assert_eq!(game.character(), &authored.character);
        assert_eq!(game.rules(), &MovementRules::new(192, 16).unwrap());
        assert_eq!(game.clone(), game);
    }

    #[test]
    fn scenario_keeps_the_rules_that_make_its_starting_footprint_legal() {
        let character = Character {
            id: ID,
            position: Position { x: 64, y: 128 },
        };
        assert_eq!(
            Scenario::new(
                1,
                1,
                vec![Terrain::Floor],
                character,
                MovementRules::new(254, 1024).unwrap()
            ),
            Err(ConstructionError::InvalidStartingPosition)
        );
        let scenario = Scenario::new(
            1,
            1,
            vec![Terrain::Floor],
            character,
            MovementRules::new(128, 1024).unwrap(),
        )
        .unwrap();
        let mut game = Game::new(scenario, 0);
        assert_step(
            &mut game,
            &[command(Direction::Right)],
            Position { x: 192, y: 128 },
        );
    }

    #[test]
    fn movement_rules_reject_invalid_values_with_footprint_errors_first() {
        for side in [0, 1, 3, 191, 255, 256, 258, u32::MAX] {
            for distance in [0, 16] {
                assert_eq!(
                    MovementRules::new(side, distance),
                    Err(ConstructionError::InvalidFootprintSide)
                );
            }
        }
        for distance in [0, 1025, u32::MAX] {
            assert_eq!(
                MovementRules::new(192, distance),
                Err(ConstructionError::InvalidMovementDistance)
            );
        }
    }

    #[test]
    fn scenario_construction_rejects_invalid_data_and_accepts_limits() {
        use ConstructionError::*;
        let base = scenario(1, 1, vec![Terrain::Floor], 128, 128);
        let defaults = MovementRules::default();
        for (width, height) in [(0, 1), (1, 0), (257, 1), (1, 257), (u32::MAX, u32::MAX)] {
            let mut bad = base.clone();
            bad.width = width;
            bad.height = height;
            bad.terrain.clear();
            bad.character.position.x = i32::MIN;
            assert_eq!(bad.parse(defaults), Err(InvalidDimensions));
        }
        for terrain in [vec![], vec![Terrain::Floor; 2]] {
            let mut bad = base.clone();
            bad.terrain = terrain;
            bad.character.position.x = i32::MIN;
            assert_eq!(bad.parse(defaults), Err(TerrainLengthMismatch));
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
                        .parse(MovementRules::new(side, distance).unwrap())
                        .unwrap(),
                    u64::MAX,
                );
                assert_eq!(game.tick(), u64::MAX);
                assert_eq!(game.character().id.value(), u32::MAX);
            }
        }
    }

    #[test]
    fn cardinal_commands_apply_only_to_their_tick() {
        let base = Game::new(
            scenario(3, 3, vec![Terrain::Floor; 9], 384, 384)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
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
        let fast = MovementRules::new(192, 512).unwrap();
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
                MovementRules::new(192, 1024).unwrap(),
                Right,
                Position { x: 416, y: 128 },
            ),
            (
                rows(&["..#.+..."], 1920, 128),
                MovementRules::new(192, 1024).unwrap(),
                Left,
                Position { x: 1376, y: 128 },
            ),
        ];
        for (authored, rules, direction, expected) in cases {
            let mut game = Game::new(authored.clone().parse(rules).unwrap(), 0);
            assert_step(&mut game, &[command(direction)], expected);
            assert_step(&mut game, &[command(direction)], expected);
            assert_step(&mut game, &[], expected);
        }
        let mut approaching = Game::new(
            rows(&[".#.."], 128, 128)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
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
                rows(&["."], 128, 128)
                    .parse(MovementRules::new(192, 1024).unwrap())
                    .unwrap(),
                0,
            );
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
                )
                .parse(MovementRules::new(side, 1024).unwrap())
                .unwrap(),
                0,
            );
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
            let mut game = Game::new(
                rows(&classic, x, 640)
                    .parse(MovementRules::default())
                    .unwrap(),
                0,
            );
            assert_step(&mut game, &[command(Direction::Up)], Position { x, y: 624 });
        }
        for x in [351, 417] {
            assert_eq!(
                rows(&classic, x, 640).parse(MovementRules::default()),
                Err(ConstructionError::InvalidStartingPosition)
            );
        }
        let mut turn = Game::new(
            rows(&classic, 400, 417)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
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

        let mut corner = Game::new(
            rows(&["..", ".#"], 160, 160)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
        assert_step(
            &mut corner,
            &[command(Direction::Right)],
            Position { x: 176, y: 160 },
        );
        let mut blocked_corner = Game::new(
            rows(&["..", ".#"], 160, 161)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
        assert_step(
            &mut blocked_corner,
            &[command(Direction::Right)],
            Position { x: 160, y: 161 },
        );
        let mut spanning = Game::new(
            rows(&["..", ".."], 256, 256)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
        assert_step(
            &mut spanning,
            &[command(Direction::Down)],
            Position { x: 256, y: 272 },
        );
        let mut perpendicular = Game::new(
            rows(&[".#.", "..."], 384, 352)
                .parse(MovementRules::default())
                .unwrap(),
            0,
        );
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
        let scenario = rows(&[".."], 128, 128)
            .parse(MovementRules::default())
            .unwrap();
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
            let authored = rows(&[".#"], start, 128);
            let mut game = Game::new(
                authored.parse(MovementRules::default()).unwrap(),
                u64::MAX - 1,
            );
            assert_step(&mut game, &frame, expected);
            assert_eq!(game.tick(), u64::MAX);
            assert_rejected(&mut game, &frame, StepError::TickExhausted);
        }
        let mut authored = rows(&[".."], 128, 128);
        authored.character.id = CharacterId::new(0);
        let mut game = Game::new(authored.parse(MovementRules::default()).unwrap(), 0);
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
    fn reference_legal(authored: &ScenarioInput, rules: MovementRules, position: Position) -> bool {
        let radius = i64::from(rules.footprint_side() / 2);
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

    fn reference_move(
        authored: &ScenarioInput,
        rules: MovementRules,
        direction: Direction,
    ) -> Position {
        let (dx, dy) = match direction {
            Direction::Up => (0, -1),
            Direction::Right => (1, 0),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
        };
        let mut position = authored.character.position;
        for _ in 0..rules.movement_distance() {
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
                            let rules = MovementRules::new(side, distance).unwrap();
                            let constructed = authored.clone().parse(rules);
                            if !reference_legal(&authored, rules, authored.character.position) {
                                assert_eq!(
                                    constructed,
                                    Err(ConstructionError::InvalidStartingPosition),
                                    "scenario={authored:?}, initial_tick=0, rules={rules:?}, step=0"
                                );
                                continue;
                            }
                            let base = Game::new(constructed.unwrap_or_else(|error| {
                                panic!("scenario={authored:?}, initial_tick=0, rules={rules:?}, step=0: {error:?}")
                            }), 0);
                            for direction in [
                                Direction::Up,
                                Direction::Right,
                                Direction::Down,
                                Direction::Left,
                            ] {
                                let mut expected = authored.clone();
                                expected.character.position =
                                    reference_move(&authored, rules, direction);
                                let expected_game = Game::new(expected.parse(rules).unwrap(), 1);
                                let mut game = base.clone();
                                let result = game.step(&[command(direction)]);
                                assert_eq!(
                                    result,
                                    Ok(()),
                                    "scenario={authored:?}, initial_tick=0, rules={rules:?}, direction={direction:?}, step=1"
                                );
                                assert_eq!(
                                    game, expected_game,
                                    "scenario={authored:?}, initial_tick=0, rules={rules:?}, direction={direction:?}, step=1"
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
            MovementRules::new(2, 1).unwrap(),
            MovementRules::new(128, 257).unwrap(),
            MovementRules::new(192, 512).unwrap(),
            MovementRules::new(254, 1024).unwrap(),
        ] {
            let initial_tick = 0;
            let mut game = Game::new(authored.clone().parse(rules).unwrap(), initial_tick);
            let mut replay = Game::new(authored.clone().parse(rules).unwrap(), initial_tick);
            let mut reference = authored.clone();
            let mut reference_tick = initial_tick;
            let mut continuation = None;
            for (step, (frame, outcome)) in frames.iter().enumerate() {
                if outcome.is_ok() {
                    if let Some(input) = frame.first() {
                        reference.character.position =
                            reference_move(&reference, rules, input.direction);
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
                    Game::new(reference.clone().parse(rules).unwrap(), reference_tick),
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
}

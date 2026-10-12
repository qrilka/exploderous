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

/// Authored data; only `Game::new` can turn it into live state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<Terrain>,
    pub character: Character,
    pub initial_tick: u64,
}

/// Fixed for a game's lifetime. Distances are integer world units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementRules {
    pub footprint_side: u32,
    pub movement_distance: u32,
}

impl Default for MovementRules {
    fn default() -> Self {
        Self {
            footprint_side: 192,
            movement_distance: 16,
        }
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
    fn footprint(position: Position, half_side: i64) -> Self {
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
    pub fn new(scenario: Scenario, rules: MovementRules) -> Result<Self, ConstructionError> {
        use ConstructionError::*;
        if !(1..=256).contains(&scenario.width) || !(1..=256).contains(&scenario.height) {
            return Err(InvalidDimensions);
        }
        let cell_count = scenario
            .width
            .checked_mul(scenario.height)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(InvalidDimensions)?;
        if scenario.terrain.len() != cell_count {
            return Err(TerrainLengthMismatch);
        }
        if !(2..=254).contains(&rules.footprint_side) || !rules.footprint_side.is_multiple_of(2) {
            return Err(InvalidFootprintSide);
        }
        if !(1..=1024).contains(&rules.movement_distance) {
            return Err(InvalidMovementDistance);
        }
        let arena = Arena {
            width: scenario.width,
            height: scenario.height,
            terrain: scenario.terrain,
        };
        let footprint = Rect::footprint(
            scenario.character.position,
            i64::from(rules.footprint_side / 2),
        );
        if footprint.left < 0
            || footprint.top < 0
            || footprint.right > i64::from(arena.width) * i64::from(WORLD_UNITS_PER_CELL)
            || footprint.bottom > i64::from(arena.height) * i64::from(WORLD_UNITS_PER_CELL)
            || arena.obstructions(footprint).next().is_some()
        {
            return Err(InvalidStartingPosition);
        }
        Ok(Self {
            arena,
            character: scenario.character,
            tick: scenario.initial_tick,
            rules,
        })
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
        let mut swept = Rect::footprint(self.character.position, half);
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

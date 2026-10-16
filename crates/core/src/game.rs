//! Deterministic movement, Bomb/Blast simulation, and validated game state.

use std::collections::{BTreeMap, BTreeSet};

const BURORU_SPEED: u32 = 8;

/// Simulation units, not pixels. Cell intervals are lower-inclusive, upper-exclusive.
pub const WORLD_UNITS_PER_CELL: i32 = 256;
const MAX_TURN_CORRECTION: u32 = (WORLD_UNITS_PER_CELL / 2) as u32;

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

/// Signed world coordinate on the horizontal axis, in simulation units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldX(i32);

impl WorldX {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }
}

/// Signed world coordinate on the vertical axis, in simulation units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldY(i32);

impl WorldY {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }
}

/// Zero-based horizontal grid coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column(u32);

impl Column {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Zero-based vertical grid coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row(u32);

impl Row {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Integer world-space point in simulation units, with the origin at the arena's outer top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    x: WorldX,
    y: WorldY,
}

impl Position {
    pub const fn new(x: WorldX, y: WorldY) -> Self {
        Self { x, y }
    }

    pub const fn x(self) -> WorldX {
        self.x
    }

    pub const fn y(self) -> WorldY {
        self.y
    }

    // Movement starts inside a validated arena; even the maximum step fits i32.
    fn advanced(self, direction: Direction, distance: u32) -> Self {
        let distance = i32::try_from(distance).expect("movement distance fits i32");
        let (dx, dy) = match direction {
            Direction::Up => (0, -distance),
            Direction::Right => (distance, 0),
            Direction::Down => (0, distance),
            Direction::Left => (-distance, 0),
        };
        Self::new(WorldX::new(self.x.0 + dx), WorldY::new(self.y.0 + dy))
    }
}

/// A zero-based Cell address. Arena operations check whether it is in bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellCoord {
    column: Column,
    row: Row,
}

impl CellCoord {
    pub const fn new(column: Column, row: Row) -> Self {
        Self { column, row }
    }

    pub const fn column(self) -> Column {
        self.column
    }

    pub const fn row(self) -> Row {
        self.row
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterKind {
    Bomber,
    Buroru,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSource {
    Human,
    Npc,
}

/// Authored Character data; Scenario validates kind/control and placement together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub position: Position,
    pub kind: CharacterKind,
    pub control: ControlSource,
}

/// Validated starting arena and character placement, bound to its game rules.
/// A scenario defines no simulation time; that belongs to the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    arena: Arena,
    character: Character,
    enemies: Vec<Character>,
    rules: GameRules,
    exit: Option<CellCoord>,
    fire_up: Option<CellCoord>,
    random_seed: u64,
}

impl Scenario {
    /// Parse authored data using valid rules. Rejection priority is dimensions,
    /// terrain length, then the starting footprint's placement.
    pub fn new(
        width: u32,
        height: u32,
        terrain: Vec<Terrain>,
        character: Character,
        rules: GameRules,
    ) -> Result<Self, ConstructionError> {
        let arena = Arena::new(width, height, terrain)?;
        arena.validate_start(character.position, rules)?;
        if character.kind != CharacterKind::Bomber || character.control != ControlSource::Human {
            return Err(ConstructionError::InvalidCharacterControl);
        }
        Ok(Self {
            arena,
            character,
            enemies: Vec::new(),
            rules,
            exit: None,
            fire_up: None,
            random_seed: 0,
        })
    }

    /// Set reproducible AI randomness before constructing the Game.
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.random_seed = seed;
        self
    }

    /// Author centred Buroru. Identities are unique; overlapping Characters are legal.
    pub fn with_enemies(mut self, mut enemies: Vec<Character>) -> Result<Self, ConstructionError> {
        let mut identities = BTreeSet::from([self.character.id]);
        for enemy in &enemies {
            if !identities.insert(enemy.id) {
                return Err(ConstructionError::DuplicateCharacterId);
            }
            if enemy.kind != CharacterKind::Buroru || enemy.control != ControlSource::Npc {
                return Err(ConstructionError::InvalidCharacterControl);
            }
            self.arena
                .validate_start(enemy.position, GameRules::default())?;
            if self
                .arena
                .cell_containing(enemy.position)
                .and_then(|cell| self.arena.cell_center(cell))
                != Some(enemy.position)
            {
                return Err(ConstructionError::InvalidStartingPosition);
            }
        }
        // Canonical identity order, independent of authored vector order.
        enemies.sort_by_key(|enemy| enemy.id);
        self.enemies = enemies;
        Ok(self)
    }

    /// Author the single Fire-up separately from terrain, on Floor or beneath a Block.
    pub fn with_fire_up(mut self, cell: CellCoord) -> Result<Self, ConstructionError> {
        if !matches!(
            self.arena.terrain_at(cell),
            Some(Terrain::Floor | Terrain::Block)
        ) {
            return Err(ConstructionError::InvalidFireUpLocation);
        }
        self.fire_up = Some(cell);
        Ok(self)
    }

    /// Author one Exit on Floor or beneath a Block, never outside the Arena or in a Wall.
    /// Scenarios without an Exit have no victory objective.
    pub fn with_exit(mut self, cell: CellCoord) -> Result<Self, ConstructionError> {
        if !matches!(
            self.arena.terrain_at(cell),
            Some(Terrain::Floor | Terrain::Block)
        ) {
            return Err(ConstructionError::InvalidExitLocation);
        }
        self.exit = Some(cell);
        Ok(self)
    }
}

/// Validated rules, fixed for a game's lifetime. Movement uses integer world units;
/// Bomb durations use simulation ticks and Blast reach counts Cells beyond the origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameRules {
    footprint_side: u32,
    movement_distance: u32,
    turn_assistance: bool,
    bomb_fuse_ticks: u64,
    blast_reach: u32,
    blast_lifetime_ticks: u64,
}

impl GameRules {
    /// Parse an even footprint side in 2..=254 and a movement distance in 1..=1024.
    /// Turn assistance is enabled; use `with_turn_assistance(false)` for strict movement.
    /// Bomb parameters default to a 120-tick fuse, one-Cell reach, and 30-tick lifetime.
    /// If both movement parameters are invalid, the footprint error takes priority.
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
            turn_assistance: true,
            bomb_fuse_ticks: 120,
            blast_reach: 1,
            blast_lifetime_ticks: 30,
        })
    }

    /// Tune Bomb parameters before Scenario construction. All three must be positive.
    /// Rejection priority is fuse, reach, then lifetime; deadlines are checked on stepping.
    pub fn with_bomb_parameters(
        mut self,
        fuse_ticks: u64,
        reach: u32,
        lifetime_ticks: u64,
    ) -> Result<Self, ConstructionError> {
        if fuse_ticks == 0 {
            return Err(ConstructionError::InvalidBombFuse);
        }
        if reach == 0 {
            return Err(ConstructionError::InvalidBlastReach);
        }
        if lifetime_ticks == 0 {
            return Err(ConstructionError::InvalidBlastLifetime);
        }
        self.bomb_fuse_ticks = fuse_ticks;
        self.blast_reach = reach;
        self.blast_lifetime_ticks = lifetime_ticks;
        Ok(self)
    }

    pub fn bomb_fuse_ticks(&self) -> u64 {
        self.bomb_fuse_ticks
    }

    pub fn blast_reach(&self) -> u32 {
        self.blast_reach
    }

    pub fn blast_lifetime_ticks(&self) -> u64 {
        self.blast_lifetime_ticks
    }

    pub fn footprint_side(&self) -> u32 {
        self.footprint_side
    }

    pub fn movement_distance(&self) -> u32 {
        self.movement_distance
    }

    /// Select assisted or strict movement before constructing a scenario/game.
    pub fn with_turn_assistance(mut self, enabled: bool) -> Self {
        self.turn_assistance = enabled;
        self
    }

    pub fn turn_assistance(&self) -> bool {
        self.turn_assistance
    }
}

impl Default for GameRules {
    fn default() -> Self {
        Self::new(192, 16).expect("default game rules are valid")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionError {
    InvalidDimensions,
    TerrainLengthMismatch,
    InvalidFootprintSide,
    InvalidMovementDistance,
    InvalidStartingPosition,
    InvalidBombFuse,
    InvalidBlastReach,
    InvalidBlastLifetime,
    InvalidExitLocation,
    InvalidFireUpLocation,
    DuplicateCharacterId,
    InvalidCharacterControl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

/// One Character's actions for one tick. Omitted actions are not remembered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterCommand {
    pub character: CharacterId,
    pub movement: Option<Direction>,
    pub place_bomb: bool,
}

/// The single live Bomb; its permission belongs to this Bomb, not a later one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bomb {
    cell: CellCoord,
    detonate_at: u64,
    blast_reach: u64,
    escape_allowed: Vec<CharacterId>,
}

impl Bomb {
    pub fn escape_allowed_for(&self, character: CharacterId) -> bool {
        self.escape_allowed.contains(&character)
    }

    pub fn cell(&self) -> CellCoord {
        self.cell
    }

    pub fn detonate_at(&self) -> u64 {
        self.detonate_at
    }

    pub fn blast_reach(&self) -> u64 {
        self.blast_reach
    }
}

/// One fixed, nonempty coverage set with an independent expiry deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blast {
    cells: Vec<CellCoord>,
    expires_at: u64,
}

impl Blast {
    pub fn origin(&self) -> CellCoord {
        self.cells[0]
    }

    pub fn cells(&self) -> &[CellCoord] {
        &self.cells
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepError {
    DuplicateCommand,
    UnknownCharacter,
    NpcCommand,
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

    fn validate_start(
        &self,
        position: Position,
        rules: GameRules,
    ) -> Result<(), ConstructionError> {
        let footprint = Rect::footprint(position, rules);
        if footprint.left < 0
            || footprint.top < 0
            || footprint.right > i64::from(self.width) * i64::from(WORLD_UNITS_PER_CELL)
            || footprint.bottom > i64::from(self.height) * i64::from(WORLD_UNITS_PER_CELL)
            || self.obstructions(footprint).next().is_some()
        {
            return Err(ConstructionError::InvalidStartingPosition);
        }
        Ok(())
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

    /// Return the in-bounds Cell containing this world-space position.
    pub fn cell_containing(&self, position: Position) -> Option<CellCoord> {
        let column = u32::try_from(position.x().value().div_euclid(WORLD_UNITS_PER_CELL)).ok()?;
        let row = u32::try_from(position.y().value().div_euclid(WORLD_UNITS_PER_CELL)).ok()?;
        let cell = CellCoord::new(Column::new(column), Row::new(row));
        self.cell_index(cell).map(|_| cell)
    }

    /// Return the world-space centre of an in-bounds Cell.
    pub fn cell_center(&self, cell: CellCoord) -> Option<Position> {
        let origin = self.cell_origin(cell)?;
        let half_cell = WORLD_UNITS_PER_CELL / 2;
        Some(Position::new(
            WorldX::new(origin.x().value().checked_add(half_cell)?),
            WorldY::new(origin.y().value().checked_add(half_cell)?),
        ))
    }

    /// Return terrain at an in-bounds Cell.
    pub fn terrain_at(&self, cell: CellCoord) -> Option<Terrain> {
        self.cell_index(cell).map(|index| self.terrain[index])
    }

    /// Iterate Cells and their terrain in the Arena's canonical row-major order.
    pub fn cells(&self) -> impl Iterator<Item = (CellCoord, Terrain)> + '_ {
        let width = usize::try_from(self.width).expect("validated arena width fits usize");
        self.terrain
            .iter()
            .copied()
            .enumerate()
            .map(move |(index, terrain)| {
                let column = u32::try_from(index % width).expect("column fits validated arena");
                let row = u32::try_from(index / width).expect("row fits validated arena");
                (CellCoord::new(Column::new(column), Row::new(row)), terrain)
            })
    }

    fn neighbor(&self, cell: CellCoord, direction: Direction) -> Option<CellCoord> {
        let (column, row) = (cell.column().value(), cell.row().value());
        let (column, row) = match direction {
            Direction::Up => (column, row.checked_sub(1)?),
            Direction::Right => (column.checked_add(1)?, row),
            Direction::Down => (column, row.checked_add(1)?),
            Direction::Left => (column.checked_sub(1)?, row),
        };
        let next = CellCoord::new(Column::new(column), Row::new(row));
        self.cell_index(next).map(|_| next)
    }

    fn cell_bounds(&self, cell: CellCoord) -> Rect {
        let origin = self
            .cell_origin(cell)
            .expect("Cell bounds requested within Arena");
        let left = i64::from(origin.x().value());
        let top = i64::from(origin.y().value());
        Rect {
            left,
            top,
            right: left + i64::from(WORLD_UNITS_PER_CELL),
            bottom: top + i64::from(WORLD_UNITS_PER_CELL),
        }
    }

    fn set_terrain(&mut self, cell: CellCoord, terrain: Terrain) {
        let index = self
            .cell_index(cell)
            .expect("changed terrain Cell is in Arena");
        self.terrain[index] = terrain;
    }

    fn cell_origin(&self, cell: CellCoord) -> Option<Position> {
        self.cell_index(cell)?;
        let units_per_cell = i64::from(WORLD_UNITS_PER_CELL);
        let x = i64::from(cell.column().value()) * units_per_cell;
        let y = i64::from(cell.row().value()) * units_per_cell;
        Some(Position::new(
            WorldX::new(i32::try_from(x).ok()?),
            WorldY::new(i32::try_from(y).ok()?),
        ))
    }

    fn cell_index(&self, cell: CellCoord) -> Option<usize> {
        let column = cell.column().value();
        let row = cell.row().value();
        if column >= self.width || row >= self.height {
            return None;
        }
        row.checked_mul(self.width)?
            .checked_add(column)
            .and_then(|index| usize::try_from(index).ok())
    }

    // Callers supply a positive-area rectangle wholly within the validated arena.
    fn obstructions(&self, rect: Rect) -> impl Iterator<Item = CellCoord> + '_ {
        let cell = i64::from(WORLD_UNITS_PER_CELL);
        let columns = rect.left / cell..=(rect.right - 1) / cell;
        let rows = rect.top / cell..=(rect.bottom - 1) / cell;
        rows.flat_map(move |row| {
            columns.clone().filter_map(move |column| {
                let cell = CellCoord::new(
                    Column::new(u32::try_from(column).expect("obstruction column is in arena")),
                    Row::new(u32::try_from(row).expect("obstruction row is in arena")),
                );
                (self
                    .terrain_at(cell)
                    .expect("obstruction cell is in validated arena")
                    != Terrain::Floor)
                    .then_some(cell)
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
    fn overlaps(self, other: Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    // Cardinal centre paths include both endpoints; Cell bounds are half-open.
    fn intersects_centre_segment(self, start: Position, end: Position) -> bool {
        i64::from(start.x.0.min(end.x.0)) < self.right
            && i64::from(start.x.0.max(end.x.0)) >= self.left
            && i64::from(start.y.0.min(end.y.0)) < self.bottom
            && i64::from(start.y.0.max(end.y.0)) >= self.top
    }

    fn footprint(position: Position, rules: GameRules) -> Self {
        let half_side = i64::from(rules.footprint_side / 2);
        Self {
            left: i64::from(position.x().value()) - half_side,
            top: i64::from(position.y().value()) - half_side,
            right: i64::from(position.x().value()) + half_side,
            bottom: i64::from(position.y().value()) + half_side,
        }
    }
}

// Actual paths share normalized tick time: constant speed while travelling, then rest.
// Cell occupancy changes only at boundary events. Test each event (half-open edges)
// and each open interval between them using exact rational times, never floats.
struct Motion {
    points: [Position; 3],
    speed: u32,
}

fn segment_length(start: Position, end: Position) -> i64 {
    i64::from((end.x.0 - start.x.0).abs() + (end.y.0 - start.y.0).abs())
}

impl Motion {
    fn cell_at(&self, arena: &Arena, numerator: i64, denominator: i64) -> Option<CellCoord> {
        let mut budget = i64::from(self.speed) * numerator;
        let mut x = i64::from(self.points[0].x.0) * denominator;
        let mut y = i64::from(self.points[0].y.0) * denominator;
        for pair in self.points.windows(2) {
            let distance = segment_length(pair[0], pair[1]) * denominator;
            let travelled = budget.min(distance);
            x += i64::from((pair[1].x.0 - pair[0].x.0).signum()) * travelled;
            y += i64::from((pair[1].y.0 - pair[0].y.0).signum()) * travelled;
            budget -= travelled;
        }
        arena.cell_containing(Position::new(
            WorldX::new((x / denominator) as i32),
            WorldY::new((y / denominator) as i32),
        ))
    }

    fn boundary_times(&self, times: &mut Vec<(i64, i64)>) {
        let mut travelled = 0;
        for pair in self.points.windows(2) {
            for (start, end) in [(pair[0].x.0, pair[1].x.0), (pair[0].y.0, pair[1].y.0)] {
                if start == end {
                    continue;
                }
                let first = start.min(end).div_euclid(WORLD_UNITS_PER_CELL) + 1;
                let last = start.max(end).div_euclid(WORLD_UNITS_PER_CELL);
                // Also include a segment starting exactly on an edge while moving negative.
                if start % WORLD_UNITS_PER_CELL == 0 {
                    times.push((travelled, i64::from(self.speed)));
                }
                for cell in first..=last {
                    let distance = (cell * WORLD_UNITS_PER_CELL - start).abs();
                    times.push((travelled + i64::from(distance), i64::from(self.speed)));
                }
            }
            travelled += segment_length(pair[0], pair[1]);
        }
    }

    fn contacts(&self, other: &Self, arena: &Arena) -> bool {
        let mut times = vec![(0, 1), (1, 1)];
        self.boundary_times(&mut times);
        other.boundary_times(&mut times);
        times.sort_by(|&(a, b), &(c, d)| (a * d).cmp(&(c * b)));
        times.dedup_by(|&mut (a, b), &mut (c, d)| a * d == c * b);
        let same_cell = |n, d| self.cell_at(arena, n, d) == other.cell_at(arena, n, d);
        times.iter().any(|&(n, d)| same_cell(n, d))
            || times.windows(2).any(|pair| {
                let [(a, b), (c, d)] = [pair[0], pair[1]];
                same_cell(a * d + c * b, 2 * b * d)
            })
    }
}

/// Terminal result of a run. A new Game starts without an outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Defeat,
    Victory,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuroruWandering {
    heading: Option<Direction>,
    remaining: u32,
}

impl BuroruWandering {
    pub fn heading(&self) -> Option<Direction> {
        self.heading
    }

    /// Active travel ticks left in the current run; zero requests a decision at a centre.
    pub fn remaining_ticks(&self) -> u32 {
        self.remaining
    }
}

// SplitMix64: the entire generator state is one word. Rejection avoids modulo bias.
fn random_choice(state: &mut u64, count: u64) -> usize {
    let threshold = count.wrapping_neg() % count;
    loop {
        *state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        if value >= threshold {
            return (value % count) as usize;
        }
    }
}

/// Complete simulation state. Equality and cloning include every authoritative field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    arena: Arena,
    character: Character,
    enemies: Vec<Character>,
    wandering: BTreeMap<CharacterId, BuroruWandering>,
    random_state: u64,
    tick: u64,
    rules: GameRules,
    bomb: Option<Bomb>,
    blasts: Vec<Blast>,
    exit: Option<CellCoord>,
    fire_up: Option<CellCoord>,
    // u32 base reach plus the one available upgrade always fits u64.
    current_blast_reach: u64,
    outcome: Option<Outcome>,
}

impl Game {
    /// Start from a validated scenario without replacing its rules or revalidating it.
    /// The game's clock is separate from the scenario; ordinary play starts at zero.
    pub fn new(scenario: Scenario, initial_tick: u64) -> Self {
        Self {
            arena: scenario.arena,
            character: scenario.character,
            wandering: scenario
                .enemies
                .iter()
                .map(|enemy| (enemy.id, BuroruWandering::default()))
                .collect(),
            random_state: scenario.random_seed,
            enemies: scenario.enemies,
            tick: initial_tick,
            rules: scenario.rules,
            bomb: None,
            blasts: Vec::new(),
            exit: scenario.exit,
            fire_up: scenario.fire_up,
            current_blast_reach: u64::from(scenario.rules.blast_reach),
            outcome: None,
        }
    }

    /// Advance one active tick; terminal runs remain frozen. No clock or held input.
    /// Errors preserve complete state, with priority duplicate, unknown, NPC, exhausted.
    pub fn step(&mut self, frame: &[CharacterCommand]) -> Result<(), StepError> {
        let mut identities = BTreeSet::new();
        for command in frame {
            if !identities.insert(command.character) {
                return Err(StepError::DuplicateCommand);
            }
        }
        if frame.iter().any(|command| {
            command.character != self.character.id
                && !self
                    .enemies
                    .iter()
                    .any(|enemy| enemy.id == command.character)
        }) {
            return Err(StepError::UnknownCharacter);
        }
        if frame
            .iter()
            .any(|command| command.character != self.character.id)
        {
            return Err(StepError::NpcCommand);
        }
        if self.outcome.is_some() {
            return Ok(());
        }
        let next_tick = self.tick.checked_add(1).ok_or(StepError::TickExhausted)?;
        // Stage policy and RNG with movement: deadline failure must roll back decisions too.
        let mut random_state = self.random_state;
        let mut wandering = self.wandering.clone();
        let mut enemies = self.enemies.clone();
        for enemy in &mut enemies {
            let policy = wandering
                .get_mut(&enemy.id)
                .expect("living enemy has policy");
            enemy.position = self.wander(*enemy, policy, &mut random_state);
        }
        let start = self.character.position;
        let [bend, position] = frame
            .first()
            .and_then(|command| command.movement)
            .map_or([start; 2], |direction| self.movement_path(direction));
        let path_touches = |cell| {
            let danger = self.arena.cell_bounds(cell);
            danger.intersects_centre_segment(start, bend)
                || danger.intersects_centre_segment(bend, position)
        };
        let path_harmed = self.blasts.iter().any(|blast| {
            blast.expires_at > next_tick && blast.cells.iter().copied().any(path_touches)
        }) || self.enemies.iter().zip(&enemies).any(|(before, after)| {
            Motion {
                points: [start, bend, position],
                speed: self.rules.movement_distance,
            }
            .contacts(
                &Motion {
                    points: [before.position, before.position, after.position],
                    speed: BURORU_SPEED,
                },
                &self.arena,
            )
        });
        let enemy_path_hits: BTreeSet<_> = self
            .enemies
            .iter()
            .zip(&enemies)
            .filter(|(before, after)| {
                self.blasts.iter().any(|blast| {
                    blast.expires_at > next_tick
                        && blast.cells.iter().any(|&cell| {
                            self.arena
                                .cell_bounds(cell)
                                .intersects_centre_segment(before.position, after.position)
                        })
                })
            })
            .map(|(before, _)| before.id)
            .collect();
        let final_cell = self
            .arena
            .cell_containing(position)
            .expect("Character stays in Arena");
        // Only the single eligible bomber collects; NPC movement must not use this phase.
        let collects_fire_up = self.fire_up == Some(final_cell)
            && self.arena.terrain_at(final_cell) == Some(Terrain::Floor);
        let current_blast_reach = self.current_blast_reach + u64::from(collects_fire_up);
        let mut bomb = self.bomb.clone();
        if frame.first().is_some_and(|command| command.place_bomb) && bomb.is_none() {
            bomb = Some(Bomb {
                cell: self
                    .arena
                    .cell_containing(position)
                    .expect("Character stays in Arena"),
                detonate_at: next_tick
                    .checked_add(self.rules.bomb_fuse_ticks)
                    .ok_or(StepError::TickExhausted)?,
                blast_reach: current_blast_reach,
                escape_allowed: std::iter::once(Character {
                    position,
                    ..self.character
                })
                .chain(enemies.iter().copied())
                .filter(|character| {
                    let rules = if character.kind == CharacterKind::Buroru {
                        GameRules::default()
                    } else {
                        self.rules
                    };
                    Rect::footprint(character.position, rules)
                        .overlaps(self.arena.cell_bounds(final_cell))
                })
                .map(|character| character.id)
                .collect(),
            });
        }
        if let Some(bomb) = &mut bomb
            && !Rect::footprint(position, self.rules).overlaps(self.arena.cell_bounds(bomb.cell))
        {
            bomb.escape_allowed.retain(|&id| id != self.character.id);
        }
        let blast = if let Some(bomb) = bomb.as_ref().filter(|bomb| {
            bomb.detonate_at <= next_tick
                || self
                    .blasts
                    .iter()
                    .any(|blast| blast.expires_at > next_tick && blast.cells.contains(&bomb.cell))
        }) {
            Some(Blast {
                cells: self.blast_coverage(bomb.cell, bomb.blast_reach),
                expires_at: next_tick
                    .checked_add(self.rules.blast_lifetime_ticks)
                    .ok_or(StepError::TickExhausted)?,
            })
        } else {
            None
        };
        // Everything fallible is checked before committing any live-state changes.
        self.enemies = enemies;
        self.wandering = wandering;
        self.random_state = random_state;
        if let Some(bomb) = &mut bomb {
            bomb.escape_allowed.retain(|id| {
                *id == self.character.id
                    || self.enemies.iter().any(|enemy| {
                        enemy.id == *id
                            && Rect::footprint(enemy.position, GameRules::default())
                                .overlaps(self.arena.cell_bounds(bomb.cell))
                    })
            });
        }
        self.blasts.retain(|blast| blast.expires_at > next_tick);
        self.character.position = position;
        if collects_fire_up {
            self.fire_up = None;
        }
        self.current_blast_reach = current_blast_reach;
        self.bomb = bomb;
        if let Some(blast) = blast {
            self.bomb = None;
            // Resolve Item hits once per new effect, before destroying its covering Block.
            // Fixed lingering coverage cannot newly expose an Item: never reapply its hit.
            // Thus the revealing effect preserves it, but any distinct later effect can hit it.
            if self.fire_up.is_some_and(|cell| {
                self.arena.terrain_at(cell) == Some(Terrain::Floor) && blast.cells.contains(&cell)
            }) {
                self.fire_up = None;
            }
            for &cell in &blast.cells {
                if self.arena.terrain_at(cell) == Some(Terrain::Block) {
                    self.arena.set_terrain(cell, Terrain::Floor);
                }
            }
            self.blasts.push(blast);
        }
        self.enemies.retain(|enemy| {
            let cell = self
                .arena
                .cell_containing(enemy.position)
                .expect("enemy stays in Arena");
            !enemy_path_hits.contains(&enemy.id)
                && !self.blasts.iter().any(|blast| blast.cells.contains(&cell))
        });
        self.wandering
            .retain(|id, _| self.enemies.iter().any(|enemy| enemy.id == *id));
        if path_harmed
            || self
                .blasts
                .iter()
                .any(|blast| blast.cells.contains(&final_cell))
        {
            self.outcome = Some(Outcome::Defeat);
        } else if self.exit == Some(final_cell) && self.exit_ready() {
            self.outcome = Some(Outcome::Victory);
        }
        self.tick = next_tick;
        Ok(())
    }

    fn wander(
        &self,
        enemy: Character,
        policy: &mut BuroruWandering,
        random_state: &mut u64,
    ) -> Position {
        let start = enemy.position;
        let sweep = |direction, distance| {
            self.swept_character_position(
                enemy.id,
                GameRules::default(),
                start,
                direction,
                distance,
            )
        };
        let centre = self
            .arena
            .cell_containing(start)
            .and_then(|cell| self.arena.cell_center(cell))
            == Some(start);
        if centre {
            let legal: Vec<_> = [
                Direction::Up,
                Direction::Right,
                Direction::Down,
                Direction::Left,
            ]
            .into_iter()
            .filter(|&direction| {
                sweep(direction, WORLD_UNITS_PER_CELL as u32)
                    == start.advanced(direction, WORLD_UNITS_PER_CELL as u32)
            })
            .collect();
            if policy.remaining == 0
                || !policy
                    .heading
                    .is_some_and(|heading| legal.contains(&heading))
            {
                if legal.is_empty() {
                    *policy = BuroruWandering::default();
                    return start;
                }
                // Ascending CharacterId, then direction (U,R,D,L), then run duration.
                policy.heading = Some(legal[random_choice(random_state, legal.len() as u64)]);
                policy.remaining = (random_choice(random_state, 4) as u32 + 1)
                    * WORLD_UNITS_PER_CELL as u32
                    / BURORU_SPEED;
            }
        }
        let Some(heading) = policy.heading else {
            return start;
        };
        let destination = sweep(heading, BURORU_SPEED);
        if destination != start.advanced(heading, BURORU_SPEED) {
            // A new non-overlapping Bomb may block a route already in progress.
            // Wait at contact; only a centre permits choosing a different heading.
            policy.remaining = 0;
        } else {
            policy.remaining = policy.remaining.saturating_sub(1);
        }
        destination
    }

    fn blast_coverage(&self, origin: CellCoord, reach: u64) -> Vec<CellCoord> {
        let mut cells = vec![origin];
        for direction in [
            Direction::Up,
            Direction::Right,
            Direction::Down,
            Direction::Left,
        ] {
            let mut cell = origin;
            for _ in 0..reach {
                let Some(next) = self.arena.neighbor(cell, direction) else {
                    break;
                };
                let terrain = self.arena.terrain_at(next).expect("ray stays in Arena");
                if terrain == Terrain::Wall {
                    break;
                }
                cells.push(next);
                if terrain == Terrain::Block {
                    break;
                }
                cell = next;
            }
        }
        cells
    }

    // The actual perpendicular bend and final position, never speculative probes.
    fn movement_path(&self, direction: Direction) -> [Position; 2] {
        let start = self.character.position;
        let distance = self.rules.movement_distance;
        let ordinary = self.swept_position(start, direction, distance);
        if !self.rules.turn_assistance || ordinary == start.advanced(direction, distance) {
            return [start, ordinary];
        }

        let perpendicular = match direction {
            Direction::Up | Direction::Down => [Direction::Left, Direction::Right],
            Direction::Left | Direction::Right => [Direction::Up, Direction::Down],
        };
        // A sweep gives each correction's safe prefix, including intervening
        // terrain and arena bounds. Search nearest-first within those prefixes.
        let corrections = perpendicular.map(|side| {
            let end = self.swept_position(start, side, MAX_TURN_CORRECTION);
            let reach = (end.x.0 - start.x.0).unsigned_abs() + (end.y.0 - start.y.0).unsigned_abs();
            (side, reach)
        });
        let max_correction = corrections.iter().map(|(_, reach)| *reach).max().unwrap();
        for correction in 1..=max_correction {
            for (side, reach) in corrections {
                if correction > reach {
                    continue;
                }
                let aligned = start.advanced(side, correction);
                if self.swept_position(aligned, direction, distance)
                    == aligned.advanced(direction, distance)
                {
                    let spent = correction.min(distance);
                    let corrected = start.advanced(side, spent);
                    return [
                        corrected,
                        self.swept_position(corrected, direction, distance - spent),
                    ];
                }
            }
        }
        [start, ordinary]
    }

    fn swept_position(&self, start: Position, direction: Direction, distance: u32) -> Position {
        self.swept_character_position(self.character.id, self.rules, start, direction, distance)
    }

    fn swept_character_position(
        &self,
        id: CharacterId,
        rules: GameRules,
        start: Position,
        direction: Direction,
        distance: u32,
    ) -> Position {
        // Re-evaluate at each segment's start, including speculative assistance.
        // A cardinal segment cannot re-enter after clearing; the next segment
        // treats the Bomb as solid even before the tick commits its permission.
        let escaping_bomb = self.bomb.as_ref().filter(|bomb| {
            bomb.escape_allowed_for(id)
                && Rect::footprint(start, rules).overlaps(self.arena.cell_bounds(bomb.cell))
        });
        let half = i64::from(rules.footprint_side / 2);
        let distance = i64::from(distance);
        let cell = i64::from(WORLD_UNITS_PER_CELL);
        let mut x = i64::from(start.x().value());
        let mut y = i64::from(start.y().value());
        let mut swept = Rect::footprint(start, rules);
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
        let blocking_bomb = self
            .bomb
            .iter()
            .filter(|bomb| {
                escaping_bomb.is_none() && swept.overlaps(self.arena.cell_bounds(bomb.cell))
            })
            .map(|bomb| bomb.cell);
        for obstruction in self.arena.obstructions(swept).chain(blocking_bomb) {
            let origin = self
                .arena
                .cell_origin(obstruction)
                .expect("obstruction cell is in validated arena");
            let left_edge = i64::from(origin.x().value());
            let top_edge = i64::from(origin.y().value());
            match direction {
                Direction::Up => y = y.max(top_edge + cell + half),
                Direction::Right => x = x.min(left_edge - half),
                Direction::Down => y = y.min(top_edge - half),
                Direction::Left => x = x.max(left_edge + cell + half),
            }
        }
        Position::new(
            WorldX::new(i32::try_from(x).expect("movement stays within validated arena extents")),
            WorldY::new(i32::try_from(y).expect("movement stays within validated arena extents")),
        )
    }

    /// Item location, including while covered; terrain determines its visibility.
    pub fn fire_up(&self) -> Option<CellCoord> {
        self.fire_up
    }

    /// The eligible bomber's current ability, separate from immutable base rules.
    pub fn current_blast_reach(&self) -> u64 {
        self.current_blast_reach
    }

    /// Derived objective eligibility, never a separately mutable unlock flag.
    pub fn exit_ready(&self) -> bool {
        self.enemies.is_empty()
            && self
                .exit
                .is_some_and(|cell| self.arena.terrain_at(cell) == Some(Terrain::Floor))
    }

    /// Read-only continuation policy for a living Buroru, absent for other identities.
    pub fn buroru_wandering(&self, id: CharacterId) -> Option<&BuroruWandering> {
        self.wandering.get(&id)
    }

    pub fn enemies(&self) -> &[Character] {
        &self.enemies
    }

    pub fn exit(&self) -> Option<CellCoord> {
        self.exit
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
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

    pub fn rules(&self) -> &GameRules {
        &self.rules
    }

    pub fn bomb(&self) -> Option<&Bomb> {
        self.bomb.as_ref()
    }

    pub fn blasts(&self) -> &[Blast] {
        &self.blasts
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn cardinal_commands_apply_only_to_their_tick() {
        let base = Game::new(
            scenario(3, 3, vec![Terrain::Floor; 9], 384, 384)
                .parse(GameRules::default())
                .unwrap(),
            0,
        );
        for (direction, expected) in [
            (Direction::Up, point(384, 368)),
            (Direction::Right, point(400, 384)),
            (Direction::Down, point(384, 400)),
            (Direction::Left, point(368, 384)),
        ] {
            let mut game = base.clone();
            assert_step(&mut game, &[command(direction)], expected);
        }
        let mut game = base;
        assert_step(&mut game, &[], point(384, 384));
        assert_step(&mut game, &[command(Direction::Right)], point(400, 384));
        assert_step(&mut game, &[command(Direction::Right)], point(416, 384));
        assert_step(&mut game, &[], point(416, 384));
        assert_step(&mut game, &[command(Direction::Left)], point(400, 384));
    }

    #[test]
    fn swept_footprints_stop_at_the_first_obstruction_without_a_gap_or_sideways_motion() {
        use Direction::*;
        let fast = GameRules::new(192, 512).unwrap();
        let cases = [
            (rows(".#..", 128, 128), fast, Right, point(160, 128)),
            (rows(".+..", 128, 128), fast, Right, point(160, 128)),
            (rows("..#.", 896, 128), fast, Left, point(864, 128)),
            (rows("..+.", 896, 128), fast, Left, point(864, 128)),
            (
                rows(
                    r#"
.
#
.
.
"#,
                    128,
                    128,
                ),
                fast,
                Down,
                point(128, 160),
            ),
            (
                rows(
                    r#"
.
+
.
.
"#,
                    128,
                    128,
                ),
                fast,
                Down,
                point(128, 160),
            ),
            (
                rows(
                    r#"
.
.
#
.
"#,
                    128,
                    896,
                ),
                fast,
                Up,
                point(128, 864),
            ),
            (
                rows(
                    r#"
.
.
+
.
"#,
                    128,
                    896,
                ),
                fast,
                Up,
                point(128, 864),
            ),
            (
                rows("..#..+..", 128, 128),
                GameRules::new(192, 1024).unwrap(),
                Right,
                point(416, 128),
            ),
            (
                rows("..#.+...", 1920, 128),
                GameRules::new(192, 1024).unwrap(),
                Left,
                point(1376, 128),
            ),
        ];
        for (authored, rules, direction, expected) in cases {
            let mut game = Game::new(authored.clone().parse(rules).unwrap(), 0);
            assert_step(&mut game, &[command(direction)], expected);
            assert_step(&mut game, &[command(direction)], expected);
            assert_step(&mut game, &[], expected);
        }
        let mut approaching = Game::new(
            rows(".#..", 128, 128).parse(GameRules::default()).unwrap(),
            0,
        );
        for x in [144, 160, 160] {
            assert_step(&mut approaching, &[command(Right)], point(x, 128));
        }
    }

    #[test]
    fn floor_edges_are_solid_and_exact_contact_is_safe() {
        for (direction, expected) in [
            (Direction::Up, point(128, 96)),
            (Direction::Right, point(160, 128)),
            (Direction::Down, point(128, 160)),
            (Direction::Left, point(96, 128)),
        ] {
            let mut game = Game::new(
                rows(".", 128, 128)
                    .parse(GameRules::new(192, 1024).unwrap())
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
                .parse(GameRules::new(side, 1024).unwrap())
                .unwrap(),
                0,
            );
            assert_step(
                &mut game,
                &[command(Direction::Right)],
                point(65536 - radius, radius + 1),
            );
            assert_step(
                &mut game,
                &[command(Direction::Up)],
                point(65536 - radius, radius),
            );
            assert_step(
                &mut game,
                &[command(Direction::Left)],
                point(64512 - radius, radius),
            );
            assert_step(
                &mut game,
                &[command(Direction::Down)],
                point(64512 - radius, radius + 1024),
            );
        }
    }

    #[test]
    fn a_blocked_turn_spends_its_tick_budget_on_the_nearest_legal_entry() {
        let mut game = Game::new(
            rows("...\n.#.\n...", 192, 160)
                .parse(GameRules::default())
                .unwrap(),
            0,
        );
        assert_step(&mut game, &[command(Direction::Down)], point(176, 160));
        let mut changed_direction = game.clone();
        assert_step(
            &mut changed_direction,
            &[command(Direction::Right)],
            point(192, 160),
        );
        assert_step(
            &mut changed_direction,
            &[command(Direction::Down)],
            point(176, 160),
        );
        assert_rejected(
            &mut game,
            &[command(Direction::Down), command(Direction::Up)],
            StepError::DuplicateCommand,
        );
        assert_step(&mut game, &[], point(176, 160));
        assert_step(&mut game, &[command(Direction::Down)], point(160, 160));
        assert_step(&mut game, &[command(Direction::Down)], point(160, 176));
    }

    #[test]
    fn a_safe_entry_can_be_away_from_the_current_cell_centreline() {
        let mut game = Game::new(
            rows("...\n#..\n...", 352, 272)
                .parse(GameRules::default())
                .unwrap(),
            0,
        );
        // The current Cell centreline is y=384 (Down), but the nearest safe
        // Left entry is y=160 (Up): 112 units of real correction.
        for y in [256, 240, 224, 208, 192, 176, 160] {
            assert_step(&mut game, &[command(Direction::Left)], point(352, y));
        }
        assert_step(&mut game, &[command(Direction::Left)], point(336, 160));
    }

    #[test]
    fn assistance_is_symmetric_and_ignores_current_cell_offsets() {
        use Direction::*;
        // A central pillar with Floor on every side. All coordinates are worked
        // examples at Footprint 192 / speed 16, independent of collision helpers.
        for (x, y, direction, expected) in [
            (208, 160, Down, point(192, 160)),
            (209, 160, Down, point(193, 160)), // formerly rejected at offset +81
            (560, 160, Down, point(576, 160)),
            (559, 160, Down, point(575, 160)), // formerly rejected at offset -81
            (208, 608, Up, point(192, 608)),
            (560, 608, Up, point(576, 608)),
            (160, 208, Right, point(160, 192)),
            (160, 209, Right, point(160, 193)),
            (160, 560, Right, point(160, 576)),
            (160, 559, Right, point(160, 575)),
            (608, 208, Left, point(608, 192)),
            (608, 560, Left, point(608, 576)),
            (161, 160, Down, point(160, 175)), // one sideways + fifteen forward
            (160, 160, Down, point(160, 176)), // legal edge: no correction
            (159, 160, Down, point(159, 176)), // legal off-centre travel unchanged
            (128, 150, Down, point(128, 166)), // centred and clear
            (160, 384, Up, point(160, 368)),   // parallel travel unchanged
            (160, 384, Down, point(160, 400)),
            (192, 150, Down, point(176, 150)), // correction first, not partial forward
        ] {
            let mut game = Game::new(
                rows("...\n.#.\n...", x, y)
                    .parse(GameRules::default())
                    .unwrap(),
                0,
            );
            assert_step(&mut game, &[command(direction)], expected);
        }
    }

    #[test]
    fn the_actual_correction_limit_is_inclusive_in_both_axes_and_directions() {
        use Direction::*;
        // Nearest legal edges are 160 and 608. Required corrections are
        // 127, 128, or 129 units; a >128 correction must retain the strict stop.
        for (coordinate, corrected) in [
            (287, 271),
            (288, 272),
            (289, 289),
            (479, 479),
            (480, 496),
            (481, 497),
        ] {
            for (direction, start, expected) in [
                (Down, point(coordinate, 160), point(corrected, 160)),
                (Up, point(coordinate, 608), point(corrected, 608)),
                (Right, point(160, coordinate), point(160, corrected)),
                (Left, point(608, coordinate), point(608, corrected)),
            ] {
                let authored = rows("...\n.#.\n...", start.x().value(), start.y().value());
                let mut assisted =
                    Game::new(authored.clone().parse(GameRules::default()).unwrap(), 0);
                let mut strict = Game::new(
                    authored
                        .parse(GameRules::default().with_turn_assistance(false))
                        .unwrap(),
                    0,
                );
                assert_step(&mut assisted, &[command(direction)], expected);
                assert_step(&mut strict, &[command(direction)], start);
            }
        }
    }

    #[test]
    fn reported_playtest_stalls_reach_safe_entries_but_real_walls_stay_blocked() {
        let layout = "#########\n#.......#\n#.#.#.#.#\n#....+..#\n#.#.#.#.#\n#.......#\n#.#.#.#.#\n#....+..#\n#########";
        for (start, direction, ticks, entry, after_forward) in [
            (
                point(864, 768),
                Direction::Left,
                6,
                point(864, 864),
                point(848, 864),
            ),
            (
                point(864, 1008),
                Direction::Left,
                5,
                point(864, 928),
                point(848, 928),
            ),
            (
                point(1008, 928),
                Direction::Down,
                5,
                point(928, 928),
                point(928, 944),
            ),
            (
                point(864, 1040),
                Direction::Left,
                7,
                point(864, 928),
                point(848, 928),
            ),
            (
                point(864, 784),
                Direction::Left,
                5,
                point(864, 864),
                point(848, 864),
            ),
            (
                point(352, 528),
                Direction::Left,
                8,
                point(352, 528),
                point(352, 528),
            ),
        ] {
            let mut game = Game::new(
                rows(layout, start.x().value(), start.y().value())
                    .parse(GameRules::default())
                    .unwrap(),
                0,
            );
            for _ in 0..ticks {
                game.step(&[command(direction)]).unwrap();
            }
            assert_eq!(game.character().position, entry);
            assert_eq!(game.tick(), ticks);
            assert_step(&mut game, &[command(direction)], after_forward);
        }
    }

    #[test]
    fn assistance_cannot_cross_an_obstruction_to_reach_a_forward_opening() {
        let mut game = Game::new(
            rows(".#.\n#..\n#..", 257, 257)
                .parse(GameRules::new(2, 1).unwrap())
                .unwrap(),
            0,
        );
        // The Left opening above is only two units away, but the Up
        // correction would penetrate the Wall directly above the Character.
        assert_step(&mut game, &[command(Direction::Left)], point(257, 257));
    }

    #[test]
    fn assistance_never_drifts_toward_a_blocked_or_unsafe_forward_path() {
        use Direction::*;
        for (layout, start, distance, direction, expected) in [
            ("...\n.+.\n...", point(192, 160), 16, Down, point(176, 160)),
            ("...\n###\n...", point(192, 150), 16, Down, point(192, 160)),
            ("...\n+++\n...", point(192, 160), 16, Down, point(192, 160)),
            (
                "...\n.#.\n#..\n...\n...",
                point(192, 160),
                1024,
                Down,
                point(192, 160),
            ),
            (
                "...\n.+.\n+..\n...\n...",
                point(192, 160),
                1024,
                Down,
                point(192, 160),
            ),
            (
                "...\n.#.\n...",
                point(192, 160),
                1024,
                Down,
                point(192, 160),
            ),
            ("...\n...\n...", point(192, 96), 16, Up, point(192, 96)),
            ("...\n...\n...", point(96, 192), 16, Left, point(96, 192)),
            ("...\n...\n...", point(672, 576), 16, Right, point(672, 576)),
            ("...\n...\n...", point(576, 672), 16, Down, point(576, 672)),
            // The nearest safe entry is 144 units away: beyond the correction limit.
            ("...\n.#.\n...", point(464, 160), 16, Down, point(464, 160)),
        ] {
            let mut game = Game::new(
                rows(layout, start.x().value(), start.y().value())
                    .parse(GameRules::new(192, distance).unwrap())
                    .unwrap(),
                0,
            );
            assert_step(&mut game, &[command(direction)], expected);
        }
    }

    #[test]
    fn strict_off_centre_corridors_and_corner_contact_do_not_align_or_slide_the_character() {
        let strict = GameRules::default().with_turn_assistance(false);
        let classic = r#"
#######
#.....#
#.#.#.#
#.....#
#.#.#.#
#.....#
#######
"#;
        for x in [352, 416] {
            let mut game = Game::new(rows(classic, x, 640).parse(strict).unwrap(), 0);
            assert_step(&mut game, &[command(Direction::Up)], point(x, 624));
        }
        for x in [351, 417] {
            assert_eq!(
                rows(classic, x, 640).parse(strict),
                Err(ConstructionError::InvalidStartingPosition)
            );
        }
        let mut turn = Game::new(rows(classic, 400, 417).parse(strict).unwrap(), 0);
        assert_step(&mut turn, &[command(Direction::Right)], point(416, 417));
        assert_step(&mut turn, &[command(Direction::Right)], point(416, 417));
        assert_step(&mut turn, &[command(Direction::Up)], point(416, 401));
        assert_step(&mut turn, &[command(Direction::Right)], point(432, 401));

        let mut corner = Game::new(
            rows(
                r#"
..
.#
"#,
                160,
                160,
            )
            .parse(strict)
            .unwrap(),
            0,
        );
        assert_step(&mut corner, &[command(Direction::Right)], point(176, 160));
        let mut blocked_corner = Game::new(
            rows(
                r#"
..
.#
"#,
                160,
                161,
            )
            .parse(strict)
            .unwrap(),
            0,
        );
        assert_step(
            &mut blocked_corner,
            &[command(Direction::Right)],
            point(160, 161),
        );
        let mut spanning = Game::new(
            rows(
                r#"
..
..
"#,
                256,
                256,
            )
            .parse(strict)
            .unwrap(),
            0,
        );
        assert_step(&mut spanning, &[command(Direction::Down)], point(256, 272));
        let mut perpendicular = Game::new(
            rows(
                r#"
.#.
...
"#,
                384,
                352,
            )
            .parse(strict)
            .unwrap(),
            0,
        );
        assert_step(
            &mut perpendicular,
            &[command(Direction::Right)],
            point(400, 352),
        );
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

    // Independent reference: enumerate ALL terrain rectangles and measure positive-area
    // overlap directly. No production geometry, cell query, or advancement helper is used.
    fn reference_legal(authored: &ScenarioInput, rules: GameRules, position: Position) -> bool {
        let radius = i64::from(rules.footprint_side() / 2);
        let (left, right) = (
            i64::from(position.x().value()) - radius,
            i64::from(position.x().value()) + radius,
        );
        let (top, bottom) = (
            i64::from(position.y().value()) - radius,
            i64::from(position.y().value()) + radius,
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

    fn reference_direction_delta(direction: Direction) -> (i32, i32) {
        match direction {
            Direction::Up => (0, -1),
            Direction::Right => (1, 0),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
        }
    }

    fn reference_cardinal(
        authored: &ScenarioInput,
        rules: GameRules,
        mut position: Position,
        direction: Direction,
        distance: u32,
    ) -> Position {
        let (dx, dy) = reference_direction_delta(direction);
        for _ in 0..distance {
            let next = point(position.x().value() + dx, position.y().value() + dy);
            if !reference_legal(authored, rules, next) {
                break;
            }
            position = next;
        }
        position
    }

    fn reference_move(
        authored: &ScenarioInput,
        rules: GameRules,
        direction: Direction,
    ) -> Position {
        let start = authored.character.position;
        let distance = rules.movement_distance();
        let (dx, dy) = reference_direction_delta(direction);
        let ordinary = reference_cardinal(authored, rules, start, direction, distance);
        let desired = point(
            start.x().value() + dx * distance as i32,
            start.y().value() + dy * distance as i32,
        );
        if !rules.turn_assistance() || ordinary == desired {
            return ordinary;
        }
        // A sideways correction cannot repair a forward arena-boundary violation.
        let radius = (rules.footprint_side() / 2) as i32;
        let (forward, extent) = if dx == 0 {
            (desired.y().value(), authored.height)
        } else {
            (desired.x().value(), authored.width)
        };
        if !(radius..=extent as i32 * 256 - radius).contains(&forward) {
            return ordinary;
        }
        let perpendicular = if dx == 0 {
            [(-1, 0), (1, 0)]
        } else {
            [(0, -1), (0, 1)]
        };
        let mut best: Option<(u32, i32, i32)> = None;
        // Independently walk each complete sideways route one unit at a time,
        // then compare its nearest safe entry with the other route's entry.
        for (sx, sy) in perpendicular {
            for correction in 1..=128 {
                let candidate = point(
                    start.x().value() + sx * correction as i32,
                    start.y().value() + sy * correction as i32,
                );
                if !reference_legal(authored, rules, candidate) {
                    break;
                }
                let desired = point(
                    candidate.x().value() + dx * distance as i32,
                    candidate.y().value() + dy * distance as i32,
                );
                if reference_cardinal(authored, rules, candidate, direction, distance) == desired {
                    let entry = (correction, sx, sy);
                    best = Some(best.map_or(entry, |other| other.min(entry)));
                    break;
                }
            }
        }
        if let Some((correction, sx, sy)) = best {
            let spent = correction.min(distance);
            let partial = point(
                start.x().value() + sx * spent as i32,
                start.y().value() + sy * spent as i32,
            );
            return reference_cardinal(authored, rules, partial, direction, distance - spent);
        }
        ordinary
    }

    #[test]
    fn off_centre_corridor_entries_make_safe_progress_within_fifteen_ticks() {
        use Direction::*;
        let rules = GameRules::default();
        // Downward entrance at y=768, with legal corridor centres x=608..=672.
        // At contact, y=672 is 96 units before the mouth. Even a 128-unit
        // correction plus 97 forward units fits in 15 ticks at 16 units/tick.
        let entrance = rows("#####\n#...#\n#...#\n##.##\n#####", 640, 672);
        for (quarter_turns, direction) in [(0, Down), (1, Left), (2, Up), (3, Right)] {
            let rotate = |mut x, mut y| {
                for _ in 0..quarter_turns {
                    (x, y) = (1280 - y, x);
                }
                point(x, y)
            };
            let mut terrain = entrance.terrain.clone();
            for _ in 0..quarter_turns {
                terrain = (0..25)
                    .map(|index| terrain[(4 - index % 5) * 5 + index / 5])
                    .collect();
            }
            let mouth = rotate(640, 768);
            let (dx, dy) = reference_direction_delta(direction);
            for side in [-1, 1] {
                for correction in 1..=128 {
                    let start = rotate(640 + side * (32 + correction), 672);
                    let authored =
                        scenario(5, 5, terrain.clone(), start.x().value(), start.y().value());
                    let mut game = Game::new(authored.clone().parse(rules).unwrap(), 0);
                    let case = (direction, side, correction);
                    let mut entered = false;
                    for tick in 1..=15 {
                        let before = game.character().position;
                        game.step(&[command(direction)]).unwrap();
                        let after = game.character().position;
                        assert_eq!(game.tick(), tick, "case={case:?}");
                        assert!(
                            reference_legal(&authored, rules, after),
                            "case={case:?}, tick={tick}"
                        );
                        let travel = before.x().value().abs_diff(after.x().value())
                            + before.y().value().abs_diff(after.y().value());
                        assert!(travel <= 16, "case={case:?}, tick={tick}, travel={travel}");

                        // Check only the observed path: sideways, then commanded
                        // forward travel. No oracle predicts the correction chosen.
                        let (bend, sideways_direction) = if dx == 0 {
                            (
                                point(after.x().value(), before.y().value()),
                                if after.x().value() < before.x().value() {
                                    Left
                                } else {
                                    Right
                                },
                            )
                        } else {
                            (
                                point(before.x().value(), after.y().value()),
                                if after.y().value() < before.y().value() {
                                    Up
                                } else {
                                    Down
                                },
                            )
                        };
                        let sideways = before.x().value().abs_diff(bend.x().value())
                            + before.y().value().abs_diff(bend.y().value());
                        let forward = (after.x().value() - bend.x().value()) * dx
                            + (after.y().value() - bend.y().value()) * dy;
                        assert!(forward >= 0, "case={case:?}, tick={tick}");
                        assert_eq!(
                            reference_cardinal(
                                &authored,
                                rules,
                                before,
                                sideways_direction,
                                sideways
                            ),
                            bend,
                            "unsafe correction: case={case:?}, tick={tick}"
                        );
                        assert_eq!(
                            reference_cardinal(&authored, rules, bend, direction, forward as u32),
                            after,
                            "unsafe forward path: case={case:?}, tick={tick}"
                        );
                        if (after.x().value() - mouth.x().value()) * dx
                            + (after.y().value() - mouth.y().value()) * dy
                            > 0
                        {
                            entered = true;
                            break;
                        }
                    }
                    assert!(
                        entered,
                        "did not enter within 15 ticks: case={case:?}, position={:?}",
                        game.character().position
                    );
                }
            }
        }
    }

    #[test]
    fn deterministic_sequences_preserve_invariants_and_clones_continue_independently() {
        use Direction::*;
        let authored = rows(
            r#"
#######
#.....#
#.#.#.#
#..+..#
#.#.#.#
#.....#
#######
"#,
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
                    vec![CharacterCommand {
                        character: CharacterId::new(43),
                        movement: Some(Right),
                        place_bomb: false,
                    }],
                    Err(StepError::UnknownCharacter),
                ));
            }
        }
        for rules in [
            GameRules::default(),
            GameRules::new(2, 1).unwrap(),
            GameRules::new(128, 257).unwrap(),
            GameRules::new(192, 512).unwrap(),
            GameRules::new(254, 1024).unwrap(),
        ]
        .into_iter()
        .flat_map(|rules| [rules.with_turn_assistance(false), rules])
        {
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
                            reference_move(&reference, rules, input.movement.unwrap());
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

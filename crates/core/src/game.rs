//! Deterministic movement, Bomb/Blast simulation, and validated game state.

use std::collections::BTreeSet;

mod rules;
mod scenario;
mod simulation;

const BURORU_MOVEMENT_DISTANCE: u32 = 8;
const GENERATED_ARENA_SIZE: u32 = 13;

/// Simulation units, not pixels. Cell intervals are lower-inclusive, upper-exclusive.
pub const WORLD_UNITS_PER_CELL: i32 = 256;
// Buroru endpoint-only Blast damage relies on movement shorter than one Cell.
const _: () = assert!(BURORU_MOVEMENT_DISTANCE < WORLD_UNITS_PER_CELL as u32);
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

/// A human-controlled bomber's authored start for Normal mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanBomberStart {
    pub id: CharacterId,
    pub position: Position,
}

/// A Buroru's authored start. Scenario resolves its Cell centre and validates placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuroruStart {
    pub id: CharacterId,
    pub cell: CellCoord,
}

/// Read-only Character observations retain kind and control as distinct concepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub position: Position,
    pub kind: CharacterKind,
    pub control: ControlSource,
}

impl From<HumanBomberStart> for Character {
    fn from(start: HumanBomberStart) -> Self {
        Self {
            id: start.id,
            position: start.position,
            kind: CharacterKind::Bomber,
            control: ControlSource::Human,
        }
    }
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
    InvalidBlockDensity,
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

// SplitMix64: the entire generator state is one word. Rejection avoids modulo bias.
fn random_choice(state: &mut u64, count: u64) -> usize {
    if count == 1 {
        return 0;
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct BuroruState {
    character: Character,
    heading: Option<Direction>,
}

/// Complete simulation state. Equality and cloning include every authoritative field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    arena: Arena,
    character: Character,
    enemies: Vec<BuroruState>,
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

#[cfg(test)]
mod tests;

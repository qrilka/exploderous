use super::*;

impl Scenario {
    /// Generate a starting Scenario for Normal mode.
    /// `block_percent` is a target in 0..=100; required placements take precedence.
    /// Advance caller-owned random state only on success. Retain this state
    /// separately from the active Game's AI state for subsequent generation.
    pub fn generate(
        rules: GameRules,
        block_percent: u8,
        random_state: &mut u64,
    ) -> Result<Self, ConstructionError> {
        if block_percent > 100 {
            return Err(ConstructionError::InvalidBlockDensity);
        }
        let mut state = *random_state;
        let terrain = (0..GENERATED_ARENA_SIZE)
            .flat_map(|row| {
                (0..GENERATED_ARENA_SIZE).map(move |column| {
                    if column % 2 == 1 && row % 2 == 1 {
                        Terrain::Wall
                    } else {
                        Terrain::Floor
                    }
                })
            })
            .collect();
        let mut arena = Arena::new(GENERATED_ARENA_SIZE, GENERATED_ARENA_SIZE, terrain)?;
        let cells: Vec<_> = arena
            .cells()
            .filter_map(|(cell, terrain)| (terrain == Terrain::Floor).then_some(cell))
            .collect();
        let mut reserved: Vec<_> = cells
            .iter()
            .copied()
            .filter(|cell| {
                let (x, y) = (cell.column().value(), cell.row().value());
                (x == 0 && y <= 2) || (y == 0 && x <= 2)
            })
            .collect();
        let player = Character {
            id: CharacterId::new(0),
            position: arena
                .cell_center(reserved[0])
                .expect("opening is in bounds"),
            kind: CharacterKind::Bomber,
            control: ControlSource::Human,
        };
        // In this classic grid, every non-pillar has a monotone route from (0,0),
        // so corridor distance ignoring Blocks is exactly column + row.
        let mut candidates: Vec<_> = cells
            .iter()
            .copied()
            .filter(|cell| cell.column().value() + cell.row().value() >= 6)
            .collect();
        let mut enemies = Vec::new();
        for id in 1..=3 {
            let index = random_choice(&mut state, candidates.len() as u64);
            let cell = candidates.remove(index);
            let neighbors: Vec<_> = [
                Direction::Up,
                Direction::Right,
                Direction::Down,
                Direction::Left,
            ]
            .into_iter()
            .filter_map(|direction| arena.neighbor(cell, direction))
            .filter(|&cell| arena.terrain_at(cell) == Some(Terrain::Floor))
            .collect();
            reserved.push(cell);
            reserved.push(neighbors[random_choice(&mut state, neighbors.len() as u64)]);
            enemies.push(Character {
                id: CharacterId::new(id),
                position: arena.cell_center(cell).expect("candidate is in bounds"),
                kind: CharacterKind::Buroru,
                control: ControlSource::Npc,
            });
        }
        let mut eligible: Vec<CellCoord> = cells
            .into_iter()
            .filter(|cell| !reserved.contains(cell))
            .collect();
        let exit_index = random_choice(&mut state, eligible.len() as u64);
        let exit = eligible.remove(exit_index);
        let item_index = random_choice(&mut state, eligible.len() as u64);
        let item = eligible.remove(item_index);
        arena.set_terrain(exit, Terrain::Block);
        arena.set_terrain(item, Terrain::Block);
        for cell in eligible {
            if random_choice(&mut state, 100) < usize::from(block_percent) {
                arena.set_terrain(cell, Terrain::Block);
            }
        }
        let scenario = Self::new(
            GENERATED_ARENA_SIZE,
            GENERATED_ARENA_SIZE,
            arena.terrain,
            player,
            rules,
        )?
        .with_enemies(enemies)?
        .with_exit(exit)?
        .with_fire_up(item)?
        .with_random_seed(state);
        *random_state = state;
        Ok(scenario)
    }

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

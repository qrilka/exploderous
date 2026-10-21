use super::*;

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
                .map(|enemy| (enemy.id, None))
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
                    speed: BURORU_MOVEMENT_DISTANCE,
                },
                &self.arena,
            )
        });
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
            // A survivor starts clear of live Blasts. Its straight, sub-Cell step
            // cannot cross a dangerous Cell with both endpoints safe.
            !self.blasts.iter().any(|blast| blast.cells.contains(&cell))
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
        heading: &mut Option<Direction>,
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
            if legal.is_empty() {
                return start;
            }
            *heading = Some(match *heading {
                None => legal[random_choice(random_state, legal.len() as u64)],
                Some(forward) => {
                    let perpendicular = match forward {
                        Direction::Up | Direction::Down => [Direction::Right, Direction::Left],
                        Direction::Right | Direction::Left => [Direction::Up, Direction::Down],
                    };
                    let sides: Vec<_> = perpendicular
                        .into_iter()
                        .filter(|direction| legal.contains(direction))
                        .collect();
                    if legal.contains(&forward) {
                        if !sides.is_empty() && random_choice(random_state, 10) == 0 {
                            sides[random_choice(random_state, sides.len() as u64)]
                        } else {
                            forward
                        }
                    } else {
                        let reverse = match forward {
                            Direction::Up => Direction::Down,
                            Direction::Right => Direction::Left,
                            Direction::Down => Direction::Up,
                            Direction::Left => Direction::Right,
                        };
                        if legal.contains(&reverse)
                            && (sides.is_empty() || random_choice(random_state, 4) != 0)
                        {
                            reverse
                        } else {
                            sides[random_choice(random_state, sides.len() as u64)]
                        }
                    }
                }
            });
        }
        heading.map_or(start, |direction| {
            sweep(direction, BURORU_MOVEMENT_DISTANCE)
        })
    }

    fn blast_coverage(&self, origin: CellCoord, reach: u64) -> Vec<CellCoord> {
        let mut cells = vec![origin];
        for direction in [
            Direction::Up,
            Direction::Right,
            Direction::Down,
            Direction::Left,
        ] {
            let axis_length = match direction {
                Direction::Up | Direction::Down => self.arena.height(),
                Direction::Right | Direction::Left => self.arena.width(),
            };
            let mut cell = origin;
            // A straight ray can cover at most axis_length - 1 Cells beyond its origin.
            for _ in 0..reach.min(u64::from(axis_length - 1)) {
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
        // A straight segment cannot re-enter after clearance. If a perpendicular
        // correction clears the Bomb, the forward leg stays clear even with permission.
        let escaping_bomb = self
            .bomb
            .as_ref()
            .filter(|bomb| bomb.escape_allowed_for(id));
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

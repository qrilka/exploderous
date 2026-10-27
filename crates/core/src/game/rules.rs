use super::*;

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
            bomb_fuse_ticks: TickSpan::new(120).unwrap(),
            blast_reach: 1,
            blast_lifetime_ticks: TickSpan::new(30).unwrap(),
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
        let fuse_ticks = TickSpan::new(fuse_ticks).ok_or(ConstructionError::InvalidBombFuse)?;
        if reach == 0 {
            return Err(ConstructionError::InvalidBlastReach);
        }
        let lifetime_ticks =
            TickSpan::new(lifetime_ticks).ok_or(ConstructionError::InvalidBlastLifetime)?;
        self.bomb_fuse_ticks = fuse_ticks;
        self.blast_reach = reach;
        self.blast_lifetime_ticks = lifetime_ticks;
        Ok(self)
    }

    pub fn bomb_fuse_ticks(&self) -> TickSpan {
        self.bomb_fuse_ticks
    }

    pub fn blast_reach(&self) -> u32 {
        self.blast_reach
    }

    pub fn blast_lifetime_ticks(&self) -> TickSpan {
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

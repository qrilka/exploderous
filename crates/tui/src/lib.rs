use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use exploderous_core::game::{
    CharacterCommand, Direction, Game, GameRules, Outcome, Position, Scenario, StepError, Terrain,
    Tick,
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    symbols::border,
    text::Line,
    widgets::{Block, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const TICK_INTERVAL: Duration = Duration::from_nanos(16_666_667);
pub const MIN_WIDTH: u16 = 19;
pub const MIN_HEIGHT: u16 = ARENA_SIZE + 4; // Exterior frame and two status rows.
const MAX_CATCH_UP_TICKS: usize = 5;
const ARENA_SIZE: u16 = 13;
const HELP: &str = r#"Controls
Arrow keys: move; latest press wins; release falls back
Space: place one Bomb; place another after it detonates
While overlapping a new Bomb, you can move toward or across it
Once your whole Footprint clears the Bomb Cell, re-entry is blocked
Blasts clear Blocks; entering a Blast Cell with your centre is lethal: Game Over
Bombs reveal an Exit hidden beneath a Block; defeat all three Buroru, then reach it alive
Buroru wander at half player speed; one Blast hit kills each
Sharing a centre Cell with a Buroru is lethal; overlapping bodies in adjacent Cells are safe
Reveal Fire-up beneath another Block; let the Blast expire before collecting it
Collect Fire-up with your centre to add one Cell of reach to later Bombs
Blast reach shows your current ability; ticking Bombs keep their old reach
A later Blast can destroy exposed Fire-up; collecting it is not required to win
Victory or Game Over freezes play; only restart or quit acts
R: restart with a freshly generated level; Q or Escape: quit
?, h, or H: open or close help
Up/Down: scroll; Escape closes help
Offset: signed X,Y world-unit delta from the current Cell centre; 0,0 is centred

Live map: 13×13 playable Cells, plus a separate exterior frame
▓ Exterior boundary (not terrain)
(blank space) Floor
█ Wall pillar
▒ Block
@ Player
e Buroru (wandering enemy)
● Bomb
┼ Blast centre
│ Blast north/south arms
─ Blast east/west arms
▧ Exit locked (enemies survive)
⬚ Exit ready
◈ Fire-up

Reserved glyphs
a AI-controlled non-Enemy Character
▣ Bomb-capacity Item"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRecord {
    pub tick: Tick,
    pub direction: Option<Direction>,
    pub place_bomb: bool,
    pub position: Position,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdvanceReport {
    pub ticks: Vec<TickRecord>,
    pub discarded_ticks: u128,
}

enum PlayClock {
    Paused,
    Running {
        next_tick: Instant,
        held: Vec<Direction>,
        placement_pending: bool,
    },
}

pub struct App {
    game: Game,
    generation_state: u64,
    clock: PlayClock,
    focused: bool,
    width: u16,
    height: u16,
    help_open: bool,
    help_scroll: usize,
    quit: bool,
}

impl App {
    pub fn new(now: Instant, width: u16, height: u16) -> Self {
        Self::new_with_rules(now, width, height, GameRules::default())
    }

    pub fn new_with_rules(now: Instant, width: u16, height: u16, rules: GameRules) -> Self {
        Self::new_with_seed(now, width, height, rules, RandomState::new().hash_one(0u8))
    }

    /// Reproducible generated session; restart advances its generation random state.
    pub fn new_with_seed(
        now: Instant,
        width: u16,
        height: u16,
        rules: GameRules,
        seed: u64,
    ) -> Self {
        let mut generation_state = seed;
        let scenario = Self::generate_scenario(rules, &mut generation_state);
        Self::from_scenario(now, width, height, scenario, generation_state)
    }

    /// Start an explicit validated Scenario for a precise replay. Restart still
    /// requests a fresh Scenario for Normal mode using the supplied random state, never
    /// restores this Scenario.
    pub fn from_scenario(
        now: Instant,
        width: u16,
        height: u16,
        scenario: Scenario,
        generation_state: u64,
    ) -> Self {
        let mut app = Self {
            game: Game::new(scenario, Tick::ZERO),
            generation_state,
            clock: PlayClock::Paused,
            focused: true,
            width,
            height,
            help_open: false,
            help_scroll: 0,
            quit: false,
        };
        app.reconcile_clock(now);
        app
    }

    pub fn handle_key(&mut self, event: KeyEvent, now: Instant) {
        self.reconcile_clock(now);
        if self.game.outcome().is_some()
            && !matches!(
                (event.code, event.kind),
                (
                    KeyCode::Char('r' | 'R' | 'q' | 'Q') | KeyCode::Esc,
                    KeyEventKind::Press
                )
            )
        {
            return;
        }
        if self.help_open {
            match (event.code, event.kind) {
                (KeyCode::Esc, KeyEventKind::Press)
                | (KeyCode::Char('?'), KeyEventKind::Press)
                | (KeyCode::Char('h' | 'H'), KeyEventKind::Press) => {
                    self.toggle_help(now);
                }
                (KeyCode::Up, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    self.scroll_help_up();
                }
                (KeyCode::Down, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    self.scroll_help_down();
                }
                _ => {}
            }
            return;
        }

        match (event.code, event.kind) {
            (KeyCode::Char('q' | 'Q') | KeyCode::Esc, KeyEventKind::Press) => {
                self.quit = true;
                self.reconcile_clock(now);
            }
            (KeyCode::Char(' '), KeyEventKind::Press) => {
                if let PlayClock::Running {
                    placement_pending, ..
                } = &mut self.clock
                {
                    *placement_pending = true;
                }
            }
            (KeyCode::Char('r' | 'R'), KeyEventKind::Press) => self.reset(now),
            (KeyCode::Char('?'), KeyEventKind::Press)
            | (KeyCode::Char('h' | 'H'), KeyEventKind::Press) => self.toggle_help(now),
            (code, KeyEventKind::Press) => {
                if let Some(direction) = direction(code)
                    && let PlayClock::Running { held, .. } = &mut self.clock
                {
                    held.retain(|held| *held != direction);
                    held.push(direction);
                }
            }
            (code, KeyEventKind::Release) => {
                if let Some(direction) = direction(code)
                    && let PlayClock::Running { held, .. } = &mut self.clock
                {
                    held.retain(|held| *held != direction);
                }
            }
            _ => {}
        }
    }

    pub fn set_focused(&mut self, focused: bool, now: Instant) {
        self.focused = focused;
        self.reconcile_clock(now);
    }

    pub fn set_terminal_size(&mut self, width: u16, height: u16, now: Instant) {
        self.width = width;
        self.height = height;
        self.reconcile_clock(now);
        self.clamp_help_scroll();
    }

    pub fn advance(&mut self, now: Instant) -> Result<AdvanceReport, StepError> {
        self.reconcile_clock(now);
        let Some(deadline) = self.next_tick() else {
            return Ok(AdvanceReport {
                ticks: Vec::new(),
                discarded_ticks: 0,
            });
        };
        if now < deadline {
            return Ok(AdvanceReport {
                ticks: Vec::new(),
                discarded_ticks: 0,
            });
        }

        let elapsed = now.duration_since(deadline);
        let overdue = elapsed.as_nanos() / TICK_INTERVAL.as_nanos() + 1;
        let tick_count = overdue.min(MAX_CATCH_UP_TICKS as u128) as usize;
        let direction = self.held_direction();
        let mut ticks = Vec::with_capacity(tick_count);
        for _ in 0..tick_count {
            let PlayClock::Running {
                placement_pending, ..
            } = &mut self.clock
            else {
                break;
            };
            let place_bomb = *placement_pending;
            let command = CharacterCommand {
                character: self.game.character().id,
                movement: direction,
                place_bomb,
            };
            self.game.step(&[command])?;
            *placement_pending = false;
            ticks.push(TickRecord {
                tick: self.game.tick(),
                direction,
                place_bomb,
                position: self.game.character().position,
            });
            if self.game.outcome().is_some() {
                self.reconcile_clock(now);
                break;
            }
        }

        if let PlayClock::Running { next_tick, .. } = &mut self.clock {
            let remainder = elapsed.as_nanos() % TICK_INTERVAL.as_nanos();
            let until_next = TICK_INTERVAL.as_nanos() - remainder;
            *next_tick = now + Duration::from_nanos(until_next as u64);
        }
        Ok(AdvanceReport {
            discarded_ticks: overdue - ticks.len() as u128,
            ticks,
        })
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn held_direction(&self) -> Option<Direction> {
        match &self.clock {
            PlayClock::Paused => None,
            PlayClock::Running { held, .. } => held.last().copied(),
        }
    }

    pub fn help_open(&self) -> bool {
        self.help_open
    }

    pub fn help_scroll(&self) -> usize {
        self.help_scroll
    }

    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    pub fn next_tick(&self) -> Option<Instant> {
        match &self.clock {
            PlayClock::Paused => None,
            PlayClock::Running { next_tick, .. } => Some(*next_tick),
        }
    }

    fn minimum_size(&self) -> (u16, u16) {
        (
            MIN_WIDTH.max(self.game.arena().width() as u16 + 2),
            MIN_HEIGHT.max(self.game.arena().height() as u16 + 4),
        )
    }

    fn can_move(&self) -> bool {
        let (width, height) = self.minimum_size();
        self.focused && self.width >= width && self.height >= height
    }

    fn paused(&self) -> bool {
        self.quit || !self.can_move() || self.help_open || self.game.outcome().is_some()
    }

    fn reconcile_clock(&mut self, now: Instant) {
        if self.paused() {
            self.clock = PlayClock::Paused;
        } else if matches!(self.clock, PlayClock::Paused) {
            self.clock = PlayClock::Running {
                next_tick: now + TICK_INTERVAL,
                held: Vec::new(),
                placement_pending: false,
            };
        }
    }

    fn generate_scenario(rules: GameRules, generation_state: &mut u64) -> Scenario {
        Scenario::generate(rules, 40, generation_state)
            .expect("generated Scenario placements and density are valid")
    }

    fn reset(&mut self, now: Instant) {
        self.game = Game::new(
            Self::generate_scenario(*self.game.rules(), &mut self.generation_state),
            Tick::ZERO,
        );
        self.clock = PlayClock::Paused;
        self.reconcile_clock(now);
    }

    fn toggle_help(&mut self, now: Instant) {
        self.help_open = !self.help_open;
        if self.help_open {
            self.help_scroll = 0;
        }
        self.clock = PlayClock::Paused;
        self.reconcile_clock(now);
    }

    fn scroll_help_up(&mut self) {
        self.help_scroll = self.help_scroll.saturating_sub(1);
    }

    fn scroll_help_down(&mut self) {
        let max = wrapped_help(self.width)
            .len()
            .saturating_sub(usize::from(self.height));
        self.help_scroll = (self.help_scroll + 1).min(max);
    }

    fn clamp_help_scroll(&mut self) {
        let max = wrapped_help(self.width)
            .len()
            .saturating_sub(usize::from(self.height));
        self.help_scroll = self.help_scroll.min(max);
    }
}

fn direction(code: KeyCode) -> Option<Direction> {
    match code {
        KeyCode::Up => Some(Direction::Up),
        KeyCode::Right => Some(Direction::Right),
        KeyCode::Down => Some(Direction::Down),
        KeyCode::Left => Some(Direction::Left),
        _ => None,
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let (width, height) = app.minimum_size();
    if area.width < width || area.height < height {
        frame.render_widget(
            Paragraph::new(format!(
                "Resize terminal to at least {width} columns × {height} rows."
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            area,
        );
    } else if app.help_open {
        render_help(frame, app, area);
    } else {
        render_game(frame, app, area);
    }
}

fn render_game(frame: &mut Frame, app: &App, area: Rect) {
    let arena = app.game.arena();
    let map_width = arena.width() as u16 + 2;
    let map_height = arena.height() as u16 + 2;
    let top = area.y + (area.height - map_height - 2) / 2;
    let map_area = Rect::new(
        area.x + (area.width - map_width) / 2,
        top,
        map_width,
        map_height,
    );
    let boundary = Block::bordered().border_set(border::Set {
        top_left: "▓",
        top_right: "▓",
        bottom_left: "▓",
        bottom_right: "▓",
        vertical_left: "▓",
        vertical_right: "▓",
        horizontal_top: "▓",
        horizontal_bottom: "▓",
    });
    let interior = boundary.inner(map_area);
    frame.render_widget(boundary, map_area);
    let position = app.game.character().position;
    let character_cell = arena
        .cell_containing(position)
        .expect("live Character position is within the Arena");
    let center = arena
        .cell_center(character_cell)
        .expect("the Character's Cell is in the Arena");
    let offset = format!(
        "Offset: {:+04},{:+04}",
        position.x().value() - center.x().value(),
        position.y().value() - center.y().value(),
    );
    let mut map = String::new();
    for (cell, terrain) in arena.cells() {
        map.push(if cell == character_cell {
            '@'
        } else if app
            .game
            .enemies()
            .any(|enemy| arena.cell_containing(enemy.position) == Some(cell))
        {
            'e'
        } else {
            match terrain {
                Terrain::Floor => {
                    if app.game.bomb().is_some_and(|bomb| bomb.cell() == cell) {
                        '●'
                    } else if let Some(blast) = app
                        .game
                        .blasts()
                        .iter()
                        .rev()
                        .find(|blast| blast.contains(cell))
                    {
                        if cell == blast.origin() {
                            '┼'
                        } else if cell.row() == blast.origin().row() {
                            '─'
                        } else {
                            '│'
                        }
                    } else if app.game.fire_up() == Some(cell) {
                        '◈'
                    } else if app.game.exit() == Some(cell) {
                        if app.game.exit_ready() { '⬚' } else { '▧' }
                    } else {
                        ' '
                    }
                }
                Terrain::Wall => '█',
                Terrain::Block => '▒',
            }
        });
        if cell.column().value() + 1 == arena.width() && cell.row().value() + 1 < arena.height() {
            map.push('\n');
        }
    }
    frame.render_widget(Paragraph::new(map), interior);
    if let Some(outcome) = app.game.outcome() {
        let message = match outcome {
            Outcome::Defeat => "GAME OVER",
            Outcome::Victory => "VICTORY!",
        };
        let modal_height = 5;
        let modal = Rect::new(
            area.x + (area.width - MIN_WIDTH) / 2,
            top + map_height / 2 - modal_height / 2,
            MIN_WIDTH,
            modal_height,
        );
        frame.render_widget(Clear, modal);
        frame.render_widget(
            Paragraph::new(format!("{message}\nR: restart\nQ/Esc: quit"))
                .alignment(Alignment::Center)
                .block(Block::bordered()),
            modal,
        );
        return;
    }
    frame.render_widget(
        Paragraph::new(format!("Blast reach: {}", app.game.current_blast_reach()))
            .alignment(Alignment::Center),
        Rect::new(area.x, top + map_height, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(offset).alignment(Alignment::Center),
        Rect::new(area.x, top + map_height + 1, area.width, 1),
    );
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let lines = wrapped_help(area.width)
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();
    let max_scroll = lines.len().saturating_sub(usize::from(area.height));
    frame.render_widget(
        Paragraph::new(lines).scroll((app.help_scroll.min(max_scroll) as u16, 0)),
        area,
    );
}

fn wrapped_help(width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    for source in HELP.lines() {
        if source.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in source.split_whitespace() {
            let candidate_width = UnicodeWidthStr::width(line.as_str())
                + usize::from(!line.is_empty())
                + UnicodeWidthStr::width(word);
            if !line.is_empty() && candidate_width > width {
                lines.push(std::mem::take(&mut line));
            }
            if UnicodeWidthStr::width(word) > width {
                for character in word.chars() {
                    let character_width = character.width().unwrap_or(0);
                    if !line.is_empty()
                        && UnicodeWidthStr::width(line.as_str()) + character_width > width
                    {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push(character);
                }
            } else if line.is_empty() {
                line.push_str(word);
            } else {
                line.push(' ');
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

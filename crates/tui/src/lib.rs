use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use exploderous_core::game::{
    CellCoord, Character, CharacterCommand, CharacterId, Column, Direction, Game, GameRules,
    Outcome, Position, Row, Scenario, StepError, Terrain, WorldX, WorldY,
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
    widgets::{Block, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const TICK_INTERVAL: Duration = Duration::from_nanos(16_666_667);
pub const MIN_WIDTH: u16 = 19;
pub const MIN_HEIGHT: u16 = 11;
const MAX_CATCH_UP_TICKS: usize = 5;
const MAP_SIZE: u16 = 9;
const STATUS: &str = "Lives: 0 | Score: 0";
const HELP: &str = r#"NORMAL LEVEL

Controls
Arrow keys: move; latest press wins; release falls back
Space: place one Bomb; place another after it detonates
Blasts clear Blocks; entering a Blast Cell with your centre is lethal: Game Over
Bombs reveal an Exit hidden beneath a Block; reach its Cell alive to win
Victory or Game Over freezes play; only restart or quit acts
R: restart during play or either outcome; Q or Escape: quit
?, h, or H: open or close help
Up/Down: scroll; Escape closes help
Offset: signed X,Y world-unit delta from the current Cell centre; 0,0 is centred

Live map
(blank space) Floor
█ Wall
▒ Block
@ Player
● Bomb
┼ Blast centre
│ Blast north/south arms
─ Blast east/west arms
⬚ Exit

Reserved glyphs
a AI-controlled non-Enemy Character
e Enemy Character (takes precedence over AI)
▣ Bomb-capacity Item
◈ Blast-reach Item"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRecord {
    pub tick: u64,
    pub direction: Option<Direction>,
    pub place_bomb: bool,
    pub position: Position,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdvanceReport {
    pub ticks: Vec<TickRecord>,
    pub discarded_ticks: u128,
}

pub struct App {
    game: Game,
    held: Vec<Direction>,
    placement_pending: bool,
    focused: bool,
    width: u16,
    height: u16,
    help_open: bool,
    help_scroll: usize,
    quit: bool,
    next_tick: Option<Instant>,
}

impl App {
    pub fn new(now: Instant, width: u16, height: u16) -> Self {
        Self::new_with_rules(now, width, height, GameRules::default())
    }

    pub fn new_with_rules(now: Instant, width: u16, height: u16, rules: GameRules) -> Self {
        let mut app = Self {
            game: new_game(rules),
            held: Vec::new(),
            placement_pending: false,
            focused: true,
            width,
            height,
            help_open: false,
            help_scroll: 0,
            quit: false,
            next_tick: None,
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
            (KeyCode::Char(' '), KeyEventKind::Press) if !self.paused() => {
                self.placement_pending = true;
            }
            (KeyCode::Char('r' | 'R'), KeyEventKind::Press) => self.reset(now),
            (KeyCode::Char('?'), KeyEventKind::Press)
            | (KeyCode::Char('h' | 'H'), KeyEventKind::Press) => self.toggle_help(now),
            (code, KeyEventKind::Press) => {
                if let Some(direction) = direction(code)
                    && self.can_move()
                {
                    self.held.retain(|held| *held != direction);
                    self.held.push(direction);
                }
            }
            (code, KeyEventKind::Release) => {
                if let Some(direction) = direction(code) {
                    self.held.retain(|held| *held != direction);
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
        let Some(deadline) = self.next_tick else {
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
            let place_bomb = self.placement_pending;
            let command = CharacterCommand {
                character: self.game.character().id,
                movement: direction,
                place_bomb,
            };
            self.game.step(&[command])?;
            self.placement_pending = false;
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

        if !self.paused() {
            let remainder = elapsed.as_nanos() % TICK_INTERVAL.as_nanos();
            let until_next = TICK_INTERVAL.as_nanos() - remainder;
            self.next_tick = Some(now + Duration::from_nanos(until_next as u64));
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
        self.held.last().copied()
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
        self.next_tick
    }

    fn can_move(&self) -> bool {
        self.focused && self.width >= MIN_WIDTH && self.height >= MIN_HEIGHT
    }

    fn paused(&self) -> bool {
        self.quit || !self.can_move() || self.help_open || self.game.outcome().is_some()
    }

    fn reconcile_clock(&mut self, now: Instant) {
        if self.paused() {
            self.next_tick = None;
            self.held.clear();
            self.placement_pending = false;
        } else if self.next_tick.is_none() {
            self.next_tick = Some(now + TICK_INTERVAL);
        }
    }

    fn reset(&mut self, now: Instant) {
        self.game = new_game(*self.game.rules());
        self.held.clear();
        self.placement_pending = false;
        self.next_tick = None;
        self.reconcile_clock(now);
    }

    fn toggle_help(&mut self, now: Instant) {
        self.help_open = !self.help_open;
        self.held.clear();
        self.placement_pending = false;
        if self.help_open {
            self.help_scroll = 0;
        }
        self.next_tick = None;
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

fn new_game(rules: GameRules) -> Game {
    let blocks = [
        CellCoord::new(Column::new(3), Row::new(3)),
        CellCoord::new(Column::new(5), Row::new(3)),
        CellCoord::new(Column::new(7), Row::new(3)),
        CellCoord::new(Column::new(3), Row::new(5)),
        CellCoord::new(Column::new(7), Row::new(5)),
        CellCoord::new(Column::new(5), Row::new(7)),
    ];
    let terrain = (0..9_u32)
        .flat_map(|row| {
            let blocks = &blocks;
            (0..9_u32).map(move |column| {
                let cell = CellCoord::new(Column::new(column), Row::new(row));
                if row == 0
                    || row == 8
                    || column == 0
                    || column == 8
                    || (row.is_multiple_of(2) && column.is_multiple_of(2))
                {
                    Terrain::Wall
                } else if blocks.contains(&cell) {
                    Terrain::Block
                } else {
                    Terrain::Floor
                }
            })
        })
        .collect();
    let scenario = Scenario::new(
        9,
        9,
        terrain,
        Character {
            id: CharacterId::new(0),
            position: Position::new(WorldX::new(384), WorldY::new(384)),
        },
        rules,
    )
    .expect("the fixed Level arena is valid")
    .with_exit(CellCoord::new(Column::new(5), Row::new(3)))
    .expect("the Exit is beneath its authored Block");
    Game::new(scenario, 0)
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new("Resize terminal to at least 19 columns × 11 rows.")
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
    let map_x = area.x + (area.width - MAP_SIZE) / 2;
    let top = area.y + (area.height - MIN_HEIGHT) / 2;
    let arena = app.game.arena();
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
                        .find(|blast| blast.cells().contains(&cell))
                    {
                        if cell == blast.origin() {
                            '┼'
                        } else if cell.row() == blast.origin().row() {
                            '─'
                        } else {
                            '│'
                        }
                    } else if app.game.exit() == Some(cell) {
                        '⬚'
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
    frame.render_widget(
        Paragraph::new(map),
        Rect::new(map_x, top, MAP_SIZE, MAP_SIZE),
    );
    if let Some(outcome) = app.game.outcome() {
        let message = match outcome {
            Outcome::Defeat => "GAME OVER",
            Outcome::Victory => "VICTORY!",
        };
        let modal = Rect::new(area.x + (area.width - MIN_WIDTH) / 2, top + 2, MIN_WIDTH, 5);
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
        Paragraph::new(STATUS).alignment(Alignment::Center),
        Rect::new(area.x, top + MAP_SIZE, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(offset).alignment(Alignment::Center),
        Rect::new(area.x, top + MAP_SIZE + 1, area.width, 1),
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

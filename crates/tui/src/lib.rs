use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use exploderous_core::game::{
    Character, CharacterId, Direction, Game, MovementCommand, MovementRules, Position, Scenario,
    StepError, Terrain, WORLD_UNITS_PER_CELL,
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
    widgets::{Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const TICK_INTERVAL: Duration = Duration::from_nanos(16_666_667);
pub const MIN_WIDTH: u16 = 19;
pub const MIN_HEIGHT: u16 = 10;
const MAX_CATCH_UP_TICKS: usize = 5;
const MAP_SIZE: u16 = 9;
const STATUS: &str = "Lives: 0 | Score: 0";
const HELP: &str = "MOVEMENT PLAYGROUND\n\nControls\nArrow keys: move; latest press wins; release falls back\nR: reset; Q or Escape: quit\n?, h, or H: open or close help\nUp/Down: scroll; Escape closes help\n\nLive map\n(blank space) Floor\n█ Wall\n▒ Block\n@ Player\n\nReserved glyphs\na AI-controlled non-Enemy Character\ne Enemy Character (takes precedence over AI)\n● Bomb\n┼ Blast centre\n│ Blast north/south arms\n─ Blast east/west arms\n⬚ Exit\n▣ Bomb-capacity Item\n◈ Blast-reach Item";

pub struct App {
    game: Game,
    held: Vec<Direction>,
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
        let mut app = Self {
            game: new_game(),
            held: Vec::new(),
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
            (KeyCode::Char('q' | 'Q') | KeyCode::Esc, KeyEventKind::Press) => self.quit = true,
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

    pub fn advance(&mut self, now: Instant) -> Result<usize, StepError> {
        self.reconcile_clock(now);
        let Some(deadline) = self.next_tick else {
            return Ok(0);
        };
        if now < deadline {
            return Ok(0);
        }

        let elapsed = now.duration_since(deadline);
        let overdue = elapsed.as_nanos() / TICK_INTERVAL.as_nanos() + 1;
        let ticks = overdue.min(MAX_CATCH_UP_TICKS as u128) as usize;
        let frame: Vec<_> = self
            .held_direction()
            .map(|direction| MovementCommand {
                character: self.game.character().id,
                direction,
            })
            .into_iter()
            .collect();
        for _ in 0..ticks {
            self.game.step(&frame)?;
        }

        let remainder = elapsed.as_nanos() % TICK_INTERVAL.as_nanos();
        let until_next = TICK_INTERVAL.as_nanos() - remainder;
        self.next_tick = Some(now + Duration::from_nanos(until_next as u64));
        Ok(ticks)
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
        !self.can_move() || self.help_open
    }

    fn reconcile_clock(&mut self, now: Instant) {
        if self.paused() {
            self.next_tick = None;
            self.held.clear();
        } else if self.next_tick.is_none() {
            self.next_tick = Some(now + TICK_INTERVAL);
        }
    }

    fn reset(&mut self, now: Instant) {
        self.game = new_game();
        self.held.clear();
        self.next_tick = None;
        self.reconcile_clock(now);
    }

    fn toggle_help(&mut self, now: Instant) {
        self.help_open = !self.help_open;
        self.held.clear();
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

fn new_game() -> Game {
    let mut terrain = vec![Terrain::Floor; 81];
    for row in 0..9_u32 {
        for column in 0..9_u32 {
            if row == 0
                || row == 8
                || column == 0
                || column == 8
                || (row.is_multiple_of(2) && column.is_multiple_of(2))
            {
                terrain[(row * 9 + column) as usize] = Terrain::Wall;
            }
        }
    }
    for (column, row) in [
        (3, 1),
        (5, 1),
        (5, 3),
        (1, 5),
        (3, 5),
        (7, 5),
        (3, 7),
        (5, 7),
    ] {
        terrain[(row * 9 + column) as usize] = Terrain::Block;
    }
    let scenario = Scenario::new(
        9,
        9,
        terrain,
        Character {
            id: CharacterId::new(0),
            position: Position { x: 384, y: 384 },
        },
        MovementRules::default(),
    )
    .expect("the fixed movement arena is valid");
    Game::new(scenario, 0)
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new("Resize terminal to at least 19 columns × 10 rows.")
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
    let character_x = u32::try_from(app.game.character().position.x).unwrap()
        / u32::try_from(WORLD_UNITS_PER_CELL).unwrap();
    let character_y = u32::try_from(app.game.character().position.y).unwrap()
        / u32::try_from(WORLD_UNITS_PER_CELL).unwrap();
    let mut map = String::new();
    for (index, terrain) in app.game.arena().terrain().iter().enumerate() {
        let column = index % 9;
        let row = index / 9;
        map.push(
            if column as u32 == character_x && row as u32 == character_y {
                '@'
            } else {
                match terrain {
                    Terrain::Floor => ' ',
                    Terrain::Wall => '█',
                    Terrain::Block => '▒',
                }
            },
        );
        if column == 8 && row != 8 {
            map.push('\n');
        }
    }
    frame.render_widget(
        Paragraph::new(map),
        Rect::new(map_x, top, MAP_SIZE, MAP_SIZE),
    );
    frame.render_widget(
        Paragraph::new(STATUS).alignment(Alignment::Center),
        Rect::new(area.x, top + MAP_SIZE, area.width, 1),
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

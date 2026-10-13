use std::{
    fs::File,
    io,
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableFocusChange, EnableFocusChange, Event, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        self, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use exploderous_core::game::StepError;
use exploderous_tui::{App, render};
use ratatui::{Terminal, backend::CrosstermBackend};
use tracing::{error, info, trace};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("exploderous-tui: {error}");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing() -> io::Result<()> {
    let file = File::create("app.log")?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::Uptime::default())
        .with_max_level(tracing::Level::TRACE)
        .with_writer(file)
        .try_init()
        .map_err(|error| io::Error::other(format!("could not initialize app.log tracing: {error}")))
}

fn run() -> io::Result<()> {
    init_tracing()?;
    info!(term = ?std::env::var_os("TERM"), "session started");
    let mut session = match TerminalSession::enter() {
        Ok(session) => session,
        Err(error) => {
            error!(%error, "terminal startup failed");
            return Err(error);
        }
    };
    info!(
        requested_keyboard_flags =
            "DISAMBIGUATE_ESCAPE_CODES|REPORT_EVENT_TYPES|REPORT_ALL_KEYS_AS_ESCAPE_CODES",
        focus_reporting = true,
        "terminal ready",
    );

    let result = run_ui();
    if let Err(error) = &result {
        error!(%error, "application failed");
    }
    let cleanup = session.restore();
    if let Err(error) = &cleanup {
        error!(%error, "terminal restore failed");
    } else {
        info!("terminal restored");
    }
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_ui() -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (width, height) = terminal::size()?;
    let started = Instant::now();
    let mut app = App::new(started, width, height);
    trace_app_state(format_args!("start size={width}x{height}"), &app);
    let position = app.game().character().position;
    info!(
        tick = app.game().tick(),
        position_x = position.x().value(),
        position_y = position.y().value(),
        arena_width = app.game().arena().width(),
        arena_height = app.game().arena().height(),
        "new game",
    );

    loop {
        let timeout = app
            .next_tick()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_millis(250));
        if event::poll(timeout)? {
            handle_event(event::read()?, &mut app);
            while event::poll(Duration::ZERO)? {
                handle_event(event::read()?, &mut app);
            }
        }

        if app.quit_requested() {
            break;
        }
        let now = Instant::now();
        let report = app.advance(now).map_err(step_error)?;
        if !report.ticks.is_empty() || report.discarded_ticks > 0 {
            trace!(
                processed = report.ticks.len(),
                discarded = report.discarded_ticks,
                "tick batch advanced",
            );
        }
        for tick in report.ticks {
            trace!(
                tick = tick.tick,
                direction = ?tick.direction,
                position_x = tick.position.x().value(),
                position_y = tick.position.y().value(),
                "game tick",
            );
        }
        terminal.draw(|frame| render(frame, &app))?;
    }
    Ok(())
}

fn handle_event(event: Event, app: &mut App) {
    let now = Instant::now();
    match event {
        Event::Key(key) => {
            let (code, kind, modifiers) = (key.code, key.kind, key.modifiers);
            app.handle_key(key, now);
            trace_app_state(
                format_args!("key code={code:?} kind={kind:?} modifiers={modifiers:?}"),
                app,
            );
        }
        Event::Resize(width, height) => {
            app.set_terminal_size(width, height, now);
            trace_app_state(format_args!("resize {width}x{height}"), app);
        }
        Event::FocusLost => {
            app.set_focused(false, now);
            trace_app_state(format_args!("focus_lost"), app);
        }
        Event::FocusGained => {
            app.set_focused(true, now);
            trace_app_state(format_args!("focus_gained"), app);
        }
        Event::Mouse(mouse) => {
            trace_app_state(format_args!("mouse={mouse:?}"), app);
        }
        Event::Paste(text) => {
            trace_app_state(
                format_args!("paste bytes={} [content redacted]", text.len()),
                app,
            );
        }
    }
}

fn trace_app_state(message: impl std::fmt::Display, app: &App) {
    let position = app.game().character().position;
    trace!(
        held_direction = ?app.held_direction(),
        help_open = app.help_open(),
        quit_requested = app.quit_requested(),
        game_tick = app.game().tick(),
        position_x = position.x().value(),
        position_y = position.y().value(),
        "{}",
        message,
    );
}

fn step_error(error: StepError) -> io::Error {
    io::Error::other(format!("movement simulation stopped: {error:?}"))
}

struct TerminalSession {
    raw_mode: bool,
    alternate_screen: bool,
    cursor_hidden: bool,
    focus_reporting: bool,
    keyboard_flags: bool,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        match crossterm::terminal::supports_keyboard_enhancement() {
            Ok(true) => {}
            Ok(false) => {
                return Err(io::Error::other(
                    "this terminal lacks the required Kitty keyboard protocol; use a terminal that supports event-type reporting and all-keys-as-escape-codes",
                ));
            }
            Err(error) => {
                return Err(io::Error::other(format!(
                    "could not detect Kitty keyboard protocol support ({error}); run inside a compatible terminal",
                )));
            }
        }

        let mut session = Self {
            raw_mode: false,
            alternate_screen: false,
            cursor_hidden: false,
            focus_reporting: false,
            keyboard_flags: false,
        };

        session.raw_mode = true;
        enable_raw_mode()?;
        session.focus_reporting = true;
        execute!(io::stdout(), EnableFocusChange)?;
        session.alternate_screen = true;
        execute!(io::stdout(), EnterAlternateScreen)?;
        session.cursor_hidden = true;
        execute!(io::stdout(), crossterm::cursor::Hide)?;
        session.keyboard_flags = true;
        execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        Ok(session)
    }

    fn restore(&mut self) -> io::Result<()> {
        let mut first_error = None;
        if self.keyboard_flags {
            self.keyboard_flags = false;
            if let Err(error) = execute!(io::stdout(), PopKeyboardEnhancementFlags) {
                first_error.get_or_insert(error);
            }
        }
        if self.cursor_hidden {
            self.cursor_hidden = false;
            if let Err(error) = execute!(io::stdout(), crossterm::cursor::Show) {
                first_error.get_or_insert(error);
            }
        }
        if self.alternate_screen {
            self.alternate_screen = false;
            if let Err(error) = execute!(io::stdout(), LeaveAlternateScreen) {
                first_error.get_or_insert(error);
            }
        }
        if self.focus_reporting {
            self.focus_reporting = false;
            if let Err(error) = execute!(io::stdout(), DisableFocusChange) {
                first_error.get_or_insert(error);
            }
        }
        if self.raw_mode {
            self.raw_mode = false;
            if let Err(error) = disable_raw_mode() {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

use std::{
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("exploderous-tui: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let mut session = TerminalSession::enter()?;
    let result = run_ui();
    let cleanup = session.restore();
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_ui() -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (width, height) = terminal::size()?;
    let mut app = App::new(Instant::now(), width, height);

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
        app.advance(Instant::now()).map_err(step_error)?;
        terminal.draw(|frame| render(frame, &app))?;
    }
    Ok(())
}

fn handle_event(event: Event, app: &mut App) {
    let now = Instant::now();
    match event {
        Event::Key(key) => app.handle_key(key, now),
        Event::Resize(width, height) => app.set_terminal_size(width, height, now),
        Event::FocusLost => app.set_focused(false, now),
        Event::FocusGained => app.set_focused(true, now),
        _ => {}
    }
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
        session.keyboard_flags = true;
        execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        session.focus_reporting = true;
        execute!(io::stdout(), EnableFocusChange)?;
        session.alternate_screen = true;
        execute!(io::stdout(), EnterAlternateScreen)?;
        session.cursor_hidden = true;
        execute!(io::stdout(), crossterm::cursor::Hide)?;
        Ok(session)
    }

    fn restore(&mut self) -> io::Result<()> {
        let mut first_error = None;
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
        if self.keyboard_flags {
            self.keyboard_flags = false;
            if let Err(error) = execute!(io::stdout(), PopKeyboardEnhancementFlags) {
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

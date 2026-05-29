use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Stdout, Write};

pub struct TerminalGuard {
    stdout: Stdout,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        queue!(stdout, EnterAlternateScreen, Hide)?;
        stdout.flush()?;
        Ok(TerminalGuard { stdout })
    }

    pub fn clear(&mut self) -> io::Result<()> {
        queue!(
            self.stdout,
            Clear(ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        )?;
        self.stdout.flush()
    }

    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        write!(self.stdout, "{}\r\n", line)
    }

    pub fn should_exit(&self) -> io::Result<bool> {
        if event::poll(std::time::Duration::from_millis(0))? {
            if let Event::Key(KeyEvent { code: KeyCode::Char('c'), .. }) = event::read()? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

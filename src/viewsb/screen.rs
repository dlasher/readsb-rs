use std::io;

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        Ok(TerminalGuard)
    }

    pub fn clear(&mut self) -> io::Result<()> {
        Ok(())
    }

    pub fn write_line(&mut self, _line: &str) -> io::Result<()> {
        Ok(())
    }

    pub fn should_exit(&self) -> io::Result<bool> {
        Ok(false)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {}
}

use crate::tracking::Aircraft;
use std::io;

pub struct JsonWriter;

impl JsonWriter {
    pub fn new(_path: &str) -> io::Result<Self> {
        Ok(JsonWriter)
    }

    pub fn write_snapshot(&self, _aircraft: &[Aircraft], _now: i64) -> io::Result<()> {
        Ok(())
    }
}

pub struct CsvWriter;

impl CsvWriter {
    pub fn new(_path: &str) -> io::Result<Self> {
        Ok(CsvWriter)
    }

    pub fn write_snapshot(&mut self, _aircraft: &[Aircraft], _now: i64) -> io::Result<()> {
        Ok(())
    }
}

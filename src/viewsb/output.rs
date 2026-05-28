use crate::tracking::Aircraft;
use crate::output::json::AircraftJson;
use std::fs;
use std::io::{self, Write};

pub struct JsonWriter {
    path: String,
}

impl JsonWriter {
    pub fn new(path: &str) -> io::Result<Self> {
        Ok(JsonWriter { path: path.to_string() })
    }

    pub fn write_snapshot(&self, aircraft: &[Aircraft], now: i64) -> io::Result<()> {
        let json_list: Vec<AircraftJson> = aircraft.iter()
            .map(|a| AircraftJson::from_aircraft(a, now))
            .collect();
        let json_str = serde_json::to_string(&json_list)?;

        // Atomic write: write to temp file, then rename
        let tmp_path = format!("{}.tmp", self.path);
        let mut file = fs::File::create(&tmp_path)?;
        writeln!(file, "{}", json_str)?;
        drop(file);
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

pub struct CsvWriter {
    path: String,
    header_written: bool,
}

impl CsvWriter {
    pub fn new(path: &str) -> io::Result<Self> {
        Ok(CsvWriter { path: path.to_string(), header_written: false })
    }

    pub fn write_snapshot(&mut self, aircraft: &[Aircraft], now: i64) -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        if !self.header_written {
            writeln!(file, "hex,flight,alt_baro,alt_geom,gs,track,lat,lon,squawk,emergency,category,messages,seen")?;
            self.header_written = true;
        }

        for a in aircraft {
            let json = AircraftJson::from_aircraft(a, now);
            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                json.hex,
                json.flight.as_deref().unwrap_or(""),
                json.alt_baro.map_or(String::new(), |v| v.to_string()),
                json.alt_geom.map_or(String::new(), |v| v.to_string()),
                json.gs.map_or(String::new(), |v| v.to_string()),
                json.track.map_or(String::new(), |v| v.to_string()),
                json.lat.map_or(String::new(), |v| v.to_string()),
                json.lon.map_or(String::new(), |v| v.to_string()),
                json.squawk.as_deref().unwrap_or(""),
                json.emergency.as_deref().unwrap_or(""),
                json.category.map_or(String::new(), |v| v.to_string()),
                json.messages,
                json.seen,
            )?;
        }

        Ok(())
    }
}

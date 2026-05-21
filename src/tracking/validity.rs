use crate::types::DataSource;

pub const TRACK_STALE: i64 = 60_000;
pub const TRACK_EXPIRE: i64 = 300_000;
pub const TRACK_EXPIRE_JAERO: i64 = 33 * 60_000;

#[derive(Debug, Clone, Copy)]
pub struct DataValidity {
    pub updated: i64,
    pub next_reduce_forward: i64,
    pub source: DataSource,
    pub last_source: DataSource,
    pub stale: bool,
}

impl DataValidity {
    pub fn new() -> Self {
        DataValidity {
            updated: 0,
            next_reduce_forward: 0,
            source: DataSource::Invalid,
            last_source: DataSource::Invalid,
            stale: false,
        }
    }

    pub fn update(&mut self, source: DataSource, now: i64) {
        if source >= self.source {
            self.last_source = self.source;
            self.source = source;
            self.updated = now;
            self.stale = false;
        }
    }

    pub fn is_valid(&self, now: i64, expiration: i64) -> bool {
        if self.source == DataSource::Invalid {
            return false;
        }
        let expire = if self.source == DataSource::Jaero {
            TRACK_EXPIRE_JAERO
        } else {
            expiration
        };
        now < self.updated + expire
    }

    pub fn update_stale(&mut self, now: i64) {
        if self.source == DataSource::Invalid {
            return;
        }
        self.stale = now >= self.updated + TRACK_STALE;
    }

    pub fn age(&self, now: i64) -> i64 {
        if self.updated >= now {
            0
        } else {
            now - self.updated
        }
    }

    pub fn invalidate(&mut self) {
        self.source = DataSource::Invalid;
        self.stale = false;
    }
}

impl Default for DataValidity {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::{LazyLock, Mutex};

const ICAO_FILTER_SIZE: usize = 4096;

struct IcaoBucket {
    entries: Vec<u32>,
}

struct IcaoFilterInner {
    buckets: Vec<IcaoBucket>,
}

impl IcaoFilterInner {
    fn new() -> Self {
        IcaoFilterInner {
            buckets: (0..ICAO_FILTER_SIZE).map(|_| IcaoBucket { entries: Vec::new() }).collect(),
        }
    }

    fn add(&mut self, addr: u32) {
        let idx = (addr as usize) % ICAO_FILTER_SIZE;
        let bucket = &mut self.buckets[idx];
        if !bucket.entries.contains(&addr) {
            bucket.entries.push(addr);
        }
    }

    fn test(&self, addr: u32) -> bool {
        let idx = (addr as usize) % ICAO_FILTER_SIZE;
        self.buckets[idx].entries.contains(&addr)
    }
}

static ICAO_FILTER: LazyLock<Mutex<IcaoFilterInner>> = LazyLock::new(|| {
    Mutex::new(IcaoFilterInner::new())
});

pub fn icao_filter_add(addr: u32) {
    if let Ok(mut filter) = ICAO_FILTER.lock() {
        filter.add(addr);
    }
}

pub fn icao_filter_test(addr: u32) -> bool {
    if let Ok(filter) = ICAO_FILTER.lock() {
        filter.test(addr)
    } else {
        false
    }
}

pub fn clear_filter() {
    if let Ok(mut filter) = ICAO_FILTER.lock() {
        filter.buckets.iter_mut().for_each(|b| b.entries.clear());
    }
}

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortColumn {
    Dist,
    Alt,
    Gs,
    Seen,
    Icao,
    Flight,
    Registration,
    Type,
}

#[derive(Debug, Clone, Parser)]
pub struct ViewsbArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 30005)]
    pub port: u16,
    #[arg(long)]
    pub no_interactive: bool,
    #[arg(long, default_value_t = 0)]
    pub count: usize,
    #[arg(long)]
    pub metric: bool,
    #[arg(long, value_enum)]
    pub sort: Option<SortColumn>,
    #[arg(long)]
    pub lat: Option<f64>,
    #[arg(long)]
    pub lon: Option<f64>,
    #[arg(long)]
    pub show_all: bool,
    #[arg(long)]
    pub modeac: bool,
    #[arg(long)]
    pub json: Option<String>,
    #[arg(long)]
    pub csv: Option<String>,
}

impl ViewsbArgs {
    pub fn default_sort(&self) -> SortColumn {
        if self.lat.is_some() && self.lon.is_some() {
            SortColumn::Dist
        } else {
            SortColumn::Seen
        }
    }
}

/// Comm-B message format inference (readsb.h:254).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommBFormat {
    #[default] Unknown,
    Ambiguous,
    EmptyResponse,
    DatalinkCaps,
    GicbCaps,
    AircraftIdent,
    AcasRA,
    VerticalIntent,
    TrackTurn,
    HeadingSpeed,
    MeteorologicalRoutine,
}

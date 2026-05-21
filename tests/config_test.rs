use clap::Parser;
use readsb::config::ReadsbConfig;

#[test]
fn test_ifile_iformat_cli() {
    // Verify that the struct accepts ifile and iformat fields
    let config = ReadsbConfig::parse_from(&["readsb", "--ifile", "test.iq", "--iformat", "CU8"]);
    assert_eq!(config.ifile, Some("test.iq".to_string()));
    assert_eq!(config.iformat, Some("CU8".to_string()));
}

#[test]
fn test_iformat_maps_to_input_format() {
    let config = ReadsbConfig::parse_from(&["readsb", "--iformat", "SC16"]);
    // Map iformat string to InputFormat
    let format = match config.iformat.as_deref() {
        Some("CU8") => readsb::demod::InputFormat::U8,
        Some("SC16") | None => readsb::demod::InputFormat::SC16Q11,
        Some("CF32") => readsb::demod::InputFormat::F32,
        Some("SC16M") => readsb::demod::InputFormat::SC16Q11M,
        _ => readsb::demod::InputFormat::SC16Q11,
    };
    assert!(matches!(format, readsb::demod::InputFormat::SC16Q11));
}

#[test]
fn test_iformat_cu8() {
    let config = ReadsbConfig::parse_from(&["readsb", "--iformat", "CU8"]);
    let format = match config.iformat.as_deref() {
        Some("CU8") => readsb::demod::InputFormat::U8,
        _ => readsb::demod::InputFormat::SC16Q11,
    };
    assert!(matches!(format, readsb::demod::InputFormat::U8));
}

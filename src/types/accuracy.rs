use crate::types::status::SilType;
use crate::types::status::NavModes;
use crate::types::status::NavAltitudeSource;

/// Accuracy/integrity fields from TSS or op status.
/// Mirrors the accuracy sub-struct in modesMessage (readsb.h:1163-1189).
#[derive(Debug, Clone, Copy)]
pub struct MessageAccuracy {
    pub nic_a_valid: bool, pub nic_b_valid: bool, pub nic_c_valid: bool,
    pub nic_baro_valid: bool, pub nac_p_valid: bool, pub nac_v_valid: bool,
    pub gva_valid: bool, pub sda_valid: bool,
    pub nic_a: bool, pub nic_b: bool, pub nic_c: bool, pub nic_baro: bool,
    pub nac_p: u32, pub nac_v: u32, pub sil: u32, pub gva: u32, pub sda: u32,
    pub sil_type: SilType,
}

impl Default for MessageAccuracy {
    fn default() -> Self {
        MessageAccuracy {
            nic_a_valid: false, nic_b_valid: false, nic_c_valid: false,
            nic_baro_valid: false, nac_p_valid: false, nac_v_valid: false,
            gva_valid: false, sda_valid: false,
            nic_a: false, nic_b: false, nic_c: false, nic_baro: false,
            nac_p: 0, nac_v: 0, sil: 0, gva: 0, sda: 0,
            sil_type: SilType::Invalid,
        }
    }
}

/// Operational Status message fields (readsb.h:1193-1224).
#[derive(Debug, Clone, Copy)]
pub struct OpStatus {
    pub valid: bool, pub version: u8, pub sil_type: SilType,
    pub om_acas_ra: bool, pub om_ident: bool, pub om_atc: bool, pub om_saf: bool,
    pub cc_acas: bool, pub cc_cdti: bool, pub cc_1090_in: bool,
    pub cc_arv: bool, pub cc_ts: bool, pub cc_tc: u8,
    pub cc_uat_in: bool, pub cc_poa: bool, pub cc_b2_low: bool,
    pub cc_lw_valid: bool, pub cc_lw: u32, pub cc_antenna_offset: u32,
}

/// Navigation state from Target State & Status + Comm-B BDS4,0.
/// Mirrors the nav sub-struct in modesMessage (readsb.h:1228-1247).
#[derive(Debug, Clone, Copy)]
pub struct NavState {
    pub fms_altitude: u32, pub mcp_altitude: u32,
    pub qnh: f32, pub heading: f32,
    pub heading_valid: bool, pub fms_altitude_valid: bool,
    pub mcp_altitude_valid: bool, pub qnh_valid: bool,
    pub modes_valid: bool,
    pub altitude_source: NavAltitudeSource,
    pub modes: NavModes,
}

impl Default for NavState {
    fn default() -> Self {
        NavState {
            fms_altitude: 0, mcp_altitude: 0, qnh: 0.0, heading: 0.0,
            heading_valid: false, fms_altitude_valid: false,
            mcp_altitude_valid: false, qnh_valid: false, modes_valid: false,
            altitude_source: NavAltitudeSource::Invalid,
            modes: NavModes::empty(),
        }
    }
}

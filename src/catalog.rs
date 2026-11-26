use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadphoneFormFactor {
    #[serde(rename = "OVER_EAR")]
    OverEar,
    #[serde(rename = "EARBUDS")]
    Earbuds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatteryBaseline {
    pub anc_hours: f64,
    pub off_hours: f64,
    pub high_dsp_anc_hours: f64,
    pub high_dsp_off_hours: f64,
}

impl BatteryBaseline {
    pub fn anc_drain_per_hour(self) -> f64 {
        100.0 / self.anc_hours
    }

    pub fn off_drain_per_hour(self) -> f64 {
        100.0 / self.off_hours
    }

    pub fn high_dsp_anc_drain_per_hour(self) -> f64 {
        100.0 / self.high_dsp_anc_hours
    }

    pub fn high_dsp_off_drain_per_hour(self) -> f64 {
        100.0 / self.high_dsp_off_hours
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceProfile {
    pub display_family: String,
    pub form_factor: HeadphoneFormFactor,
    pub battery_baseline: BatteryBaseline,
}

pub const DEFAULT_OVER_EAR_BASELINE: BatteryBaseline = BatteryBaseline {
    anc_hours: 30.0,
    off_hours: 40.0,
    high_dsp_anc_hours: 23.0,
    high_dsp_off_hours: 31.0,
};

pub const DEFAULT_EARBUDS_BASELINE: BatteryBaseline = BatteryBaseline {
    anc_hours: 8.0,
    off_hours: 11.0,
    high_dsp_anc_hours: 6.0,
    high_dsp_off_hours: 8.5,
};

struct CatalogEntry {
    aliases: &'static [&'static str],
    family: &'static str,
    form_factor: HeadphoneFormFactor,
    baseline: BatteryBaseline,
}

const fn b(anc: f64, off: f64, hd_anc: f64, hd_off: f64) -> BatteryBaseline {
    BatteryBaseline {
        anc_hours: anc,
        off_hours: off,
        high_dsp_anc_hours: hd_anc,
        high_dsp_off_hours: hd_off,
    }
}

const CATALOG: &[CatalogEntry] = &[
    // Sony Over-ear models
    CatalogEntry {
        aliases: &["WH-1000XM6", "WH1000XM6"],
        family: "WH-1000XM6",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 40.0, 24.0, 32.0),
    },
    CatalogEntry {
        aliases: &["WH-1000XM5", "WH1000XM5"],
        family: "WH-1000XM5",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: DEFAULT_OVER_EAR_BASELINE,
    },
    CatalogEntry {
        aliases: &["WH-1000XM4", "WH1000XM4"],
        family: "WH-1000XM4",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 38.0, 22.0, 30.0),
    },
    CatalogEntry {
        aliases: &["ULT WEAR", "WH-ULT900N", "WHULT900N"],
        family: "ULT WEAR",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 50.0, 23.0, 38.0),
    },
    CatalogEntry {
        aliases: &["WH-CH720N", "WHCH720N"],
        family: "WH-CH720N",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(35.0, 50.0, 30.0, 42.0),
    },
    CatalogEntry {
        aliases: &["WH-CH520", "WHCH520"],
        family: "WH-CH520",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(50.0, 50.0, 45.0, 45.0),
    },
    CatalogEntry {
        aliases: &["WH-XB910N", "WHXB910N"],
        family: "WH-XB910N",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 50.0, 22.0, 36.0),
    },
    // Sony V1 Over-ear models
    CatalogEntry {
        aliases: &["WH-1000XM3", "WH1000XM3"],
        family: "WH-1000XM3",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 38.0, 22.0, 30.0),
    },
    CatalogEntry {
        aliases: &["WH-1000XM2", "WH1000XM2"],
        family: "WH-1000XM2",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 40.0, 22.0, 30.0),
    },
    CatalogEntry {
        aliases: &["MDR-1000X", "MDR1000X"],
        family: "MDR-1000X",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(20.0, 22.0, 16.0, 18.0),
    },
    CatalogEntry {
        aliases: &["WH-H900N", "WHH900N", "H.EAR ON 2"],
        family: "WH-H900N",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(28.0, 34.0, 22.0, 28.0),
    },
    CatalogEntry {
        aliases: &["WH-XB900N", "WHXB900N"],
        family: "WH-XB900N",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(30.0, 35.0, 24.0, 30.0),
    },
    // Sony Earbuds
    CatalogEntry {
        aliases: &["WF-1000XM5", "WF1000XM5"],
        family: "WF-1000XM5",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(8.0, 12.0, 6.0, 9.0),
    },
    CatalogEntry {
        aliases: &["WF-1000XM4", "WF1000XM4"],
        family: "WF-1000XM4",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(8.0, 12.0, 5.5, 8.5),
    },
    CatalogEntry {
        aliases: &["WF-1000XM3", "WF1000XM3"],
        family: "WF-1000XM3",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 8.0, 5.0, 7.0),
    },
    CatalogEntry {
        aliases: &["WI-1000XM2", "WI1000XM2", "WI-1000X", "WI1000X"],
        family: "WI-1000X",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(10.0, 14.0, 8.0, 11.0),
    },
    CatalogEntry {
        aliases: &[
            "LINKBUDS S",
            "WF-LS900N",
            "WFLS900N",
            "LINKBUDS FIT",
            "WF-LS910N",
            "WFLS910N",
        ],
        family: "LinkBuds S",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 9.0, 4.5, 7.0),
    },
    CatalogEntry {
        aliases: &["WF-C700N", "WFC700N", "WF-C710N", "WFC710N"],
        family: "WF-C700N",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(7.5, 10.0, 6.0, 8.5),
    },
    CatalogEntry {
        aliases: &["WF-C500", "WFC500", "WF-C510", "WFC510"],
        family: "WF-C500",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(10.0, 10.0, 8.5, 8.5),
    },
    // Bose
    CatalogEntry {
        aliases: &[
            "QC ULTRA EARBUDS",
            "QUIETCOMFORT ULTRA EARBUDS",
            "QC EARBUDS",
            "QUIETCOMFORT EARBUDS",
        ],
        family: "Bose QC Earbuds",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 8.0, 5.5, 7.5),
    },
    CatalogEntry {
        aliases: &["QC ULTRA", "QUIETCOMFORT ULTRA"],
        family: "Bose QC Ultra",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(24.0, 30.0, 18.0, 24.0),
    },
    CatalogEntry {
        aliases: &["QC45", "QUIETCOMFORT 45", "QUIETCOMFORT HEADPHONES", "BOSE QC"],
        family: "Bose QuietComfort",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(24.0, 28.0, 22.0, 26.0),
    },
    CatalogEntry {
        aliases: &["QC35", "QUIETCOMFORT 35"],
        family: "Bose QC35",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(20.0, 40.0, 18.0, 36.0),
    },
    CatalogEntry {
        aliases: &["NC 700", "NC700", "NOISE CANCELLING HEADPHONES 700"],
        family: "Bose NC 700",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(20.0, 24.0, 18.0, 22.0),
    },
    // Anker Soundcore
    CatalogEntry {
        aliases: &["SPACE Q45", "SOUNDCORE Q45"],
        family: "Soundcore Space Q45",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(50.0, 65.0, 35.0, 45.0),
    },
    CatalogEntry {
        aliases: &["SPACE ONE"],
        family: "Soundcore Space One",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(40.0, 55.0, 30.0, 42.0),
    },
    CatalogEntry {
        aliases: &["LIFE Q30", "LIFE Q35", "Q30", "Q35"],
        family: "Soundcore Life Q30",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(40.0, 60.0, 32.0, 48.0),
    },
    CatalogEntry {
        aliases: &["LIBERTY 4", "SPACE A40"],
        family: "Soundcore Earbuds",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(8.0, 10.0, 6.0, 8.0),
    },
    // Samsung Galaxy Buds
    CatalogEntry {
        aliases: &["GALAXY BUDS2 PRO", "BUDS2 PRO"],
        family: "Galaxy Buds2 Pro",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(5.0, 8.0, 4.5, 7.0),
    },
    CatalogEntry {
        aliases: &["GALAXY BUDS2", "BUDS2"],
        family: "Galaxy Buds2",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(5.0, 7.5, 4.5, 7.0),
    },
    CatalogEntry {
        aliases: &["GALAXY BUDS PRO", "BUDS PRO"],
        family: "Galaxy Buds Pro",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(5.0, 8.0, 4.5, 7.0),
    },
    CatalogEntry {
        aliases: &["GALAXY BUDS FE", "BUDS FE"],
        family: "Galaxy Buds FE",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 8.5, 5.5, 8.0),
    },
    CatalogEntry {
        aliases: &["GALAXY BUDS LIVE", "BUDS LIVE"],
        family: "Galaxy Buds Live",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 8.0, 5.5, 7.5),
    },
    CatalogEntry {
        aliases: &["GALAXY BUDS+", "BUDS+"],
        family: "Galaxy Buds+",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(11.0, 11.0, 10.0, 10.0),
    },
    // Apple AirPods
    CatalogEntry {
        aliases: &["AIRPODS MAX"],
        family: "AirPods Max",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(20.0, 20.0, 18.0, 18.0),
    },
    CatalogEntry {
        aliases: &["AIRPODS PRO 3", "AIRPODS PRO 2", "AIRPODS PRO"],
        family: "AirPods Pro",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 7.0, 5.5, 6.5),
    },
    CatalogEntry {
        aliases: &["AIRPODS 4", "AIRPODS 3", "AIRPODS"],
        family: "AirPods",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(5.0, 6.0, 4.5, 5.5),
    },
    // Beats Over-ear & On-ear
    CatalogEntry {
        aliases: &[
            "BEATS STUDIO PRO",
            "BEATS SOLO 4",
            "BEATS SOLO PRO",
            "BEATS STUDIO3",
            "BEATS SOLO3",
        ],
        family: "Beats",
        form_factor: HeadphoneFormFactor::OverEar,
        baseline: b(24.0, 40.0, 22.0, 36.0),
    },
    // Beats Earbuds & Neckband
    CatalogEntry {
        aliases: &[
            "POWERBEATS PRO",
            "BEATS FIT PRO",
            "BEATS STUDIO BUDS+",
            "BEATS STUDIO BUDS",
            "BEATS FLEX",
            "BEATSX",
        ],
        family: "Beats Buds",
        form_factor: HeadphoneFormFactor::Earbuds,
        baseline: b(6.0, 8.0, 5.5, 7.5),
    },
];

pub fn resolve_device_profile(model_name: &str, has_dual_battery: bool) -> DeviceProfile {
    let upper = model_name.trim().to_uppercase();
    for entry in CATALOG {
        if entry.aliases.iter().any(|alias| upper.contains(alias)) {
            return DeviceProfile {
                display_family: entry.family.to_string(),
                form_factor: entry.form_factor,
                battery_baseline: entry.baseline,
            };
        }
    }

    let looks_like_earbuds = has_dual_battery
        || upper.starts_with("WF-")
        || upper.starts_with("WF")
        || upper.contains("LINKBUDS")
        || upper.contains("AIRPODS")
        || upper.contains("FREEBUDS")
        || upper.contains("EARBUDS")
        || upper.contains("BUDS")
        || upper.contains("PODS")
        || upper.contains("TWS");

    if looks_like_earbuds {
        DeviceProfile {
            display_family: if model_name.trim().is_empty() {
                "Wireless Earbuds".to_string()
            } else {
                model_name.trim().to_string()
            },
            form_factor: HeadphoneFormFactor::Earbuds,
            battery_baseline: DEFAULT_EARBUDS_BASELINE,
        }
    } else {
        DeviceProfile {
            display_family: if model_name.trim().is_empty() {
                "Bluetooth Headphones".to_string()
            } else {
                model_name.trim().to_string()
            },
            form_factor: HeadphoneFormFactor::OverEar,
            battery_baseline: DEFAULT_OVER_EAR_BASELINE,
        }
    }
}

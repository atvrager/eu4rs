//! UI region definitions for EU4 screen extraction.
//!
//! These coordinates are calibrated for 1920x1080 vanilla EU4.
//! Use `calibrate.html` to adjust visually.

/// A rectangular region on the screen for OCR extraction.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// Human-readable name for this region
    pub name: &'static str,
    /// X coordinate (pixels from left edge)
    pub x: u32,
    /// Y coordinate (pixels from top edge)
    pub y: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// RGB color for calibration overlay
    pub color: [u8; 3],
}

impl Region {
    /// Create a new region.
    pub const fn new(
        name: &'static str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: [u8; 3],
    ) -> Self {
        Self {
            name,
            x,
            y,
            width,
            height,
            color,
        }
    }
}

// ============================================================================
// Calibrated UI Regions (1920x1080)
// ============================================================================

// Resources (top bar, row 1)
pub const TREASURY: Region = Region::new("Treasury", 169, 13, 48, 21, [0, 255, 0]);
pub const MANPOWER: Region = Region::new("Manpower", 255, 12, 50, 24, [0, 136, 255]);
pub const SAILORS: Region = Region::new("Sailors", 336, 11, 50, 24, [0, 255, 255]);

// Monarch points (row 2)
pub const ADM_MANA: Region = Region::new("ADM Mana", 520, 55, 34, 20, [255, 68, 68]);
pub const DIP_MANA: Region = Region::new("DIP Mana", 577, 55, 33, 19, [68, 255, 68]);
pub const MIL_MANA: Region = Region::new("MIL Mana", 639, 56, 34, 21, [68, 68, 255]);

// Country stats
pub const STABILITY: Region = Region::new("Stability", 419, 16, 30, 20, [255, 170, 0]);
pub const CORRUPTION: Region = Region::new("Corruption", 485, 14, 50, 24, [255, 0, 170]);
pub const PRESTIGE: Region = Region::new("Prestige", 545, 17, 37, 19, [170, 255, 0]);
pub const GOVT_STRENGTH: Region = Region::new("Govt Strength", 615, 15, 37, 22, [255, 255, 0]);
pub const POWER_PROJ: Region = Region::new("Power Proj", 700, 14, 40, 20, [170, 0, 255]);

// Envoys (N/M format)
pub const MERCHANTS: Region = Region::new("Merchants", 734, 32, 40, 20, [255, 136, 0]);
pub const COLONISTS: Region = Region::new("Colonists", 774, 39, 40, 13, [136, 255, 0]);
pub const DIPLOMATS: Region = Region::new("Diplomats", 816, 35, 37, 17, [0, 136, 255]);
pub const MISSIONARIES: Region = Region::new("Missionaries", 859, 35, 34, 18, [255, 0, 136]);

// Info displays
pub const COUNTRY: Region = Region::new("Country", 146, 49, 344, 30, [255, 255, 255]);
pub const AGE: Region = Region::new("Age", 740, 54, 160, 21, [136, 136, 136]);
pub const DATE: Region = Region::new("Date", 1697, 16, 132, 21, [255, 0, 0]);

// ============================================================================
// Province Panel Regions (when province selected)
// Calibrated 2024-12 via calibrate.html against Vienna screenshot
// ============================================================================

// Province info (header area)
pub const PROV_NAME: Region = Region::new("Prov Name", 106, 418, 157, 24, [255, 200, 0]);
pub const PROV_STATE: Region = Region::new("Prov State", 269, 421, 171, 24, [200, 255, 0]);

// Development values (below terrain image)
pub const PROV_TAX: Region = Region::new("Prov Tax", 83, 552, 25, 18, [255, 100, 100]);
pub const PROV_PROD: Region = Region::new("Prov Prod", 160, 554, 25, 18, [100, 255, 100]);
pub const PROV_MANP: Region = Region::new("Prov Manp", 238, 553, 25, 18, [100, 100, 255]);

// Development + buttons (clickable targets - to LEFT of dev values)
pub const PROV_TAX_BTN: Region = Region::new("Tax +Btn", 48, 553, 22, 22, [255, 50, 50]);
pub const PROV_PROD_BTN: Region = Region::new("Prod +Btn", 125, 557, 22, 22, [50, 255, 50]);
pub const PROV_MANP_BTN: Region = Region::new("Manp +Btn", 204, 555, 22, 22, [50, 50, 255]);

/// Top bar regions (always visible).
#[allow(dead_code)]
pub const TOP_BAR_REGIONS: &[Region] = &[
    TREASURY,
    MANPOWER,
    SAILORS,
    ADM_MANA,
    DIP_MANA,
    MIL_MANA,
    STABILITY,
    CORRUPTION,
    PRESTIGE,
    GOVT_STRENGTH,
    POWER_PROJ,
    MERCHANTS,
    COLONISTS,
    DIPLOMATS,
    MISSIONARIES,
    COUNTRY,
    AGE,
    DATE,
];

/// Province panel regions (when province selected).
#[allow(dead_code)]
pub const PROVINCE_PANEL_REGIONS: &[Region] = &[
    PROV_NAME,
    PROV_STATE,
    PROV_TAX,
    PROV_PROD,
    PROV_MANP,
    PROV_TAX_BTN,
    PROV_PROD_BTN,
    PROV_MANP_BTN,
];

/// All defined regions for iteration.
pub const ALL_REGIONS: &[Region] = &[
    // Top bar
    TREASURY,
    MANPOWER,
    SAILORS,
    ADM_MANA,
    DIP_MANA,
    MIL_MANA,
    STABILITY,
    CORRUPTION,
    PRESTIGE,
    GOVT_STRENGTH,
    POWER_PROJ,
    MERCHANTS,
    COLONISTS,
    DIPLOMATS,
    MISSIONARIES,
    COUNTRY,
    AGE,
    DATE,
    // Province panel
    PROV_NAME,
    PROV_STATE,
    PROV_TAX,
    PROV_PROD,
    PROV_MANP,
    PROV_TAX_BTN,
    PROV_PROD_BTN,
    PROV_MANP_BTN,
];

/// Print a legend of all regions to the console.
pub fn print_legend() {
    println!("Region Legend (1920x1080):");
    println!("{:-<70}", "");
    for region in ALL_REGIONS {
        println!(
            "  {:16} x={:>4}, y={:>3}, w={:>3}, h={:>2}",
            region.name, region.x, region.y, region.width, region.height
        );
    }
    println!("{:-<70}", "");
}

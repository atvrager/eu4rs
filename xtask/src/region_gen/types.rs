//! Type definitions for region generation.

/// Which GUI file contains the target element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiFile {
    TopBar,
    SpeedControls,
    ProvinceView,
}

/// Type of GUI element to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Fields used for future GUI parsing
pub enum ElementType {
    Text,
    Icon,
    Button,
}

/// Logical grouping for generated code organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionGroup {
    TopBar,
    ProvincePanel,
}

/// Maps an OCR region to its source GUI element.
#[derive(Debug, Clone)]
pub struct RegionMapping {
    /// Const name in generated code (e.g., "TREASURY")
    pub const_name: &'static str,
    /// Display name for the region (e.g., "Treasury")
    pub display_name: &'static str,
    /// Which GUI file contains this element
    #[allow(dead_code)] // Used for future GUI parsing
    pub gui_file: GuiFile,
    /// Possible element names to match (tried in order)
    #[allow(dead_code)] // Used for future GUI parsing
    pub element_patterns: &'static [&'static str],
    /// Type of element
    #[allow(dead_code)] // Used for future GUI parsing
    pub element_type: ElementType,
    /// RGB color for calibration overlays
    pub color: [u8; 3],
    /// Logical grouping
    pub group: RegionGroup,
}

/// A successfully resolved region with calculated coordinates.
#[derive(Debug, Clone)]
pub struct ResolvedRegion {
    pub const_name: String,
    pub display_name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: [u8; 3],
    pub group: RegionGroup,
    /// Optional: the matched element name for debugging
    #[allow(dead_code)] // Used for future GUI parsing
    pub matched_element: Option<String>,
}

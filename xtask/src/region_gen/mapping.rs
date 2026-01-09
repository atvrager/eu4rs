//! Mapping table from OCR regions to GUI elements.

use super::types::{ElementType, GuiFile, RegionGroup, RegionMapping};

/// Complete mapping table for all 26 OCR regions.
///
/// Each mapping specifies:
/// - The const name and display name for code generation
/// - Which GUI file contains the element
/// - Possible element names to match (fuzzy matching applied)
/// - Element type, color, and grouping
pub const REGION_MAPPINGS: &[RegionMapping] = &[
    // ========================================================================
    // Top Bar Regions (18)
    // ========================================================================

    // Resources (row 1)
    RegionMapping {
        const_name: "TREASURY",
        display_name: "Treasury",
        gui_file: GuiFile::TopBar,
        element_patterns: &[
            "text_treasury",
            "treasury_text",
            "treasury_value",
            "treasury",
        ],
        element_type: ElementType::Text,
        color: [0, 255, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "MANPOWER",
        display_name: "Manpower",
        gui_file: GuiFile::TopBar,
        element_patterns: &[
            "text_manpower",
            "manpower_text",
            "manpower_value",
            "manpower",
        ],
        element_type: ElementType::Text,
        color: [0, 136, 255],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "SAILORS",
        display_name: "Sailors",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_sailors", "sailors_text", "sailors_value", "sailors"],
        element_type: ElementType::Text,
        color: [0, 255, 255],
        group: RegionGroup::TopBar,
    },
    // Monarch points (row 2)
    RegionMapping {
        const_name: "ADM_MANA",
        display_name: "ADM Mana",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_adm_power", "adm_power", "text_power_adm", "power_adm"],
        element_type: ElementType::Text,
        color: [255, 68, 68],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "DIP_MANA",
        display_name: "DIP Mana",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_dip_power", "dip_power", "text_power_dip", "power_dip"],
        element_type: ElementType::Text,
        color: [68, 255, 68],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "MIL_MANA",
        display_name: "MIL Mana",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_mil_power", "mil_power", "text_power_mil", "power_mil"],
        element_type: ElementType::Text,
        color: [68, 68, 255],
        group: RegionGroup::TopBar,
    },
    // Country stats
    RegionMapping {
        const_name: "STABILITY",
        display_name: "Stability",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_stability", "stability_text", "stability"],
        element_type: ElementType::Text,
        color: [255, 170, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "CORRUPTION",
        display_name: "Corruption",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_corruption", "corruption_text", "corruption"],
        element_type: ElementType::Text,
        color: [255, 0, 170],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "PRESTIGE",
        display_name: "Prestige",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_prestige", "prestige_text", "prestige"],
        element_type: ElementType::Text,
        color: [170, 255, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "GOVT_STRENGTH",
        display_name: "Govt Strength",
        gui_file: GuiFile::TopBar,
        element_patterns: &[
            "text_legitimacy",
            "legitimacy_text",
            "legitimacy",
            "government_power",
            "govt_strength",
        ],
        element_type: ElementType::Text,
        color: [255, 255, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "POWER_PROJ",
        display_name: "Power Proj",
        gui_file: GuiFile::TopBar,
        element_patterns: &[
            "text_power_projection",
            "power_projection",
            "power_proj",
            "projection",
        ],
        element_type: ElementType::Text,
        color: [170, 0, 255],
        group: RegionGroup::TopBar,
    },
    // Envoys (N/M format)
    RegionMapping {
        const_name: "MERCHANTS",
        display_name: "Merchants",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_merchants", "merchants_text", "merchants"],
        element_type: ElementType::Text,
        color: [255, 136, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "COLONISTS",
        display_name: "Colonists",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_colonists", "colonists_text", "colonists"],
        element_type: ElementType::Text,
        color: [136, 255, 0],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "DIPLOMATS",
        display_name: "Diplomats",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_diplomats", "diplomats_text", "diplomats"],
        element_type: ElementType::Text,
        color: [0, 136, 255],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "MISSIONARIES",
        display_name: "Missionaries",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_missionaries", "missionaries_text", "missionaries"],
        element_type: ElementType::Text,
        color: [255, 0, 136],
        group: RegionGroup::TopBar,
    },
    // Info displays
    RegionMapping {
        const_name: "COUNTRY",
        display_name: "Country",
        gui_file: GuiFile::TopBar,
        element_patterns: &[
            "country_name",
            "text_country",
            "countryname",
            "player_country",
            "country",
        ],
        element_type: ElementType::Text,
        color: [255, 255, 255],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "AGE",
        display_name: "Age",
        gui_file: GuiFile::TopBar,
        element_patterns: &["text_age", "age_text", "current_age", "age"],
        element_type: ElementType::Text,
        color: [136, 136, 136],
        group: RegionGroup::TopBar,
    },
    RegionMapping {
        const_name: "DATE",
        display_name: "Date",
        gui_file: GuiFile::SpeedControls,
        element_patterns: &["DateText", "date_text", "date"],
        element_type: ElementType::Text,
        color: [255, 0, 0],
        group: RegionGroup::TopBar,
    },
    // ========================================================================
    // Province Panel Regions (8)
    // ========================================================================

    // Province info (header area)
    RegionMapping {
        const_name: "PROV_NAME",
        display_name: "Prov Name",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &["province_name", "provincename", "name", "province_title"],
        element_type: ElementType::Text,
        color: [255, 200, 0],
        group: RegionGroup::ProvincePanel,
    },
    RegionMapping {
        const_name: "PROV_STATE",
        display_name: "Prov State",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &["state_name", "statename", "state", "province_state"],
        element_type: ElementType::Text,
        color: [200, 255, 0],
        group: RegionGroup::ProvincePanel,
    },
    // Development values
    RegionMapping {
        const_name: "PROV_TAX",
        display_name: "Prov Tax",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &["tax_value", "base_tax", "province_tax", "tax"],
        element_type: ElementType::Text,
        color: [255, 100, 100],
        group: RegionGroup::ProvincePanel,
    },
    RegionMapping {
        const_name: "PROV_PROD",
        display_name: "Prov Prod",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &[
            "production_value",
            "base_production",
            "province_production",
            "production",
        ],
        element_type: ElementType::Text,
        color: [100, 255, 100],
        group: RegionGroup::ProvincePanel,
    },
    RegionMapping {
        const_name: "PROV_MANP",
        display_name: "Prov Manp",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &[
            "manpower_value",
            "base_manpower",
            "province_manpower",
            "manpower",
        ],
        element_type: ElementType::Text,
        color: [100, 100, 255],
        group: RegionGroup::ProvincePanel,
    },
    // Development buttons (clickable targets)
    RegionMapping {
        const_name: "PROV_TAX_BTN",
        display_name: "Tax +Btn",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &[
            "tax_increase_button",
            "increase_tax",
            "tax_button",
            "tax_plus",
            "button_tax",
        ],
        element_type: ElementType::Button,
        color: [255, 50, 50],
        group: RegionGroup::ProvincePanel,
    },
    RegionMapping {
        const_name: "PROV_PROD_BTN",
        display_name: "Prod +Btn",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &[
            "production_increase_button",
            "increase_production",
            "production_button",
            "production_plus",
            "button_production",
        ],
        element_type: ElementType::Button,
        color: [50, 255, 50],
        group: RegionGroup::ProvincePanel,
    },
    RegionMapping {
        const_name: "PROV_MANP_BTN",
        display_name: "Manp +Btn",
        gui_file: GuiFile::ProvinceView,
        element_patterns: &[
            "manpower_increase_button",
            "increase_manpower",
            "manpower_button",
            "manpower_plus",
            "button_manpower",
        ],
        element_type: ElementType::Button,
        color: [50, 50, 255],
        group: RegionGroup::ProvincePanel,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_mapping_count() {
        assert_eq!(REGION_MAPPINGS.len(), 26, "Should have exactly 26 mappings");
    }

    #[test]
    fn test_no_duplicate_const_names() {
        let mut names = HashSet::new();
        for mapping in REGION_MAPPINGS {
            assert!(
                names.insert(mapping.const_name),
                "Duplicate const name: {}",
                mapping.const_name
            );
        }
    }

    #[test]
    fn test_color_uniqueness_note() {
        // Note: The original regions.rs allows some duplicate colors
        // (e.g., MANPOWER and DIPLOMATS both use [0, 136, 255])
        // This is acceptable for visual calibration purposes
        let mut colors = HashSet::new();
        let mut duplicate_count = 0;
        for mapping in REGION_MAPPINGS {
            if !colors.insert(mapping.color) {
                duplicate_count += 1;
            }
        }
        // Just verify that MOST colors are unique (allow a few duplicates)
        assert!(
            duplicate_count <= 3,
            "Too many duplicate colors: {}",
            duplicate_count
        );
    }

    #[test]
    fn test_group_counts() {
        let top_bar = REGION_MAPPINGS
            .iter()
            .filter(|m| m.group == RegionGroup::TopBar)
            .count();
        let province_panel = REGION_MAPPINGS
            .iter()
            .filter(|m| m.group == RegionGroup::ProvincePanel)
            .count();

        assert_eq!(top_bar, 18, "Should have 18 top bar regions");
        assert_eq!(province_panel, 8, "Should have 8 province panel regions");
    }

    #[test]
    fn test_all_have_patterns() {
        for mapping in REGION_MAPPINGS {
            assert!(
                !mapping.element_patterns.is_empty(),
                "{} has no element patterns",
                mapping.const_name
            );
        }
    }
}

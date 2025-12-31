//! GUI primitives - basic building blocks for UI panels.
//!
//! This module contains the typed widget wrappers that connect
//! Rust code to parsed GUI elements via the Binder system.
//!
//! ## Production Status (Phase 3.5)
//!
//! - **GuiText**: ✅ Actively used in TopBar, SpeedControls, CountrySelectPanel
//! - **GuiIcon**: ✅ Actively used in SpeedControls, CountrySelectPanel
//! - **GuiButton**: ⚠️ Available but not yet used in production panels (Phase 4)
//! - **GuiContainer**: ⚠️ Available but not yet used in production panels (Phase 4+)

mod button;
mod container;
mod icon;
mod text;

#[allow(unused_imports)] // Reserved for future interactive UI panels (Phase 4)
pub use button::{ButtonState, GuiButton};
#[allow(unused_imports)] // Reserved for future nested UI panel layouts
pub use container::GuiContainer;
pub use icon::GuiIcon;
pub use text::GuiText;

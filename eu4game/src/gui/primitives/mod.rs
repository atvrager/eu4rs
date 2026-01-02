//! GUI primitives - basic building blocks for UI panels.
//!
//! This module contains the typed widget wrappers that connect
//! Rust code to parsed GUI elements via the Binder system.
//!
//! ## Production Status
//!
//! - **GuiText**: ✅ Actively used in TopBar, SpeedControls, CountrySelectRightPanel (Phase 3.5)
//! - **GuiIcon**: ✅ Actively used in SpeedControls, CountrySelectRightPanel (Phase 3.5)
//! - **GuiButton**: ⚠️ Available but not yet used in production panels (Phase 4)
//! - **GuiContainer**: ⚠️ Available but not yet used in production panels (Phase 4+)
//! - **GuiListbox**: ⚠️ Core primitive implemented (Phase 7.3), rendering and interaction pending (Phase 7.4-7.5)

mod button;
mod checkbox;
mod container;
mod editbox;
mod icon;
mod listbox;
mod text;

#[allow(unused_imports)] // Reserved for future interactive UI panels (Phase 4)
pub use button::{ButtonState, GuiButton};
#[allow(unused_imports)] // Reserved for future interactive UI panels (Phase 4.2)
pub use checkbox::GuiCheckbox;
#[allow(unused_imports)] // Reserved for future nested UI panel layouts
pub use container::GuiContainer;
#[allow(unused_imports)] // Reserved for future interactive UI panels (Phase 4.3)
pub use editbox::GuiEditBox;
pub use icon::GuiIcon;
#[allow(unused_imports)] // Reserved for Phase 7.4 (Rendering) and Phase 7.5 (Interaction)
pub use listbox::{GuiListbox, ListAdapter};
pub use text::GuiText;

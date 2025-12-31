#![allow(dead_code)]
//! GUI primitives - basic building blocks for UI panels.
//!
//! This module contains the typed widget wrappers that connect
//! Rust code to parsed GUI elements via the Binder system.

mod button;
mod container;
mod icon;
mod text;

#[allow(unused_imports)] // Will be used when panels are converted to binder pattern
pub use button::{ButtonState, GuiButton};
#[allow(unused_imports)]
pub use container::GuiContainer;
#[allow(unused_imports)]
pub use icon::GuiIcon;
#[allow(unused_imports)]
pub use text::GuiText;

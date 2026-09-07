//! Native system tray integration designed specifically for GPUI.
//!
//! Tray menus use [`gpui::MenuItem`] directly and dispatch ordinary GPUI
//! actions. Native backends only receive immutable, platform-neutral snapshots;
//! they never retain or access [`gpui::App`].
//!
//! Select exactly one UI dependency: `gpui` (the default) for upstream GPUI,
//! or `gpui-kit` with default features disabled for GPUI Kit's `gpui-pre` types.

#[cfg(all(feature = "gpui", feature = "gpui-kit"))]
compile_error!(
    "features `gpui` and `gpui-kit` are mutually exclusive; use default-features = false to select `gpui-kit`"
);

#[cfg(not(any(feature = "gpui", feature = "gpui-kit")))]
compile_error!("enable exactly one of the `gpui` or `gpui-kit` features");

#[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
extern crate gpui_kit_backend as gpui;

#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
mod backend;
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
mod error;
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
mod icon;
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
mod menu;
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
mod tray;

#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
pub use error::{Error, Result};
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
pub use icon::Icon;
#[cfg(any(feature = "gpui", feature = "gpui-kit"))]
pub use tray::{Tray, TrayBuilder};

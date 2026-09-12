//! Native system tray integration designed specifically for GPUI.
//!
//! Tray menus use [`gpui::MenuItem`] directly and dispatch ordinary GPUI
//! actions. Native backends only receive immutable, platform-neutral snapshots;
//! they never retain or access [`gpui::App`].
//!
//! Select exactly one UI dependency: `gpui` (the default) for upstream GPUI,
//! `gpui-kit` for GPUI Kit's `gpui-pre` types, or `gpui-ce` for GPUI Community
//! Edition. Disable default features when selecting either alternative.

#[cfg(any(
    all(feature = "gpui", feature = "gpui-kit"),
    all(feature = "gpui", feature = "gpui-ce"),
    all(feature = "gpui-kit", feature = "gpui-ce"),
))]
compile_error!(
    "features `gpui`, `gpui-kit`, and `gpui-ce` are mutually exclusive; use default-features = false and select exactly one backend"
);

#[cfg(not(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce")))]
compile_error!("enable exactly one of the `gpui`, `gpui-kit`, or `gpui-ce` features");

#[cfg(all(feature = "gpui-ce", not(any(feature = "gpui", feature = "gpui-kit"))))]
extern crate gpui_ce_backend as gpui;
#[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
extern crate gpui_kit_backend as gpui;

#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod backend;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod error;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod icon;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod menu;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
mod tray;

#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
pub use error::{Error, Result};
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
pub use icon::Icon;
#[cfg(any(feature = "gpui", feature = "gpui-kit", feature = "gpui-ce"))]
pub use tray::{Tray, TrayBuilder};

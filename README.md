# gpui-tray

Native system tray integration for GPUI. Menus use `gpui::MenuItem` and dispatch
GPUI actions through the application's foreground executor.

| Platform | Native backend |
| --- | --- |
| macOS | AppKit through `objc2` |
| Windows | `Shell_NotifyIconW` and Win32 menus |
| Linux | StatusNotifierItem and DBusMenu through `zbus`, without GTK |

## Install

Upstream GPUI is the default:

```toml
gpui-tray = "0.2"
```

For GPUI Kit:

```toml
gpui-kit = "0.6"
gpui-tray = { version = "0.2", default-features = false, features = ["gpui-kit", "menu-state"] }
```

For [GPUI Community Edition](https://github.com/gpui-ce/gpui-ce):

```toml
gpui = { package = "gpui-ce", version = "0.2.2", default-features = false }
gpui-tray = { version = "0.2", default-features = false, features = ["gpui-ce", "menu-state"] }
```

Select exactly one of `gpui`, `gpui-kit`, or `gpui-ce`. Your application and the
tray must use the same GPUI source and compatible versions to share Rust types.
GPUI Kit uses `gpui-pre` 0.3.4; Community Edition uses `gpui-ce` 0.2.2, with
compatible updates allowed.

## Quick start

Create an icon, attach menu actions, and keep the returned `Tray` alive:

```rust,ignore
use gpui::{App, MenuItem, actions};
use gpui_tray::{Icon, Tray};

actions!(tray_example, [Open, Quit]);

fn build_tray(cx: &mut App, rgba: Vec<u8>) -> gpui_tray::Result<Tray> {
    let icon = Icon::from_rgba(rgba, 32, 32).build()?;
    Tray::builder()
        .icon(icon)
        .title("My App")
        .tooltip("My App")
        .menu(|_cx| {
            vec![
                MenuItem::action("Open", Open),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ]
        })
        .build(cx)
}
```

Register handlers for your actions in the GPUI app. Menus can also contain
submenus through `MenuItem::submenu`. The [complete example](examples/tray.rs)
shows action handlers, submenus, and checked and disabled state.

The menu refreshes after each tray action. Call `Tray::refresh_menu` when its
state changes elsewhere. To forward checked and disabled state, enable
[`menu-state`](#checked-and-disabled-menu-state).

macOS displays the title beside the icon. Linux hosts choose how to present it;
Windows ignores it. Use `.tooltip(...)` for hover text.

## Icons

Use tightly packed RGBA8 pixels or a GPUI image, including SVG. Image decoding
and validation happen when `.build()` is called:

```rust,ignore
let icon = Icon::from_rgba(rgba, 32, 32).build()?;
let icon = Icon::from_gpui(&image, cx).build()?;
```

Replace an icon with `tray.set_icon(icon, cx)?` or remove it with
`tray.clear_icon(cx)?`. A macOS title or a Linux icon-theme name can remain
visible after clearing the image; use `set_visible` to hide the whole item.

### macOS

For artwork with two pixels per point, use `.scale_factor(2.0)`. Enable
`.template(true)` for artwork that adapts to the menu bar appearance:

```rust,ignore
let icon = Icon::from_rgba(rgba, 36, 36)
    .scale_factor(2.0)
    .template(true)
    .build()?;
```

This displays at 18×18 points. Template artwork uses a transparent background.

To use an SF Symbol:

```rust,ignore
use gpui_tray::{Icon, SymbolScale, SymbolWeight};

let icon = Icon::system_symbol("externaldrive")
    .accessibility_description("External drive")
    .point_size(16.0)
    .weight(SymbolWeight::Regular)
    .scale(SymbolScale::Medium)
    .build()?;
```

You can pass a retained `NSImage` directly to `.icon(...)` or `set_icon(...)`.
`Icon::to_ns_image()` returns the same image for a native icon and a new image
for a bitmap. After editing it, call `tray.set_icon(image, cx)?` to apply it.

### Linux

Use an icon-theme name to let the desktop shell recolor a symbolic icon for its
panel. Keep a bitmap as the fallback:

```rust,ignore
let builder = Tray::builder().icon(icon);

#[cfg(target_os = "linux")]
let builder = builder
    .icon_name("my-app-symbolic")
    .icon_theme_path("/path/to/icons");

let tray = builder.build(cx)?;
```

These icon-theme methods are available only on Linux.

## Activation and lifecycle

Use `.on_activate(action)` for primary tray activation. On macOS, an attached
menu opens on the primary click; activation is dispatched when there is no menu.

`Tray::close` removes the native item and is safe to call more than once.
Dropping the last clone performs the same cleanup on a best-effort basis.
The crate uses GPUI's existing event loop.

### Wayland activation tokens

Use `.on_activate_with(|token, cx| { ... })` to receive the optional activation
token supplied by a Linux tray host. Each token is delivered once with the next
primary activation. Hosts without token support and other platforms pass `None`.

Forward the token to a window API that accepts an external Wayland activation
token. If opening the window is deferred, move the token into that callback.
Do not store it in the process environment or reuse it for later clicks.
GPUI's ordinary `activate_window()` cannot consume it, and the compositor
controls whether focus is granted.

The callback runs on GPUI's foreground executor, then refreshes the menu.
It replaces `.on_activate(...)`; the last activation handler configured wins.

## Checked and disabled menu state

The opt-in `menu-state` feature forwards `MenuItem::is_checked` and
`MenuItem::is_disabled`. Without it, entries are enabled and unchecked.
GPUI Kit and Community Edition support it directly.

Published crates use crates.io `gpui` 0.2.2 for the default backend. To enable
`menu-state` with upstream GPUI, add the following to your application's root
`Cargo.toml`:

```toml
[dependencies]
gpui-tray = { version = "0.2", features = ["menu-state"] }

[patch.crates-io]
gpui = { git = "https://github.com/zed-industries/zed.git", rev = "8166e3d7b8b42d8aaf4d4dee7fcd25ab4ec65105" }
```

This is the same GPUI revision used in this repository. Cargo replaces Git
dependencies with registry dependencies when packaging; the patch makes your
application use the pinned source too.

## Run the example

From a graphical desktop session:

```sh
# Upstream GPUI
cargo run --example tray --features menu-state

# GPUI Kit
cargo run --example tray --no-default-features --features gpui-kit,menu-state

# GPUI Community Edition
cargo run --example tray --no-default-features --features gpui-ce,menu-state
```

The example has no application window. Its menu lets you change checked and
disabled state, increment a counter, and quit. The bitmap is generated in code.
Test each backend separately: `--all-features` enables mutually exclusive
GPUI dependencies and is unsupported.

## Migrating from 0.1

- Add `.build()?` after `Icon::from_rgba(...)` and `Icon::from_gpui(...)`.
- Replace `tray.set_icon(Some(icon), cx)?` with `tray.set_icon(icon, cx)?`.
- Replace `tray.set_icon(None, cx)?` with `tray.clear_icon(cx)?`.
- Guard icon-theme methods with `#[cfg(target_os = "linux")]` in portable code.
- Linux now uses zbus 6. This changes the D-Bus dependency without changing the
  public tray API.

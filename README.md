# gpui-tray

Native system tray integration built specifically for GPUI. Tray menu entries
are ordinary `gpui::MenuItem`s and clicks dispatch ordinary GPUI actions.

The crate has three native backends:

- macOS: AppKit through `objc2`
- Windows: `Shell_NotifyIconW` plus a hidden top-level window and Win32 menus
- Linux: StatusNotifierItem and DBusMenu through `zbus` (no GTK)

It does not start a second application event loop. Native callbacks send small,
platform-neutral events to a task on GPUI's foreground executor.

## Choose a GPUI dependency

Select exactly one backend feature. Upstream GPUI is the default:

```toml
gpui-tray = "0.1"
```

For GPUI Kit, disable the default and select `gpui-kit`:

```toml
gpui-kit = "0.6"
gpui-tray = { version = "0.1", default-features = false, features = ["gpui-kit", "menu-state"] }
```

The `gpui-kit` feature uses `gpui-pre` 0.3.4 or a compatible 0.3.x release,
so tray methods accept GPUI Kit's `App`, `Action`, `Image`, and `MenuItem` types
directly.

For [GPUI Community Edition](https://github.com/gpui-ce/gpui-ce), disable the
default and select `gpui-ce`:

```toml
gpui = { package = "gpui-ce", version = "0.2.2", default-features = false }
gpui-tray = { version = "0.1", default-features = false, features = ["gpui-ce", "menu-state"] }
```

The `gpui-ce` feature uses the crates.io `gpui-ce` 0.2.2 release (or a compatible
0.2.x release) and accepts its GPUI types directly. The native tray implementation
is shared by all three features.

The default `gpui` feature uses a pinned Zed Git dependency in this repository.
Published crates use crates.io `gpui` 0.2.2 because Cargo removes Git sources
when packaging. To use the pinned upstream revision from a published crate,
including its `menu-state` support, add this patch to your application's root
`Cargo.toml`:

```toml
[patch.crates-io]
gpui = { git = "https://github.com/zed-industries/zed.git", rev = "8166e3d7b8b42d8aaf4d4dee7fcd25ab4ec65105" }
```

Your application must resolve to the same GPUI source and version as the tray;
a different source or Git revision has distinct Rust types.
Cargo features are additive: enabling multiple backend features anywhere in the
dependency graph is an error, as is disabling all three. `--all-features` therefore
isn't a supported build; test each backend separately.

## Run the example

From a graphical desktop session, run:

```sh
cargo run --example tray --features menu-state
```

To run the same example using GPUI Kit:

```sh
cargo run --example tray --no-default-features --features gpui-kit,menu-state
```

To run the same example using GPUI Community Edition:

```sh
cargo run --example tray --no-default-features --features gpui-ce,menu-state
```

The example has no application window. Use its tray menu to change checked and
disabled state, increment the counter, and quit. It uses a generated bitmap,
so no external assets are required.

```rust,ignore
use gpui::{App, Menu, MenuItem, actions};
use gpui_tray::{Icon, Tray};

actions!(tray_example, [Open, ToggleMode, Quit]);

fn build_tray(cx: &mut App, icon: Icon) -> gpui_tray::Result<Tray> {
    Tray::builder()
        .icon(icon)
        .title("My App")
        .tooltip("My App")
        .menu(|_cx| {
            vec![
                MenuItem::action("Open", Open),
                MenuItem::submenu(
                    Menu::new("View")
                        .items([MenuItem::action("Toggle mode", ToggleMode)]),
                ),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ]
        })
        .build(cx)
}
```

Call `Tray::refresh_menu` after state changes outside tray actions. The menu is
automatically rebuilt after a tray action is dispatched. With `menu-state`
enabled, checked/disabled state produced by synchronous action handlers updates
immediately.

macOS displays the title beside the icon. Linux hosts choose how to present it;
Windows ignores it. Use `.tooltip("My App")` for hover text.

## Icons

Create an icon from RGBA8 pixels:

```rust,ignore
let icon = Icon::from_rgba(rgba, 32, 32).build()?;
let tray = Tray::builder().icon(icon).build(cx)?;
```

For a GPUI image, including SVG:

```rust,ignore
let icon = Icon::from_gpui(&image, cx).build()?;
```

### macOS

Use `.scale_factor(2.0)` for artwork with two pixels per point and
`.template(true)` for black artwork on a transparent background that adapts to
the menu bar appearance:

```rust,ignore
let icon = Icon::from_rgba(rgba, 36, 36)
    .scale_factor(2.0)
    .template(true)
    .build()?;
```

This displays at 18×18 points.

Display an SF Symbol by its system name, such as `externaldrive`:

```rust,ignore
use gpui_tray::{Icon, SymbolScale, SymbolWeight};

let icon = Icon::system_symbol("externaldrive")
    .accessibility_description("External drive")
    .point_size(16.0)
    .weight(SymbolWeight::Regular)
    .scale(SymbolScale::Medium)
    .build()?;

let tray = Tray::builder().icon(icon).build(cx)?;
```

You can also pass an `NSImage` directly to `.icon(...)` or `set_icon(...)`:

```rust,ignore
let tray = Tray::builder().icon(image).build(cx)?;
```

To customize an existing icon, use `to_ns_image()`. It returns the same image
for a native icon and a new image for a bitmap. After editing it, call
`tray.set_icon(image, cx)?` to update the tray.

### Linux

Use the icon-theme settings to let the desktop shell recolor a symbolic icon
for its panel. Keep `.icon(...)` as the macOS, Windows, and Linux fallback:

```rust,ignore
let builder = Tray::builder().icon(icon);

#[cfg(target_os = "linux")]
let builder = builder
    .icon_name("my-app-symbolic")
    .icon_theme_path("/path/to/icons");

let tray = builder.build(cx)?;
```

## Activation and lifecycle

Use `.on_activate(action)` to dispatch an action for the platform's primary
tray activation. On macOS, an attached menu takes precedence and opens on the
primary click; activation is dispatched when the tray has no menu.

`Tray::close` removes the native item deterministically and is safe to call more
than once. Dropping the last clone performs the same cleanup on a best-effort
basis.

## Checked and disabled menu state

Enable the `menu-state` feature to forward `MenuItem::is_checked` and
`MenuItem::is_disabled`. The pinned upstream revision, GPUI Kit, and GPUI
Community Edition support these methods. When using the default backend from
crates.io, apply the upstream patch above before enabling this feature:

```toml
gpui-tray = { version = "0.1", features = ["menu-state"] }
```

The feature remains opt-in. Without it, tray menu entries are treated as enabled
and unchecked.

## Wayland activation tokens

Use `.on_activate_with(|token, cx| { ... })` when the application needs the
optional activation token supplied by its tray host. This replaces the action
handler configured by `.on_activate(...)`; the last call wins. The callback
runs on GPUI's foreground executor and the menu refreshes afterward.

On Linux, `ProvideXdgActivationToken` supplies a token for the next primary
activation. Each token is delivered once. Hosts without token support and
other platforms deliver `None`. Forward a supplied token to the target
window's Wayland activation API when presenting it. If window access must be
deferred, move the token into that deferred callback. Do not put it in the
process environment or reuse it for a later click. The compositor still
controls whether focus is granted.

This callback does not require a particular GPUI fork to implement token
activation. Applications need a window API that accepts an external token;
GPUI's ordinary `activate_window()` alone cannot use the tray host's token.

#[cfg(target_os = "linux")]
mod linux;

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    thread::{self, ThreadId},
};

use crate::{
    Error, Icon, Result,
    backend::{BackendEvent, PlatformTray},
    menu::{ActionTable, MenuSnapshot, compile_menu},
};

type ActivateCallback = dyn Fn(Option<String>, &mut gpui::App);

type MenuBuilder = dyn Fn(&mut gpui::App) -> Vec<gpui::MenuItem>;

/// A cloneable handle to a native tray icon.
///
/// `Tray` is intentionally not `Send`: native tray objects and GPUI application
/// state are bound to the thread on which the tray was built.
#[derive(Clone)]
pub struct Tray {
    inner: Rc<TrayInner>,
}

/// Configures and creates a [`Tray`].
#[must_use]
pub struct TrayBuilder {
    icon: Option<Icon>,
    #[cfg(target_os = "linux")]
    icon_name: Option<String>,
    #[cfg(target_os = "linux")]
    icon_theme_path: Option<String>,
    title: Option<String>,
    tooltip: Option<String>,
    visible: bool,
    activate_handler: Option<Box<ActivateCallback>>,
    menu_builder: Option<Box<MenuBuilder>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraySnapshot {
    pub(crate) icon: Option<Icon>,
    /// Incremented by every explicit icon replacement or clear, so backends
    /// reapply the image even when its source compares equal. Native image
    /// equality checks identity and cannot detect edits to the same NSImage.
    pub(crate) icon_revision: u64,
    #[cfg(target_os = "linux")]
    pub(crate) icon_name: Option<String>,
    #[cfg(target_os = "linux")]
    pub(crate) icon_theme_path: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) tooltip: Option<String>,
    pub(crate) visible: bool,
    pub(crate) activatable: bool,
    pub(crate) menu: Option<MenuSnapshot>,
}

struct TrayInner {
    thread_id: ThreadId,
    closed: Cell<bool>,
    generation: Cell<u64>,
    snapshot: RefCell<TraySnapshot>,
    actions: RefCell<ActionTable>,
    activate_handler: Option<Box<ActivateCallback>>,
    menu_builder: Option<Box<MenuBuilder>>,
    backend: RefCell<Option<PlatformTray>>,
}

impl Tray {
    /// Starts building a tray icon.
    pub fn builder() -> TrayBuilder {
        TrayBuilder::default()
    }

    /// Replaces the tray icon, even if the same source was previously installed.
    ///
    /// On macOS, call this again after editing an icon's native `NSImage`.
    /// Use [`Self::clear_icon`] to remove the image.
    pub fn set_icon(&self, icon: impl Into<Icon>, _cx: &mut gpui::App) -> Result<()> {
        let icon = icon.into();
        self.inner
            .update_snapshot(|snapshot| snapshot.replace_icon(Some(icon)))
    }

    /// Removes the tray's pixel or native image. The tray itself remains alive.
    ///
    /// A macOS title or a separately configured Linux icon name may still be
    /// displayed. Use [`Self::set_visible`] to hide the entire tray item.
    pub fn clear_icon(&self, _cx: &mut gpui::App) -> Result<()> {
        self.inner
            .update_snapshot(|snapshot| snapshot.replace_icon(None))
    }

    /// Replaces or removes the tray title.
    ///
    /// macOS displays it beside the icon. Linux hosts choose how to present it.
    /// Windows ignores this setting.
    pub fn set_title(&self, title: Option<impl Into<String>>, _cx: &mut gpui::App) -> Result<()> {
        self.inner
            .update_snapshot(|snapshot| snapshot.title = title.map(Into::into))
    }

    /// Replaces or removes the native tooltip.
    pub fn set_tooltip(
        &self,
        tooltip: Option<impl Into<String>>,
        _cx: &mut gpui::App,
    ) -> Result<()> {
        self.inner
            .update_snapshot(|snapshot| snapshot.tooltip = tooltip.map(Into::into))
    }

    /// Shows or hides the tray icon.
    pub fn set_visible(&self, visible: bool, _cx: &mut gpui::App) -> Result<()> {
        self.inner
            .update_snapshot(|snapshot| snapshot.visible = visible)
    }

    /// Runs the menu builder against current GPUI state and applies the result.
    pub fn refresh_menu(&self, cx: &mut gpui::App) -> Result<()> {
        self.inner.refresh_menu(cx)
    }

    /// Deterministically removes the tray icon. This method is idempotent.
    pub fn close(&self, _cx: &mut gpui::App) -> Result<()> {
        self.inner.close()
    }
}

impl Default for TrayBuilder {
    fn default() -> Self {
        Self {
            icon: None,
            #[cfg(target_os = "linux")]
            icon_name: None,
            #[cfg(target_os = "linux")]
            icon_theme_path: None,
            title: None,
            tooltip: None,
            visible: true,
            activate_handler: None,
            menu_builder: None,
        }
    }
}

impl TrayBuilder {
    /// Sets the initial icon.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets the initial tray title.
    ///
    /// macOS displays it beside the icon. Linux hosts choose how to present it.
    /// Windows ignores this setting.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the initial native tooltip.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Sets initial visibility. Tray icons are visible by default.
    pub const fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Dispatches a GPUI action when the tray icon is primarily activated.
    /// On macOS, an attached menu takes precedence and opens on the primary
    /// click; the activation action is used when the tray has no menu.
    /// This replaces any callback configured with [`Self::on_activate_with`].
    pub fn on_activate(self, action: impl gpui::Action) -> Self {
        self.on_activate_with(move |_, cx| cx.dispatch_action(&action))
    }

    /// Handles primary activation, including an optional Wayland activation token.
    ///
    /// Linux tray hosts may supply a token using `ProvideXdgActivationToken`.
    /// Forward it to the target window's activation API; it belongs to this
    /// click and must not be cached or reused. Other platforms and hosts that
    /// do not support tokens pass `None`. This replaces [`Self::on_activate`].
    pub fn on_activate_with(
        mut self,
        callback: impl Fn(Option<String>, &mut gpui::App) + 'static,
    ) -> Self {
        self.activate_handler = Some(Box::new(callback));
        self
    }

    /// Sets the GPUI-aware menu builder.
    pub fn menu(mut self, build: impl Fn(&mut gpui::App) -> Vec<gpui::MenuItem> + 'static) -> Self {
        self.menu_builder = Some(Box::new(build));
        self
    }

    /// Creates the native tray icon and its GPUI event receiver.
    pub fn build(self, cx: &mut gpui::App) -> Result<Tray> {
        let generation = u64::from(self.menu_builder.is_some());
        let (menu, actions) = if let Some(build) = self.menu_builder.as_ref() {
            let (menu, actions) = compile_menu(generation, build(cx))?;
            (Some(menu), actions)
        } else {
            (None, ActionTable::empty(generation))
        };

        let snapshot = TraySnapshot {
            icon: self.icon,
            icon_revision: 0,
            #[cfg(target_os = "linux")]
            icon_name: self.icon_name,
            #[cfg(target_os = "linux")]
            icon_theme_path: self.icon_theme_path,
            title: self.title,
            tooltip: self.tooltip,
            visible: self.visible,
            activatable: self.activate_handler.is_some(),
            menu,
        };
        let (events_tx, events_rx) = async_channel::unbounded();
        let backend = PlatformTray::new(&snapshot, events_tx)?;
        let inner = Rc::new(TrayInner {
            thread_id: thread::current().id(),
            closed: Cell::new(false),
            generation: Cell::new(generation),
            snapshot: RefCell::new(snapshot),
            actions: RefCell::new(actions),
            activate_handler: self.activate_handler,
            menu_builder: self.menu_builder,
            backend: RefCell::new(Some(backend)),
        });

        spawn_event_receiver(cx, Rc::downgrade(&inner), events_rx);
        Ok(Tray { inner })
    }
}

impl TraySnapshot {
    fn replace_icon(&mut self, icon: Option<Icon>) {
        self.icon = icon;
        self.icon_revision = self.icon_revision.wrapping_add(1);
    }
}

fn spawn_event_receiver(
    cx: &gpui::App,
    inner: Weak<TrayInner>,
    events: async_channel::Receiver<BackendEvent>,
) {
    cx.spawn(async move |cx| {
        while let Ok(event) = events.recv().await {
            let Some(inner) = inner.upgrade() else {
                break;
            };
            cx.update(|cx| inner.handle_event(event, cx));
        }
    })
    .detach();
}

impl TrayInner {
    fn ensure_open_on_creator_thread(&self) -> Result<()> {
        if thread::current().id() != self.thread_id {
            return Err(Error::WrongThread);
        }
        if self.closed.get() {
            return Err(Error::Closed);
        }
        Ok(())
    }

    fn update_snapshot(&self, update: impl FnOnce(&mut TraySnapshot)) -> Result<()> {
        self.ensure_open_on_creator_thread()?;
        let old = self.snapshot.borrow().clone();
        let mut new = old.clone();
        update(&mut new);
        if old == new {
            return Ok(());
        }

        self.backend
            .borrow_mut()
            .as_mut()
            .ok_or(Error::Closed)?
            .apply(&old, &new)?;
        *self.snapshot.borrow_mut() = new;
        Ok(())
    }

    fn refresh_menu(&self, cx: &mut gpui::App) -> Result<()> {
        self.ensure_open_on_creator_thread()?;
        let Some(build) = self.menu_builder.as_ref() else {
            return Ok(());
        };

        let generation = self.generation.get().wrapping_add(1).max(1);
        let (menu, actions) = compile_menu(generation, build(cx))?;
        let old = self.snapshot.borrow().clone();
        let mut new = old.clone();
        new.menu = Some(menu);
        self.backend
            .borrow_mut()
            .as_mut()
            .ok_or(Error::Closed)?
            .apply(&old, &new)?;

        self.generation.set(generation);
        *self.actions.borrow_mut() = actions;
        *self.snapshot.borrow_mut() = new;
        Ok(())
    }

    fn handle_event(&self, event: BackendEvent, cx: &mut gpui::App) {
        if self.closed.get() {
            return;
        }
        match event {
            BackendEvent::Activated { token } => {
                if let Some(handler) = self.activate_handler.as_ref() {
                    handler(token, cx);
                    if !self.closed.get()
                        && let Err(error) = self.refresh_menu(cx)
                    {
                        log::error!("failed to refresh tray menu after action: {error}");
                    }
                }
            }
            BackendEvent::MenuItemClicked { generation, id } => {
                let action = {
                    let actions = self.actions.borrow();
                    (actions.generation == generation)
                        .then(|| actions.action(id))
                        .flatten()
                };
                if let Some(action) = action {
                    cx.dispatch_action(action.as_ref());
                    if !self.closed.get()
                        && let Err(error) = self.refresh_menu(cx)
                    {
                        log::error!("failed to refresh tray menu after action: {error}");
                    }
                }
            }
        }
    }

    fn close(&self) -> Result<()> {
        if self.closed.get() {
            return Ok(());
        }
        if thread::current().id() != self.thread_id {
            return Err(Error::WrongThread);
        }

        self.closed.set(true);
        self.backend
            .borrow_mut()
            .take()
            .map_or(Ok(()), |mut backend| backend.close())
    }
}

impl Drop for TrayInner {
    fn drop(&mut self) {
        if let Some(backend) = self.backend.get_mut().as_mut()
            && let Err(error) = backend.close()
        {
            log::error!("failed to close tray while dropping it: {error}");
        }
        self.closed.set(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_not_impl_any!(Tray: Send, Sync);

    #[test]
    fn builder_defaults_to_visible() {
        let builder = TrayBuilder::default();
        assert!(builder.visible);
        assert!(builder.icon.is_none());
        #[cfg(target_os = "linux")]
        {
            assert!(builder.icon_name.is_none());
            assert!(builder.icon_theme_path.is_none());
        }
        assert!(builder.activate_handler.is_none());
        assert!(builder.menu_builder.is_none());
    }

    #[test]
    fn builder_records_activation_action() {
        let builder = TrayBuilder::default().on_activate(gpui::NoAction);
        assert!(builder.activate_handler.is_some());
    }

    #[test]
    fn explicit_icon_updates_invalidate_equal_sources_and_clear() {
        let icon = Icon::from_rgba(vec![0; 4], 1, 1).build().unwrap();
        let mut snapshot = TraySnapshot {
            icon: Some(icon.clone()),
            icon_revision: 0,
            #[cfg(target_os = "linux")]
            icon_name: None,
            #[cfg(target_os = "linux")]
            icon_theme_path: None,
            title: None,
            tooltip: None,
            visible: true,
            activatable: false,
            menu: None,
        };
        let previous = snapshot.clone();
        snapshot.replace_icon(Some(icon));
        assert_eq!(previous.icon, snapshot.icon);
        assert_ne!(previous, snapshot);
        let previous_revision = snapshot.icon_revision;
        snapshot.replace_icon(None);
        assert!(snapshot.icon.is_none());
        assert_ne!(previous_revision, snapshot.icon_revision);
    }
}

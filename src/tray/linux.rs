use super::{Tray, TrayBuilder};
use crate::Result;

impl Tray {
    /// Sets a desktop icon-theme name. Shells can recolor names ending in
    /// `-symbolic` to match the system panel.
    pub fn set_icon_name(
        &self,
        icon_name: Option<impl Into<String>>,
        _cx: &mut gpui::App,
    ) -> Result<()> {
        self.inner
            .update_snapshot(|snapshot| snapshot.icon_name = icon_name.map(Into::into))
    }

    /// Sets the directory used to resolve a named icon on Linux.
    pub fn set_icon_theme_path(
        &self,
        icon_theme_path: Option<impl Into<String>>,
        _cx: &mut gpui::App,
    ) -> Result<()> {
        self.inner.update_snapshot(|snapshot| {
            snapshot.icon_theme_path = icon_theme_path.map(Into::into);
        })
    }
}

impl TrayBuilder {
    /// Sets an icon-theme name, such as `example-app-symbolic`.
    /// Supply a pixel icon as a fallback for hosts without the named icon.
    pub fn icon_name(mut self, icon_name: impl Into<String>) -> Self {
        self.icon_name = Some(icon_name.into());
        self
    }

    /// Sets the Linux icon-theme lookup directory for [`Self::icon_name`].
    pub fn icon_theme_path(mut self, icon_theme_path: impl Into<String>) -> Self {
        self.icon_theme_path = Some(icon_theme_path.into());
        self
    }
}

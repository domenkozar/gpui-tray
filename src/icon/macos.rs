use std::ptr;

use image::ImageEncoder as _;
use objc2::{AnyThread, rc::Retained};
use objc2_app_kit::{
    NSFontWeight, NSFontWeightBlack, NSFontWeightBold, NSFontWeightHeavy, NSFontWeightLight,
    NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSFontWeightThin,
    NSFontWeightUltraLight, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
};
use objc2_foundation::{NSData, NSSize, NSString};

use super::{BitmapIconBuilder, Icon, IconSource};
use crate::{Error, Result};

#[derive(Clone, Debug)]
pub(super) struct AppKitImage(Retained<NSImage>);

impl PartialEq for AppKitImage {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq::<NSImage>(self.0.as_ref(), other.0.as_ref())
    }
}

impl Eq for AppKitImage {}

/// The typographic weight of an SF Symbol.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolWeight {
    UltraLight,
    Thin,
    Light,
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
    Black,
}

impl SymbolWeight {
    fn native(self) -> NSFontWeight {
        // SAFETY: These foreign statics are immutable AppKit font weights.
        // Reading a foreign static requires unsafe, even when it is immutable.
        unsafe {
            match self {
                Self::UltraLight => NSFontWeightUltraLight,
                Self::Thin => NSFontWeightThin,
                Self::Light => NSFontWeightLight,
                Self::Regular => NSFontWeightRegular,
                Self::Medium => NSFontWeightMedium,
                Self::Semibold => NSFontWeightSemibold,
                Self::Bold => NSFontWeightBold,
                Self::Heavy => NSFontWeightHeavy,
                Self::Black => NSFontWeightBlack,
            }
        }
    }
}

/// An SF Symbol's size relative to its point size.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolScale {
    Small,
    Medium,
    Large,
}

impl SymbolScale {
    fn native(self) -> NSImageSymbolScale {
        match self {
            Self::Small => NSImageSymbolScale::Small,
            Self::Medium => NSImageSymbolScale::Medium,
            Self::Large => NSImageSymbolScale::Large,
        }
    }
}

/// Configures an SF Symbol before calling [`Self::build`].
#[derive(Clone, Debug)]
#[must_use]
pub struct SymbolIconBuilder {
    name: String,
    accessibility_description: Option<String>,
    point_size: Option<f64>,
    weight: Option<SymbolWeight>,
    scale: Option<SymbolScale>,
}

impl Icon {
    /// Builds a native SF Symbol icon.
    ///
    /// Requires macOS 11 or later. Individual symbols may require newer OS
    /// versions; [`SymbolIconBuilder::build`] reports unavailable names.
    ///
    /// ```no_run
    /// # fn main() -> gpui_tray::Result<()> {
    /// use gpui_tray::{Icon, SymbolScale, SymbolWeight};
    /// let icon = Icon::system_symbol("externaldrive")
    ///     .accessibility_description("External drive")
    ///     .point_size(16.0)
    ///     .weight(SymbolWeight::Regular)
    ///     .scale(SymbolScale::Medium)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn system_symbol(name: impl Into<String>) -> SymbolIconBuilder {
        SymbolIconBuilder {
            name: name.into(),
            accessibility_description: None,
            point_size: None,
            weight: None,
            scale: None,
        }
    }

    /// Uses an NSImage as a tray icon, preserving its size and appearance.
    ///
    /// After editing the image, call [`crate::Tray::set_icon`] to update the tray.
    pub fn from_ns_image(image: Retained<NSImage>) -> Self {
        Self {
            source: IconSource::AppKit(AppKitImage(image)),
        }
    }

    /// Returns an NSImage for customization with AppKit.
    ///
    /// Native icons share their image; bitmap icons produce a new image.
    /// After editing it, pass the image to [`crate::Tray::set_icon`].
    pub fn to_ns_image(&self) -> Result<Retained<NSImage>> {
        match &self.source {
            IconSource::AppKit(image) => Ok(image.0.clone()),
            IconSource::Bitmap(bitmap) => {
                let mut png = Vec::new();
                image::codecs::png::PngEncoder::new(&mut png)
                    .write_image(
                        &bitmap.rgba,
                        bitmap.width,
                        bitmap.height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(Error::native)?;
                // SAFETY: NSData copies the bytes before `png` is dropped.
                let data = unsafe { NSData::dataWithBytes_length(png.as_ptr().cast(), png.len()) };
                let image = NSImage::initWithData(NSImage::alloc(), &data)
                    .ok_or_else(|| Error::native_message("AppKit rejected tray icon data"))?;
                image.setSize(NSSize::new(
                    f64::from(bitmap.width) / bitmap.scale,
                    f64::from(bitmap.height) / bitmap.scale,
                ));
                image.setTemplate(bitmap.template);
                Ok(image)
            }
        }
    }
}

impl From<Retained<NSImage>> for Icon {
    fn from(image: Retained<NSImage>) -> Self {
        Self::from_ns_image(image)
    }
}

impl BitmapIconBuilder {
    /// Declares the supplied bitmap's pixel density. Defaults to `1.0`.
    ///
    /// A 36×36 bitmap at `2.0` displays at 18×18 points.
    pub fn scale_factor(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    /// Enables template rendering. Defaults to `false`.
    ///
    /// Supply black artwork on a transparent background. AppKit uses its alpha
    /// mask to match the menu bar appearance and selection state.
    pub fn template(mut self, template: bool) -> Self {
        self.template = template;
        self
    }
}

impl SymbolIconBuilder {
    /// Sets a localized accessibility description, such as "External drive".
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.accessibility_description = Some(description.into());
        self
    }

    /// Sets a finite, positive typographic point size, not a bounding-box size.
    pub fn point_size(mut self, size: f64) -> Self {
        self.point_size = Some(size);
        self
    }

    /// Sets the symbol's stroke weight.
    pub fn weight(mut self, weight: SymbolWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Sets the symbol's typographic scale, independently of display density.
    pub fn scale(mut self, scale: SymbolScale) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Creates the SF Symbol icon.
    ///
    /// An unavailable name returns [`Error::UnavailableSystemSymbol`]. No
    /// fallback is selected automatically. Invalid point sizes are rejected.
    pub fn build(self) -> Result<Icon> {
        if let Some(size) = self.point_size
            && (!size.is_finite() || size <= 0.0)
        {
            return Err(Error::InvalidIcon(format!(
                "symbol point size must be finite and positive, got {size}"
            )));
        }
        if !objc2::available!(macos = 11.0) {
            return Err(Error::native_message(
                "system symbols require macOS 11 or later",
            ));
        }
        let description = self
            .accessibility_description
            .as_deref()
            .map(NSString::from_str);
        let mut image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(&self.name),
            description.as_deref(),
        )
        .ok_or_else(|| Error::UnavailableSystemSymbol(self.name.clone()))?;

        // Zero leaves the point size unspecified when only a weight is set.
        // Do not create any configuration for a completely unconfigured symbol.
        let mut configuration = if self.point_size.is_some() || self.weight.is_some() {
            Some(
                NSImageSymbolConfiguration::configurationWithPointSize_weight(
                    self.point_size.unwrap_or(0.0),
                    self.weight.unwrap_or_default().native(),
                ),
            )
        } else {
            None
        };
        if let Some(scale) = self.scale {
            let scale = NSImageSymbolConfiguration::configurationWithScale(scale.native());
            configuration = Some(match configuration {
                Some(configuration) => configuration.configurationByApplyingConfiguration(&scale),
                None => scale,
            });
        }
        if let Some(configuration) = configuration {
            image = image
                .imageWithSymbolConfiguration(&configuration)
                .ok_or_else(|| Error::native_message("AppKit rejected symbol configuration"))?;
        }
        Ok(Icon::from_ns_image(image))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_conversion_preserves_pixels_and_sets_logical_size() {
        let icon = Icon::from_rgba(vec![0; 36 * 36 * 4], 36, 36)
            .scale_factor(2.0)
            .template(true)
            .build()
            .unwrap();
        let image = icon.to_ns_image().unwrap();
        assert_eq!(image.size(), NSSize::new(18.0, 18.0));
        assert!(image.isTemplate());
        // Native image scaling must not resample the backing pixels.
        let representations = image.representations();
        let representation = representations.objectAtIndex(0);
        assert_eq!(
            (representation.pixelsWide(), representation.pixelsHigh()),
            (36, 36)
        );
        let another = icon.to_ns_image().unwrap();
        assert!(!ptr::eq::<NSImage>(image.as_ref(), another.as_ref()));
    }

    #[test]
    fn native_round_trip_retains_identity_and_properties() {
        static_assertions::assert_not_impl_any!(Icon: Send, Sync);
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(23.0, 17.0));
        image.setTemplate(true);
        let icon = Icon::from_ns_image(image.clone());
        let round_trip = icon.to_ns_image().unwrap();
        assert!(ptr::eq::<NSImage>(image.as_ref(), round_trip.as_ref()));
        assert_eq!(round_trip.size(), NSSize::new(23.0, 17.0));
        assert!(round_trip.isTemplate());
        assert_eq!(icon, icon.clone());
        assert_eq!(icon, Icon::from_ns_image(image));
        let other = NSImage::initWithSize(NSImage::alloc(), NSSize::new(23.0, 17.0));
        assert_ne!(icon, Icon::from_ns_image(other));
    }

    #[test]
    fn resolves_symbols_and_reports_unavailable_names() {
        if !objc2::available!(macos = 11.0) {
            return;
        }
        let icon = Icon::system_symbol("externaldrive")
            .accessibility_description("External drive")
            .point_size(16.0)
            .weight(SymbolWeight::Regular)
            .scale(SymbolScale::Medium)
            .build()
            .unwrap();
        let image = icon.to_ns_image().unwrap();
        assert!(
            image
                .imageWithSymbolConfiguration(&NSImageSymbolConfiguration::configurationWithScale(
                    NSImageSymbolScale::Large
                ))
                .is_some()
        );
        assert_eq!(
            image.accessibilityDescription().unwrap().to_string(),
            "External drive"
        );
        assert!(matches!(
            Icon::system_symbol("gpui-tray.nonexistent-symbol").build(),
            Err(Error::UnavailableSystemSymbol(_))
        ));
    }

    #[test]
    fn validates_symbol_point_size() {
        for size in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                Icon::system_symbol("externaldrive")
                    .point_size(size)
                    .build(),
                Err(Error::InvalidIcon(_))
            ));
        }
    }

    #[test]
    fn symbol_configuration_keeps_unspecified_defaults_and_changes_size() {
        if !objc2::available!(macos = 11.0) {
            return;
        }
        let plain = Icon::system_symbol("externaldrive").build().unwrap();
        let weighted = Icon::system_symbol("externaldrive")
            .weight(SymbolWeight::Bold)
            .build()
            .unwrap();
        for icon in [plain, weighted] {
            let size = icon.to_ns_image().unwrap().size();
            assert!(size.width > 0.0 && size.height > 0.0);
        }
        let small = Icon::system_symbol("externaldrive")
            .point_size(12.0)
            .build()
            .unwrap()
            .to_ns_image()
            .unwrap();
        let large = Icon::system_symbol("externaldrive")
            .point_size(24.0)
            .build()
            .unwrap()
            .to_ns_image()
            .unwrap();
        assert!(large.size().width > small.size().width);
        assert!(large.size().height > small.size().height);
    }
}

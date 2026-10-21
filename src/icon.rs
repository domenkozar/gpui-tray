use std::{fmt, sync::Arc};

use crate::{Error, Result};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{SymbolIconBuilder, SymbolScale, SymbolWeight};

/// A tray icon created from RGBA pixels, a GPUI image, or a native image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    source: IconSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum IconSource {
    Bitmap(BitmapIcon),
    #[cfg(target_os = "macos")]
    AppKit(macos::AppKitImage),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "macos"), derive(Eq))]
struct BitmapIcon {
    rgba: Arc<[u8]>,
    width: u32,
    height: u32,
    #[cfg(target_os = "macos")]
    scale: f64,
    #[cfg(target_os = "macos")]
    template: bool,
}

// Construction only permits finite, positive scales, so equality is reflexive.
#[cfg(target_os = "macos")]
impl Eq for BitmapIcon {}

/// Configures a bitmap icon before calling [`Self::build`].
#[derive(Clone, Debug)]
#[must_use]
pub struct BitmapIconBuilder {
    input: BitmapInput,
    #[cfg(target_os = "macos")]
    scale: f64,
    #[cfg(target_os = "macos")]
    template: bool,
}

// Keep GPUI's renderer type inferred: crates.io GPUI does not re-export it.
type RenderGpuiImage = dyn Fn(&gpui::Image) -> Result<Arc<gpui::RenderImage>> + Send + Sync;

#[derive(Clone)]
enum BitmapInput {
    Rgba {
        rgba: Arc<[u8]>,
        width: u32,
        height: u32,
    },
    Gpui {
        image: gpui::Image,
        render: Arc<RenderGpuiImage>,
    },
}

impl fmt::Debug for BitmapInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rgba {
                rgba,
                width,
                height,
            } => f
                .debug_struct("Rgba")
                .field("byte_length", &rgba.len())
                .field("width", width)
                .field("height", height)
                .finish(),
            Self::Gpui { image, .. } => f
                .debug_struct("Gpui")
                .field("image", image)
                .finish_non_exhaustive(),
        }
    }
}

impl Icon {
    /// Builds an icon from tightly packed, row-major, non-premultiplied RGBA8 pixels.
    ///
    /// ```
    /// # fn main() -> gpui_tray::Result<()> {
    /// let icon = gpui_tray::Icon::from_rgba(vec![0; 36 * 36 * 4], 36, 36)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_rgba(rgba: impl Into<Arc<[u8]>>, width: u32, height: u32) -> BitmapIconBuilder {
        BitmapIconBuilder::new(BitmapInput::Rgba {
            rgba: rgba.into(),
            width,
            height,
        })
    }

    /// Builds an icon from the first frame of a GPUI image, including SVG.
    ///
    /// Decoding and rendering happen when [`BitmapIconBuilder::build`] is called.
    ///
    /// ```no_run
    /// # #[cfg(all(feature = "gpui-kit", not(feature = "gpui")))]
    /// # use gpui_kit_backend as gpui;
    /// # #[cfg(all(feature = "gpui-ce", not(any(feature = "gpui", feature = "gpui-kit"))))]
    /// # use gpui_ce_backend as gpui;
    /// # fn example(image: &gpui::Image, cx: &gpui::App) -> gpui_tray::Result<()> {
    /// let icon = gpui_tray::Icon::from_gpui(image, cx).build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_gpui(image: &gpui::Image, cx: &gpui::App) -> BitmapIconBuilder {
        let renderer = cx.svg_renderer();
        BitmapIconBuilder::from_gpui(image.clone(), move |image| {
            image
                .to_image_data(renderer.clone())
                .map_err(|error| Error::InvalidIcon(error.to_string()))
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn rgba_bytes(&self) -> &[u8] {
        let IconSource::Bitmap(bitmap) = &self.source;
        &bitmap.rgba
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn width(&self) -> u32 {
        let IconSource::Bitmap(bitmap) = &self.source;
        bitmap.width
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn height(&self) -> u32 {
        let IconSource::Bitmap(bitmap) = &self.source;
        bitmap.height
    }
}

impl BitmapIconBuilder {
    fn new(input: BitmapInput) -> Self {
        Self {
            input,
            #[cfg(target_os = "macos")]
            scale: 1.0,
            #[cfg(target_os = "macos")]
            template: false,
        }
    }

    fn from_gpui(
        image: gpui::Image,
        render: impl Fn(&gpui::Image) -> Result<Arc<gpui::RenderImage>> + Send + Sync + 'static,
    ) -> Self {
        Self::new(BitmapInput::Gpui {
            image,
            render: Arc::new(render),
        })
    }

    /// Decodes the source image, validates the bitmap, and creates an icon.
    pub fn build(self) -> Result<Icon> {
        let (rgba, width, height) = match self.input {
            BitmapInput::Rgba {
                rgba,
                width,
                height,
            } => (rgba, width, height),
            BitmapInput::Gpui { image, render } => {
                let rendered = render(&image)?;
                let mut rgba = rendered
                    .as_bytes(0)
                    .ok_or_else(|| Error::InvalidIcon("GPUI image has no frames".into()))?
                    .to_vec();
                let size = rendered.size(0);
                // GPUI render images are BGRA; native backends receive RGBA.
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
                (Arc::from(rgba), size.width.into(), size.height.into())
            }
        };
        let bitmap = BitmapIcon {
            rgba,
            width,
            height,
            #[cfg(target_os = "macos")]
            scale: self.scale,
            #[cfg(target_os = "macos")]
            template: self.template,
        };
        if bitmap.width == 0 || bitmap.height == 0 {
            return Err(Error::InvalidIcon("dimensions must be non-zero".into()));
        }
        let expected = bitmap
            .width
            .checked_mul(bitmap.height)
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or_else(|| Error::InvalidIcon("dimensions overflow".into()))?;
        if bitmap.rgba.len() != expected {
            return Err(Error::InvalidIcon(format!(
                "expected {expected} RGBA bytes for {}x{}, got {}",
                bitmap.width,
                bitmap.height,
                bitmap.rgba.len()
            )));
        }
        #[cfg(target_os = "macos")]
        {
            if !bitmap.scale.is_finite() || bitmap.scale <= 0.0 {
                return Err(Error::InvalidIcon(format!(
                    "scale factor must be finite and positive, got {}",
                    bitmap.scale
                )));
            }
            // Even a finite scale can overflow when converted to point dimensions.
            for pixels in [bitmap.width, bitmap.height] {
                let points = f64::from(pixels) / bitmap.scale;
                if !points.is_finite() || points <= 0.0 {
                    return Err(Error::InvalidIcon(
                        "logical dimensions must be finite and positive".into(),
                    ));
                }
            }
        }
        Ok(Icon {
            source: IconSource::Bitmap(bitmap),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builder_for_image(image: gpui::Image) -> BitmapIconBuilder {
        #[cfg(feature = "gpui-ce")]
        let assets = gpui::AssetRegistry::default();
        #[cfg(not(feature = "gpui-ce"))]
        let assets = ();
        let renderer = gpui::SvgRenderer::new(Arc::new(assets));
        BitmapIconBuilder::from_gpui(image, move |image| {
            image
                .to_image_data(renderer.clone())
                .map_err(|error| Error::InvalidIcon(error.to_string()))
        })
    }

    #[test]
    fn gpui_bitmap_conversion_preserves_rgba_and_configuration() {
        let image = gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 96,
                100, 98, 102, 1, 0, 0, 25, 0, 11, 231, 90, 70, 164, 0, 0, 0, 0, 73, 69, 78, 68,
                174, 66, 96, 130,
            ],
        );
        let builder = builder_for_image(image);
        #[cfg(target_os = "macos")]
        let builder = builder.scale_factor(2.0).template(true);
        let icon = builder.build().unwrap();
        let bitmap = match icon.source {
            IconSource::Bitmap(bitmap) => bitmap,
            #[cfg(target_os = "macos")]
            IconSource::AppKit(_) => panic!("GPUI images should produce a bitmap"),
        };
        assert_eq!(&*bitmap.rgba, &[1, 2, 3, 4]);
        assert_eq!((bitmap.width, bitmap.height), (1, 1));
        #[cfg(target_os = "macos")]
        {
            assert_eq!(bitmap.scale, 2.0);
            assert!(bitmap.template);
        }
    }

    #[test]
    fn gpui_decode_errors_are_reported_by_build() {
        let builder = builder_for_image(gpui::Image::empty());
        assert!(matches!(builder.build(), Err(Error::InvalidIcon(_))));
    }

    #[test]
    fn gpui_svg_is_rendered_by_build() {
        let image = gpui::Image::from_bytes(
            gpui::ImageFormat::Svg,
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="#123456"/></svg>"##.to_vec(),
        );
        let icon = builder_for_image(image).build().unwrap();
        let bitmap = match icon.source {
            IconSource::Bitmap(bitmap) => bitmap,
            #[cfg(target_os = "macos")]
            IconSource::AppKit(_) => panic!("GPUI images should produce a bitmap"),
        };
        // GPUI rasterizes SVGs at twice their nominal dimensions.
        assert_eq!((bitmap.width, bitmap.height), (4, 6));
        assert!(
            bitmap
                .rgba
                .chunks_exact(4)
                .all(|pixel| pixel == [0x12, 0x34, 0x56, 0xff])
        );
    }

    #[test]
    fn validates_bitmap_data() {
        assert!(Icon::from_rgba(vec![0; 16], 2, 2).build().is_ok());
        assert!(Icon::from_rgba(vec![0; 15], 2, 2).build().is_err());
        assert!(Icon::from_rgba(Vec::new(), 0, 0).build().is_err());
        assert!(
            Icon::from_rgba(Vec::new(), u32::MAX, u32::MAX)
                .build()
                .is_err()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn validates_bitmap_density() {
        for scale in [
            0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::from_bits(1),
        ] {
            assert!(
                Icon::from_rgba(vec![0; 16], 2, 2)
                    .scale_factor(scale)
                    .build()
                    .is_err()
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bitmap_metadata_participates_in_equality() {
        let builder = Icon::from_rgba(vec![0; 16], 2, 2);
        let icon = builder.clone().build().unwrap();
        assert_eq!(icon, builder.clone().build().unwrap());
        assert_ne!(icon, builder.clone().scale_factor(2.0).build().unwrap());
        assert_ne!(icon, builder.template(true).build().unwrap());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn bitmap_preserves_pixels_and_dimensions() {
        static_assertions::assert_impl_all!(Icon: Send, Sync);
        let icon = Icon::from_rgba(vec![1, 2, 3, 4], 1, 1).build().unwrap();
        assert_eq!(icon.rgba_bytes(), &[1, 2, 3, 4]);
        assert_eq!((icon.width(), icon.height()), (1, 1));
    }
}

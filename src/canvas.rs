use foreign_types_shared::ForeignType;

use skia_safe::{
    gpu::{
        mtl::{self, BackendContext},
        DirectContext,
    },
    Color, Image, Paint, SamplingOptions, Surface,
};
use std::mem;
use std::ptr;
use tracing::{event, span, Level};

#[cfg(target_os = "macos")]
use objc::rc::autoreleasepool;

pub struct Canvas {
    surface: Surface,
    paint: Paint,
    #[cfg(target_os = "macos")]
    _context: Option<DirectContext>, // This is just stored for the lifetime of the canvas.
    #[cfg(target_os = "macos")]
    _backend: Option<BackendContext>,
}

impl Canvas {
    #[cfg(target_os = "macos")]
    pub fn new_metal(width: u32, height: u32) -> Option<Canvas> {
        autoreleasepool(|| {
            let span = span!(Level::INFO, "Canvas::new_metal");
            let _guard = span.enter();

            use metal::Device;
            use skia_safe::{gpu::SurfaceOrigin, Budgeted, ImageInfo};

            let mut paint = Paint::default();
            paint.set_color(Color::BLACK);
            paint.set_stroke_width(1.0);
            paint.set_blend_mode(skia_safe::BlendMode::SrcOver);

            let device = Device::system_default();
            if device.is_none() {
                event!(
                    Level::INFO,
                    "Failed to create Metal device, falling back to CPU."
                );
                return None;
            }
            let device = device.unwrap();
            let queue = device.new_command_queue();

            let backend = unsafe {
                // Handles are released when BackendContext is dropped.
                mtl::BackendContext::new(
                    device.as_ptr() as mtl::Handle,
                    queue.as_ptr() as mtl::Handle,
                    ptr::null(),
                )
            };
            let mut context = DirectContext::new_metal(&backend, None).unwrap();

            let image_info = ImageInfo::new_n32_premul((width as i32, height as i32), None);
            let surface = Surface::new_render_target(
                &mut context,
                Budgeted::Yes,
                &image_info,
                None,
                SurfaceOrigin::TopLeft,
                None,
                false,
            );
            if surface.is_none() {
                event!(
                    Level::INFO,
                    "Failed to create Metal surface, falling back to CPU."
                );
                return None;
            }

            let mut surface = surface.unwrap();

            surface.canvas().clear(Color::RED);

            event!(Level::INFO, "Created new Metal backed canvas");

            Some(Canvas {
                surface,
                paint,
                _context: Some(context),
                _backend: Some(backend), // Make sure backend isn't dropped.
            })
        })
    }

    pub fn new(width: u32, height: u32) -> Canvas {
        let span = span!(Level::INFO, "Canvas::new");
        let _guard = span.enter();
        // Fallback to CPU.
        let mut surface =
            Surface::new_raster_n32_premul((width as i32, height as i32)).expect("no surface!");
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.set_stroke_width(1.0);
        paint.set_blend_mode(skia_safe::BlendMode::SrcOver);
        surface.canvas().clear(Color::WHITE);

        event!(Level::INFO, "Created new CPU backed canvas");
        Canvas {
            surface,
            paint,
            #[cfg(target_os = "macos")]
            _context: None,
            #[cfg(target_os = "macos")]
            _backend: None,
        }
    }

    pub fn width(&self) -> usize {
        self.surface.width() as usize
    }

    pub fn height(&self) -> usize {
        self.surface.height() as usize
    }

    pub fn draw_raw_rgb_scale(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        scale: u32,
        pixels: Option<&[u8]>,
        image: Option<Image>,
    ) -> Option<Image> {
        fn draw_raw_rgb_scale_internal(
            canvas: &mut Canvas,
            x: i32,
            y: i32,
            w: u32,
            h: u32,
            scale: u32,
            pixels: Option<&[u8]>,
            image: Option<Image>,
        ) -> Option<Image> {
            let span = span!(Level::INFO, "Canvas::draw_raw_rgb_scale");
            let _guard = span.enter();
            let info = skia_safe::ImageInfo::new(
                skia_safe::ISize::new(w as i32, h as i32),
                skia_safe::ColorType::BGRA8888,
                skia_safe::AlphaType::Premul,
                None,
            );

            // Pixels must live as long as sdata, which is clearly
            // evident in this operation. Not sure why they didn't
            // use lifetimes and require we use unsafe...
            let image = if let Some(image) = image {
                Some(image)
            } else if let Some(pixels) = pixels {
                let sdata = unsafe { skia_safe::Data::new_bytes(pixels) };
                let image = skia_safe::Image::from_raster_data(
                    &info,
                    sdata,
                    w as usize * mem::size_of::<u32>(),
                );
                image
            } else {
                None
            };

            if image.is_none() {
                event!(Level::ERROR, "Failed to create image from raster data.");
                return None;
            }
            let image = image.unwrap();
            let result = canvas.draw_image_scale(&image, x, y, scale);
            event!(Level::TRACE, "Draw image scale: {}", result);
            Some(image)
        }
        #[cfg(target_os = "macos")]
        return autoreleasepool(|| {
            draw_raw_rgb_scale_internal(self, x, y, w, h, scale, pixels, image)
        });
        #[cfg(not(target_os = "macos"))]
        draw_raw_rgb_scale_internal(self, x, y, w, h, scale, pixels)
    }

    pub fn draw_image_scale(
        &mut self,
        image: &skia_safe::Image,
        x: i32,
        y: i32,
        scale: u32,
    ) -> bool {
        fn draw_image_scale_internal(
            canvas: &mut Canvas,
            image: &skia_safe::Image,
            x: i32,
            y: i32,
            scale: u32,
        ) -> bool {
            let span = span!(Level::INFO, "Canvas::draw_image_scale");
            let _guard = span.enter();
            let paint = &canvas.paint;
            let scaled_rect = skia_safe::Rect::new(
                x as f32,
                y as f32,
                (x + (image.width() / scale as i32)) as f32,
                (y + (image.height() / scale as i32)) as f32,
            );

            // ToDo: Experiment with and profile different downscaling configurations.
            // This preserves text readability quite well.
            let options = SamplingOptions::new(
                skia_safe::FilterMode::Linear,
                skia_safe::MipmapMode::Nearest,
            );
            event!(
                Level::TRACE,
                "Draw image scale, {}x{}, {}x{}, {}",
                x,
                y,
                image.width() / scale as i32,
                image.height() / scale as i32,
                scale
            );
            canvas
                .surface
                .canvas()
                .draw_image_rect_with_sampling_options(image, None, scaled_rect, options, paint);
            true
        }
        #[cfg(target_os = "macos")]
        return autoreleasepool(|| draw_image_scale_internal(self, image, x, y, scale));
        #[cfg(not(target_os = "macos"))]
        draw_image_scale_internal(self, image, x, y, scale)
    }

    #[inline]
    pub fn skia_canvas(&mut self) -> &mut skia_safe::Canvas {
        self.surface.canvas()
    }
}

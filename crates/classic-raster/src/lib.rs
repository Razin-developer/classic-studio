//! Deterministic CPU PNG export for Classic documents.
//!
//! This crate deliberately covers a small, robust rendering subset: solid-color
//! frames, rectangles, ellipses, and lines. Text, image, path, mask, effect,
//! gradient, and blend-mode layers are skipped. Rotation is currently ignored.

use std::{error::Error as StdError, fmt, path::Path};

use classic_core::{DesignDocument, DesignNode, NodeKind, Page};
use image::{ImageError, ImageFormat, Rgba, RgbaImage};

/// Destination canvas and coordinate transform for a raster export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterOptions {
    /// PNG width in physical pixels.
    pub width: u32,
    /// PNG height in physical pixels.
    pub height: u32,
    /// Number of output pixels for one document coordinate.
    pub scale: f32,
    /// Document x-coordinate at the left edge of the bitmap.
    pub origin_x: f32,
    /// Document y-coordinate at the top edge of the bitmap.
    pub origin_y: f32,
    /// Color painted before nodes. Alpha may be zero for a transparent PNG.
    pub background: Rgba<u8>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            width: 1440,
            height: 1024,
            scale: 1.0,
            origin_x: 0.0,
            origin_y: 0.0,
            background: Rgba([255, 255, 255, 255]),
        }
    }
}

#[derive(Debug)]
pub enum RasterError {
    InvalidOptions(&'static str),
    MissingPage(String),
    Image(ImageError),
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(f, "invalid raster options: {message}"),
            Self::MissingPage(id) => write!(f, "page not found: {id}"),
            Self::Image(error) => write!(f, "PNG export failed: {error}"),
        }
    }
}

impl StdError for RasterError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Image(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ImageError> for RasterError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}

/// Render a page into an RGBA bitmap. Nodes are painted in document order.
pub fn render_page(page: &Page, options: RasterOptions) -> Result<RgbaImage, RasterError> {
    validate_options(options)?;
    let mut image = RgbaImage::from_pixel(options.width, options.height, options.background);
    render_nodes(&mut image, &page.nodes, options);
    Ok(image)
}

/// Render a document page identified by `page_id`.
pub fn render_document_page(
    document: &DesignDocument,
    page_id: &str,
    options: RasterOptions,
) -> Result<RgbaImage, RasterError> {
    let page = document
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .ok_or_else(|| RasterError::MissingPage(page_id.to_owned()))?;
    render_page(page, options)
}

/// Render the document's active page.
pub fn render_active_page(
    document: &DesignDocument,
    options: RasterOptions,
) -> Result<RgbaImage, RasterError> {
    render_document_page(document, &document.active_page_id, options)
}

/// Export a page directly as PNG. The parent directory must already exist.
pub fn export_page_png(
    page: &Page,
    path: impl AsRef<Path>,
    options: RasterOptions,
) -> Result<(), RasterError> {
    render_page(page, options)?.save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

/// Export the active document page directly as PNG.
pub fn export_active_page_png(
    document: &DesignDocument,
    path: impl AsRef<Path>,
    options: RasterOptions,
) -> Result<(), RasterError> {
    render_active_page(document, options)?.save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

fn validate_options(options: RasterOptions) -> Result<(), RasterError> {
    if options.width == 0 || options.height == 0 {
        return Err(RasterError::InvalidOptions(
            "width and height must be non-zero",
        ));
    }
    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err(RasterError::InvalidOptions(
            "scale must be finite and greater than zero",
        ));
    }
    if !options.origin_x.is_finite() || !options.origin_y.is_finite() {
        return Err(RasterError::InvalidOptions("origins must be finite"));
    }
    Ok(())
}

fn render_nodes(image: &mut RgbaImage, nodes: &[DesignNode], options: RasterOptions) {
    for node in nodes {
        if !node.visible {
            continue;
        }
        render_node(image, node, options);
        render_nodes(image, &node.children, options);
    }
}

fn render_node(image: &mut RgbaImage, node: &DesignNode, options: RasterOptions) {
    let opacity = node.opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let fill = parse_color(&node.fill).map(|color| with_opacity(color, opacity));
    let stroke = parse_color(&node.stroke).map(|color| with_opacity(color, opacity));

    match node.kind {
        NodeKind::Frame | NodeKind::Rectangle | NodeKind::Image => {
            draw_rounded_rect(image, node, fill, stroke, options);
        }
        NodeKind::Ellipse => draw_ellipse(image, node, fill, stroke, options),
        NodeKind::Polygon => draw_polygon(image, node, fill, stroke, options),
        NodeKind::Line => {
            if let Some(color) = stroke.or(fill) {
                draw_line(image, node, color, options);
            }
        }
        NodeKind::Text => {}
    }
}

fn draw_polygon(
    image: &mut RgbaImage,
    node: &DesignNode,
    fill: Option<Rgba<u8>>,
    stroke: Option<Rgba<u8>>,
    options: RasterOptions,
) {
    const SIDES: usize = 5;
    let (left, top, right, bottom) = bounds(node, options, node.stroke_width.max(0.0) * 0.5);
    let cx = (left + right) * 0.5;
    let cy = (top + bottom) * 0.5;
    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let points = (0..SIDES)
        .map(|index| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::TAU / SIDES as f32;
            (cx + rx * angle.cos(), cy + ry * angle.sin())
        })
        .collect::<Vec<_>>();
    let half_stroke = node.stroke_width.max(0.0) * options.scale * 0.5;
    visit_bounds(image, left, top, right, bottom, |image, x, y| {
        let px = x as f32 + 0.5;
        let py = y as f32 + 0.5;
        let inside = point_in_polygon(px, py, &points);
        let edge_distance = points
            .iter()
            .enumerate()
            .map(|(index, &(x1, y1))| {
                let (x2, y2) = points[(index + 1) % points.len()];
                point_segment_distance(px, py, x1, y1, x2, y2)
            })
            .fold(f32::INFINITY, f32::min);
        if edge_distance <= half_stroke {
            if let Some(color) = stroke {
                blend_pixel(image, x, y, color);
            } else if inside {
                if let Some(color) = fill {
                    blend_pixel(image, x, y, color);
                }
            }
        } else if inside {
            if let Some(color) = fill {
                blend_pixel(image, x, y, color);
            }
        }
    });
}

fn draw_rounded_rect(
    image: &mut RgbaImage,
    node: &DesignNode,
    fill: Option<Rgba<u8>>,
    stroke: Option<Rgba<u8>>,
    options: RasterOptions,
) {
    let (left, top, right, bottom) = bounds(node, options, node.stroke_width.max(0.0) * 0.5);
    let outer_radius = (node.corner_radius.max(0.0) * options.scale)
        .min((right - left).max(0.0) * 0.5)
        .min((bottom - top).max(0.0) * 0.5);
    let stroke_width = node.stroke_width.max(0.0) * options.scale;
    let inner_radius = (outer_radius - stroke_width).max(0.0);
    visit_bounds(image, left, top, right, bottom, |image, x, y| {
        let px = x as f32 + 0.5;
        let py = y as f32 + 0.5;
        if !inside_rounded_rect(px, py, left, top, right, bottom, outer_radius) {
            return;
        }
        let inside_inner = stroke_width <= 0.0
            || inside_rounded_rect(
                px,
                py,
                left + stroke_width,
                top + stroke_width,
                right - stroke_width,
                bottom - stroke_width,
                inner_radius,
            );
        if !inside_inner {
            if let Some(color) = stroke {
                blend_pixel(image, x, y, color);
            } else if let Some(color) = fill {
                blend_pixel(image, x, y, color);
            }
        } else if let Some(color) = fill {
            blend_pixel(image, x, y, color);
        }
    });
}

fn draw_ellipse(
    image: &mut RgbaImage,
    node: &DesignNode,
    fill: Option<Rgba<u8>>,
    stroke: Option<Rgba<u8>>,
    options: RasterOptions,
) {
    let (left, top, right, bottom) = bounds(node, options, node.stroke_width.max(0.0) * 0.5);
    let cx = (left + right) * 0.5;
    let cy = (top + bottom) * 0.5;
    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let stroke_width = node.stroke_width.max(0.0) * options.scale;
    visit_bounds(image, left, top, right, bottom, |image, x, y| {
        let dx = (x as f32 + 0.5 - cx) / rx;
        let dy = (y as f32 + 0.5 - cy) / ry;
        let distance = dx * dx + dy * dy;
        if distance > 1.0 {
            return;
        }
        let inner_rx = (rx - stroke_width).max(0.0);
        let inner_ry = (ry - stroke_width).max(0.0);
        let inside_inner = inner_rx > 0.0
            && inner_ry > 0.0
            && ((x as f32 + 0.5 - cx) / inner_rx).powi(2)
                + ((y as f32 + 0.5 - cy) / inner_ry).powi(2)
                <= 1.0;
        if !inside_inner {
            if let Some(color) = stroke {
                blend_pixel(image, x, y, color);
            } else if let Some(color) = fill {
                blend_pixel(image, x, y, color);
            }
        } else if let Some(color) = fill {
            blend_pixel(image, x, y, color);
        }
    });
}

fn draw_line(image: &mut RgbaImage, node: &DesignNode, color: Rgba<u8>, options: RasterOptions) {
    let x1 = (node.x - options.origin_x) * options.scale;
    let y1 = (node.y - options.origin_y) * options.scale;
    let x2 = (node.x + node.width - options.origin_x) * options.scale;
    let y2 = (node.y + node.height - options.origin_y) * options.scale;
    let half_width = (node.stroke_width.max(1.0) * options.scale) * 0.5;
    let left = x1.min(x2) - half_width;
    let top = y1.min(y2) - half_width;
    let right = x1.max(x2) + half_width;
    let bottom = y1.max(y2) + half_width;
    visit_bounds(image, left, top, right, bottom, |image, x, y| {
        let px = x as f32 + 0.5;
        let py = y as f32 + 0.5;
        if point_segment_distance(px, py, x1, y1, x2, y2) <= half_width {
            blend_pixel(image, x, y, color);
        }
    });
}

fn bounds(node: &DesignNode, options: RasterOptions, outset: f32) -> (f32, f32, f32, f32) {
    let left = (node.x - options.origin_x) * options.scale - outset * options.scale;
    let top = (node.y - options.origin_y) * options.scale - outset * options.scale;
    let right = (node.x + node.width - options.origin_x) * options.scale + outset * options.scale;
    let bottom = (node.y + node.height - options.origin_y) * options.scale + outset * options.scale;
    (left, top, right, bottom)
}

fn visit_bounds(
    image: &mut RgbaImage,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    mut draw: impl FnMut(&mut RgbaImage, u32, u32),
) {
    let x_start = left.floor().max(0.0) as u32;
    let y_start = top.floor().max(0.0) as u32;
    let x_end = right.ceil().min(image.width() as f32).max(0.0) as u32;
    let y_end = bottom.ceil().min(image.height() as f32).max(0.0) as u32;
    for y in y_start..y_end {
        for x in x_start..x_end {
            draw(image, x, y);
        }
    }
}

fn inside_rounded_rect(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
) -> bool {
    if x < left || x >= right || y < top || y >= bottom {
        return false;
    }
    if radius <= 0.0 {
        return true;
    }
    let cx = x.clamp(left + radius, right - radius);
    let cy = y.clamp(top + radius, bottom - radius);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= radius * radius
}

fn point_segment_distance(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return (px - x1).hypot(py - y1);
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / length_squared).clamp(0.0, 1.0);
    (px - (x1 + t * dx)).hypot(py - (y1 + t * dy))
}

fn point_in_polygon(px: f32, py: f32, points: &[(f32, f32)]) -> bool {
    let mut inside = false;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        if (y1 > py) != (y2 > py) && px < (x2 - x1) * (py - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

fn parse_color(value: &str) -> Option<Rgba<u8>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
        return None;
    }
    let hex = value.strip_prefix('#')?;
    let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    match hex.len() {
        3 => Some(Rgba([
            u8::from_str_radix(&hex[0..1], 16).ok()? * 17,
            u8::from_str_radix(&hex[1..2], 16).ok()? * 17,
            u8::from_str_radix(&hex[2..3], 16).ok()? * 17,
            255,
        ])),
        4 => Some(Rgba([
            u8::from_str_radix(&hex[0..1], 16).ok()? * 17,
            u8::from_str_radix(&hex[1..2], 16).ok()? * 17,
            u8::from_str_radix(&hex[2..3], 16).ok()? * 17,
            u8::from_str_radix(&hex[3..4], 16).ok()? * 17,
        ])),
        6 => Some(Rgba([byte(0)?, byte(2)?, byte(4)?, 255])),
        8 => Some(Rgba([byte(0)?, byte(2)?, byte(4)?, byte(6)?])),
        _ => None,
    }
}

fn with_opacity(mut color: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    color[3] = (color[3] as f32 * opacity).round() as u8;
    color
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, source: Rgba<u8>) {
    let destination = image.get_pixel_mut(x, y);
    let source_alpha = source[3] as f32 / 255.0;
    let destination_alpha = destination[3] as f32 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        *destination = Rgba([0, 0, 0, 0]);
        return;
    }
    for channel in 0..3 {
        let source_channel = source[channel] as f32 / 255.0;
        let destination_channel = destination[channel] as f32 / 255.0;
        let output = (source_channel * source_alpha
            + destination_channel * destination_alpha * (1.0 - source_alpha))
            / output_alpha;
        destination[channel] = (output * 255.0).round() as u8;
    }
    destination[3] = (output_alpha * 255.0).round() as u8;
}

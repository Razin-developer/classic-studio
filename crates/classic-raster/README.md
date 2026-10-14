# classic-raster

`classic-raster` exports a selected Classic page to a PNG using deterministic CPU
rasterization. It supports visible frames, rectangles, ellipses, and lines with
solid hex colors, stroke width, rounded rectangle corners, and node opacity.

The renderer intentionally has a small, dependable contract. It does not yet
render text, image layers, paths, gradients, blend modes, masks, effects, clips,
or rotation. Child nodes use the same canvas coordinates as their parents, and
frames paint as shapes but do not clip their children.

Call `render_page` to obtain an `image::RgbaImage`, or `export_page_png` to save
a page directly. Use explicit `RasterOptions` to define the output canvas and
background.

# Classic Studio

Classic Studio is an independent, clean-room Rust rebuild inspired by Figma's design-editor workflows. It has an original Classic identity and document format and is not affiliated with Figma. The goal is an open-source design editor; the current application is an early foundation and does not claim complete feature parity.

The application is **Rust only**. `eframe`/`egui` provide the native window and editor interface; `eframe`'s default `wgpu` renderer routes canvas/UI drawing through the GPU backend when an adapter is available. Adapter selection/fallback is handled by `wgpu`; this application does not implement a custom renderer. The document model, canvas interactions, panels, file handling, and application code are Rust. The SVG/PNG/ICO files in `assets/` are the original app logo and window icon.

## Run

Install Rust 1.95 or later and run from the project root:

```powershell
cargo run --release
```

The release profile enables full optimization and thin LTO.

## Keyboard controls

- `V`, `F`, `R`, `O`, `L`, `T`: Move, Frame, Rectangle, Ellipse, Line, Text
- `Ctrl/Cmd+Z`: Undo; `Ctrl/Cmd+Shift+Z` or `Ctrl/Cmd+Y`: Redo
- `Ctrl/Cmd+D`: Duplicate selection; `Delete` or `Backspace`: Delete selection
- `Shift` + click: Add or remove an item from the selection; `Ctrl/Cmd+A`: Select all visible layers
- Arrow keys: Nudge selection; hold `Shift` for a 10-unit move
- Hold `Space` and drag: Pan; scroll over the canvas: Pan; `Ctrl/Cmd+scroll`: Zoom around the pointer
- `+` / `-`: Zoom; `Home`: Fit artwork; `Escape`: Return to Move and clear selection

## Included editor foundation

- Native desktop layout with document bar, tools, layer/page sidebar, canvas, and inspector
- Dockable Layers, Canvas, and Inspector workspaces, including detachable tabs
- Phosphor icon font for the tool palette, shortcut hints, themed right-click actions, and hover tooltips
- GPU-backed `wgpu` rendering through the native Rust UI stack
- Select/move, frame, rectangle, ellipse, line, and text tools
- Starter landing-page canvas with editable layers
- Canvas grid, scroll/keyboard zoom, Space-drag pan, multi-selection, drag-to-move, arrow-key nudges, fit-to-view, and resize handles
- Position and size editing, fill, stroke, opacity, radius, rotation, and text editing
- Page creation, layer visibility and lock controls, and duplication/deletion
- Undo/redo, keyboard tool shortcuts, native Open/Save dialogs
- Canvas context menu, arrow-key nudging, zoom-to-pointer, and fit-to-artwork shortcut
- Alignment and distribution controls for multi-layer selections
- Canvas ruler-guide menu: add horizontal or vertical guides, toggle visibility/snap behavior, remove unlocked guides, and retain them in `.classic` documents
- Assets panel: import or drag/drop local files and embed their bytes, size, and SHA-256 integrity hash into `.classic` projects; PNG, JPEG, GIF, BMP, and WebP raster assets decode into cached GPU textures for canvas and presentation preview
- Comments panel: add pinned local review comments to layers or canvas points, resolve/reopen/delete them, and retain them in `.classic` documents
- Local presentation preview: present fitted prototype frames, execute saved click navigation, restart the flow, and exit with the control or `Escape`
- Components menu: create local component definitions from selected layers and place linked instances on the active canvas
- In-app keyboard reference: use the `?` toolbar button to view editing, selection, navigation, and presentation bindings
- Prototype inspector: create, inspect, and remove saved click-to-navigate interactions between selected layers and active-page frames
- Local recovery: dirty documents autosave a recovery snapshot every 30 seconds; use the top-bar Recover action when a snapshot is available
- Dev inspector: select Dev in the inspector to inspect a selected layer and copy scoped CSS for handoff
- Layout menu: stack two or more selected unlocked layers horizontally or vertically with an editable gap
- Accessibility check: compare a selected layer fill to an editable background and see WCAG normal/large-text contrast results
- Windows tray icon with restore and quit actions; closing the window hides it in the tray
- Versioned `.classic` JSON documents with validation and safer replacement saves
- Basic SVG import from Open and SVG export from the inspector; paths, gradients, filters, and full SVG typography are unsupported
- Active-page PNG export for solid frames, rectangles, ellipses, and lines; advanced layer types are skipped
- Renderer-independent geometry, snapping, alignment, auto-layout, responsive constraints, and prototype graph models

Cloud sharing, real-time collaboration, PDF export, nested-group editing, component variants, and full design-system workflows are not implemented. Frames, rectangles, raster images, ellipses, and lines rotate on the canvas; text rotation and rotated raster export remain scoped. Raster-image export, crop modes, filters, and vector-asset rendering are still scoped. Proprietary Figma project formats are intentionally unsupported.

## Structure

- `src/main.rs`: native Rust application, GPU-rendered canvas, panels, and interactions.
- `crates/classic-core`: versioned page/layer model, validation, and document commands.
- `crates/classic-geometry`: canvas geometry, snapping, alignment, and distribution helpers.
- `crates/classic-layout`: stack layout, responsive constraints, and prototype graph models.
- `crates/classic-formats`: versioned `.classic` persistence and basic SVG import/export.
- `crates/classic-raster`: deterministic active-page PNG export for a scoped geometry subset.
- `crates/classic-comments`: serializable local review-thread domain model.
- `crates/classic-components`: serializable component, variant, and instance domain model.
- `crates/classic-assets`: serializable local asset catalog model.
- `crates/classic-guides`: serializable ruler-guide and deterministic snap model.
- `crates/classic-tokens`: serializable design-token collections with alias resolution and cycle validation.

The full 100-capability comparison target, its current status, and delivery order are tracked in [PRO_FEATURE_ROADMAP.md](PRO_FEATURE_ROADMAP.md).
- `src/shortcuts.rs` and `src/context_menu.rs`: key mapping and undo-routed canvas context actions.
- `assets/`: original Classic SVG mark and generated app icons.

The UI uses `egui_dock` for movable/detachable panels and `egui-phosphor` for the icon font. Tray background mode currently targets Windows; other operating systems close the app with the window.

The local investigation located Figma Desktop `126.6.14`, but the REA artifact inventory stopped at an invalid `.codesign` range, and no running UI was available to observe. No proprietary source was recovered; the editor follows common design-editor workflows rather than verified Figma UI details.

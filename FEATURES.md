# Classic Studio feature catalog

This catalog lists 50 product capabilities and their current status. “Implemented” means the capability is wired into the current desktop app; “Partial” means a limited version is usable; “Foundation” means Rust models or algorithms exist but are not exposed as a complete workflow; “Planned” means it is not implemented yet. The roadmap is for an independent Rust design editor and does not claim feature parity with Figma.

## Canvas and editing

1. **[Implemented] GPU-backed desktop canvas** — the native egui UI renders through eframe's wgpu backend when a GPU adapter is available.
2. **[Implemented] Canvas grid** — toggle a point grid while editing.
3. **[Implemented] Pan and zoom** — navigate the canvas with pointer interaction and zoom controls.
4. **[Implemented] Fit canvas to artwork** — frame the visible document content in the canvas view.
5. **[Implemented] Multi-layer selection** — select canvas items or layers; use Shift-click to add/remove and Ctrl/Cmd+A to select all visible layers.
6. **[Implemented] Move and resize items** — drag a selected set together and use resize handles on selected items.
7. **[Implemented] Snap guides and rulers** — show alignment guides while moving near visible items; add visible horizontal or vertical ruler guides at the canvas center, snap selected layers to them, and save their settings in `.classic` documents.
8. **[Implemented] Pages** — create and switch document pages.
9. **[Implemented] Layer visibility and locking** — hide or lock individual layers from the layer list.
10. **[Implemented] Duplicate and delete** — duplicate or remove the selected item.
11. **[Implemented] Right-click item menu** — select, duplicate, hide/show, lock/unlock, change stacking order, and delete.
12. **[Implemented] Keyboard shortcuts** — tool selection, undo/redo, delete, duplicate, select-all, nudging, zoom, fit, escape, and temporary pan are mapped.

## Vector editing and styling

13. **[Implemented] Basic shape tools** — drag to create frames, rectangles, ellipses, lines, and text items; Shift constrains square and circular shapes.
14. **[Implemented] Fill and stroke controls** — edit solid colors and stroke width.
15. **[Implemented] Opacity and corner radius** — adjust item opacity and rectangle rounding; the saved document foundation also includes a validated design-token library for future reusable values.
16. **[Partial] Rotation** — rotate frames, rectangles, raster images, ellipses, and lines in the canvas with rotated selection bounds and hit checks; text rotation and rotated raster export remain pending.
17. **[Partial] Text editing** — edit simple text content and color; advanced typography and layout are not supported.
18. **[Planned] Pen and Bézier editing** — draw and edit vector paths with nodes and handles.
19. **[Planned] Boolean shape operations** — combine, subtract, intersect, and exclude vector shapes.

## Design systems and layout

20. **[Implemented] Alignment and distribution** — align selected unlocked layers by edges or centers, or distribute three or more layers evenly from toolbar controls.
21. **[Partial] Stack auto-layout** — arrange selected unlocked layers into horizontal or vertical stacks with a configurable gap using the Rust layout solver; saved frame-attached auto-layout settings are pending.
22. **[Foundation] Responsive resize constraints** — a Rust model computes anchored, centered, stretched, and scaled child geometry.
23. **[Partial] Editable auto-layout controls** — adjust stack gap and direction from the Layout toolbar menu; attaching layout rules to frames is pending.
24. **[Planned] Nested frames and groups** — create and edit hierarchical containers in the canvas and layer tree.
25. **[Partial] Reusable components** — create a saved local component definition from a selected layer and place linked instances through the Components menu; editing definitions and instance overrides are pending.
26. **[Foundation] Component variants** — the persisted component model stores variant axes and resolves named variants; editor controls are pending.
27. **[Partial] Assets catalog and panel** — import/remove local assets through the Assets tab or operating-system file drag/drop, embed bytes with SHA-256 integrity metadata inside `.classic` documents, place raster/vector assets as canvas image layers, decode PNG/JPEG/GIF/BMP/WebP assets to cached GPU textures, and use a right-click remove action; thumbnails, crop modes, filters, and vector rendering are pending.

## Prototyping

28. **[Partial] Prototype graph model** — persist local interactions in `.classic` documents and inspect them in the Prototype panel.
29. **[Partial] Prototype triggers and actions** — create click-to-navigate interactions from selected layers to chosen frames; advanced triggers/actions are modeled but not yet editable.
30. **[Partial] Transition definitions** — created navigation interactions use a persisted dissolve transition; other transition controls are pending.
31. **[Partial] Visual interaction editor** — add, inspect, and remove local navigation interactions from the inspector.
32. **[Partial] Interactive preview** — Present executes saved click-to-navigate interactions and fits the destination frame; overlays, scrolling, device shells, and full transition animation are pending.
33. **[Planned] Overlay and scrolling behavior** — configure working overlays, scrolling, and flow starts in the editor.
34. **[Partial] Local presentation preview** — Present renders the active page in a distraction-free, fitted viewer with an exit control and Escape shortcut; prototype interactions, device shells, and shared links are pending.

## Collaboration and documents

35. **[Implemented] Native project files** — save and open versioned .classic JSON documents.
36. **[Implemented] Document validation** — reject unsupported schema versions and invalid document data.
37. **[Implemented] Undo and redo** — revert and restore recent local document edits.
38. **[Partial] Autosave and crash recovery** — while a document is dirty, Classic saves a validated local recovery snapshot every 30 seconds and exposes a Recover action; snapshot history and conflict handling are pending.
39. **[Partial] Comments and review threads** — add local pinned review threads to selected layers or canvas points, resolve/reopen/delete them in the Comments panel, render unresolved layer badges, and retain them in `.classic` documents; replies, mentions, shared sync, and notifications are pending.
40. **[Planned] Shared projects and live collaboration** — synchronize edits, presence, and conflict recovery across users.

## Import and export

41. **[Partial] SVG import** — import a limited subset of SVG shapes and presentation attributes; paths and advanced SVG features are unsupported.
42. **[Partial] SVG export** — export basic document geometry, styling, and text as SVG.
43. **[Partial] PNG export** — export the active page as a PNG with solid frames, rectangles, ellipses, and lines; text, images, paths, effects, rotation, and selected-region export remain pending.
44. **[Planned] PDF export** — create shareable and print-ready PDF output.
45. **[Planned] Open-format coverage** — add clearly scoped import/export support for common design assets and document formats.

## Performance and platform

46. **[Planned] Large-document rendering** — add scene culling and rendering strategies for very large documents.
47. **[Planned] Performance benchmarks** — track startup, interaction latency, memory, and large-scene rendering.
48. **[Planned] Background work and responsive saving** — keep expensive persistence and asset operations from blocking editing.
49. **[Implemented] Dockable workspace panels** — arrange editor panels with egui_dock; the toolbar uses the Phosphor icon font and common controls include tooltips.
50. **[Implemented on Windows] System-tray background mode** — closing hides the app to a tray icon; tray actions restore the window or quit the process.


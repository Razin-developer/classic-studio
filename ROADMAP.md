# Classic Studio roadmap

## Editing

- Multi-selection, canvas grid guides, basic snap guides, and library-level alignment/distribution
- Layer reordering and keyboard nudges
- Groups, nested layers, keyboard nudges, and robust undo transactions
- Bezier pen and vector editing, shape operations, and richer typography controls

## Documents and output

- Schema migrations, crash recovery, autosave, and large-scene performance benchmarks
- Connect the SVG library to UI; add PNG export, then PDF with explicit compatibility limits
- Import common open formats and image assets with explicit compatibility limits
- Integrate the existing stack-layout and resize-constraint models into editable documents
- Add nested frames, reusable components, and component variants

## Prototype and collaboration

- Integrate the prototype graph models into an interaction editor and preview mode
- Authenticated project sync, presence, comments, offline changes, and conflict recovery
- Start collaboration only after stable IDs, command history, and persistence semantics

## Desktop workflow

- Add tray integration for Linux and macOS while preserving platform-specific menu conventions.
- Persist dock layouts and editor preferences between launches.
- Add accessibility labels, shortcut discoverability, and user-customizable key maps.
- Track the 50 scoped capabilities and their current status in [FEATURES.md](FEATURES.md).

The Rust document and rendering layers are designed to stay independent from persistence, export, and future collaboration services.

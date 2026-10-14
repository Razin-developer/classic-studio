//! Keyboard shortcut mapping for the Classic Studio editor.
//!
//! Call [`collect`] once near the start of the app's `ui` method, then dispatch
//! its actions into the editor state. The mapper consumes handled key events
//! and leaves text fields alone while egui reports that a widget wants text
//! input. Hold Space to temporarily pan; the existing canvas drag handling can
//! query [`space_pan_active`] each frame.
//!
//! | Shortcut | Action |
//! | --- | --- |
//! | V / F / R / O / L / T | Select / Frame / Rectangle / Ellipse / Line / Text tool |
//! | Ctrl/Command+Z | Undo |
//! | Ctrl/Command+Shift+Z or Ctrl/Command+Y | Redo |
//! | Ctrl/Command+A | Select all layers |
//! | Ctrl/Command+D | Duplicate selection |
//! | Delete / Backspace | Delete selection |
//! | Arrow keys | Nudge selection by 1 canvas unit (10 with Shift) |
//! | + / =, - | Zoom in / out |
//! | Home | Fit canvas |
//! | Escape | Cancel current tool or selection |
//! | Space (hold) | Temporary pan mode |

use eframe::egui::{self, Key, Modifiers};

/// A tool selected through its single-key shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolShortcut {
    Select,
    Frame,
    Rectangle,
    Ellipse,
    Line,
    Text,
}

/// An editor action produced by a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    SelectTool(ToolShortcut),
    Undo,
    Redo,
    DeleteSelection,
    SelectAll,
    DuplicateSelection,
    /// Move the selected object in document-space units.
    Nudge {
        x: i32,
        y: i32,
    },
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    Escape,
}

/// Consume and return shortcut actions pressed in the current egui frame.
///
/// Dispatch actions in the returned order. Only one action per key is emitted.
/// Text widgets retain full control of their keyboard input.
pub fn collect(ctx: &egui::Context) -> Vec<ShortcutAction> {
    if ctx.egui_wants_keyboard_input() {
        return Vec::new();
    }

    let mut actions = Vec::with_capacity(3);
    let command = Modifiers::COMMAND;

    let redo = ctx.input_mut(|input| {
        input.consume_key(command | Modifiers::SHIFT, Key::Z) || input.consume_key(command, Key::Y)
    });
    let undo = ctx.input_mut(|input| input.consume_key(command, Key::Z));
    if undo {
        actions.push(ShortcutAction::Undo);
    } else if redo {
        actions.push(ShortcutAction::Redo);
    }

    if consume_any(ctx, Modifiers::NONE, &[Key::Delete, Key::Backspace]) {
        actions.push(ShortcutAction::DeleteSelection);
    }
    if ctx.input_mut(|input| input.consume_key(command, Key::A)) {
        actions.push(ShortcutAction::SelectAll);
    }
    if ctx.input_mut(|input| input.consume_key(command, Key::D)) {
        actions.push(ShortcutAction::DuplicateSelection);
    }
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        actions.push(ShortcutAction::Escape);
    }

    // Arrow keys are reserved for geometry movement only without command or
    // alt modifiers; holding Shift gives a predictable ten-unit large step.
    let (shift, arrows) = ctx.input(|input| {
        let modifiers = input.modifiers;
        (
            modifiers.shift,
            modifiers.ctrl || modifiers.command || modifiers.alt,
        )
    });
    if !arrows {
        let step = if shift { 10 } else { 1 };
        for (key, x, y) in [
            (Key::ArrowLeft, -step, 0),
            (Key::ArrowRight, step, 0),
            (Key::ArrowUp, 0, -step),
            (Key::ArrowDown, 0, step),
        ] {
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key)) {
                actions.push(ShortcutAction::Nudge { x, y });
            }
        }
    }

    if ctx.input_mut(|input| {
        input.consume_key(Modifiers::NONE, Key::Plus)
            || input.consume_key(Modifiers::SHIFT, Key::Equals)
            || input.consume_key(Modifiers::NONE, Key::Equals)
    }) {
        actions.push(ShortcutAction::ZoomIn);
    }
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Minus)) {
        actions.push(ShortcutAction::ZoomOut);
    }
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Home)) {
        actions.push(ShortcutAction::ZoomToFit);
    }

    for (key, tool) in [
        (Key::V, ToolShortcut::Select),
        (Key::F, ToolShortcut::Frame),
        (Key::R, ToolShortcut::Rectangle),
        (Key::O, ToolShortcut::Ellipse),
        (Key::L, ToolShortcut::Line),
        (Key::T, ToolShortcut::Text),
    ] {
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key)) {
            actions.push(ShortcutAction::SelectTool(tool));
        }
    }

    actions
}

/// Whether Space is held for temporary canvas panning this frame.
///
/// This is state-based rather than an edge-triggered action so the canvas can
/// keep its pan cursor and drag behavior active for the entire key hold.
pub fn space_pan_active(ctx: &egui::Context) -> bool {
    !ctx.egui_wants_keyboard_input() && ctx.input(|input| input.key_down(Key::Space))
}

fn consume_any(ctx: &egui::Context, modifiers: Modifiers, keys: &[Key]) -> bool {
    keys.iter()
        .any(|key| ctx.input_mut(|input| input.consume_key(modifiers, *key)))
}

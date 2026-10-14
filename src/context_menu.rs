//! Compact, themed context menu for canvas objects.
//!
//! Call [`show`] with the response for the canvas item hit target. The returned action
//! is intentionally separate from document mutation so the editor can route it through
//! its normal undoable command path.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, Vec2,
};

const MENU_BG: Color32 = Color32::from_rgb(34, 35, 42);
const MENU_BORDER: Color32 = Color32::from_rgb(65, 66, 76);
const MENU_TEXT: Color32 = Color32::from_rgb(235, 236, 242);
const MENU_MUTED: Color32 = Color32::from_rgb(151, 153, 164);
const MENU_HOVER: Color32 = Color32::from_rgb(55, 56, 66);
const MENU_DANGER: Color32 = Color32::from_rgb(241, 111, 121);
const MENU_ACCENT: Color32 = Color32::from_rgb(176, 162, 255);

/// A requested canvas operation. The caller applies it and records undo history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeAction {
    Select,
    Duplicate,
    ToggleVisibility,
    ToggleLock,
    BringToFront,
    SendToBack,
    Delete,
}

/// Shows a polished context menu anchored to `item_response` and returns the selected action.
///
/// `visible` and `locked` control the inverse-action labels (Hide/Show and Lock/Unlock).
/// Pass the response associated with the node's canvas hit target, not the whole canvas.
pub fn show(item_response: &Response, visible: bool, locked: bool) -> Option<NodeAction> {
    let mut action = None;
    item_response.context_menu(|ui| {
        ui.set_min_width(226.0);
        ui.spacing_mut().item_spacing.y = 2.0;

        egui::Frame::new()
            .fill(MENU_BG)
            .stroke(Stroke::new(1.0, MENU_BORDER))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::same(7))
            .show(ui, |ui| {
                ui.set_min_width(212.0);
                ui.label(
                    egui::RichText::new("OBJECT")
                        .size(10.0)
                        .strong()
                        .color(MENU_MUTED),
                );
                ui.add_space(4.0);

                if action_row(
                    ui,
                    egui_phosphor::regular::CURSOR,
                    "Select",
                    "Enter",
                    MENU_ACCENT,
                ) {
                    action = Some(NodeAction::Select);
                }
                if action_row(
                    ui,
                    egui_phosphor::regular::COPY,
                    "Duplicate",
                    "Ctrl+D",
                    MENU_TEXT,
                ) {
                    action = Some(NodeAction::Duplicate);
                }

                separator(ui);

                let (visibility_icon, visibility_label) = if visible {
                    (egui_phosphor::regular::EYE_SLASH, "Hide")
                } else {
                    (egui_phosphor::regular::EYE, "Show")
                };
                if action_row(ui, visibility_icon, visibility_label, "⇧ Ctrl+H", MENU_TEXT) {
                    action = Some(NodeAction::ToggleVisibility);
                }

                let (lock_icon, lock_label) = if locked {
                    (egui_phosphor::regular::LOCK_OPEN, "Unlock")
                } else {
                    (egui_phosphor::regular::LOCK, "Lock")
                };
                if action_row(ui, lock_icon, lock_label, "⇧ Ctrl+L", MENU_TEXT) {
                    action = Some(NodeAction::ToggleLock);
                }

                separator(ui);

                if action_row(
                    ui,
                    egui_phosphor::regular::ARROW_LINE_UP,
                    "Bring to front",
                    "Ctrl+]",
                    MENU_TEXT,
                ) {
                    action = Some(NodeAction::BringToFront);
                }
                if action_row(
                    ui,
                    egui_phosphor::regular::ARROW_LINE_DOWN,
                    "Send to back",
                    "Ctrl+[",
                    MENU_TEXT,
                ) {
                    action = Some(NodeAction::SendToBack);
                }

                separator(ui);

                if action_row(
                    ui,
                    egui_phosphor::regular::TRASH,
                    "Delete",
                    "Backspace",
                    MENU_DANGER,
                ) {
                    action = Some(NodeAction::Delete);
                }
            });
    });
    action
}

fn action_row(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, color: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(212.0, 29.0), Sense::click());
    let response = response.on_hover_text(format!("{label} ({shortcut})"));

    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), MENU_HOVER);
    }
    let center_y = rect.center().y;
    ui.painter().text(
        Pos2::new(rect.left() + 9.0, center_y),
        Align2::LEFT_CENTER,
        icon,
        FontId::proportional(15.0),
        color,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, center_y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        color,
    );
    ui.painter().text(
        Pos2::new(rect.right() - 8.0, center_y),
        Align2::RIGHT_CENTER,
        shortcut,
        FontId::proportional(11.0),
        MENU_MUTED,
    );
    response.clicked()
}

fn separator(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(212.0, 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, MENU_BORDER),
    );
    ui.add_space(3.0);
}

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use classic_assets::{AssetId, AssetKind, AssetMetadata, AssetRecord};
use classic_comments::{Actor, CommentAnchor, CommentMessage, ReviewThread, ThreadId};
use classic_components::{ComponentDefinition, ComponentNode};
use classic_core::{DesignDocument, DesignNode, EditorCommand, NodeKind};
use classic_formats::{export_svg, import_svg, load_classic_json, save_classic_json};
use classic_geometry::{
    Alignment, Axis, Rect as WorldRect, SnapCandidate, alignment_offsets, distribution_offsets,
    snap_candidates,
};
use classic_guides::{GuideAxis, nearest_snap};
use classic_layout::{
    Action as PrototypeAction, CrossAxisAlignment, Interaction, MainAxisDistribution,
    Rect as LayoutRect, StackAxis, StackChild, StackLayout, Transition, Trigger, solve_stack,
};
use classic_raster::{RasterOptions, export_active_page_png};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, PointerButton, Pos2, Rect, Sense, Stroke,
    StrokeKind, Vec2,
};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod context_menu;
mod shortcuts;
#[cfg(target_os = "windows")]
mod tray;

const BG: Color32 = Color32::from_rgb(24, 25, 30);
const PANEL: Color32 = Color32::from_rgb(31, 32, 38);
const PANEL_ALT: Color32 = Color32::from_rgb(27, 28, 33);
const BORDER: Color32 = Color32::from_rgb(53, 54, 62);
const MUTED: Color32 = Color32::from_rgb(143, 145, 155);
const TEXT: Color32 = Color32::from_rgb(232, 233, 239);
const PURPLE: Color32 = Color32::from_rgb(144, 124, 246);
const GRID: Color32 = Color32::from_rgb(61, 62, 70);

fn main() -> eframe::Result<()> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/classic-app-icon.png"))
        .ok()
        .map(Arc::new);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Classic Studio")
            .with_inner_size([1480.0, 940.0])
            .with_min_inner_size([1060.0, 680.0])
            .with_icon(icon.unwrap_or_else(|| Arc::new(eframe::egui::IconData::default()))),
        ..Default::default()
    };
    eframe::run_native(
        "Classic Studio",
        options,
        Box::new(|cc| {
            let mut app = ClassicApp::new(cc);
            #[cfg(target_os = "windows")]
            {
                if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!(
                    "../assets/classic-app-icon.png"
                )) {
                    match tray::TrayController::new(
                        cc.egui_ctx.clone(),
                        icon.rgba,
                        icon.width,
                        icon.height,
                    ) {
                        Ok(tray) => app.tray = Some(tray),
                        Err(error) => app.status = format!("Tray unavailable: {error}"),
                    }
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Frame,
    Rectangle,
    Ellipse,
    Line,
    Text,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Move",
            Self::Frame => "Frame",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Line => "Line",
            Self::Text => "Text",
        }
    }
    fn shortcut(self) -> &'static str {
        match self {
            Self::Select => "V",
            Self::Frame => "F",
            Self::Rectangle => "R",
            Self::Ellipse => "O",
            Self::Line => "L",
            Self::Text => "T",
        }
    }
    fn kind(self) -> Option<NodeKind> {
        match self {
            Self::Select => None,
            Self::Frame => Some(NodeKind::Frame),
            Self::Rectangle => Some(NodeKind::Rectangle),
            Self::Ellipse => Some(NodeKind::Ellipse),
            Self::Line => Some(NodeKind::Line),
            Self::Text => Some(NodeKind::Text),
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Self::Select => egui_phosphor::regular::CURSOR,
            Self::Frame => egui_phosphor::regular::FRAME_CORNERS,
            Self::Rectangle => egui_phosphor::regular::RECTANGLE,
            Self::Ellipse => egui_phosphor::regular::CIRCLE,
            Self::Line => egui_phosphor::regular::LINE_SEGMENT,
            Self::Text => egui_phosphor::regular::TEXT_T,
        }
    }
}

struct ClassicApp {
    document: DesignDocument,
    selected: Option<String>,
    selected_ids: BTreeSet<String>,
    tool: Tool,
    zoom: f32,
    pan: Vec2,
    show_grid: bool,
    undo: Vec<DesignDocument>,
    redo: Vec<DesignDocument>,
    active_tab: InspectorTab,
    sidebar_tab: SidebarTab,
    layer_filter: String,
    comment_draft: String,
    prototype_target_id: Option<String>,
    layout_gap: f32,
    contrast_background: Color32,
    status: String,
    save_path: Option<PathBuf>,
    is_dirty: bool,
    recovery_path: PathBuf,
    last_recovery_save: Instant,
    drag_snapshot: Option<DesignDocument>,
    resize_origin: Option<(f32, f32)>,
    present: bool,
    show_project_hub: bool,
    presentation_frame_id: Option<String>,
    show_shortcuts: bool,
    snap_guides: Vec<SnapCandidate>,
    creation_origin: Option<Vec2>,
    canvas_view_size: Vec2,
    asset_textures: HashMap<String, egui::TextureHandle>,
    #[cfg(target_os = "windows")]
    tray: Option<tray::TrayController>,
    #[cfg(target_os = "windows")]
    quit_from_tray: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Design,
    Prototype,
    Dev,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarTab {
    Layers,
    Assets,
    Comments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceTab {
    Layers,
    Canvas,
    Inspector,
}

struct EditorTabViewer<'a> {
    app: &'a mut ClassicApp,
}

fn initial_dock_state() -> DockState<WorkspaceTab> {
    let mut dock_state = DockState::new(vec![WorkspaceTab::Canvas]);
    let surface = dock_state.main_surface_mut();
    let [canvas, _layers] = surface.split_left(NodeIndex::root(), 0.22, vec![WorkspaceTab::Layers]);
    let _ = surface.split_right(canvas, 0.26, vec![WorkspaceTab::Inspector]);
    dock_state
}

impl ClassicApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = PANEL;
        style.visuals.panel_fill = PANEL;
        style.visuals.extreme_bg_color = PANEL_ALT;
        style.visuals.faint_bg_color = Color32::from_rgb(39, 40, 46);
        style.visuals.widgets.noninteractive.bg_fill = PANEL;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 43, 50);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(57, 58, 67);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 61, 106);
        style.visuals.selection.bg_fill = Color32::from_rgb(72, 63, 113);
        style.visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(183, 169, 255));
        style.spacing.item_spacing = Vec2::new(7.0, 7.0);
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);

        let document = DesignDocument::starter();
        let recovery_path = recovery_file_path();
        let selected = document.active_page().and_then(|page| {
            page.nodes
                .iter()
                .find(|node| node.name == "Hero title")
                .map(|node| node.id.clone())
        });
        let selected_ids = selected.iter().cloned().collect();
        Self {
            document,
            selected,
            selected_ids,
            tool: Tool::Select,
            zoom: 0.82,
            pan: Vec2::ZERO,
            show_grid: true,
            undo: Vec::new(),
            redo: Vec::new(),
            active_tab: InspectorTab::Design,
            sidebar_tab: SidebarTab::Layers,
            layer_filter: String::new(),
            comment_draft: String::new(),
            prototype_target_id: None,
            layout_gap: 12.0,
            contrast_background: Color32::WHITE,
            status: "All changes saved".into(),
            save_path: None,
            is_dirty: false,
            recovery_path,
            last_recovery_save: Instant::now(),
            drag_snapshot: None,
            resize_origin: None,
            present: false,
            show_project_hub: true,
            presentation_frame_id: None,
            show_shortcuts: false,
            snap_guides: Vec::new(),
            creation_origin: None,
            canvas_view_size: Vec2::ZERO,
            asset_textures: HashMap::new(),
            #[cfg(target_os = "windows")]
            tray: None,
            #[cfg(target_os = "windows")]
            quit_from_tray: false,
        }
    }

    fn project_hub(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.available_rect_before_wrap();
        let width = bounds.width().min(700.0);
        ui.add_space(((bounds.height() - 460.0) * 0.5).max(20.0));
        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
                egui::Frame::new()
                    .fill(Color32::from_rgb(31, 32, 38))
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(30))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("CLASSIC STUDIO")
                                    .size(13.0)
                                    .strong()
                                    .color(PURPLE),
                            );
                            ui.add_space(10.0);
                            ui.heading("Start a design project");
                            ui.label(
                                egui::RichText::new(
                                    "A local-first Rust design workspace. Create a multi-page .classic file or reopen a saved project.",
                                )
                                .color(MUTED),
                            );
                            ui.add_space(26.0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("New design file").strong());
                                    ui.label(
                                        egui::RichText::new(
                                            "Open the starter canvas with pages, assets, comments, and prototype tools.",
                                        )
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                    if ui
                                        .add_sized([250.0, 36.0], egui::Button::new("Create canvas"))
                                        .on_hover_text("Open a new local multi-page design document")
                                        .clicked()
                                    {
                                        self.show_project_hub = false;
                                        self.status = "New local design project".into();
                                    }
                                });
                                ui.add_space(22.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("Open saved project").strong());
                                    ui.label(
                                        egui::RichText::new(
                                            "Open a .classic project or import a scoped SVG file from your computer.",
                                        )
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                    if ui
                                        .add_sized([250.0, 36.0], egui::Button::new("Open project"))
                                        .on_hover_text("Choose a .classic project or SVG file")
                                        .clicked()
                                    {
                                        self.open();
                                        self.show_project_hub = false;
                                    }
                                });
                            });
                            ui.add_space(28.0);
                            ui.separator();
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Local-first").color(PURPLE));
                                ui.label(egui::RichText::new(".classic stores pages, layers, assets, comments, and guides").color(MUTED));
                            });
                            if self.recovery_path.is_file()
                                && ui
                                    .button("Recover latest local snapshot")
                                    .on_hover_text("Restore the latest automatic recovery snapshot")
                                    .clicked()
                            {
                                self.recover_local_snapshot();
                                self.show_project_hub = false;
                            }
                        });
                    });
        });
    }

    fn active_nodes(&self) -> &[DesignNode] {
        self.document
            .active_page()
            .map(|page| page.nodes.as_slice())
            .unwrap_or(&[])
    }
    fn active_nodes_mut(&mut self) -> &mut Vec<DesignNode> {
        let active = self.document.active_page_id.clone();
        &mut self
            .document
            .pages
            .iter_mut()
            .find(|page| page.id == active)
            .expect("active page exists")
            .nodes
    }
    fn is_selected(&self, node_id: &str) -> bool {
        self.selected_ids.contains(node_id)
    }
    fn select_only(&mut self, node_id: impl Into<String>) {
        let node_id = node_id.into();
        self.selected_ids.clear();
        self.selected_ids.insert(node_id.clone());
        self.selected = Some(node_id);
    }
    fn toggle_selection(&mut self, node_id: impl Into<String>) {
        let node_id = node_id.into();
        if !self.selected_ids.insert(node_id.clone()) {
            self.selected_ids.remove(&node_id);
        }
        self.selected = self
            .selected
            .clone()
            .filter(|primary| self.selected_ids.contains(primary))
            .or_else(|| self.selected_ids.iter().next().cloned());
    }
    fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_ids.clear();
    }
    fn selected_node_ids(&self) -> Vec<String> {
        self.active_nodes()
            .iter()
            .filter(|node| self.selected_ids.contains(&node.id))
            .map(|node| node.id.clone())
            .collect()
    }
    fn canvas_center_in_document(&self) -> Vec2 {
        (self.canvas_view_size * 0.5 - self.pan) / self.zoom
    }
    fn add_guide(&mut self, axis: GuideAxis) {
        let center = self.canvas_center_in_document();
        let position = match axis {
            GuideAxis::Horizontal => center.y,
            GuideAxis::Vertical => center.x,
        };
        let before = self.document.clone();
        match self.document.guides.add(axis, position) {
            Ok(_) => {
                self.push_undo_snapshot(before);
                self.status = "Added ruler guide at the canvas center".into();
            }
            Err(error) => self.status = format!("Could not add ruler guide: {error}"),
        }
    }
    fn import_asset(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Supported assets",
                &[
                    "png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "ttf", "otf", "woff",
                    "woff2", "mp4", "webm", "mov", "mp3", "wav", "ogg", "pdf",
                ],
            )
            .pick_file()
        else {
            return;
        };
        self.import_asset_path(path);
    }
    fn import_asset_path(&mut self, path: PathBuf) {
        let mut metadata = AssetMetadata::new(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Imported asset"),
            media_type_for_path(&path),
        );
        metadata.byte_len = std::fs::metadata(&path).ok().map(|file| file.len());
        let record = AssetRecord::new(
            asset_kind_for_path(&path),
            metadata,
            path.to_string_lossy().into_owned(),
        );
        let before = self.document.clone();
        match self.document.assets.insert(record) {
            Ok(()) => {
                self.push_undo_snapshot(before);
                self.status = "Imported local asset reference".into();
            }
            Err(error) => self.status = format!("Could not import asset: {error}"),
        }
    }
    fn remove_asset(&mut self, id: AssetId) {
        let before = self.document.clone();
        match self.document.assets.remove(&id) {
            Ok(_) => {
                self.asset_textures.remove(id.as_str());
                self.push_undo_snapshot(before);
                self.status = "Removed asset from this document".into();
            }
            Err(error) => self.status = format!("Could not remove asset: {error}"),
        }
    }
    fn texture_for_asset(
        &mut self,
        ctx: &egui::Context,
        asset_id: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.asset_textures.get(asset_id) {
            return Some(texture.clone());
        }
        let asset = self
            .document
            .assets
            .iter()
            .find(|asset| asset.id.as_str() == asset_id)?;
        if asset.kind != AssetKind::RasterImage {
            return None;
        }
        let decoded = image::open(&asset.source_uri).ok()?.into_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        if size[0] == 0 || size[1] == 0 {
            return None;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        let texture = ctx.load_texture(
            format!("classic-asset-{asset_id}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.asset_textures
            .insert(asset_id.to_owned(), texture.clone());
        Some(texture)
    }
    fn place_asset_on_canvas(&mut self, asset_id: AssetId) {
        let Some(asset) = self.document.assets.get(&asset_id).cloned() else {
            self.status = "The selected asset no longer exists".into();
            return;
        };
        let center = self.canvas_center_in_document();
        let mut node = DesignNode::new(NodeKind::Image, center.x - 160.0, center.y - 110.0);
        node.id = format!("node-{}", uuid::Uuid::new_v4());
        node.name = asset.metadata.name;
        node.asset_id = Some(asset_id.as_str().to_owned());
        let id = node.id.clone();
        self.execute(EditorCommand::AddNode {
            page_id: self.document.active_page_id.clone(),
            node,
        });
        self.select_only(id);
        self.status = "Placed asset on canvas".into();
    }
    fn add_comment(&mut self) {
        let body = self.comment_draft.trim().to_owned();
        if body.is_empty() {
            self.status = "Write a comment before adding it".into();
            return;
        }
        let actor = local_comment_actor();
        let timestamp = current_time_ms();
        let anchor = self
            .selected
            .as_ref()
            .map(|node_id| CommentAnchor::node(self.document.active_page_id.clone(), node_id))
            .unwrap_or_else(|| {
                let point = self.canvas_center_in_document();
                CommentAnchor::point(self.document.active_page_id.clone(), point.x, point.y)
            });
        let message = CommentMessage::new(actor.clone(), body, timestamp);
        let thread = ReviewThread::new(self.document.id.clone(), anchor, actor, message);
        let before = self.document.clone();
        match self.document.comments.insert(thread) {
            Ok(()) => {
                self.push_undo_snapshot(before);
                self.comment_draft.clear();
                self.status = "Added pinned review comment".into();
            }
            Err(error) => self.status = format!("Could not add comment: {error}"),
        }
    }
    fn toggle_comment_resolution(&mut self, id: ThreadId) {
        let before = self.document.clone();
        let timestamp = current_time_ms();
        let result = self
            .document
            .comments
            .get_mut(&id)
            .ok_or_else(|| "Comment thread no longer exists".to_owned())
            .and_then(|thread| {
                if thread.is_resolved() {
                    thread.reopen(timestamp).map_err(|error| error.to_string())
                } else {
                    thread
                        .resolve(local_comment_actor(), timestamp)
                        .map_err(|error| error.to_string())
                }
            });
        match result {
            Ok(()) => {
                self.push_undo_snapshot(before);
                self.status = "Updated comment resolution".into();
            }
            Err(error) => self.status = format!("Could not update comment: {error}"),
        }
    }
    fn remove_comment(&mut self, id: ThreadId) {
        let before = self.document.clone();
        let previous = self.document.comments.threads.len();
        self.document
            .comments
            .threads
            .retain(|thread| thread.id != id);
        if self.document.comments.threads.len() == previous {
            self.status = "Comment thread no longer exists".into();
            return;
        }
        self.push_undo_snapshot(before);
        self.status = "Deleted review comment".into();
    }
    fn create_component_from_selection(&mut self) {
        let Some(node) = self
            .selected
            .as_ref()
            .and_then(|id| self.active_nodes().iter().find(|node| &node.id == id))
            .cloned()
        else {
            self.status = "Select one layer before creating a component".into();
            return;
        };
        let definition = ComponentDefinition::new(
            node.name.clone(),
            ComponentNode::new(node.name, node_kind_label(&node.kind)),
        );
        let before = self.document.clone();
        match self.document.components.add_definition(definition) {
            Ok(()) => {
                self.push_undo_snapshot(before);
                self.status = "Created local component definition".into();
            }
            Err(error) => self.status = format!("Could not create component: {error}"),
        }
    }
    fn place_component_instance(&mut self, component_id: &str) {
        let Some(definition) = self.document.components.definition(component_id).cloned() else {
            self.status = "Component definition no longer exists".into();
            return;
        };
        let center = self.canvas_center_in_document();
        let mut node = DesignNode::new(NodeKind::Frame, center.x - 120.0, center.y - 80.0);
        node.id = format!("node-{}", uuid::Uuid::new_v4());
        node.name = format!("{} instance", definition.name);
        node.width = 240.0;
        node.height = 160.0;
        node.component_id = Some(definition.id);
        let id = node.id.clone();
        self.execute(EditorCommand::AddNode {
            page_id: self.document.active_page_id.clone(),
            node,
        });
        self.select_only(id);
        self.status = "Placed component instance".into();
    }
    fn add_navigation_interaction(&mut self, target_frame_id: String) {
        let Some(source_node_id) = self.selected.clone() else {
            self.status = "Select a layer to create a prototype interaction".into();
            return;
        };
        if source_node_id == target_frame_id {
            self.status = "Choose a different destination frame".into();
            return;
        }
        if !self
            .active_nodes()
            .iter()
            .any(|node| node.id == target_frame_id && node.kind == NodeKind::Frame)
        {
            self.status = "Choose a frame on the active page as the destination".into();
            return;
        }
        let before = self.document.clone();
        self.document.prototype.add_interaction(Interaction {
            id: format!("interaction-{}", uuid::Uuid::new_v4()),
            source_node_id,
            trigger: Trigger::OnClick,
            action: PrototypeAction::NavigateTo {
                frame_id: target_frame_id,
            },
            transition: Transition::Dissolve { duration_ms: 200 },
        });
        self.push_undo_snapshot(before);
        self.status = "Added click-to-navigate prototype interaction".into();
    }
    fn remove_navigation_interaction(&mut self, id: &str) {
        let before = self.document.clone();
        let previous = self.document.prototype.interactions.len();
        self.document
            .prototype
            .interactions
            .retain(|interaction| interaction.id != id);
        if self.document.prototype.interactions.len() == previous {
            self.status = "Prototype interaction no longer exists".into();
            return;
        }
        self.push_undo_snapshot(before);
        self.status = "Removed prototype interaction".into();
    }
    fn align_selection(&mut self, alignment: Alignment) {
        let selected: Vec<(String, WorldRect)> = self
            .active_nodes()
            .iter()
            .filter(|node| self.selected_ids.contains(&node.id) && !node.locked)
            .map(|node| {
                (
                    node.id.clone(),
                    WorldRect::new(node.x, node.y, node.width, node.height),
                )
            })
            .collect();
        if selected.len() < 2 {
            self.status = "Select at least two unlocked layers to align".into();
            return;
        }
        let Some(offsets) = alignment_offsets(
            &selected.iter().map(|(_, rect)| *rect).collect::<Vec<_>>(),
            alignment,
        ) else {
            self.status = "Could not align invalid layer geometry".into();
            return;
        };
        self.snapshot();
        for ((id, _), offset) in selected.iter().zip(offsets) {
            if let Some(node) = self
                .active_nodes_mut()
                .iter_mut()
                .find(|node| node.id == *id)
            {
                node.x += offset.x;
                node.y += offset.y;
            }
        }
        self.status = "Aligned selection".into();
    }
    fn distribute_selection(&mut self, axis: Axis) {
        let selected: Vec<(String, WorldRect)> = self
            .active_nodes()
            .iter()
            .filter(|node| self.selected_ids.contains(&node.id) && !node.locked)
            .map(|node| {
                (
                    node.id.clone(),
                    WorldRect::new(node.x, node.y, node.width, node.height),
                )
            })
            .collect();
        if selected.len() < 3 {
            self.status = "Select at least three unlocked layers to distribute".into();
            return;
        }
        let Some(offsets) = distribution_offsets(
            &selected.iter().map(|(_, rect)| *rect).collect::<Vec<_>>(),
            axis,
        ) else {
            self.status = "Could not distribute invalid layer geometry".into();
            return;
        };
        self.snapshot();
        for ((id, _), offset) in selected.iter().zip(offsets) {
            if let Some(node) = self
                .active_nodes_mut()
                .iter_mut()
                .find(|node| node.id == *id)
            {
                node.x += offset.x;
                node.y += offset.y;
            }
        }
        self.status = "Distributed selection".into();
    }
    fn stack_selection(&mut self, axis: StackAxis) {
        let mut selected: Vec<_> = self
            .active_nodes()
            .iter()
            .filter(|node| self.selected_ids.contains(&node.id) && !node.locked)
            .cloned()
            .collect();
        if selected.len() < 2 {
            self.status = "Select at least two unlocked layers to apply stack layout".into();
            return;
        }
        selected.sort_by(|left, right| {
            let left_position = if axis == StackAxis::Horizontal {
                left.x
            } else {
                left.y
            };
            let right_position = if axis == StackAxis::Horizontal {
                right.x
            } else {
                right.y
            };
            left_position
                .total_cmp(&right_position)
                .then_with(|| left.id.cmp(&right.id))
        });
        let left = selected
            .iter()
            .map(|node| node.x)
            .fold(f32::INFINITY, f32::min);
        let top = selected
            .iter()
            .map(|node| node.y)
            .fold(f32::INFINITY, f32::min);
        let max_width = selected.iter().map(|node| node.width).fold(0.0, f32::max);
        let max_height = selected.iter().map(|node| node.height).fold(0.0, f32::max);
        let main_extent: f32 = selected
            .iter()
            .map(|node| {
                if axis == StackAxis::Horizontal {
                    node.width
                } else {
                    node.height
                }
            })
            .sum::<f32>()
            + self.layout_gap.max(0.0) * (selected.len() - 1) as f32;
        let container = match axis {
            StackAxis::Horizontal => LayoutRect::new(left, top, main_extent, max_height),
            StackAxis::Vertical => LayoutRect::new(left, top, max_width, main_extent),
        };
        let children: Vec<_> = selected
            .iter()
            .enumerate()
            .map(|(index, node)| StackChild::fixed(index as u64, node.width, node.height))
            .collect();
        let positioned = solve_stack(
            container,
            StackLayout {
                axis,
                gap: self.layout_gap,
                padding: Default::default(),
                cross_alignment: CrossAxisAlignment::Start,
                distribution: MainAxisDistribution::Start,
            },
            &children,
        );
        self.snapshot();
        for position in positioned {
            let node_id = &selected[position.id as usize].id;
            if let Some(node) = self
                .active_nodes_mut()
                .iter_mut()
                .find(|node| node.id == *node_id)
            {
                node.x = position.rect.x;
                node.y = position.rect.y;
                node.width = position.rect.width;
                node.height = position.rect.height;
            }
        }
        self.status = "Applied stack auto-layout to selection".into();
    }
    fn snapshot(&mut self) {
        self.push_undo_snapshot(self.document.clone());
        self.status = "Unsaved changes".into();
    }
    fn push_undo_snapshot(&mut self, before: DesignDocument) {
        self.undo.push(before);
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.is_dirty = true;
    }
    fn execute(&mut self, command: EditorCommand) {
        let before = self.document.clone();
        match self.document.apply(command) {
            Ok(()) => {
                self.undo.push(before);
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
                self.redo.clear();
                self.status = "Unsaved changes".into();
                self.is_dirty = true;
            }
            Err(message) => self.status = message,
        }
    }
    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo
                .push(std::mem::replace(&mut self.document, previous));
            self.status = "Undid last change".into();
            self.is_dirty = true;
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.document, next));
            self.status = "Redid change".into();
            self.is_dirty = true;
        }
    }
    fn save(&mut self) {
        let path = self.save_path.clone().or_else(|| {
            rfd::FileDialog::new()
                .add_filter("Classic project", &["classic", "json"])
                .set_file_name(format!("{}.classic", self.document.name))
                .save_file()
        });
        let Some(path) = path else {
            return;
        };
        match save_classic_json(&path, &self.document).map_err(|e| e.to_string()) {
            Ok(()) => {
                self.status = "All changes saved".into();
                self.save_path = Some(path);
                self.is_dirty = false;
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }
    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Classic or SVG", &["classic", "json", "svg"])
            .pick_file()
        else {
            return;
        };
        let is_svg = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
        let document_result = if is_svg {
            std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|source| {
                    let name = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("Imported SVG");
                    import_svg(&source, name).map_err(|error| error.to_string())
                })
        } else {
            load_classic_json(&path).map_err(|error| error.to_string())
        };
        match document_result {
            Ok(document) => {
                self.undo
                    .push(std::mem::replace(&mut self.document, document));
                self.redo.clear();
                self.save_path = (!is_svg).then(|| path.clone());
                self.clear_selection();
                self.asset_textures.clear();
                self.is_dirty = false;
                self.status = format!(
                    "{} {}",
                    if is_svg { "Imported" } else { "Opened" },
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                );
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }
    fn autosave_recovery_if_due(&mut self) {
        if !self.is_dirty || self.last_recovery_save.elapsed() < Duration::from_secs(30) {
            return;
        }
        if let Some(parent) = self.recovery_path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status = format!("Recovery folder unavailable: {error}");
                self.last_recovery_save = Instant::now();
                return;
            }
        }
        match save_classic_json(&self.recovery_path, &self.document) {
            Ok(()) => self.status = "Saved local recovery snapshot".into(),
            Err(error) => self.status = format!("Recovery save failed: {error}"),
        }
        self.last_recovery_save = Instant::now();
    }
    fn recover_local_snapshot(&mut self) {
        match load_classic_json(&self.recovery_path) {
            Ok(document) => {
                self.undo
                    .push(std::mem::replace(&mut self.document, document));
                self.redo.clear();
                self.clear_selection();
                self.asset_textures.clear();
                self.is_dirty = true;
                self.status = "Recovered local snapshot; save to keep it".into();
            }
            Err(error) => self.status = format!("Recovery failed: {error}"),
        }
    }
    fn export_svg(&mut self) {
        let Some(page_id) = self.document.active_page().map(|page| page.id.clone()) else {
            self.status = "There is no active page to export".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG vector image", &["svg"])
            .set_file_name(format!("{}.svg", self.document.name))
            .save_file()
        else {
            return;
        };
        match export_svg(&self.document, &page_id)
            .map_err(|error| error.to_string())
            .and_then(|source| std::fs::write(&path, source).map_err(|error| error.to_string()))
        {
            Ok(()) => {
                self.status = format!(
                    "Exported {}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("SVG")
                )
            }
            Err(error) => self.status = format!("SVG export failed: {error}"),
        }
    }
    fn export_png(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name(format!("{}.png", self.document.name))
            .save_file()
        else {
            return;
        };
        match export_active_page_png(&self.document, &path, RasterOptions::default()) {
            Ok(()) => {
                self.status = format!(
                    "Exported {}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("PNG")
                )
            }
            Err(error) => self.status = format!("PNG export failed: {error}"),
        }
    }
    fn fit_canvas(&mut self) {
        let bounds: Option<(f32, f32, f32, f32)> = self
            .active_nodes()
            .iter()
            .filter(|node| node.visible)
            .fold(None, |bounds, node| {
                let next = (node.x, node.y, node.x + node.width, node.y + node.height);
                Some(match bounds {
                    Some((left, top, right, bottom)) => (
                        left.min(next.0),
                        top.min(next.1),
                        right.max(next.2),
                        bottom.max(next.3),
                    ),
                    None => next,
                })
            });
        let Some((left, top, right, bottom)) = bounds else {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
            return;
        };
        let width = (right - left).max(1.0);
        let height = (bottom - top).max(1.0);
        self.zoom = ((self.canvas_view_size.x - 80.0) / width)
            .min((self.canvas_view_size.y - 80.0) / height)
            .clamp(0.2, 2.5);
        self.pan = Vec2::new(
            (self.canvas_view_size.x - width * self.zoom) * 0.5 - left * self.zoom,
            (self.canvas_view_size.y - height * self.zoom) * 0.5 - top * self.zoom,
        );
    }
    fn handle_shortcut(&mut self, action: shortcuts::ShortcutAction) {
        use shortcuts::{ShortcutAction as Action, ToolShortcut};
        match action {
            Action::SelectTool(tool) => {
                self.tool = match tool {
                    ToolShortcut::Select => Tool::Select,
                    ToolShortcut::Frame => Tool::Frame,
                    ToolShortcut::Rectangle => Tool::Rectangle,
                    ToolShortcut::Ellipse => Tool::Ellipse,
                    ToolShortcut::Line => Tool::Line,
                    ToolShortcut::Text => Tool::Text,
                };
            }
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::DeleteSelection => {
                let node_ids = self.selected_node_ids();
                for node_id in node_ids {
                    self.execute(EditorCommand::DeleteNode {
                        page_id: self.document.active_page_id.clone(),
                        node_id,
                    });
                }
                self.clear_selection();
            }
            Action::SelectAll => {
                self.selected_ids = self
                    .active_nodes()
                    .iter()
                    .filter(|node| node.visible)
                    .map(|node| node.id.clone())
                    .collect();
                self.selected = self.selected_ids.iter().next().cloned();
                self.status = format!("Selected {} layer(s)", self.selected_ids.len());
            }
            Action::DuplicateSelection => {
                let source_ids = self.selected_node_ids();
                let mut duplicates = Vec::new();
                for node in self
                    .active_nodes()
                    .iter()
                    .filter(|node| source_ids.contains(&node.id))
                    .cloned()
                {
                    let mut duplicate = node;
                    duplicate.id = format!("node-{}", uuid::Uuid::new_v4());
                    duplicate.name.push_str(" copy");
                    duplicate.x += 24.0;
                    duplicate.y += 24.0;
                    duplicates.push(duplicate);
                }
                if !duplicates.is_empty() {
                    self.snapshot();
                    self.selected_ids = duplicates.iter().map(|node| node.id.clone()).collect();
                    self.selected = self.selected_ids.iter().next().cloned();
                    self.active_nodes_mut().extend(duplicates);
                }
            }
            Action::Nudge { x, y } => {
                let selected_ids = self.selected_ids.clone();
                if !selected_ids.is_empty() {
                    self.snapshot();
                    for node in self.active_nodes_mut() {
                        if selected_ids.contains(&node.id) && !node.locked {
                            node.x += x as f32;
                            node.y += y as f32;
                        }
                    }
                }
            }
            Action::ZoomIn => self.zoom = (self.zoom + 0.1).min(2.5),
            Action::ZoomOut => self.zoom = (self.zoom - 0.1).max(0.2),
            Action::ZoomToFit => self.fit_canvas(),
            Action::Escape => {
                self.tool = Tool::Select;
                self.clear_selection();
            }
        }
    }
    fn apply_context_action(&mut self, node_id: &str, action: context_menu::NodeAction) {
        use context_menu::NodeAction;
        let Some(index) = self
            .active_nodes()
            .iter()
            .position(|node| node.id == node_id)
        else {
            return;
        };
        match action {
            NodeAction::Select => self.select_only(node_id),
            NodeAction::Duplicate => {
                let mut duplicate = self.active_nodes()[index].clone();
                duplicate.id = format!("node-{}", uuid::Uuid::new_v4());
                duplicate.name.push_str(" copy");
                duplicate.x += 24.0;
                duplicate.y += 24.0;
                self.select_only(duplicate.id.clone());
                self.execute(EditorCommand::AddNode {
                    page_id: self.document.active_page_id.clone(),
                    node: duplicate,
                });
            }
            NodeAction::ToggleVisibility | NodeAction::ToggleLock => {
                self.snapshot();
                let node = &mut self.active_nodes_mut()[index];
                if action == NodeAction::ToggleVisibility {
                    node.visible = !node.visible;
                } else {
                    node.locked = !node.locked;
                }
            }
            NodeAction::BringToFront | NodeAction::SendToBack => {
                self.snapshot();
                let nodes = self.active_nodes_mut();
                let node = nodes.remove(index);
                if action == NodeAction::BringToFront {
                    nodes.push(node);
                } else {
                    nodes.insert(0, node);
                }
            }
            NodeAction::Delete => {
                self.execute(EditorCommand::DeleteNode {
                    page_id: self.document.active_page_id.clone(),
                    node_id: node_id.to_owned(),
                });
                self.selected_ids.remove(node_id);
                self.selected = self.selected_ids.iter().next().cloned();
            }
        }
    }
    fn create_node(&mut self, kind: NodeKind, position: Pos2) {
        let mut node = DesignNode::new(
            kind,
            ((position.x - self.pan.x) / self.zoom).max(0.0),
            ((position.y - self.pan.y) / self.zoom).max(0.0),
        );
        node.id = format!("node-{}", uuid::Uuid::new_v4());
        let id = node.id.clone();
        self.execute(EditorCommand::AddNode {
            page_id: self.document.active_page_id.clone(),
            node,
        });
        self.select_only(id);
        self.tool = Tool::Select;
    }
    fn create_node_from_drag(&mut self, kind: NodeKind, start: Vec2, end: Vec2, constrain: bool) {
        if matches!(kind, NodeKind::Text) {
            self.create_node(kind, Pos2::new(start.x, start.y));
            return;
        }
        let start = (start - self.pan) / self.zoom;
        let end = (end - self.pan) / self.zoom;
        let mut x = start.x.min(end.x);
        let mut y = start.y.min(end.y);
        let mut width = (end.x - start.x).abs();
        let mut height = (end.y - start.y).abs();
        if constrain && !matches!(kind, NodeKind::Line) {
            let size = width.max(height);
            if end.x < start.x {
                x = start.x - size;
            }
            if end.y < start.y {
                y = start.y - size;
            }
            width = size;
            height = size;
        }
        if matches!(kind, NodeKind::Line) {
            width = width.max(1.0);
            height = height.max(1.0);
        } else if width < 4.0 || height < 4.0 {
            self.create_node(kind, Pos2::new(start.x, start.y));
            return;
        }
        let mut node = DesignNode::new(kind, x, y);
        node.id = format!("node-{}", uuid::Uuid::new_v4());
        node.width = width;
        node.height = height;
        let id = node.id.clone();
        self.execute(EditorCommand::AddNode {
            page_id: self.document.active_page_id.clone(),
            node,
        });
        self.select_only(id);
        self.tool = Tool::Select;
    }
    fn node_at_screen(&self, node: &DesignNode, canvas: Rect) -> Rect {
        let origin = canvas.min + self.pan + Vec2::new(node.x, node.y) * self.zoom;
        Rect::from_min_size(
            origin,
            Vec2::new(node.width.max(1.0), node.height.max(1.0)) * self.zoom,
        )
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("topbar")
            .default_size(50.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(25, 26, 31))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(7.0);
                    brand_mark(ui, 29.0);
                    ui.label(
                        egui::RichText::new("classic")
                            .strong()
                            .size(16.0)
                            .color(TEXT),
                    );
                    small_chip(ui, "Studio⌄");
                    separator(ui);
                    let mut name = self.document.name.clone();
                    let name_edit = ui.add(
                        egui::TextEdit::singleline(&mut name)
                            .desired_width(155.0)
                            .frame(egui::Frame::NONE)
                            .text_color(TEXT),
                    );
                    if name_edit.changed() {
                        self.document.name = name;
                        self.status = "Unsaved changes".into();
                        self.is_dirty = true;
                    }
                    if ui.button("⌄").clicked() {
                        self.save();
                    }
                    ui.add_space(5.0);
                    let status_color = if self.status == "All changes saved" {
                        Color32::from_rgb(113, 195, 151)
                    } else {
                        MUTED
                    };
                    ui.colored_label(status_color, "●");
                    ui.label(egui::RichText::new(&self.status).size(10.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new("▶  Present")
                                    .fill(PURPLE)
                                    .stroke(Stroke::new(0.0, PURPLE)),
                            )
                            .clicked()
                        {
                            self.present = !self.present;
                            if self.present {
                                self.presentation_frame_id = self
                                    .selected
                                    .as_ref()
                                    .and_then(|id| {
                                        self.active_nodes()
                                            .iter()
                                            .find(|node| {
                                                &node.id == id && node.kind == NodeKind::Frame
                                            })
                                            .map(|node| node.id.clone())
                                    })
                                    .or_else(|| {
                                        self.active_nodes()
                                            .iter()
                                            .find(|node| node.kind == NodeKind::Frame)
                                            .map(|node| node.id.clone())
                                    });
                            }
                            self.status = if self.present {
                                "Presentation preview".into()
                            } else {
                                "Editor view".into()
                            };
                        }
                        if ui.button("↗  Share").clicked() {
                            self.status =
                                "Cloud sharing is not available in this local build".into();
                        }
                        ui.label(
                            egui::RichText::new("A   M").color(Color32::from_rgb(216, 166, 156)),
                        );
                        if ui.button("Open").clicked() {
                            self.open();
                        }
                        if ui
                            .add_enabled(self.recovery_path.is_file(), egui::Button::new("Recover"))
                            .on_hover_text("Recover the latest local autosave snapshot")
                            .clicked()
                        {
                            self.recover_local_snapshot();
                        }
                        if ui.button("Save").clicked() {
                            self.save();
                        }
                        separator(ui);
                        if ui
                            .add_enabled(!self.redo.is_empty(), egui::Button::new("↷"))
                            .on_hover_text("Redo (Ctrl+Shift+Z)")
                            .clicked()
                        {
                            self.redo();
                        }
                        if ui
                            .add_enabled(!self.undo.is_empty(), egui::Button::new("↶"))
                            .on_hover_text("Undo (Ctrl+Z)")
                            .clicked()
                        {
                            self.undo();
                        }
                    });
                });
            });
    }

    fn comments_pane(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Review comments").strong());
        ui.label(
            egui::RichText::new(
                "Add a comment to the selected layer, or to the current canvas location when nothing is selected.",
            )
            .size(10.0)
            .color(MUTED),
        );
        ui.add(
            egui::TextEdit::multiline(&mut self.comment_draft)
                .hint_text("Write a comment…")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Add comment").clicked() {
            self.add_comment();
        }
        ui.separator();
        let page_id = self.document.active_page_id.clone();
        let threads: Vec<_> = self
            .document
            .comments
            .threads
            .iter()
            .filter(|thread| thread.anchor.page_id == page_id)
            .cloned()
            .collect();
        if threads.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("☏").size(24.0).color(PURPLE));
                ui.label("No comments on this page");
            });
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for thread in threads {
                let state = if thread.is_resolved() {
                    "Resolved"
                } else {
                    "Open"
                };
                let target = thread
                    .anchor
                    .node_id
                    .as_deref()
                    .map(|id| format!("Layer {id}"))
                    .unwrap_or_else(|| "Canvas point".into());
                let message = thread
                    .messages
                    .first()
                    .map(|message| message.body.as_str())
                    .unwrap_or("");
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(state).size(10.0).color(
                            if thread.is_resolved() {
                                Color32::from_rgb(113, 195, 151)
                            } else {
                                PURPLE
                            },
                        ));
                        ui.label(egui::RichText::new(target).size(10.0).color(MUTED));
                    });
                    ui.label(message);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} message(s)",
                            thread.created_by.display_name,
                            thread.messages.len()
                        ))
                        .size(10.0)
                        .color(MUTED),
                    );
                    ui.horizontal(|ui| {
                        let action = if thread.is_resolved() {
                            "Reopen"
                        } else {
                            "Resolve"
                        };
                        if ui.button(action).clicked() {
                            self.toggle_comment_resolution(thread.id.clone());
                        }
                        if ui.small_button("Delete").clicked() {
                            self.remove_comment(thread.id.clone());
                        }
                    });
                });
            }
        });
    }

    fn layers_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.sidebar_tab == SidebarTab::Layers, "Layers")
                .clicked()
            {
                self.sidebar_tab = SidebarTab::Layers;
            }
            if ui
                .selectable_label(self.sidebar_tab == SidebarTab::Assets, "Assets")
                .clicked()
            {
                self.sidebar_tab = SidebarTab::Assets;
            }
            if ui
                .selectable_label(self.sidebar_tab == SidebarTab::Comments, "Comments")
                .clicked()
            {
                self.sidebar_tab = SidebarTab::Comments;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let _ = ui.button("☷");
            });
        });
        ui.separator();
        if self.sidebar_tab == SidebarTab::Assets {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Local assets").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Import")
                        .on_hover_text("Import a local asset reference")
                        .clicked()
                    {
                        self.import_asset();
                    }
                });
            });
            ui.add_space(4.0);
            if self.document.assets.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new("◇").size(24.0).color(PURPLE));
                    ui.label("No assets yet");
                    ui.label(
                        egui::RichText::new("Import local images, media, fonts, or documents.")
                            .color(MUTED),
                    );
                });
            } else {
                let assets: Vec<_> = self.document.assets.iter().cloned().collect();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for asset in assets {
                        let asset_id = asset.id.clone();
                        let card = ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(asset_kind_icon(asset.kind));
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&asset.metadata.name).strong());
                                    ui.label(
                                        egui::RichText::new(&asset.metadata.media_type)
                                            .size(10.0)
                                            .color(MUTED),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("×")
                                            .on_hover_text("Remove asset")
                                            .clicked()
                                        {
                                            self.remove_asset(asset.id.clone());
                                        }
                                        if ui
                                            .add_enabled(
                                                matches!(
                                                    asset.kind,
                                                    AssetKind::RasterImage | AssetKind::VectorImage
                                                ),
                                                egui::Button::new("Place"),
                                            )
                                            .on_hover_text("Place an image layer on the canvas")
                                            .clicked()
                                        {
                                            self.place_asset_on_canvas(asset.id.clone());
                                        }
                                    },
                                );
                            });
                            if let Some(bytes) = asset.metadata.byte_len {
                                ui.label(
                                    egui::RichText::new(format!("{bytes} bytes"))
                                        .size(9.0)
                                        .color(MUTED),
                                );
                            }
                        });
                        card.response.context_menu(|ui| {
                            if matches!(asset.kind, AssetKind::RasterImage | AssetKind::VectorImage)
                                && ui.button("Place on canvas").clicked()
                            {
                                self.place_asset_on_canvas(asset_id.clone());
                                ui.close();
                            }
                            if ui.button("Remove asset").clicked() {
                                self.remove_asset(asset_id.clone());
                                ui.close();
                            }
                        });
                    }
                });
            }
            return;
        }
        if self.sidebar_tab == SidebarTab::Comments {
            self.comments_pane(ui);
            return;
        }
        ui.horizontal(|ui| {
            ui.label("◈");
            ui.label(egui::RichText::new("Studio Team").color(Color32::from_rgb(195, 196, 203)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("···");
            });
        });
        ui.indent("file", |ui| {
            ui.horizontal(|ui| {
                ui.label("▾");
                ui.label("▤");
                ui.label(&self.document.name);
            });
            ui.indent("pages", |ui| {
                ui.horizontal(|ui| {
                    ui.label("▾");
                    ui.label("Pages");
                    if ui.small_button("+").clicked() {
                        let n = self.document.pages.len() + 1;
                        self.execute(EditorCommand::AddPage {
                            name: format!("Page {n}"),
                        });
                    }
                });
                for page in self.document.pages.clone() {
                    let current = self.document.active_page_id == page.id;
                    if ui
                        .selectable_label(current, format!("◦  {}", page.name))
                        .clicked()
                    {
                        self.document.active_page_id = page.id.clone();
                        self.clear_selection();
                    }
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Layers").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+ Frame").clicked() {
                    self.create_node(NodeKind::Frame, Pos2::new(120.0, 120.0));
                }
                if ui.small_button("···").clicked() {
                    self.status = "Layer actions".into();
                }
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.layer_filter)
                .hint_text("⌕  Search layers")
                .desired_width(f32::INFINITY),
        );
        let nodes = self.active_nodes().to_vec();
        let filter = self.layer_filter.to_lowercase();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for node in nodes
                    .iter()
                    .rev()
                    .filter(|node| filter.is_empty() || node.name.to_lowercase().contains(&filter))
                {
                    let selected = self.is_selected(&node.id);
                    let icon = match node.kind {
                        NodeKind::Frame => "▣",
                        NodeKind::Rectangle => "□",
                        NodeKind::Ellipse => "○",
                        NodeKind::Image => "▧",
                        NodeKind::Line => "╱",
                        NodeKind::Text => "T",
                    };
                    let response = ui.selectable_label(selected, format!("{icon}  {}", node.name));
                    if response.clicked() {
                        if ui.input(|input| input.modifiers.shift) {
                            self.toggle_selection(node.id.clone());
                        } else {
                            self.select_only(node.id.clone());
                        }
                    }
                    if let Some(action) = context_menu::show(&response, node.visible, node.locked) {
                        self.apply_context_action(&node.id, action);
                    }
                }
            });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("ⓘ");
                ui.label(egui::RichText::new("Keyboard shortcuts").color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("A");
                });
            });
        });
    }

    fn inspector_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.active_tab == InspectorTab::Design, "Design")
                .clicked()
            {
                self.active_tab = InspectorTab::Design;
            }
            if ui
                .selectable_label(self.active_tab == InspectorTab::Prototype, "Prototype")
                .clicked()
            {
                self.active_tab = InspectorTab::Prototype;
            }
            if ui
                .selectable_label(self.active_tab == InspectorTab::Dev, "Dev")
                .clicked()
            {
                self.active_tab = InspectorTab::Dev;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("···");
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.active_tab {
                InspectorTab::Design => self.design_panel(ui),
                InspectorTab::Prototype => self.prototype_panel(ui),
                InspectorTab::Dev => self.dev_panel(ui),
            });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("◷");
                ui.label(egui::RichText::new("Version history").color(MUTED));
            });
        });
    }

    fn design_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            ui.label("Select a layer to inspect it.");
            return;
        };
        let Some(index) = self.active_nodes().iter().position(|node| node.id == id) else {
            ui.label("Select a layer to inspect it.");
            return;
        };
        let current = self.active_nodes()[index].clone();
        ui.horizontal(|ui| {
            ui.label("◈");
            let mut name = current.name.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut name)
                        .frame(egui::Frame::NONE)
                        .desired_width(145.0),
                )
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].name = name;
            }
            if ui.small_button("···").clicked() {
                self.status = "Layer menu".into();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button(if current.visible { "◉" } else { "○" })
                .on_hover_text("Toggle visibility")
                .clicked()
            {
                self.snapshot();
                self.active_nodes_mut()[index].visible = !current.visible;
            }
            if ui
                .button(if current.locked { "▣" } else { "♧" })
                .on_hover_text("Toggle lock")
                .clicked()
            {
                self.snapshot();
                self.active_nodes_mut()[index].locked = !current.locked;
            }
            if ui.button("▢").on_hover_text("Duplicate").clicked() {
                let mut duplicate = current.clone();
                duplicate.id = format!("node-{}", uuid::Uuid::new_v4());
                duplicate.name.push_str(" copy");
                duplicate.x += 24.0;
                duplicate.y += 24.0;
                let new_id = duplicate.id.clone();
                self.execute(EditorCommand::AddNode {
                    page_id: self.document.active_page_id.clone(),
                    node: duplicate,
                });
                self.select_only(new_id);
            }
            if ui.button("▣").on_hover_text("Delete").clicked() {
                self.execute(EditorCommand::DeleteNode {
                    page_id: self.document.active_page_id.clone(),
                    node_id: id.clone(),
                });
                self.clear_selection();
            }
        });
        section(ui, "Position");
        let mut node = self.active_nodes()[index].clone();
        ui.columns(2, |columns| {
            columns[0].horizontal(|ui| {
                ui.label("X");
                if ui
                    .add(
                        egui::DragValue::new(&mut node.x)
                            .speed(1.0)
                            .range(-10000.0..=10000.0),
                    )
                    .changed()
                {
                    self.snapshot();
                    self.active_nodes_mut()[index].x = node.x;
                }
            });
            columns[1].horizontal(|ui| {
                ui.label("Y");
                if ui
                    .add(
                        egui::DragValue::new(&mut node.y)
                            .speed(1.0)
                            .range(-10000.0..=10000.0),
                    )
                    .changed()
                {
                    self.snapshot();
                    self.active_nodes_mut()[index].y = node.y;
                }
            });
            columns[0].horizontal(|ui| {
                ui.label("W");
                if ui
                    .add(
                        egui::DragValue::new(&mut node.width)
                            .speed(1.0)
                            .range(1.0..=10000.0),
                    )
                    .changed()
                {
                    self.snapshot();
                    self.active_nodes_mut()[index].width = node.width;
                }
            });
            columns[1].horizontal(|ui| {
                ui.label("H");
                if ui
                    .add(
                        egui::DragValue::new(&mut node.height)
                            .speed(1.0)
                            .range(1.0..=10000.0),
                    )
                    .changed()
                {
                    self.snapshot();
                    self.active_nodes_mut()[index].height = node.height;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Rotation");
            if ui
                .add(
                    egui::DragValue::new(&mut node.rotation)
                        .speed(1.0)
                        .suffix("°"),
                )
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].rotation = node.rotation;
            }
        });
        section(ui, "Appearance");
        ui.horizontal(|ui| {
            ui.label("Fill");
            let mut fill = parse_color(&node.fill);
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut fill,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].fill = color_hex(fill);
            }
            ui.label(node.fill.clone());
        });
        ui.horizontal(|ui| {
            ui.label("Stroke");
            let mut stroke = parse_color(&node.stroke);
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut stroke,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].stroke = color_hex(stroke);
            }
            ui.label(node.stroke.clone());
        });
        ui.horizontal(|ui| {
            ui.label("Stroke width");
            if ui
                .add(egui::DragValue::new(&mut node.stroke_width).range(0.0..=128.0))
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].stroke_width = node.stroke_width;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Radius");
            if ui
                .add(egui::DragValue::new(&mut node.corner_radius).range(0.0..=500.0))
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].corner_radius = node.corner_radius;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Opacity");
            if ui
                .add(egui::Slider::new(&mut node.opacity, 0.0..=1.0).show_value(true))
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].opacity = node.opacity;
            }
        });
        section(ui, "Accessibility");
        ui.horizontal(|ui| {
            ui.label("Check against");
            egui::color_picker::color_edit_button_srgba(
                ui,
                &mut self.contrast_background,
                egui::color_picker::Alpha::Opaque,
            );
        });
        let ratio = contrast_ratio(parse_color(&node.fill), self.contrast_background);
        let normal_pass = ratio >= 4.5;
        let large_pass = ratio >= 3.0;
        let color = if normal_pass {
            Color32::from_rgb(113, 195, 151)
        } else if large_pass {
            Color32::from_rgb(245, 190, 97)
        } else {
            Color32::from_rgb(241, 111, 121)
        };
        ui.colored_label(color, format!("Contrast {:.2}:1", ratio));
        ui.label(
            egui::RichText::new(format!(
                "Normal text: {} · Large text: {}",
                if normal_pass { "Pass" } else { "Fail" },
                if large_pass { "Pass" } else { "Fail" }
            ))
            .size(10.0)
            .color(MUTED),
        );
        if node.kind == NodeKind::Text {
            section(ui, "Text");
            let mut value = node.text.unwrap_or_default();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut value)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                self.snapshot();
                self.active_nodes_mut()[index].text = Some(value);
            }
        }
        section(ui, "Export");
        if ui.button("Export PNG").clicked() {
            self.export_png();
        }
        if ui.button("Export SVG").clicked() {
            self.export_svg();
        }
    }

    fn prototype_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Prototype interactions")
                .strong()
                .color(TEXT),
        );
        ui.label(
            egui::RichText::new(
                "Create a local click interaction from the selected layer to a frame on this page.",
            )
            .size(10.0)
            .color(MUTED),
        );
        let frames: Vec<_> = self
            .active_nodes()
            .iter()
            .filter(|node| node.kind == NodeKind::Frame)
            .map(|node| (node.id.clone(), node.name.clone()))
            .collect();
        if self
            .prototype_target_id
            .as_ref()
            .is_some_and(|id| !frames.iter().any(|(frame_id, _)| frame_id == id))
        {
            self.prototype_target_id = None;
        }
        egui::ComboBox::from_id_salt("prototype_destination")
            .selected_text(
                self.prototype_target_id
                    .as_ref()
                    .and_then(|id| {
                        frames
                            .iter()
                            .find(|(frame_id, _)| frame_id == id)
                            .map(|(_, name)| name.as_str())
                    })
                    .unwrap_or("Choose destination frame"),
            )
            .show_ui(ui, |ui| {
                for (id, name) in &frames {
                    ui.selectable_value(&mut self.prototype_target_id, Some(id.clone()), name);
                }
            });
        if ui
            .add_enabled(
                self.selected.is_some() && self.prototype_target_id.is_some(),
                egui::Button::new("Add click navigation"),
            )
            .clicked()
        {
            if let Some(target) = self.prototype_target_id.clone() {
                self.add_navigation_interaction(target);
            }
        }
        ui.separator();
        let interactions = self.document.prototype.interactions.clone();
        if interactions.is_empty() {
            ui.label(egui::RichText::new("No interactions yet").color(MUTED));
            return;
        }
        for interaction in interactions {
            ui.group(|ui| {
                ui.label(egui::RichText::new("On click").size(10.0).color(PURPLE));
                ui.label(format!("Source: {}", interaction.source_node_id));
                let target = match &interaction.action {
                    PrototypeAction::NavigateTo { frame_id } => format!("Navigate to: {frame_id}"),
                    PrototypeAction::Back => "Go back".into(),
                    PrototypeAction::OpenOverlay { frame_id } => {
                        format!("Open overlay: {frame_id}")
                    }
                    PrototypeAction::CloseOverlay => "Close overlay".into(),
                    PrototypeAction::ScrollTo { node_id } => format!("Scroll to: {node_id}"),
                    PrototypeAction::SetVariable { name, value } => format!("Set {name} = {value}"),
                };
                ui.label(egui::RichText::new(target).size(10.0).color(MUTED));
                if ui.small_button("Remove").clicked() {
                    self.remove_navigation_interaction(&interaction.id);
                }
            });
        }
    }

    fn dev_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Developer handoff")
                .strong()
                .color(TEXT),
        );
        ui.label(
            egui::RichText::new("Inspect the selected layer and copy scoped CSS.")
                .size(10.0)
                .color(MUTED),
        );
        let Some(node) = self
            .selected
            .as_ref()
            .and_then(|id| self.active_nodes().iter().find(|node| &node.id == id))
            .cloned()
        else {
            ui.add_space(18.0);
            ui.label("Select a layer to inspect its CSS.");
            return;
        };
        section(ui, "Selection");
        ui.label(format!("{} · {}", node.name, node_kind_label(&node.kind)));
        ui.label(
            egui::RichText::new(format!(
                "{:.0} × {:.0} at {:.0}, {:.0}",
                node.width, node.height, node.x, node.y
            ))
            .size(10.0)
            .color(MUTED),
        );
        section(ui, "CSS");
        let css = css_for_node(&node);
        let mut display_css = css.clone();
        ui.add(
            egui::TextEdit::multiline(&mut display_css)
                .code_editor()
                .desired_rows(14)
                .desired_width(f32::INFINITY)
                .interactive(false),
        );
        if ui.button("Copy CSS").clicked() {
            ui.ctx().copy_text(css);
            self.status = "Copied selected layer CSS".into();
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Values reflect Classic’s current single-layer geometry and appearance model.",
            )
            .size(10.0)
            .color(MUTED),
        );
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tool in [
                Tool::Select,
                Tool::Frame,
                Tool::Rectangle,
                Tool::Ellipse,
                Tool::Line,
                Tool::Text,
            ] {
                let selected = self.tool == tool;
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(tool.glyph()).size(16.0).color(
                            if selected {
                                Color32::from_rgb(213, 205, 255)
                            } else {
                                TEXT
                            },
                        ))
                        .fill(if selected {
                            Color32::from_rgb(68, 60, 102)
                        } else {
                            PANEL_ALT
                        }),
                    )
                    .on_hover_text(format!("{} ({})", tool.label(), tool.shortcut()))
                    .clicked()
                {
                    self.tool = tool;
                }
            }
            ui.separator();
            ui.menu_button("Layout ⌄", |ui| {
                ui.label(
                    egui::RichText::new("Stack selected layers")
                        .size(10.0)
                        .color(MUTED),
                );
                ui.horizontal(|ui| {
                    ui.label("Gap");
                    ui.add(
                        egui::DragValue::new(&mut self.layout_gap)
                            .range(0.0..=1000.0)
                            .speed(1.0),
                    );
                });
                ui.separator();
                let enabled = self.selected_ids.len() >= 2;
                if ui
                    .add_enabled(enabled, egui::Button::new("Stack horizontally"))
                    .clicked()
                {
                    self.stack_selection(StackAxis::Horizontal);
                }
                if ui
                    .add_enabled(enabled, egui::Button::new("Stack vertically"))
                    .clicked()
                {
                    self.stack_selection(StackAxis::Vertical);
                }
            });
            ui.menu_button("Guides ⌄", |ui| {
                let mut visible = self.document.guides.visible;
                if ui.checkbox(&mut visible, "Show guides").changed() {
                    let before = self.document.clone();
                    self.document.guides.visible = visible;
                    self.push_undo_snapshot(before);
                    self.status = "Updated ruler-guide visibility".into();
                }
                let mut snap_enabled = self.document.guides.snap_enabled;
                if ui.checkbox(&mut snap_enabled, "Snap to guides").changed() {
                    let before = self.document.clone();
                    self.document.guides.snap_enabled = snap_enabled;
                    self.push_undo_snapshot(before);
                    self.status = "Updated ruler-guide snapping".into();
                }
                ui.separator();
                if ui.button("Add horizontal guide").clicked() {
                    self.add_guide(GuideAxis::Horizontal);
                }
                if ui.button("Add vertical guide").clicked() {
                    self.add_guide(GuideAxis::Vertical);
                }
                if self.document.guides.guides.is_empty() {
                    ui.label(egui::RichText::new("No ruler guides").color(MUTED));
                } else {
                    ui.separator();
                    let items: Vec<_> = self
                        .document
                        .guides
                        .guides
                        .iter()
                        .map(|guide| (guide.id.clone(), guide.axis, guide.position, guide.locked))
                        .collect();
                    let mut remove = None;
                    for (id, axis, position, locked) in items {
                        ui.horizontal(|ui| {
                            let label = match axis {
                                GuideAxis::Horizontal => "Horizontal",
                                GuideAxis::Vertical => "Vertical",
                            };
                            ui.label(format!("{label} {:.0}", position));
                            if locked {
                                ui.label(egui_phosphor::regular::LOCK_SIMPLE);
                            } else if ui.small_button("×").on_hover_text("Remove guide").clicked()
                            {
                                remove = Some(id);
                            }
                        });
                    }
                    if let Some(id) = remove {
                        let before = self.document.clone();
                        match self.document.guides.remove(&id) {
                            Ok(_) => {
                                self.push_undo_snapshot(before);
                                self.status = "Removed ruler guide".into();
                            }
                            Err(error) => {
                                self.status = format!("Could not remove ruler guide: {error}")
                            }
                        }
                    }
                }
            });
            if self.selected_ids.len() >= 2 {
                if ui.button("⇤").on_hover_text("Align left").clicked() {
                    self.align_selection(Alignment::Left);
                }
                if ui
                    .button("↔")
                    .on_hover_text("Align horizontal centers")
                    .clicked()
                {
                    self.align_selection(Alignment::HorizontalCenter);
                }
                if ui.button("⇥").on_hover_text("Align right").clicked() {
                    self.align_selection(Alignment::Right);
                }
                if ui.button("⇡").on_hover_text("Align top").clicked() {
                    self.align_selection(Alignment::Top);
                }
                if ui
                    .button("↕")
                    .on_hover_text("Align vertical centers")
                    .clicked()
                {
                    self.align_selection(Alignment::VerticalCenter);
                }
                if ui.button("⇣").on_hover_text("Align bottom").clicked() {
                    self.align_selection(Alignment::Bottom);
                }
                if self.selected_ids.len() >= 3 {
                    if ui
                        .button("⇆")
                        .on_hover_text("Distribute horizontally")
                        .clicked()
                    {
                        self.distribute_selection(Axis::Horizontal);
                    }
                    if ui
                        .button("⇅")
                        .on_hover_text("Distribute vertically")
                        .clicked()
                    {
                        self.distribute_selection(Axis::Vertical);
                    }
                }
                ui.separator();
            }
            ui.menu_button("◇  Components ⌄", |ui| {
                if ui
                    .add_enabled(
                        self.selected.is_some(),
                        egui::Button::new("Create component from selection"),
                    )
                    .clicked()
                {
                    self.create_component_from_selection();
                }
                if self.document.components.definitions.is_empty() {
                    ui.label(egui::RichText::new("No local components yet").color(MUTED));
                } else {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Local components")
                            .size(10.0)
                            .color(MUTED),
                    );
                    let definitions: Vec<_> = self
                        .document
                        .components
                        .definitions
                        .iter()
                        .map(|definition| (definition.id.clone(), definition.name.clone()))
                        .collect();
                    for (id, name) in definitions {
                        if ui.button(format!("Place {name}")).clicked() {
                            self.place_component_instance(&id);
                        }
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("···").clicked() {
                    self.show_grid = !self.show_grid;
                }
                if ui.button("?").on_hover_text("Keyboard shortcuts").clicked() {
                    self.show_shortcuts = true;
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom + 0.1).min(2.5);
                }
                ui.label(format!("{}%", (self.zoom * 100.0).round() as i32));
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom - 0.1).max(0.2);
                }
                if ui.button("▦").clicked() {
                    self.show_grid = !self.show_grid;
                }
            });
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size_before_wrap().max(Vec2::splat(50.0));
        self.canvas_view_size = available;
        let (canvas, background_response) =
            ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 0.0, BG);
        if self.show_grid {
            let step = (20.0 * self.zoom).max(8.0);
            let offset = Vec2::new(self.pan.x.rem_euclid(step), self.pan.y.rem_euclid(step));
            let mut x = canvas.left() + offset.x;
            while x < canvas.right() {
                let mut y = canvas.top() + offset.y;
                while y < canvas.bottom() {
                    painter.circle_filled(Pos2::new(x, y), 0.8, GRID);
                    y += step;
                }
                x += step;
            }
        }
        if self.document.guides.visible {
            for guide in self
                .document
                .guides
                .guides
                .iter()
                .filter(|guide| guide.visible)
            {
                let guide_color = Color32::from_rgb(76, 196, 190);
                match guide.axis {
                    GuideAxis::Horizontal => {
                        let y = canvas.top() + self.pan.y + guide.position * self.zoom;
                        painter.line_segment(
                            [Pos2::new(canvas.left(), y), Pos2::new(canvas.right(), y)],
                            Stroke::new(1.0, guide_color),
                        );
                    }
                    GuideAxis::Vertical => {
                        let x = canvas.left() + self.pan.x + guide.position * self.zoom;
                        painter.line_segment(
                            [Pos2::new(x, canvas.top()), Pos2::new(x, canvas.bottom())],
                            Stroke::new(1.0, guide_color),
                        );
                    }
                }
            }
        }
        let space_down = shortcuts::space_pan_active(ui.ctx());
        if background_response.dragged_by(PointerButton::Primary) && space_down {
            self.pan += background_response.drag_delta();
        }
        let (wheel, control_down) = ui.input(|input| {
            (
                input.smooth_scroll_delta(),
                input.modifiers.ctrl || input.modifiers.command,
            )
        });
        if background_response.hovered() && wheel != Vec2::ZERO && !space_down {
            if control_down {
                let pointer = ui
                    .input(|input| input.pointer.hover_pos())
                    .unwrap_or(canvas.center())
                    - canvas.min;
                let old_zoom = self.zoom;
                self.zoom = (self.zoom * (wheel.y * 0.002).exp()).clamp(0.2, 2.5);
                let world = (pointer - self.pan) / old_zoom;
                self.pan = pointer - world * self.zoom;
            } else {
                self.pan += Vec2::new(wheel.x, -wheel.y);
            }
        }
        if let Some(kind) = self.tool.kind() {
            if background_response.drag_started_by(PointerButton::Primary) && !space_down {
                self.creation_origin = background_response
                    .interact_pointer_pos()
                    .map(|position| position - canvas.min);
            }
            if let Some(origin) = self.creation_origin {
                let current = ui
                    .input(|input| input.pointer.hover_pos())
                    .map(|position| position - canvas.min)
                    .unwrap_or(origin);
                let preview = Rect::from_two_pos(canvas.min + origin, canvas.min + current);
                painter.rect_stroke(preview, 1.0, Stroke::new(1.0, PURPLE), StrokeKind::Outside);
                if background_response.drag_stopped_by(PointerButton::Primary) {
                    self.creation_origin = None;
                    self.create_node_from_drag(
                        kind,
                        origin,
                        current,
                        ui.input(|input| input.modifiers.shift),
                    );
                }
            }
        }
        if background_response.clicked() {
            if let Some(kind) = self.tool.kind().filter(|_| self.creation_origin.is_none()) {
                if let Some(pos) = background_response.interact_pointer_pos() {
                    self.create_node(kind, Pos2::new(pos.x - canvas.min.x, pos.y - canvas.min.y));
                }
            } else {
                self.clear_selection();
            }
        }
        let nodes = self.active_nodes().to_vec();
        for node in nodes {
            if !node.visible {
                continue;
            }
            let rect = self.node_at_screen(&node, canvas);
            let response = ui.interact(
                rect,
                Id::new(format!("node:{}", node.id)),
                Sense::click_and_drag(),
            );
            let fill = with_opacity(parse_color(&node.fill), node.opacity);
            let stroke_color = with_opacity(parse_color(&node.stroke), node.opacity);
            match node.kind {
                NodeKind::Frame | NodeKind::Rectangle | NodeKind::Image => {
                    let radius = CornerRadius::same(
                        (node.corner_radius * self.zoom).clamp(0.0, 255.0) as u8,
                    );
                    painter.rect_filled(rect, radius, fill);
                    if node.kind == NodeKind::Image {
                        let texture = node
                            .asset_id
                            .as_deref()
                            .and_then(|asset_id| self.texture_for_asset(ui.ctx(), asset_id));
                        if let Some(texture) = texture {
                            painter.image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::from_white_alpha(
                                    (node.opacity.clamp(0.0, 1.0) * 255.0) as u8,
                                ),
                            );
                        }
                    }
                    if node.stroke != "transparent" {
                        painter.rect_stroke(
                            rect,
                            radius,
                            Stroke::new(node.stroke_width * self.zoom, stroke_color),
                            StrokeKind::Inside,
                        );
                    }
                    if node.kind == NodeKind::Frame {
                        painter.text(
                            rect.left_top() + Vec2::new(7.0, -18.0),
                            Align2::LEFT_TOP,
                            &node.name,
                            FontId::proportional(10.0),
                            MUTED,
                        );
                    }
                    if node.kind == NodeKind::Image
                        && node
                            .asset_id
                            .as_deref()
                            .and_then(|asset_id| self.texture_for_asset(ui.ctx(), asset_id))
                            .is_none()
                    {
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            format!("▧\n{}", node.name),
                            FontId::proportional(12.0 * self.zoom),
                            Color32::from_rgb(65, 71, 84),
                        );
                    }
                }
                NodeKind::Ellipse => {
                    painter.circle_filled(
                        rect.center(),
                        rect.width().min(rect.height()) / 2.0,
                        fill,
                    );
                    if node.stroke != "transparent" {
                        painter.circle_stroke(
                            rect.center(),
                            rect.width().min(rect.height()) / 2.0,
                            Stroke::new(node.stroke_width * self.zoom, stroke_color),
                        );
                    }
                }
                NodeKind::Line => {
                    painter.line_segment(
                        [rect.left_top(), rect.right_bottom()],
                        Stroke::new(node.stroke_width.max(1.0) * self.zoom, stroke_color),
                    );
                }
                NodeKind::Text => {
                    painter.text(
                        rect.left_top(),
                        Align2::LEFT_TOP,
                        node.text.clone().unwrap_or_default(),
                        FontId::proportional(
                            if node.name == "Hero title" {
                                39.0
                            } else {
                                15.0
                            } * self.zoom,
                        ),
                        fill,
                    );
                }
            }
            let open_comment_count = self
                .document
                .comments
                .threads
                .iter()
                .filter(|thread| {
                    !thread.is_resolved()
                        && thread.anchor.node_id.as_deref() == Some(node.id.as_str())
                })
                .count();
            if open_comment_count > 0 {
                let badge_center = rect.right_top() + Vec2::new(-4.0, 5.0);
                painter.circle_filled(badge_center, 9.0, PURPLE);
                painter.text(
                    badge_center,
                    Align2::CENTER_CENTER,
                    open_comment_count.to_string(),
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(if node.locked {
                    egui::CursorIcon::NotAllowed
                } else {
                    egui::CursorIcon::Grab
                });
            }
            if response.clicked() {
                if ui.input(|input| input.modifiers.shift) {
                    self.toggle_selection(node.id.clone());
                } else {
                    self.select_only(node.id.clone());
                }
            }
            if let Some(action) = context_menu::show(&response, node.visible, node.locked) {
                self.apply_context_action(&node.id, action);
            }
            if response.drag_started() && !node.locked {
                if !self.is_selected(&node.id) {
                    self.select_only(node.id.clone());
                }
                self.drag_snapshot = Some(self.document.clone());
            }
            if response.dragged() && !node.locked && self.tool == Tool::Select && !space_down {
                let delta = response.drag_delta() / self.zoom;
                let selected_ids = self.selected_ids.clone();
                let origins: Vec<_> = self
                    .drag_snapshot
                    .as_ref()
                    .and_then(|document| document.active_page())
                    .map(|page| {
                        page.nodes
                            .iter()
                            .filter(|candidate| selected_ids.contains(&candidate.id))
                            .map(|candidate| {
                                (
                                    candidate.id.clone(),
                                    candidate.x,
                                    candidate.y,
                                    candidate.width,
                                    candidate.height,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        vec![(node.id.clone(), node.x, node.y, node.width, node.height)]
                    });
                let (left, top, right, bottom) = origins.iter().fold(
                    (
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    ),
                    |(left, top, right, bottom), (_, x, y, width, height)| {
                        (
                            left.min(*x),
                            top.min(*y),
                            right.max(*x + *width),
                            bottom.max(*y + *height),
                        )
                    },
                );
                let moving =
                    WorldRect::new(left + delta.x, top + delta.y, right - left, bottom - top);
                let references: Vec<_> = self
                    .active_nodes()
                    .iter()
                    .filter(|other| !selected_ids.contains(&other.id) && other.visible)
                    .map(|other| WorldRect::new(other.x, other.y, other.width, other.height))
                    .collect();
                let candidates = snap_candidates(moving, &references, 8.0 / self.zoom);
                let mut snap_x = 0.0;
                let mut snap_y = 0.0;
                let mut got_x = false;
                let mut got_y = false;
                self.snap_guides.clear();
                for candidate in candidates {
                    match candidate.axis {
                        Axis::Horizontal if !got_x => {
                            snap_x = candidate.delta;
                            got_x = true;
                            self.snap_guides.push(candidate);
                        }
                        Axis::Vertical if !got_y => {
                            snap_y = candidate.delta;
                            got_y = true;
                            self.snap_guides.push(candidate);
                        }
                        _ => {}
                    }
                }
                if self.document.guides.snap_enabled {
                    if let Some(hit) = nearest_snap(
                        self.document.guides.snap_candidates(),
                        GuideAxis::Vertical,
                        moving.left(),
                        8.0 / self.zoom,
                    ) {
                        if !got_x || hit.delta.abs() < snap_x.abs() {
                            snap_x = hit.delta;
                        }
                    }
                    if let Some(hit) = nearest_snap(
                        self.document.guides.snap_candidates(),
                        GuideAxis::Horizontal,
                        moving.top(),
                        8.0 / self.zoom,
                    ) {
                        if !got_y || hit.delta.abs() < snap_y.abs() {
                            snap_y = hit.delta;
                        }
                    }
                }
                for (id, x, y, _, _) in origins {
                    if let Some(target) = self
                        .active_nodes_mut()
                        .iter_mut()
                        .find(|target| target.id == id)
                    {
                        target.x = x + delta.x + snap_x;
                        target.y = y + delta.y + snap_y;
                    }
                }
            }
            if response.drag_stopped() {
                if let Some(before) = self.drag_snapshot.take() {
                    self.undo.push(before);
                    self.redo.clear();
                    self.status = "Unsaved changes".into();
                }
                self.snap_guides.clear();
            }
            if self.is_selected(&node.id) {
                painter.rect_stroke(
                    rect.expand(3.0),
                    1.0,
                    Stroke::new(1.2, PURPLE),
                    StrokeKind::Outside,
                );
                painter.circle_filled(rect.right_bottom() + Vec2::splat(3.0), 4.0, Color32::WHITE);
                let handle = Rect::from_center_size(
                    rect.right_bottom() + Vec2::splat(3.0),
                    Vec2::splat(12.0),
                );
                let resize = ui.interact(
                    handle,
                    Id::new(format!("resize:{}", node.id)),
                    Sense::drag(),
                );
                if resize.drag_started() {
                    self.resize_origin = Some((node.width, node.height));
                    self.drag_snapshot = Some(self.document.clone());
                }
                if resize.dragged() {
                    let delta = resize.drag_delta() / self.zoom;
                    if let Some(origin) = self.resize_origin {
                        if let Some(target) = self
                            .active_nodes_mut()
                            .iter_mut()
                            .find(|target| target.id == node.id)
                        {
                            target.width = (origin.0 + delta.x).max(4.0);
                            target.height = (origin.1 + delta.y).max(4.0);
                        }
                    }
                }
                if resize.drag_stopped() {
                    self.resize_origin = None;
                    if let Some(before) = self.drag_snapshot.take() {
                        self.undo.push(before);
                        self.redo.clear();
                    }
                }
            }
        }
        for guide in &self.snap_guides {
            let start = canvas.min
                + self.pan
                + Vec2::new(guide.guide_start.x, guide.guide_start.y) * self.zoom;
            let end =
                canvas.min + self.pan + Vec2::new(guide.guide_end.x, guide.guide_end.y) * self.zoom;
            painter.line_segment(
                [start, end],
                Stroke::new(1.0, Color32::from_rgb(255, 92, 140)),
            );
        }
        painter.text(
            canvas.left_bottom() + Vec2::new(14.0, -14.0),
            Align2::LEFT_BOTTOM,
            format!(
                "{} layers  ·  {}%",
                self.active_nodes().len(),
                (self.zoom * 100.0).round() as i32
            ),
            FontId::proportional(10.0),
            MUTED,
        );
        if self.tool != Tool::Select {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    fn presentation_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Presentation preview")
                    .strong()
                    .color(TEXT),
            );
            ui.label(
                egui::RichText::new("Active page · local preview")
                    .size(10.0)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Exit preview").clicked() {
                    self.present = false;
                    self.status = "Editor view".into();
                }
                if ui.button("Restart").clicked() {
                    self.presentation_frame_id = self
                        .active_nodes()
                        .iter()
                        .find(|node| node.kind == NodeKind::Frame)
                        .map(|node| node.id.clone());
                }
            });
        });
        ui.separator();
        let available = ui.available_size_before_wrap().max(Vec2::splat(50.0));
        let (canvas, _) = ui.allocate_exact_size(available, Sense::hover());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 0.0, Color32::from_rgb(18, 19, 23));
        let focused_frame = self.presentation_frame_id.as_ref().and_then(|id| {
            self.active_nodes()
                .iter()
                .find(|node| node.id == *id && node.kind == NodeKind::Frame)
                .cloned()
        });
        let nodes: Vec<_> = self
            .active_nodes()
            .iter()
            .filter(|node| {
                node.visible
                    && focused_frame.as_ref().is_none_or(|frame| {
                        node.id == frame.id
                            || (node.x >= frame.x
                                && node.y >= frame.y
                                && node.x + node.width <= frame.x + frame.width
                                && node.y + node.height <= frame.y + frame.height)
                    })
            })
            .cloned()
            .collect();
        if nodes.is_empty() {
            painter.text(
                canvas.center(),
                Align2::CENTER_CENTER,
                "Nothing to present on this page",
                FontId::proportional(16.0),
                MUTED,
            );
            return;
        }
        let (left, top, right, bottom) = nodes.iter().fold(
            (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |(left, top, right, bottom), node| {
                (
                    left.min(node.x),
                    top.min(node.y),
                    right.max(node.x + node.width),
                    bottom.max(node.y + node.height),
                )
            },
        );
        let size = Vec2::new((right - left).max(1.0), (bottom - top).max(1.0));
        let scale = (available.x * 0.88 / size.x)
            .min(available.y * 0.88 / size.y)
            .clamp(0.1, 4.0);
        let origin = canvas.center() - size * scale * 0.5 - Vec2::new(left, top) * scale;
        for node in nodes {
            let rect = Rect::from_min_size(
                origin + Vec2::new(node.x, node.y) * scale,
                Vec2::new(node.width.max(1.0), node.height.max(1.0)) * scale,
            );
            let fill = with_opacity(parse_color(&node.fill), node.opacity);
            let stroke = with_opacity(parse_color(&node.stroke), node.opacity);
            match node.kind {
                NodeKind::Frame | NodeKind::Rectangle | NodeKind::Image => {
                    let radius =
                        CornerRadius::same((node.corner_radius * scale).clamp(0.0, 255.0) as u8);
                    painter.rect_filled(rect, radius, fill);
                    if node.kind == NodeKind::Image {
                        if let Some(texture) = node
                            .asset_id
                            .as_deref()
                            .and_then(|asset_id| self.texture_for_asset(ui.ctx(), asset_id))
                        {
                            painter.image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::from_white_alpha(
                                    (node.opacity.clamp(0.0, 1.0) * 255.0) as u8,
                                ),
                            );
                        }
                    }
                    if node.stroke != "transparent" {
                        painter.rect_stroke(
                            rect,
                            radius,
                            Stroke::new(node.stroke_width.max(1.0) * scale, stroke),
                            StrokeKind::Inside,
                        );
                    }
                    if node.kind == NodeKind::Image
                        && node
                            .asset_id
                            .as_deref()
                            .and_then(|asset_id| self.texture_for_asset(ui.ctx(), asset_id))
                            .is_none()
                    {
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            format!("▧\n{}", node.name),
                            FontId::proportional((12.0 * scale).clamp(9.0, 28.0)),
                            Color32::from_rgb(65, 71, 84),
                        );
                    }
                }
                NodeKind::Ellipse => {
                    painter.circle_filled(
                        rect.center(),
                        rect.width().min(rect.height()) * 0.5,
                        fill,
                    );
                    if node.stroke != "transparent" {
                        painter.circle_stroke(
                            rect.center(),
                            rect.width().min(rect.height()) * 0.5,
                            Stroke::new(node.stroke_width.max(1.0) * scale, stroke),
                        );
                    }
                }
                NodeKind::Line => {
                    painter.line_segment(
                        [rect.left_top(), rect.right_bottom()],
                        Stroke::new(node.stroke_width.max(1.0) * scale, stroke),
                    );
                }
                NodeKind::Text => {
                    painter.text(
                        rect.left_top(),
                        Align2::LEFT_TOP,
                        node.text.unwrap_or_default(),
                        FontId::proportional((15.0 * scale).clamp(10.0, 54.0)),
                        fill,
                    );
                }
            }
            let navigation_target =
                self.document
                    .prototype
                    .interactions
                    .iter()
                    .find_map(|interaction| {
                        (interaction.source_node_id == node.id).then(|| {
                            match &interaction.action {
                                PrototypeAction::NavigateTo { frame_id } => Some(frame_id.clone()),
                                _ => None,
                            }
                        })?
                    });
            if let Some(target) = navigation_target {
                let response = ui.interact(
                    rect,
                    Id::new(format!("prototype-preview:{}", node.id)),
                    Sense::click(),
                );
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    self.presentation_frame_id = Some(target);
                    self.status = "Prototype navigation".into();
                }
            }
        }
    }

    fn shortcut_reference(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Keyboard shortcuts")
            .open(&mut self.show_shortcuts)
            .resizable(false)
            .default_width(430.0)
            .show(ui.ctx(), |ui| {
                ui.label(
                    egui::RichText::new("Classic Studio editor bindings")
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(6.0);
                shortcut_row(ui, "Move / Select", "V");
                shortcut_row(ui, "Frame", "F");
                shortcut_row(ui, "Rectangle", "R");
                shortcut_row(ui, "Ellipse", "O");
                shortcut_row(ui, "Line", "L");
                shortcut_row(ui, "Text", "T");
                ui.separator();
                shortcut_row(ui, "Undo", "Ctrl/Cmd + Z");
                shortcut_row(ui, "Redo", "Ctrl/Cmd + Shift + Z or Y");
                shortcut_row(ui, "Duplicate", "Ctrl/Cmd + D");
                shortcut_row(ui, "Select all visible layers", "Ctrl/Cmd + A");
                shortcut_row(ui, "Delete selection", "Delete or Backspace");
                shortcut_row(ui, "Nudge selection", "Arrow keys; Shift = 10 units");
                ui.separator();
                shortcut_row(ui, "Add/remove from selection", "Shift + click");
                shortcut_row(ui, "Pan", "Hold Space and drag");
                shortcut_row(ui, "Zoom", "Ctrl/Cmd + scroll, +, -");
                shortcut_row(ui, "Fit artwork", "Home");
                shortcut_row(ui, "Exit presentation / clear selection", "Escape");
            });
    }
}

impl TabViewer for EditorTabViewer<'_> {
    type Tab = WorkspaceTab;

    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        Id::new(*tab as u8)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let (icon, name) = match tab {
            WorkspaceTab::Layers => (egui_phosphor::regular::STACK, "Layers"),
            WorkspaceTab::Canvas => (egui_phosphor::regular::FRAME_CORNERS, "Canvas"),
            WorkspaceTab::Inspector => (egui_phosphor::regular::SLIDERS_HORIZONTAL, "Inspector"),
        };
        format!("{icon}  {name}").into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            WorkspaceTab::Layers => self.app.layers_pane(ui),
            WorkspaceTab::Canvas => self.app.canvas(ui),
            WorkspaceTab::Inspector => self.app.inspector_pane(ui),
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }
}

impl eframe::App for ClassicApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "windows")]
        {
            match self
                .tray
                .as_ref()
                .and_then(tray::TrayController::poll_action)
            {
                Some(tray::TrayAction::Show) => {
                    self.quit_from_tray = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Some(tray::TrayAction::Quit) => {
                    self.quit_from_tray = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                None => {}
            }
            let close_requested = ctx.input(|input| input.viewport().close_requested());
            if close_requested && self.tray.is_some() && !self.quit_from_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.status = "Running in the system tray".into();
            }
        }
        if self.show_project_hub {
            egui::CentralPanel::no_frame().show(ui, |ui| self.project_hub(ui));
            return;
        }
        for action in shortcuts::collect(&ctx) {
            self.handle_shortcut(action);
        }
        let dropped_paths: Vec<_> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        for path in dropped_paths {
            self.import_asset_path(path);
        }
        self.autosave_recovery_if_due();
        if self.present {
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.present = false;
                self.status = "Editor view".into();
            }
            egui::CentralPanel::no_frame().show(ui, |ui| self.presentation_view(ui));
            return;
        }
        self.top_bar(ui);
        egui::Panel::top("tools")
            .default_size(42.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(31, 32, 38))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| self.toolbar(ui));
        self.shortcut_reference(ui);
        egui::Panel::bottom("status")
            .default_size(30.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 31, 36))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("▤  Page 1  ⌄");
                    ui.separator();
                    ui.label(format!("{} layers", self.active_nodes().len()));
                    ui.separator();
                    ui.label(
                        self.selected
                            .as_ref()
                            .and_then(|id| self.active_nodes().iter().find(|n| &n.id == id))
                            .map(|n| format!("{} × {}", n.width as i32, n.height as i32))
                            .unwrap_or_else(|| "Canvas".into()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "{} tool  ·  Click to place, drag to move",
                            self.tool.label()
                        ));
                    });
                });
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let mut viewer = EditorTabViewer { app: self };
            ui.ctx().clone().data_mut(|data| {
                let dock_state = data
                    .get_temp_mut_or_insert_with(Id::new("classic_dock_state"), initial_dock_state);
                DockArea::new(dock_state)
                    .style(egui_dock::Style::from_egui(ui.style()))
                    .show_inside(ui, &mut viewer);
            });
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label("···");
        });
    });
}
fn shortcut_row(ui: &mut egui::Ui, action: &str, binding: &str) {
    ui.horizontal(|ui| {
        ui.label(action);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(binding).monospace().color(MUTED));
        });
    });
}
fn small_chip(ui: &mut egui::Ui, text: &str) {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(10.0).color(MUTED))
            .fill(Color32::from_rgb(41, 42, 48))
            .stroke(Stroke::new(1.0, BORDER)),
    );
}
fn separator(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
}
fn parse_color(value: &str) -> Color32 {
    let value = value.trim_start_matches('#');
    if value.len() == 6 {
        if let Ok(rgb) = u32::from_str_radix(value, 16) {
            return Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        }
    }
    Color32::TRANSPARENT
}
fn color_hex(value: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", value.r(), value.g(), value.b())
}
fn with_opacity(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (opacity.clamp(0.0, 1.0) * 255.0) as u8,
    )
}
fn contrast_ratio(foreground: Color32, background: Color32) -> f32 {
    let luminance = |color: Color32| {
        let linear = |channel: u8| {
            let value = channel as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    };
    let (light, dark) = {
        let first = luminance(foreground);
        let second = luminance(background);
        if first >= second {
            (first, second)
        } else {
            (second, first)
        }
    };
    (light + 0.05) / (dark + 0.05)
}
fn asset_kind_for_path(path: &Path) -> AssetKind {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "avif") => AssetKind::RasterImage,
        Some("svg") => AssetKind::VectorImage,
        Some("ttf" | "otf" | "woff" | "woff2") => AssetKind::Font,
        Some("mp4" | "webm" | "mov") => AssetKind::Video,
        Some("mp3" | "wav" | "ogg" | "flac") => AssetKind::Audio,
        Some("pdf" | "classic") => AssetKind::Document,
        _ => AssetKind::Other,
    }
}
fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}
fn asset_kind_icon(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::RasterImage => "▧",
        AssetKind::VectorImage => "◇",
        AssetKind::Font => "T",
        AssetKind::Video => "▶",
        AssetKind::Audio => "♪",
        AssetKind::Document => "▤",
        AssetKind::Other => "•",
    }
}
fn node_kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Frame => "frame",
        NodeKind::Rectangle => "rectangle",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Image => "image",
        NodeKind::Text => "text",
        NodeKind::Line => "line",
    }
}
fn css_for_node(node: &DesignNode) -> String {
    let mut css = format!(
        ".{} {{\n  position: absolute;\n  left: {:.0}px;\n  top: {:.0}px;\n  width: {:.0}px;\n  height: {:.0}px;\n  opacity: {:.2};\n",
        css_identifier(&node.name),
        node.x,
        node.y,
        node.width,
        node.height,
        node.opacity
    );
    match node.kind {
        NodeKind::Text => {
            css.push_str(&format!("  color: {};\n", node.fill));
            css.push_str("  white-space: pre-wrap;\n");
        }
        NodeKind::Line => {
            css.push_str(&format!(
                "  border-top: {:.0}px solid {};\n",
                node.stroke_width, node.stroke
            ));
        }
        _ => {
            css.push_str(&format!("  background: {};\n", node.fill));
            if node.stroke != "transparent" {
                css.push_str(&format!(
                    "  border: {:.0}px solid {};\n",
                    node.stroke_width, node.stroke
                ));
            }
            if node.kind == NodeKind::Ellipse {
                css.push_str("  border-radius: 50%;\n");
            } else if node.corner_radius > 0.0 {
                css.push_str(&format!("  border-radius: {:.0}px;\n", node.corner_radius));
            }
            if node.kind == NodeKind::Image {
                css.push_str(
                    "  /* Linked local image layer; decode/export source separately. */\n",
                );
            }
        }
    }
    if node.rotation != 0.0 {
        css.push_str(&format!("  transform: rotate({:.1}deg);\n", node.rotation));
    }
    css.push('}');
    css
}
fn css_identifier(name: &str) -> String {
    let identifier: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = identifier.trim_matches('-');
    if trimmed.is_empty() {
        "layer".into()
    } else {
        trimmed.to_owned()
    }
}
fn local_comment_actor() -> Actor {
    Actor::new("classic-local-user", "You")
}
fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}
fn recovery_file_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ClassicStudio")
        .join("recovery.classic")
}
fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, Color32::from_rgb(31, 32, 45));
    let c = rect.center();
    painter.circle_stroke(
        c,
        size * 0.28,
        Stroke::new(3.0, Color32::from_rgb(133, 125, 246)),
    );
    painter.rect_filled(
        Rect::from_center_size(
            c + Vec2::new(2.0, -1.0),
            Vec2::new(size * 0.22, size * 0.22),
        ),
        2.0,
        Color32::from_rgb(232, 236, 255),
    );
    painter.circle_filled(
        c + Vec2::new(size * 0.22, size * 0.11),
        size * 0.045,
        Color32::from_rgb(255, 209, 102),
    );
}

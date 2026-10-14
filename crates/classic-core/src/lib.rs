use classic_assets::{AssetCatalog, AssetId};
use classic_comments::CommentStore;
use classic_components::ComponentLibrary;
use classic_guides::GuideStore;
use classic_layout::PrototypeGraph;
use classic_tokens::TokenLibrary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignDocument {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub pages: Vec<Page>,
    pub active_page_id: String,
    #[serde(default)]
    pub guides: GuideStore,
    #[serde(default)]
    pub assets: AssetCatalog,
    #[serde(default)]
    pub tokens: TokenLibrary,
    #[serde(default)]
    pub comments: CommentStore,
    #[serde(default)]
    pub components: ComponentLibrary,
    #[serde(default)]
    pub prototype: PrototypeGraph,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub name: String,
    pub nodes: Vec<DesignNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignNode {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub visible: bool,
    pub locked: bool,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f32,
    pub opacity: f32,
    pub corner_radius: f32,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub component_id: Option<String>,
    #[serde(default)]
    pub children: Vec<DesignNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Frame,
    Rectangle,
    Ellipse,
    Image,
    Text,
    Line,
}

impl DesignDocument {
    pub fn starter() -> Self {
        let page_id = "page-home".to_string();
        let mut artboard = DesignNode::new(NodeKind::Frame, 170.0, 104.0);
        artboard.id = "frame-home".into();
        artboard.name = "Homepage · Desktop".into();
        artboard.width = 760.0;
        artboard.height = 500.0;
        artboard.fill = "#f7f5f1".into();
        artboard.stroke = "#e7e3dc".into();

        let mut title = DesignNode::new(NodeKind::Text, 224.0, 206.0);
        title.id = "hero-title".into();
        title.name = "Hero title".into();
        title.width = 390.0;
        title.height = 114.0;
        title.text = Some("Make room\nfor ideas.".into());
        title.fill = "#17191d".into();

        let mut copy = DesignNode::new(NodeKind::Text, 226.0, 342.0);
        copy.id = "supporting-copy".into();
        copy.name = "Supporting copy".into();
        copy.width = 320.0;
        copy.height = 50.0;
        copy.text = Some("A considered space for your next big thing.".into());
        copy.fill = "#777a82".into();

        let mut button = DesignNode::new(NodeKind::Rectangle, 226.0, 423.0);
        button.id = "get-started".into();
        button.name = "Get started button".into();
        button.width = 150.0;
        button.height = 48.0;
        button.fill = "#7c63e8".into();
        button.stroke = "#7c63e8".into();
        button.corner_radius = 24.0;

        let mut label = DesignNode::new(NodeKind::Text, 253.0, 437.0);
        label.id = "button-label".into();
        label.name = "Button label".into();
        label.width = 120.0;
        label.height = 24.0;
        label.text = Some("Get started  ↗".into());
        label.fill = "#ffffff".into();

        let mut orb = DesignNode::new(NodeKind::Ellipse, 706.0, 220.0);
        orb.id = "gradient-form".into();
        orb.name = "Gradient form".into();
        orb.width = 148.0;
        orb.height = 148.0;
        orb.fill = "#ffb79e".into();
        orb.stroke = "#ffb79e".into();

        let mut accent = DesignNode::new(NodeKind::Ellipse, 755.0, 270.0);
        accent.id = "accent".into();
        accent.name = "Accent".into();
        accent.width = 92.0;
        accent.height = 92.0;
        accent.fill = "#facb76".into();
        accent.stroke = "#facb76".into();
        Self {
            schema_version: 1,
            id: "classic-starter".to_string(),
            name: "Studio landing page".to_string(),
            active_page_id: page_id.clone(),
            pages: vec![Page {
                id: page_id,
                name: "Page 1".into(),
                nodes: vec![artboard, title, copy, button, label, orb, accent],
            }],
            guides: GuideStore::new(),
            assets: AssetCatalog::new(),
            tokens: TokenLibrary::new(),
            comments: CommentStore::new(),
            components: ComponentLibrary::new(),
            prototype: PrototypeGraph::default(),
        }
    }

    pub fn active_page(&self) -> Option<&Page> {
        self.pages
            .iter()
            .find(|page| page.id == self.active_page_id)
    }

    /// Returns self-contained bytes for a document asset when the asset was
    /// saved inside this `.classic` document. Linked assets return `None` and
    /// remain the responsibility of the application-level asset resolver.
    pub fn embedded_asset_bytes(&self, asset_id: &str) -> Option<&[u8]> {
        self.assets
            .get(&AssetId::from(asset_id))
            .and_then(|asset| asset.embedded_bytes())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported document schema version {}",
                self.schema_version
            ));
        }
        if self.pages.is_empty() || self.active_page().is_none() {
            return Err("The document must contain its active page".into());
        }
        let mut page_ids = std::collections::HashSet::new();
        let mut node_ids = std::collections::HashSet::new();
        for page in &self.pages {
            if !page_ids.insert(&page.id) {
                return Err(format!("Duplicate page id: {}", page.id));
            }
            validate_nodes(&page.nodes, &mut node_ids, &self.assets)?;
        }
        self.guides
            .validate()
            .map_err(|error| format!("Invalid ruler guides: {error}"))?;
        self.assets
            .validate()
            .map_err(|error| format!("Invalid asset catalog: {error}"))?;
        self.tokens
            .validate()
            .map_err(|error| format!("Invalid token library: {error}"))?;
        self.comments
            .validate()
            .map_err(|error| format!("Invalid comment store: {error}"))?;
        self.components
            .validate()
            .map_err(|error| format!("Invalid component library: {error}"))?;
        Ok(())
    }

    pub fn apply(&mut self, command: EditorCommand) -> Result<(), String> {
        let mut next = self.clone();
        next.apply_unchecked(command)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn apply_unchecked(&mut self, command: EditorCommand) -> Result<(), String> {
        match command {
            EditorCommand::AddNode { page_id, node } => {
                let page = self
                    .pages
                    .iter_mut()
                    .find(|page| page.id == page_id)
                    .ok_or("Page not found")?;
                if page.nodes.iter().any(|existing| existing.id == node.id) {
                    return Err(format!("Layer id already exists: {}", node.id));
                }
                page.nodes.push(node);
            }
            EditorCommand::DeleteNode { page_id, node_id } => {
                let page = self
                    .pages
                    .iter_mut()
                    .find(|page| page.id == page_id)
                    .ok_or("Page not found")?;
                let previous = page.nodes.len();
                page.nodes.retain(|node| node.id != node_id);
                if previous == page.nodes.len() {
                    return Err(format!("Layer not found: {node_id}"));
                }
            }
            EditorCommand::SetNodeGeometry {
                page_id,
                node_id,
                x,
                y,
                width,
                height,
                rotation,
            } => {
                let node = self.find_node_mut(&page_id, &node_id)?;
                node.x = x;
                node.y = y;
                node.width = width;
                node.height = height;
                node.rotation = rotation;
            }
            EditorCommand::SetNodeAppearance {
                page_id,
                node_id,
                fill,
                stroke,
                stroke_width,
                opacity,
                corner_radius,
            } => {
                let node = self.find_node_mut(&page_id, &node_id)?;
                node.fill = fill;
                node.stroke = stroke;
                node.stroke_width = stroke_width;
                node.opacity = opacity;
                node.corner_radius = corner_radius;
            }
            EditorCommand::SetNodeMetadata {
                page_id,
                node_id,
                name,
                visible,
                locked,
                text,
            } => {
                let node = self.find_node_mut(&page_id, &node_id)?;
                node.name = name;
                node.visible = visible;
                node.locked = locked;
                node.text = text;
            }
            EditorCommand::AddPage { name } => {
                let id = format!("page-{}", self.pages.len() + 1);
                self.pages.push(Page {
                    id: id.clone(),
                    name,
                    nodes: Vec::new(),
                });
                self.active_page_id = id;
            }
        }
        Ok(())
    }

    fn find_node_mut(&mut self, page_id: &str, node_id: &str) -> Result<&mut DesignNode, String> {
        self.pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or("Page not found")?
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .ok_or_else(|| format!("Layer not found: {node_id}"))
    }
}

fn validate_nodes(
    nodes: &[DesignNode],
    ids: &mut std::collections::HashSet<String>,
    assets: &AssetCatalog,
) -> Result<(), String> {
    for node in nodes {
        if !ids.insert(node.id.clone()) {
            return Err(format!("Duplicate node id: {}", node.id));
        }
        if !node.x.is_finite()
            || !node.y.is_finite()
            || !node.width.is_finite()
            || !node.height.is_finite()
            || !node.rotation.is_finite()
            || !node.stroke_width.is_finite()
            || !node.opacity.is_finite()
            || !node.corner_radius.is_finite()
            || node.width < 0.0
            || node.height < 0.0
        {
            return Err(format!("Invalid geometry or style for layer {}", node.name));
        }
        if let Some(asset_id) = &node.asset_id {
            let asset_id = AssetId::from(asset_id.as_str());
            if !assets.contains(&asset_id) {
                return Err(format!(
                    "Layer {} references an asset that is absent from the document: {}",
                    node.name,
                    asset_id.as_str()
                ));
            }
        }
        validate_nodes(&node.children, ids, assets)?;
    }
    Ok(())
}

impl DesignNode {
    pub fn new(kind: NodeKind, x: f32, y: f32) -> Self {
        let (name, width, height, fill, stroke, radius, text) = match kind {
            NodeKind::Frame => ("Frame", 420.0, 300.0, "#ffffff", "#d7d9df", 0.0, None),
            NodeKind::Rectangle => ("Rectangle", 150.0, 110.0, "#c8b6ff", "#7557c8", 12.0, None),
            NodeKind::Ellipse => ("Ellipse", 110.0, 110.0, "#ffbe98", "#de7951", 0.0, None),
            NodeKind::Image => ("Image", 320.0, 220.0, "#d8dde8", "#777e90", 8.0, None),
            NodeKind::Text => (
                "Text",
                250.0,
                52.0,
                "#20212a",
                "transparent",
                0.0,
                Some("Your text".to_string()),
            ),
            NodeKind::Line => ("Line", 140.0, 1.0, "transparent", "#686b76", 0.0, None),
        };
        Self {
            id: format!("node-{}", uuid::Uuid::new_v4()),
            name: name.into(),
            kind,
            x,
            y,
            width,
            height,
            rotation: 0.0,
            visible: true,
            locked: false,
            fill: fill.into(),
            stroke: stroke.into(),
            stroke_width: 1.0,
            opacity: 1.0,
            corner_radius: radius,
            text,
            asset_id: None,
            component_id: None,
            children: Vec::new(),
        }
    }
}

impl NodeKind {}

pub enum EditorCommand {
    AddNode {
        page_id: String,
        node: DesignNode,
    },
    DeleteNode {
        page_id: String,
        node_id: String,
    },
    SetNodeGeometry {
        page_id: String,
        node_id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rotation: f32,
    },
    SetNodeAppearance {
        page_id: String,
        node_id: String,
        fill: String,
        stroke: String,
        stroke_width: f32,
        opacity: f32,
        corner_radius: f32,
    },
    SetNodeMetadata {
        page_id: String,
        node_id: String,
        name: String,
        visible: bool,
        locked: bool,
        text: Option<String>,
    },
    AddPage {
        name: String,
    },
}

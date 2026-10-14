//! Clean-room file interchange for Classic documents.
//!
//! The `.classic` format is a versioned JSON wrapper around the public Classic
//! document model. It does not read or write Figma's proprietary formats.

use classic_core::{DesignDocument, DesignNode, NodeKind, Page};
use quick_xml::{events::Event, Reader};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error as StdError,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const CLASSIC_FORMAT_VERSION: u32 = 1;
const CLASSIC_FORMAT_NAME: &str = "classic";

#[derive(Debug)]
pub enum FormatError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Xml(String),
    Validation(String),
    UnsupportedVersion(u32),
    MissingPage(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "file operation failed: {e}"),
            Self::Json(e) => write!(f, "invalid Classic JSON: {e}"),
            Self::Xml(e) => write!(f, "invalid or unsupported SVG: {e}"),
            Self::Validation(e) => write!(f, "invalid Classic document: {e}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported Classic format version {v}"),
            Self::MissingPage(id) => write!(f, "page not found: {id}"),
        }
    }
}

impl StdError for FormatError {}
impl From<std::io::Error> for FormatError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<serde_json::Error> for FormatError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClassicFile {
    format: String,
    format_version: u32,
    document: DesignDocument,
}

/// Parse a versioned `.classic` JSON payload and validate its design document.
pub fn parse_classic_json(source: &str) -> Result<DesignDocument, FormatError> {
    let file: ClassicFile = serde_json::from_str(source)?;
    if file.format != CLASSIC_FORMAT_NAME {
        return Err(FormatError::Validation(format!(
            "expected format `{CLASSIC_FORMAT_NAME}`"
        )));
    }
    if file.format_version != CLASSIC_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion(file.format_version));
    }
    validate_document(&file.document)?;
    Ok(file.document)
}

/// Serialize a validated document as pretty, versioned `.classic` JSON.
pub fn serialize_classic_json(document: &DesignDocument) -> Result<String, FormatError> {
    validate_document(document)?;
    Ok(serde_json::to_string_pretty(&ClassicFile {
        format: CLASSIC_FORMAT_NAME.into(),
        format_version: CLASSIC_FORMAT_VERSION,
        document: document.clone(),
    })?)
}

/// Load a `.classic` JSON file. This API intentionally does not accept Figma files.
pub fn load_classic_json(path: impl AsRef<Path>) -> Result<DesignDocument, FormatError> {
    let mut source = String::new();
    File::open(path)?.read_to_string(&mut source)?;
    parse_classic_json(&source)
}

/// Write `.classic` JSON through a unique sibling temporary file and rename it
/// into place. The old destination remains intact if serialization or writing
/// fails; the temporary is removed on failure.
pub fn save_classic_json(
    path: impl AsRef<Path>,
    document: &DesignDocument,
) -> Result<(), FormatError> {
    let path = path.as_ref();
    let serialized = serialize_classic_json(document)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| FormatError::Validation("destination must name a file".into()))?;
    let mut temp_path = PathBuf::new();
    let mut temp_file = None;
    for _ in 0..32 {
        temp_path = parent.join(format!(
            ".{}.{}.tmp",
            file_name.to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                temp_file = Some(file);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    let mut file = temp_file.ok_or_else(|| {
        FormatError::Validation("could not allocate a unique temporary file".into())
    })?;
    let write_result = (|| -> Result<(), std::io::Error> {
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }
    Ok(())
}

fn validate_document(document: &DesignDocument) -> Result<(), FormatError> {
    document.validate().map_err(FormatError::Validation)?;
    let mut page_ids = std::collections::HashSet::new();
    for page in &document.pages {
        if !page_ids.insert(&page.id) {
            return Err(FormatError::Validation(format!(
                "duplicate page id: {}",
                page.id
            )));
        }
        let mut node_ids = std::collections::HashSet::new();
        validate_nodes(&page.nodes, &mut node_ids)?;
    }
    Ok(())
}

fn validate_nodes(
    nodes: &[DesignNode],
    ids: &mut std::collections::HashSet<String>,
) -> Result<(), FormatError> {
    for node in nodes {
        if !ids.insert(node.id.clone()) {
            return Err(FormatError::Validation(format!(
                "duplicate node id: {}",
                node.id
            )));
        }
        if !node.rotation.is_finite()
            || !node.stroke_width.is_finite()
            || !node.opacity.is_finite()
            || !node.corner_radius.is_finite()
        {
            return Err(FormatError::Validation(format!(
                "non-finite style or rotation for layer {}",
                node.name
            )));
        }
        validate_nodes(&node.children, ids)?;
    }
    Ok(())
}

/// Export one page as SVG. Nested groups are emitted recursively; text uses a
/// basic SVG `<text>` element and retains only the model's single fill color.
pub fn export_svg(document: &DesignDocument, page_id: &str) -> Result<String, FormatError> {
    validate_document(document)?;
    let page = document
        .pages
        .iter()
        .find(|p| p.id == page_id)
        .ok_or_else(|| FormatError::MissingPage(page_id.into()))?;
    let (width, height) = page_bounds(&page.nodes);
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    for node in &page.nodes {
        write_svg_node(&mut out, node, 1);
    }
    out.push_str("</svg>\n");
    Ok(out)
}

fn page_bounds(nodes: &[DesignNode]) -> (f32, f32) {
    let mut w: f32 = 1.0;
    let mut h: f32 = 1.0;
    for n in nodes.iter().filter(|n| n.visible) {
        w = w.max(n.x + n.width);
        h = h.max(n.y + n.height);
    }
    (w.ceil(), h.ceil())
}

fn write_svg_node(out: &mut String, n: &DesignNode, depth: usize) {
    let pad = "  ".repeat(depth);
    if !n.visible {
        return;
    }
    if !n.children.is_empty() || matches!(n.kind, NodeKind::Frame) {
        out.push_str(&format!(
            "{pad}<g id=\"{}\" opacity=\"{}\"{}>\n",
            xml_escape(&n.id),
            n.opacity,
            if n.rotation != 0.0 {
                format!(
                    " transform=\"rotate({} {} {})\"",
                    n.rotation,
                    n.x + n.width / 2.0,
                    n.y + n.height / 2.0
                )
            } else {
                String::new()
            }
        ));
        if n.kind == NodeKind::Frame {
            out.push_str(&format!("{}  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n", pad, n.x, n.y, n.width, n.height, n.corner_radius, xml_escape(&n.fill), xml_escape(&n.stroke), n.stroke_width));
        }
        for child in &n.children {
            write_svg_node(out, child, depth + 1);
        }
        out.push_str(&format!("{pad}</g>\n"));
        return;
    }
    let transform = if n.rotation != 0.0 {
        format!(
            " transform=\"rotate({} {} {})\"",
            n.rotation,
            n.x + n.width / 2.0,
            n.y + n.height / 2.0
        )
    } else {
        String::new()
    };
    let common = format!(
        " id=\"{}\" opacity=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{}",
        xml_escape(&n.id),
        n.opacity,
        xml_escape(&n.fill),
        xml_escape(&n.stroke),
        n.stroke_width,
        transform
    );
    match n.kind {
        NodeKind::Rectangle => out.push_str(&format!(
            "{pad}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{common}/>\n",
            n.x, n.y, n.width, n.height, n.corner_radius
        )),
        NodeKind::Image => out.push_str(&format!(
            "{pad}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{common}/><!-- Classic image layer: {} -->\n",
            n.x,
            n.y,
            n.width,
            n.height,
            n.corner_radius,
            xml_escape(n.asset_id.as_deref().unwrap_or("unlinked"))
        )),
        NodeKind::Ellipse => out.push_str(&format!(
            "{pad}<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{common}/>\n",
            n.x + n.width / 2.0,
            n.y + n.height / 2.0,
            n.width / 2.0,
            n.height / 2.0
        )),
        NodeKind::Polygon => out.push_str(&format!(
            "{pad}<polygon points=\"{}\"{common}/>\n",
            polygon_points(n)
        )),
        NodeKind::Line => out.push_str(&format!(
            "{pad}<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{common}/>\n",
            n.x,
            n.y,
            n.x + n.width,
            n.y + n.height
        )),
        NodeKind::Text => {
            let text = n.text.as_deref().unwrap_or("");
            let mut lines = text.split('\n').peekable();
            out.push_str(&format!(
                "{pad}<text x=\"{}\" y=\"{}\" font-size=\"{}\"{common}>",
                n.x,
                n.y + n.height,
                n.height / 1.2
            ));
            if let Some(first) = lines.next() {
                out.push_str(&xml_escape(first));
            }
            for line in lines {
                out.push_str(&format!(
                    "<tspan x=\"{}\" dy=\"1.2em\">{}</tspan>",
                    n.x,
                    xml_escape(line)
                ));
            }
            out.push_str("</text>\n");
        }
        NodeKind::Frame => {}
    }
}

fn polygon_points(node: &DesignNode) -> String {
    const SIDES: usize = 5;
    let cx = node.x + node.width * 0.5;
    let cy = node.y + node.height * 0.5;
    let rx = node.width * 0.5;
    let ry = node.height * 0.5;
    (0..SIDES)
        .map(|index| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::TAU / SIDES as f32;
            format!("{},{}", cx + rx * angle.cos(), cy + ry * angle.sin())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Import a deliberately small SVG subset as a new one-page Classic document.
/// Supports rect, ellipse, polygon, line, and text elements with basic geometry and
/// inline presentation attributes. Other elements are ignored; paths, gradients,
/// filters, CSS stylesheets, clipping, and full SVG text layout are unsupported.
pub fn import_svg(
    source: &str,
    document_name: impl Into<String>,
) -> Result<DesignDocument, FormatError> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut nodes = Vec::new();
    let mut stack: Vec<HashMap<String, String>> = Vec::new();
    let mut current_text: Option<(DesignNode, String)> = None;
    let mut width = 1000.0;
    let mut height = 800.0;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
                let (e, empty) = match event {
                    Event::Start(e) => (e, false),
                    Event::Empty(e) => (e, true),
                    _ => unreachable!(),
                };
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                let mut attrs = HashMap::new();
                for attr in e.attributes().with_checks(false) {
                    let attr = attr.map_err(|e| FormatError::Xml(e.to_string()))?;
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref())
                        .to_ascii_lowercase();
                    let value = attr
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::default(),
                            reader.decoder(),
                        )
                        .map_err(|e| FormatError::Xml(e.to_string()))?
                        .into_owned();
                    attrs.insert(key, value);
                }
                if tag == "svg" {
                    width = attrs.get("width").and_then(|v| number(v)).unwrap_or(width);
                    height = attrs
                        .get("height")
                        .and_then(|v| number(v))
                        .unwrap_or(height);
                    if let Some(vb) = attrs.get("viewbox") {
                        let nums: Vec<f32> = vb
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .filter_map(number)
                            .collect();
                        if nums.len() == 4 {
                            width = nums[2];
                            height = nums[3];
                        }
                    }
                }
                let mut inherited = HashMap::new();
                for parent_attrs in &stack {
                    inherited.extend(parent_attrs.clone());
                }
                let node = parse_svg_node(&tag, &attrs, &inherited);
                if tag == "text" {
                    if let Some(mut n) = node {
                        n.text = Some(String::new());
                        current_text = Some((n, String::new()));
                    }
                } else if let Some(n) = node {
                    nodes.push(n);
                }
                if !empty {
                    stack.push(attrs);
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "text" {
                    if let Some((mut node, content)) = current_text.take() {
                        node.text = Some(content);
                        nodes.push(node);
                    }
                }
                stack.pop();
            }
            Ok(Event::Text(e)) => {
                if let Some((_, content)) = current_text.as_mut() {
                    let decoded = e.decode().map_err(|e| FormatError::Xml(e.to_string()))?;
                    let unescaped = quick_xml::escape::unescape(&decoded)
                        .map_err(|e| FormatError::Xml(e.to_string()))?;
                    content.push_str(&unescaped);
                }
            }
            Ok(Event::CData(e)) => {
                if let Some((_, content)) = current_text.as_mut() {
                    content.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(FormatError::Xml(e.to_string())),
        }
        buf.clear();
    }
    let page_id = format!("page-{}", uuid::Uuid::new_v4());
    let doc = DesignDocument {
        schema_version: 1,
        id: format!("doc-{}", uuid::Uuid::new_v4()),
        name: document_name.into(),
        active_page_id: page_id.clone(),
        pages: vec![Page {
            id: page_id,
            name: "Imported SVG".into(),
            nodes,
        }],
        guides: Default::default(),
        assets: Default::default(),
        tokens: Default::default(),
        comments: Default::default(),
        components: Default::default(),
        prototype: Default::default(),
    };
    let _ = (width, height);
    validate_document(&doc)?;
    Ok(doc)
}

fn parse_svg_node(
    tag: &str,
    attrs: &HashMap<String, String>,
    inherited: &HashMap<String, String>,
) -> Option<DesignNode> {
    let kind = match tag {
        "rect" => NodeKind::Rectangle,
        "ellipse" | "circle" => NodeKind::Ellipse,
        "polygon" => NodeKind::Polygon,
        "line" => NodeKind::Line,
        "text" => NodeKind::Text,
        _ => return None,
    };
    let mut a = inherited.clone();
    a.extend(attrs.clone());
    if let Some(style) = a.get("style").cloned() {
        for declaration in style.split(';') {
            if let Some((key, value)) = declaration.split_once(':') {
                a.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }
    }
    let get = |key: &str, default: f32| a.get(key).and_then(|v| number(v)).unwrap_or(default);
    let (x, y, w, h) = match tag {
        "rect" => (
            get("x", 0.0),
            get("y", 0.0),
            get("width", 0.0),
            get("height", 0.0),
        ),
        "ellipse" => {
            let rx = get("rx", 0.0);
            let ry = get("ry", rx);
            (get("cx", 0.0) - rx, get("cy", 0.0) - ry, rx * 2.0, ry * 2.0)
        }
        "circle" => {
            let r = get("r", 0.0);
            (get("cx", 0.0) - r, get("cy", 0.0) - r, r * 2.0, r * 2.0)
        }
        "line" => {
            let x1 = get("x1", 0.0);
            let y1 = get("y1", 0.0);
            (x1, y1, get("x2", x1) - x1, get("y2", y1) - y1)
        }
        "polygon" => polygon_bounds(a.get("points")?)?,
        _ => (
            get("x", 0.0),
            get("y", 0.0),
            get("width", 100.0),
            get("height", 24.0),
        ),
    };
    let mut node = DesignNode::new(kind.clone(), x, y);
    node.id = a
        .get("id")
        .cloned()
        .unwrap_or_else(|| format!("svg-{}", uuid::Uuid::new_v4()));
    node.name = a
        .get("id")
        .cloned()
        .unwrap_or_else(|| format!("SVG {}", tag));
    node.width = w.max(0.0);
    node.height = h.max(0.0);
    node.fill = a.get("fill").cloned().unwrap_or_else(|| {
        if tag == "line" {
            "transparent".into()
        } else {
            "#000000".into()
        }
    });
    node.stroke = a
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| "transparent".into());
    node.stroke_width = get("stroke-width", 1.0);
    node.opacity = get("opacity", 1.0) * get("fill-opacity", 1.0);
    node.corner_radius = get("rx", 0.0);
    node.visible = a.get("display").is_none_or(|v| v != "none")
        && a.get("visibility").is_none_or(|v| v != "hidden");
    if kind == NodeKind::Text {
        let font_size = get("font-size", 16.0);
        node.y -= font_size;
        node.height = font_size * 1.2;
        node.width = get("width", 100.0);
    }
    if let Some(transform) = a.get("transform") {
        node.rotation = parse_rotate(transform).unwrap_or(0.0);
    }
    Some(node)
}

fn polygon_bounds(points: &str) -> Option<(f32, f32, f32, f32)> {
    let values = points
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .filter_map(number)
        .collect::<Vec<_>>();
    if values.len() < 6 || values.len() % 2 != 0 {
        return None;
    }
    let mut xs = values.iter().step_by(2).copied();
    let mut ys = values.iter().skip(1).step_by(2).copied();
    let first_x = xs.next()?;
    let first_y = ys.next()?;
    let (mut min_x, mut max_x) = (first_x, first_x);
    let (mut min_y, mut max_y) = (first_y, first_y);
    for x in xs {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
    }
    for y in ys {
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    Some((min_x, min_y, max_x - min_x, max_y - min_y))
}

fn number(value: &str) -> Option<f32> {
    let trimmed = value.trim().trim_end_matches("px").trim_end_matches("pt");
    trimmed.parse::<f32>().ok().filter(|v| v.is_finite())
}

fn parse_rotate(s: &str) -> Option<f32> {
    let start = s.find("rotate(")? + 7;
    let end = s[start..].find(')')? + start;
    s[start..end]
        .split(|c: char| c.is_whitespace() || c == ',')
        .find_map(number)
}

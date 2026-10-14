//! A deliberately small, read-only MCP server for local `.classic` documents.
//!
//! The server speaks JSON-RPC 2.0 over MCP's `Content-Length` stdio framing.
//! It never opens a network connection, evaluates supplied paths, or writes a
//! document. Paths are canonicalized and must remain below `CLASSIC_MCP_ROOT`.

use serde_json::{Map, Value, json};
use std::{
    env, fmt, fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

const JSON_RPC: &str = "2.0";
const MAX_MESSAGE_BYTES: usize = 1_048_576;
const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
const PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Canonical directory that contains every document this process may read.
    pub allowed_root: PathBuf,
    pub max_document_bytes: u64,
}

impl ServerConfig {
    /// Resolves `CLASSIC_MCP_ROOT`, falling back to the process working directory.
    pub fn from_environment() -> Result<Self, ServerError> {
        let root = env::var_os("CLASSIC_MCP_ROOT")
            .map(PathBuf::from)
            .unwrap_or(env::current_dir().map_err(ServerError::Io)?);
        Self::new(root)
    }

    pub fn new(root: impl AsRef<Path>) -> Result<Self, ServerError> {
        let root = fs::canonicalize(root).map_err(ServerError::Io)?;
        if !root.is_dir() {
            return Err(ServerError::InvalidRoot(root));
        }
        Ok(Self {
            allowed_root: root,
            max_document_bytes: MAX_DOCUMENT_BYTES,
        })
    }

    fn resolve_document(&self, supplied: &str) -> Result<PathBuf, RpcFailure> {
        if supplied.is_empty() || supplied.contains('\0') {
            return Err(RpcFailure::invalid_params(
                "path must be a non-empty file path",
            ));
        }
        let requested = Path::new(supplied);
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.allowed_root.join(requested)
        };
        let canonical = fs::canonicalize(&candidate).map_err(|_| {
            RpcFailure::invalid_params("path must identify an existing readable file")
        })?;
        if !canonical.starts_with(&self.allowed_root) {
            return Err(RpcFailure::invalid_params(
                "path must resolve under CLASSIC_MCP_ROOT",
            ));
        }
        let extension_is_classic = canonical
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("classic"));
        if !extension_is_classic {
            return Err(RpcFailure::invalid_params(
                "path must have a .classic extension",
            ));
        }
        let metadata = fs::metadata(&canonical)
            .map_err(|_| RpcFailure::invalid_params("path must identify a readable file"))?;
        if !metadata.is_file() {
            return Err(RpcFailure::invalid_params(
                "path must identify a regular file",
            ));
        }
        if metadata.len() > self.max_document_bytes {
            return Err(RpcFailure::invalid_params(format!(
                "document exceeds the {} byte limit",
                self.max_document_bytes
            )));
        }
        Ok(canonical)
    }
}

#[derive(Debug)]
pub enum ServerError {
    Io(io::Error),
    InvalidRoot(PathBuf),
    Framing(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidRoot(root) => {
                write!(f, "allowed root is not a directory: {}", root.display())
            }
            Self::Framing(message) => write!(f, "invalid stdio framing: {message}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Runs a blocking local stdio server. This function has no network behavior.
pub fn run_stdio(config: ServerConfig) -> Result<(), ServerError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    while let Some(message) = read_message(&mut reader)? {
        if let Some(response) = handle_json_message(&config, &message) {
            write_message(&mut writer, &response)?;
        }
    }
    Ok(())
}

/// Handles a decoded JSON-RPC request. Public for embedding in a local host.
pub fn handle_json_message(config: &ServerConfig, message: &[u8]) -> Option<Value> {
    let value = match serde_json::from_slice::<Value>(message) {
        Ok(value) => value,
        Err(_) => return Some(error_response(Value::Null, -32700, "Parse error", None)),
    };
    let object = match value.as_object() {
        Some(object) => object,
        None => return Some(error_response(Value::Null, -32600, "Invalid Request", None)),
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some(JSON_RPC) {
        return Some(error_response(
            object.get("id").cloned().unwrap_or(Value::Null),
            -32600,
            "Invalid Request",
            Some(json!({ "reason": "jsonrpc must be 2.0" })),
        ));
    }
    let id = object.get("id").cloned();
    let method = match object.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            return id.map(|id| {
                error_response(
                    id,
                    -32600,
                    "Invalid Request",
                    Some(json!({ "reason": "method is required" })),
                )
            });
        }
    };
    let result = dispatch(config, method, object.get("params"));
    id.map(|id| match result {
        Ok(result) => json!({ "jsonrpc": JSON_RPC, "id": id, "result": result }),
        Err(failure) => error_response(id, failure.code, failure.message, failure.data),
    })
}

fn dispatch(
    config: &ServerConfig,
    method: &str,
    params: Option<&Value>,
) -> Result<Value, RpcFailure> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "classic-mcp", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Local, read-only access to structurally validated .classic documents under CLASSIC_MCP_ROOT."
        })),
        "notifications/initialized" => Ok(Value::Null),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(config, params),
        _ => Err(RpcFailure::method_not_found(method)),
    }
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "inspect_document",
            "description": "Read a structurally validated local .classic document and return document, page, asset, token, component, comment, guide, and prototype counts. Read-only.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": { "path": { "type": "string", "description": "A .classic path below CLASSIC_MCP_ROOT." } }
            }
        }),
        json!({
            "name": "list_layers",
            "description": "List flattened layers from one page of a structurally validated local .classic document. Read-only.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "A .classic path below CLASSIC_MCP_ROOT." },
                    "pageId": { "type": "string", "description": "Optional page id; defaults to the document active page." }
                }
            }
        }),
    ]
}

fn call_tool(config: &ServerConfig, params: Option<&Value>) -> Result<Value, RpcFailure> {
    let params = params
        .and_then(Value::as_object)
        .ok_or_else(|| RpcFailure::invalid_params("tools/call params must be an object"))?;
    reject_unknown(params, &["name", "arguments"])?;
    let name = required_string(params, "name")?;
    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .ok_or_else(|| RpcFailure::invalid_params("arguments must be an object"))?;
    let output = match name {
        "inspect_document" => inspect_document(config, arguments)?,
        "list_layers" => list_layers(config, arguments)?,
        _ => return Err(RpcFailure::invalid_params("unknown tool name")),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&output).expect("JSON value serializes") }],
        "structuredContent": output,
        "isError": false
    }))
}

fn inspect_document(
    config: &ServerConfig,
    arguments: &Map<String, Value>,
) -> Result<Value, RpcFailure> {
    reject_unknown(arguments, &["path"])?;
    let path = config.resolve_document(required_string(arguments, "path")?)?;
    let document = read_document(&path)?;
    let object = required_object(&document, "document")?;
    let pages = required_array(object, "pages")?;
    let node_count = pages
        .iter()
        .map(|page| {
            page.as_object().map_or(0, |page| {
                count_nodes(required_array(page, "nodes").unwrap_or(&[]))
            })
        })
        .sum::<usize>();
    let pages = pages
        .iter()
        .filter_map(Value::as_object)
        .map(|page| {
            json!({
                "id": page.get("id").and_then(Value::as_str).unwrap_or_default(),
                "name": page.get("name").and_then(Value::as_str).unwrap_or_default(),
                "layerCount": count_nodes(required_array(page, "nodes").unwrap_or(&[]))
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "path": display_relative(&config.allowed_root, &path),
        "document": {
            "id": required_string(object, "id")?,
            "name": required_string(object, "name")?,
            "schemaVersion": object.get("schemaVersion").and_then(Value::as_u64).unwrap_or_default(),
            "activePageId": required_string(object, "activePageId")?,
            "pageCount": pages.len(),
            "layerCount": node_count,
            "assetCount": nested_map_count(object, "assets", "assets"),
            "tokenCount": token_count(object),
            "componentCount": nested_array_count(object, "components", "definitions"),
            "commentCount": nested_array_count(object, "comments", "threads"),
            "guideCount": nested_array_count(object, "guides", "guides"),
            "prototypeInteractionCount": nested_array_count(object, "prototype", "interactions"),
            "pages": pages
        }
    }))
}

fn list_layers(config: &ServerConfig, arguments: &Map<String, Value>) -> Result<Value, RpcFailure> {
    reject_unknown(arguments, &["path", "pageId"])?;
    let path = config.resolve_document(required_string(arguments, "path")?)?;
    let document = read_document(&path)?;
    let object = required_object(&document, "document")?;
    let page_id =
        optional_string(arguments, "pageId")?.unwrap_or(required_string(object, "activePageId")?);
    let page = required_array(object, "pages")?
        .iter()
        .filter_map(Value::as_object)
        .find(|page| page.get("id").and_then(Value::as_str) == Some(page_id))
        .ok_or_else(|| {
            RpcFailure::invalid_params(format!(
                "pageId `{page_id}` does not exist in this document"
            ))
        })?;
    let mut layers = Vec::new();
    flatten_layers(required_array(page, "nodes")?, None, 0, &mut layers);
    Ok(json!({
        "path": display_relative(&config.allowed_root, &path),
        "page": { "id": required_string(page, "id")?, "name": required_string(page, "name")? },
        "layers": layers
    }))
}

fn read_document(path: &Path) -> Result<Value, RpcFailure> {
    let source = fs::read_to_string(path)
        .map_err(|_| RpcFailure::tool_error("could not read the document"))?;
    let file = serde_json::from_str::<Value>(&source)
        .map_err(|_| RpcFailure::tool_error("document is not valid JSON"))?;
    let file = file
        .as_object()
        .ok_or_else(|| RpcFailure::tool_error("document root must be an object"))?;
    if file.get("format").and_then(Value::as_str) != Some("classic") {
        return Err(RpcFailure::tool_error("document format must be `classic`"));
    }
    if file.get("formatVersion").and_then(Value::as_u64) != Some(1) {
        return Err(RpcFailure::tool_error("unsupported Classic format version"));
    }
    let document = file
        .get("document")
        .ok_or_else(|| RpcFailure::tool_error("document payload is missing"))?;
    validate_document_shape(document)?;
    Ok(document.clone())
}

fn validate_document_shape(document: &Value) -> Result<(), RpcFailure> {
    let object = required_object(document, "document")?;
    let active_page_id = required_string(object, "activePageId")?;
    required_string(object, "id")?;
    required_string(object, "name")?;
    let pages = required_array(object, "pages")?;
    if pages.is_empty() {
        return Err(RpcFailure::tool_error(
            "document must contain at least one page",
        ));
    }
    let mut active_page_found = false;
    for page in pages {
        let page = required_object(page, "page")?;
        if required_string(page, "id")? == active_page_id {
            active_page_found = true;
        }
        required_string(page, "name")?;
        for node in required_array(page, "nodes")? {
            validate_node_shape(node)?;
        }
    }
    if !active_page_found {
        return Err(RpcFailure::tool_error(
            "activePageId does not identify a page",
        ));
    }
    Ok(())
}

fn validate_node_shape(node: &Value) -> Result<(), RpcFailure> {
    let node = required_object(node, "layer")?;
    required_string(node, "id")?;
    required_string(node, "name")?;
    required_string(node, "kind")?;
    if let Some(children) = node.get("children") {
        for child in children
            .as_array()
            .ok_or_else(|| RpcFailure::tool_error("layer children must be an array"))?
        {
            validate_node_shape(child)?;
        }
    }
    Ok(())
}

fn flatten_layers(nodes: &[Value], parent_id: Option<&str>, depth: usize, output: &mut Vec<Value>) {
    for node in nodes {
        let Some(node) = node.as_object() else {
            continue;
        };
        let id = node.get("id").and_then(Value::as_str).unwrap_or_default();
        output.push(json!({
            "id": id,
            "name": node.get("name").and_then(Value::as_str).unwrap_or_default(),
            "kind": node.get("kind").and_then(Value::as_str).unwrap_or_default(),
            "parentId": parent_id,
            "depth": depth,
            "visible": node.get("visible").and_then(Value::as_bool).unwrap_or(true),
            "locked": node.get("locked").and_then(Value::as_bool).unwrap_or(false),
            "bounds": {
                "x": node.get("x").and_then(Value::as_f64).unwrap_or_default(),
                "y": node.get("y").and_then(Value::as_f64).unwrap_or_default(),
                "width": node.get("width").and_then(Value::as_f64).unwrap_or_default(),
                "height": node.get("height").and_then(Value::as_f64).unwrap_or_default()
            },
            "childCount": node.get("children").and_then(Value::as_array).map_or(0, Vec::len),
            "assetId": node.get("assetId"),
            "componentId": node.get("componentId")
        }));
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            flatten_layers(children, Some(id), depth + 1, output);
        }
    }
}

fn count_nodes(nodes: &[Value]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + node
                .as_object()
                .and_then(|node| node.get("children"))
                .and_then(Value::as_array)
                .map_or(0, |children| count_nodes(children))
        })
        .sum()
}

fn nested_array_count(document: &Map<String, Value>, parent: &str, child: &str) -> usize {
    document
        .get(parent)
        .and_then(Value::as_object)
        .and_then(|value| value.get(child))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn nested_map_count(document: &Map<String, Value>, parent: &str, child: &str) -> usize {
    document
        .get(parent)
        .and_then(Value::as_object)
        .and_then(|value| value.get(child))
        .and_then(Value::as_object)
        .map_or(0, Map::len)
}

fn token_count(document: &Map<String, Value>) -> usize {
    document
        .get("tokens")
        .and_then(Value::as_object)
        .and_then(|tokens| tokens.get("collections"))
        .and_then(Value::as_object)
        .map_or(0, |collections| {
            collections
                .values()
                .map(|collection| {
                    collection
                        .as_object()
                        .and_then(|collection| collection.get("tokens"))
                        .and_then(Value::as_object)
                        .map_or(0, Map::len)
                })
                .sum()
        })
}

fn required_object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>, RpcFailure> {
    value
        .as_object()
        .ok_or_else(|| RpcFailure::tool_error(format!("{name} must be an object")))
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a [Value], RpcFailure> {
    object
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| RpcFailure::tool_error(format!("{name} must be an array")))
}

fn required_string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, RpcFailure> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RpcFailure::tool_error(format!("{name} must be a non-empty string")))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, RpcFailure> {
    match object.get(name) {
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| RpcFailure::invalid_params(format!("{name} must be a string"))),
        None => Ok(None),
    }
}

fn reject_unknown(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RpcFailure> {
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(RpcFailure::invalid_params(format!(
            "unknown parameter `{name}`"
        )));
    }
    Ok(())
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
struct RpcFailure {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcFailure {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: "Invalid params".into(),
            data: Some(json!({ "reason": message.into() })),
        }
    }
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: "Method not found".into(),
            data: Some(json!({ "method": method })),
        }
    }
    fn tool_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: "Tool execution failed".into(),
            data: Some(json!({ "reason": message.into() })),
        }
    }
}

fn error_response(id: Value, code: i64, message: impl Into<String>, data: Option<Value>) -> Value {
    let mut error = Map::new();
    error.insert("code".into(), Value::from(code));
    error.insert("message".into(), Value::String(message.into()));
    if let Some(data) = data {
        error.insert("data".into(), data);
    }
    json!({ "jsonrpc": JSON_RPC, "id": id, "error": error })
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, ServerError> {
    let mut content_length = None;
    let mut saw_header = false;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).map_err(ServerError::Io)?;
        if bytes == 0 {
            if saw_header {
                return Err(ServerError::Framing("unexpected EOF in headers".into()));
            }
            return Ok(None);
        }
        saw_header = true;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ServerError::Framing("malformed header".into()))?;
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(ServerError::Framing("duplicate Content-Length".into()));
            }
            let length = value
                .trim()
                .parse::<usize>()
                .map_err(|_| ServerError::Framing("invalid Content-Length".into()))?;
            if length > MAX_MESSAGE_BYTES {
                return Err(ServerError::Framing("message exceeds size limit".into()));
            }
            content_length = Some(length);
        }
    }
    let length =
        content_length.ok_or_else(|| ServerError::Framing("missing Content-Length".into()))?;
    let mut message = vec![0; length];
    reader.read_exact(&mut message).map_err(ServerError::Io)?;
    Ok(Some(message))
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<(), ServerError> {
    let encoded =
        serde_json::to_vec(message).map_err(|error| ServerError::Framing(error.to_string()))?;
    write!(writer, "Content-Length: {}\r\n\r\n", encoded.len()).map_err(ServerError::Io)?;
    writer.write_all(&encoded).map_err(ServerError::Io)?;
    writer.flush().map_err(ServerError::Io)
}

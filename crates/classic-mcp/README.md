# classic-mcp

`classic-mcp` is a local, read-only Model Context Protocol server for valid
Classic Studio `.classic` files. It communicates over stdio using JSON-RPC 2.0
with `Content-Length` framing. It does not connect to a network and it does not
write, modify, delete, or export a document.

## Run

Set the only directory the process is allowed to read, then run the binary:

```powershell
$env:CLASSIC_MCP_ROOT = "C:\Users\me\Documents\Classic"
cargo run --manifest-path crates/classic-mcp/Cargo.toml
```

When `CLASSIC_MCP_ROOT` is absent, the process working directory is used. The
root must already exist. Every tool path is canonicalized, must resolve below
that root, must be a regular file with a `.classic` extension, and is limited to
32 MiB. Those checks also prevent `..` traversal and symlink escape.

## Implemented tools

| Tool | Result |
| --- | --- |
| `inspect_document` | Validated document metadata and counts, including pages, layers, assets, tokens, comments, components, guides, and prototype interactions. |
| `list_layers` | A flattened layer tree for the active page or a supplied `pageId`. |

`initialize` and `tools/list` are implemented. The service deliberately does
not expose any document editing, collaboration, filesystem listing, arbitrary
file read, shell execution, HTTP, or cloud capability.

## MCP client configuration

Point a stdio MCP client at the compiled `classic-mcp` executable and set
`CLASSIC_MCP_ROOT` in the server environment. Keep the allowed root limited to
a dedicated Classic-project directory instead of a broad home directory.

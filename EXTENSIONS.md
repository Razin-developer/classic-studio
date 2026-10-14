# Extensions and local MCP roadmap

Classic Studio will expose extensions through a versioned, capability-limited interface. The extension system and local MCP server are planned; there is no supported third-party plugin API yet.

## Design goals

- Keep `.classic` data portable and valid when an extension is missing.
- Give each extension only the permissions it requests and the user approves.
- Run extensions out of process or inside a sandboxed host boundary.
- Make document mutation explicit, undoable, validated, and attributable.
- Support local-first workflows before optional cloud integration.

## Extension manifest concept

An extension package should declare an identifier, semantic version, supported host range, entrypoint, UI contributions, and requested capabilities. Proposed capability groups include:

| Capability | Purpose |
|---|---|
| `document.read` | Read a scoped document snapshot |
| `document.write` | Submit validated editor commands through the undo system |
| `selection.read` | Read the active selection |
| `assets.read` | Access user-approved project assets |
| `assets.write` | Import validated assets into a project |
| `network.fetch` | Make network requests to user-approved origins |
| `storage.local` | Keep extension-local settings |
| `ui.panel` | Add a dockable extension panel |
| `export.provider` | Register a validated export target |
| `mcp.tool` | Offer an MCP tool through the local host |

The host should display requested permissions at installation and whenever a later release expands them. Extensions must not receive raw application credentials, database connection strings, or unrestricted filesystem access.

## Host API shape

The proposed API uses commands instead of direct mutable references:

1. An extension requests a read-only snapshot for its permitted scope.
2. It creates typed editor commands or asset-import requests.
3. The host validates commands against the current `.classic` schema.
4. The host applies accepted commands as one undoable transaction and returns the new revision.
5. The extension receives document-change notifications for its permitted scope.

This avoids partial edits, makes collaboration operations auditable, and allows future compatibility checks.

## Local MCP server concept

The planned local MCP server lets a user-approved model or developer tool inspect and edit a running Classic Studio session. Initial tools should stay narrow:

- Read a document summary, pages, layers, components, tokens, and selection.
- Create validated layers, frames, text, and styles.
- Apply a named layout command.
- Import an approved local asset.
- Export a selected page.
- Return diagnostics for invalid commands.

Mutating calls should require a visible local approval policy and always enter the normal undo history. The server should bind to loopback by default, use a per-session token, limit request sizes and rates, and redact credentials from responses and logs.

## Delivery checkpoints

1. Publish the `.classic` schema and command validation contract.
2. Ship a built-in extension-host proof of concept with a single read-only panel.
3. Add permission prompts, isolation, command transactions, and compatibility tests.
4. Add the local MCP server with loopback-only defaults and explicit write approval.
5. Document review, signing, revocation, and a future public registry process.

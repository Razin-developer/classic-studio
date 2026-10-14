# `classic-extensions`

`classic-extensions` is a Rust-only schema and validator for Classic Studio extension manifests. It is deliberately not an extension runtime: it never loads an extension, invokes a process, reads an asset, accesses the network, or grants capabilities.

## Security boundary

An extension package declares what it wants in `requestedCapabilities`; this declaration grants nothing. The desktop host validates the manifest against an `ExtensionPolicy` that records its trusted package roots, trusted registry publishers, allowed capabilities, and contribution limit.

The default policy permits only constrained host APIs (`documentRead`, `userInterface`, and `export`) and trusts no package location. `network`, `filesystemRead`, `filesystemWrite`, and `processExecution` remain denied until a user or administrator adds explicit consent. A tool also needs `documentWrite` and `userInterface`; an exporter needs `documentRead` and `export`.

Local packages must use an absolute path below a configured trusted root and may not contain `..` traversal. Registry packages must be from an allow-listed registry and publisher and include a signing-key identifier when signature enforcement is on. Signature verification itself belongs in the package installer before this policy check.

## Manifest example

```json
{
  "schemaVersion": 1,
  "id": "acme-color-audit",
  "name": "Acme Color Audit",
  "version": "1.2.3",
  "origin": {
    "kind": "local",
    "path": "C:\\ClassicExtensions\\acme-color-audit"
  },
  "requestedCapabilities": ["documentRead", "documentWrite", "userInterface"],
  "contributions": [
    {
      "type": "tool",
      "definition": {
        "id": "color-audit",
        "label": "Color audit",
        "icon": "palette"
      }
    }
  ]
}
```

## Host validation

```rust
use classic_extensions::ExtensionPolicy;

let policy = ExtensionPolicy::for_local_development(r"C:\ClassicExtensions");
manifest.validate(&policy)?;
```

`for_local_development` is opt-in and is appropriate only for a developer-controlled directory. A production host should construct an `ExtensionPolicy` from persisted user or administrator consent and independently verify package signatures before accepting a registry origin.

## Contribution points

The schema accepts three finite contribution kinds:

- `tool`: a named design-toolbar command;
- `exporter`: a controlled document export format and MIME types;
- `panel`: a dockable UI panel at a host-defined placement.

The manifest contains no unrestricted JavaScript, shell command, URL launcher, filesystem path grants, or generic plugin entry point. Hosts can map approved contributions to a sandboxed Rust/WASM extension runtime later without weakening this validation boundary.

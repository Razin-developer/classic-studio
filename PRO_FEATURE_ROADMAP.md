# Classic Studio capability roadmap

Classic Studio is an independent, Rust-native design editor with its own product identity and `.classic` format. This is a delivery plan, not a claim of compatibility or equivalence with another product.

## Status legend

| Marker | Meaning |
|---|---|
| 🟢 | Implemented in the local desktop application |
| 🟡 | Partial implementation or data-model foundation; important workflow gaps remain |
| 🔴 | Planned; no user-ready implementation yet |

## Design, document, and prototype capabilities

| # | Capability | Status |
|---:|---|---|
| 1 | Infinite vector canvas | 🟡 GPU canvas with pan and zoom |
| 2 | Multi-page files | 🟢 Native pages in `.classic` documents |
| 3 | Frames and artboards | 🟡 Frame layers and basic editing |
| 4 | Vector network editing | 🔴 Planned |
| 5 | Boolean operations | 🔴 Planned |
| 6 | Masking | 🔴 Planned |
| 7 | Shape library | 🟡 Rectangles, ellipses, lines, frames, and text |
| 8 | Pen and pencil drawing | 🔴 Planned |
| 9 | Image placement and editing | 🟡 Linked image layers; scoped raster rendering |
| 10 | Blend modes | 🔴 Planned |
| 11 | Effects | 🔴 Planned |
| 12 | Grid and layout guides | 🟡 Grid, rulers, persisted guides, and snapping |
| 13 | Auto layout | 🟡 Horizontal and vertical stack commands |
| 14 | Responsive constraints | 🟡 Foundation only |
| 15 | Rich text formatting | 🟡 Basic text layers |
| 16 | Text auto-size | 🔴 Planned |
| 17 | Local styles | 🟡 Token library foundation |
| 18 | Shared team libraries | 🔴 Planned |
| 19 | Design tokens and variables | 🟡 Token persistence foundation |
| 20 | Variable modes | 🔴 Planned |
| 21 | Component publishing | 🟡 Local saved definitions |
| 22 | Component instances | 🟡 Local linked placements |
| 23 | Component properties | 🟡 Data-model foundation |
| 24 | Interactive components | 🔴 Planned |
| 25 | Component documentation | 🔴 Planned |
| 26 | Local font management | 🟡 Catalog foundation |
| 27 | Outline text conversion | 🔴 Planned |
| 28 | Rulers and measurement guides | 🟡 Interactive rulers and guides |
| 29 | Prototype connections | 🟡 Persisted local links |
| 30 | Prototype triggers | 🟡 Click trigger authoring |
| 31 | Prototype actions | 🟡 Navigate action authoring |
| 32 | Transition types | 🟡 Dissolve transition data |
| 33 | Smart animation | 🔴 Planned |
| 34 | Overlays | 🔴 Planned |
| 35 | Scroll overflow | 🔴 Planned |
| 36 | Fixed-position elements | 🔴 Planned |
| 37 | Video and GIF support | 🔴 Planned |
| 38 | Device shell frames | 🔴 Planned |
| 39 | Presentation mode | 🟡 Local interactive frame viewer |
| 40 | Prototype link sharing | 🔴 Planned |

## Collaboration, review, and workspace capabilities

| # | Capability | Status |
|---:|---|---|
| 41 | Real-time multi-cursor collaboration | 🔴 Planned |
| 42 | Live audio conversations | 🔴 Planned |
| 43 | Pinned comments and threads | 🟡 Local persisted threads |
| 44 | Follow mode | 🔴 Planned |
| 45 | Version history | 🔴 Planned |
| 46 | Version comparison and restore | 🔴 Planned |
| 47 | Private projects | 🔴 Planned |
| 48 | Granular permissions | 🔴 Planned |
| 49 | Branching and merging | 🔴 Planned |
| 50 | Activity logs and audit trails | 🔴 Planned |
| 51 | Guest collaborators | 🔴 Planned |
| 52 | View-only seats | 🔴 Planned |
| 53 | Link previews | 🔴 Planned |
| 54 | Spotlight mode | 🔴 Planned |
| 55 | User presence | 🔴 Planned |

## Handoff, ecosystem, and AI capabilities

| # | Capability | Status |
|---:|---|---|
| 56 | Developer mode | 🟡 Native Dev inspector |
| 57 | Code generation | 🟡 Selected-layer CSS |
| 58 | Asset inspection and export | 🟡 Scoped SVG and PNG export |
| 59 | Spacing and dimension inspection | 🟡 Basic inspection |
| 60 | Token mapping inspection | 🟡 Foundation |
| 61 | Ready-for-development status | 🔴 Planned |
| 62 | Component sandbox | 🔴 Planned |
| 63 | Plugins and extensions | 🔴 Planned |
| 64 | Code Connect-style bindings | 🔴 Planned |
| 65 | Annotations and specifications | 🔴 Planned |
| 66 | Copy layer attributes | 🟡 Copy selected CSS |
| 67 | File comparison | 🔴 Planned |
| 68 | API export integration | 🔴 Planned |
| 69 | Plugin marketplace | 🔴 Planned |
| 70 | Canvas widgets | 🔴 Planned |
| 71 | Plugin sandbox/API | 🔴 Planned |
| 72 | Programmatic file API | 🔴 Planned |
| 73 | Webhook notifications | 🔴 Planned |
| 74 | Community publishing | 🔴 Planned |
| 75 | AI screen generation | 🔴 Planned |
| 76 | AI background removal | 🔴 Planned |
| 77 | AI text tools | 🔴 Planned |
| 78 | AI visual search | 🔴 Planned |
| 79 | AI asset renaming | 🔴 Planned |
| 80 | AI auto-layout suggestions | 🔴 Planned |
| 81 | Presentation workspace | 🔴 Planned |
| 82 | Whiteboard workspace | 🔴 Planned |
| 83 | Accessibility checks | 🟡 Editable-background contrast check |
| 84 | Spell check and localization | 🔴 Planned |
| 85 | Open-format migration/import | 🟡 Scoped SVG import |
| 86 | Cloud file storage | 🔴 Planned |
| 87 | Folder hierarchy | 🔴 Planned |
| 88 | Drafts workspace | 🔴 Planned |
| 89 | Trash and recovery | 🟡 Local recovery snapshot |
| 90 | Global search | 🔴 Planned |
| 91 | Recent-file quick switch | 🔴 Planned |
| 92 | Starred files | 🔴 Planned |
| 93 | Workspace analytics | 🔴 Planned |
| 94 | Team branding | 🔴 Planned |
| 95 | Seat management | 🔴 Planned |
| 96 | Single sign-on | 🔴 Planned |
| 97 | Domain restrictions | 🔴 Planned |
| 98 | Export logs and archival | 🔴 Planned |
| 99 | Keyboard shortcuts manager | 🟡 In-app shortcut reference |
| 100 | Help and community portal | 🔴 Planned |

## Hosted, distribution, and platform expansion

| # | Capability | Status |
|---:|---|---|
| 101 | Versioned `.classic` document envelope | 🟢 Persisted local document schema |
| 102 | Embedded asset payloads in project files | 🔴 Planned |
| 103 | File drop asset ingestion | 🟡 Local operating-system file drop |
| 104 | Broad import adapters with safe type detection | 🔴 Planned |
| 105 | Export presets for raster, vector, and document formats | 🟡 Scoped SVG and PNG export |
| 106 | Node.js Hono collaboration API | 🔴 Planned |
| 107 | WebSocket room synchronization | 🔴 Planned |
| 108 | Operation-based conflict resolution | 🔴 Planned |
| 109 | Live cursor, selection, and viewport presence | 🔴 Planned |
| 110 | Reconnect, resync, and offline outbox | 🔴 Planned |
| 111 | Project, file, page, and asset service endpoints | 🔴 Planned |
| 112 | MongoDB persistence with schema migrations | 🔴 Planned |
| 113 | Object storage for embedded asset blobs | 🔴 Planned |
| 114 | Per-route and per-identity rate limits | 🔴 Planned |
| 115 | Payload validation, output encoding, and XSS defenses | 🔴 Planned |
| 116 | Content-size limits, upload scanning, and abuse controls | 🔴 Planned |
| 117 | Authenticated sessions and token rotation | 🔴 Planned |
| 118 | Organization roles and project permissions | 🔴 Planned |
| 119 | Administrator audit view and incident controls | 🔴 Planned |
| 120 | Backup, retention, and recovery policy | 🔴 Planned |
| 121 | Optional model-assisted design actions | 🔴 Planned |
| 122 | AI provider proxy with opt-in consent and quotas | 🔴 Planned |
| 123 | Local MCP server for model-to-document workflows | 🔴 Planned |
| 124 | Extension manifest, capability grants, and versioning | 🔴 Planned |
| 125 | Extension sandbox and permission prompts | 🔴 Planned |
| 126 | Project landing workspace and recent projects | 🔴 Planned |
| 127 | Marketing site, legal pages, documentation, and blog | 🔴 Planned |
| 128 | GitHub source, issue templates, and contributor process | 🔴 Planned |
| 129 | GitHub Actions checks and signed release artifacts | 🔴 Planned |
| 130 | Windows, macOS, and Linux distribution pipeline | 🔴 Planned |
| 131 | Cloudflare deployment pipeline for hosted service | 🔴 Planned |
| 132 | Hosted admin dashboard templates | 🔴 Planned |
| 133 | Telemetry consent and privacy controls | 🔴 Planned |
| 134 | Dependency, security, and supply-chain checks | 🔴 Planned |
| 135 | Accessibility review for desktop and web surfaces | 🔴 Planned |
| 136 | Localization-ready product and website copy | 🔴 Planned |
| 137 | Public extension registry and discovery | 🔴 Planned |
| 138 | Community templates and free tools | 🔴 Planned |
| 139 | Headless conversion and export runner | 🔴 Planned |
| 140 | Compatibility test corpus for the `.classic` specification | 🔴 Planned |

## Delivery order

1. Finish reliable local authoring: paths, grouping, layout, typography, image rendering, embedded assets, effects, and complete export behavior.
2. Establish a documented `.classic` specification, deterministic migration rules, and a compatibility corpus before cloud synchronization.
3. Build the opt-in hosted service with identity, authorization, validation, rate limiting, auditability, and recovery before collaboration rooms.
4. Add real-time document operations, conflict handling, presence, review workflows, sharing, and project administration.
5. Publish a stable extension API and local MCP server with explicit capability grants before opening an extension registry.
6. Add optional AI workflows only through user-authorized requests, scoped project access, quotas, and provider-agnostic service boundaries.
7. Automate reproducible Windows, macOS, and Linux releases, then launch a documentation-led public site and contributor program.

Implementation must keep the desktop editor usable offline, avoid storing credentials in documents or source control, validate every untrusted payload at service boundaries, and maintain clear compatibility boundaries.

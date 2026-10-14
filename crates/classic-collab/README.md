# Classic collaboration domain

`classic-collab` is a transport-independent Rust foundation for a collaboration
service. It models identities, rooms, memberships, presence heartbeats, live
cursors and selections, and a strictly ordered stream of document operations.

It deliberately contains **no** live networking, HTTP handler, WebSocket
server, database driver, authentication provider, or UI code. A Hono API,
MongoDB persistence layer, or native desktop client can deserialize these
models, authenticate a request externally, call `validate`, and use
`RoomState::accept_operation` / `RoomState::accept_presence` to enforce the
same ordering rules.

## Ordering contract

Each accepted operation has:

- a room ID and actor/session identity;
- a `base_version` equal to the room's current version;
- a `sequence` equal to `current_version + 1`; and
- a unique operation ID.

Presence has a separate, strictly increasing session-local heartbeat sequence.
These checks make stale updates and duplicate deliveries explicit conflicts for
the transport to resolve or retry. The crate does not perform operational
transforms or CRDT merges; a server may add one of those policies around this
validated command boundary.

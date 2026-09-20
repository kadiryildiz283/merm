# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

---

## Security Architecture

`merm` is built with defensive systems engineering principles:

1. **100% Memory-Safe Safe Rust:** No `unsafe` blocks in business logic, rendering transforms, or AST manipulation.
2. **IPC Access Control via Linux `SO_PEERCRED`:**
   - The Unix Domain Socket IPC listener checks the peer's PID and UID via Linux kernel credentials (`getsockopt(fd, SOL_SOCKET, SO_PEERCRED, ...)`).
   - Only processes executed by the exact same user account (`uid == current_uid`) are permitted to send JSON-RPC commands. Foreign processes are rejected immediately with `PermissionDenied`.
3. **Strict Input Sanitization & XML Escaping:**
   - Diagram titles, node labels, members, edge comments, and stereotypes undergo strict entity escaping (`escape_xml`) to prevent SVG injection attacks.
4. **Watchdog Timeout Execution:**
   - Layout calculation runs with a thread-isolated watchdog timer preventing ReDoS or unbounded execution from freezing the system.

---

## Reporting a Vulnerability

If you discover a security vulnerability in `merm`, please DO NOT open a public issue.

Instead, please send an advisory email directly to:
**`kadiry.ra@gmail.com`**

Include:
- A description of the issue.
- Proof of concept steps or sample Markdown/Mermaid payload.
- Potential impact.

We will acknowledge receipt within 48 hours and work with you on a coordinated disclosure.

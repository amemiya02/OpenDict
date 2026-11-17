---
status: accepted
---

# GPL-3.0 for the application, MIT/Apache-2.0 for the MDict parsing library

Dictionary apps are routinely repackaged as closed-source clones, so the application is licensed GPL-3.0-or-later to keep derivatives open. The MDict parsing library is licensed separately under MIT OR Apache-2.0 so that other projects can reuse it without adopting the GPL.

## Consequences

- The parsing library must be a clean implementation written from public descriptions of the format. It cannot be ported from GPL-licensed readers, and its dependencies must be permissively licensed.
- GPL-3.0 conflicts with the Apple App Store terms, which rules out an iOS version. That was accepted on 2026-11-17: OpenDict ships for desktop (and possibly Android later), never iOS, so contributions need no CLA and the licence carries no app-store exception. Reversing this later would need every contributor's consent.

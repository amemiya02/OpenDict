---
status: accepted
---

# GPL-3.0 for the application, MIT/Apache-2.0 for the MDict parsing library

Dictionary apps are routinely repackaged as closed-source clones, so the application is licensed GPL-3.0-or-later to keep derivatives open. The MDict parsing library is licensed separately under MIT OR Apache-2.0 so that other projects can reuse it without adopting the GPL.

## Consequences

- The parsing library must be a clean implementation written from public descriptions of the format. It cannot be ported from GPL-licensed readers, and its dependencies must be permissively licensed.
- GPL-3.0 conflicts with the Apple App Store terms. Shipping on iOS later requires either a single copyright holder who can grant a separate licence (so outside contributions need a CLA) or relicensing the application.

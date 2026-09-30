# F22 Device Nodes — v0.69

Source implementation now includes:

- one-time enrollment token hashing and atomic consumption;
- Ed25519 device identity generated locally;
- private seed stored via the OS keyring provider;
- signed envelopes with nonce, SHA-256 body hash, sequence and replay reservation contract;
- WSS-only node client using rustls/web PKI roots;
- explicit Windows, Linux, macOS, Android and iOS contracts;
- fencing token and protected-capability approval rules;
- tenant-scoped PostgreSQL platform/pairing audit tables with FORCE RLS.

Physical multi-platform execution remains a release gate.

# v0.31 migration repair included in v0.32

Known affected SHA-256: 681eb4411f88375dcd151c158cf33b90c34e23e772c87e929ecc2eb32d65774e

Replacement SHA-256: 267210a39b5263c83fd1a4479d90cc001ed56dc86818a3ebb88d393732a2e2b1

The repair only applies when the existing file exactly matches the known affected hash. Any other content fails closed for manual reconciliation.

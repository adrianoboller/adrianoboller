# Implement SHA-256, HMAC and PBKDF2 with official test vectors
# 27/08 19:00

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod error;\npub mod json;","pub mod error;\npub mod hash;\npub mod json;")
open(p,'w').write(s)

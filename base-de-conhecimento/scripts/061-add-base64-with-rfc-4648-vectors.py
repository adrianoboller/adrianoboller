# Add Base64 with RFC 4648 vectors
# 27/08 19:20

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod crc;\npub mod datahora;","pub mod base64;\npub mod crc;\npub mod datahora;")
open(p,'w').write(s)

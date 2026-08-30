# Add date/time helpers to core
# 27/08 17:55

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod crc;\npub mod error;","pub mod crc;\npub mod datahora;\npub mod error;")
open(p,'w').write(s)

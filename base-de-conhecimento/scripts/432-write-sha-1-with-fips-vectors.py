# Write SHA-1 with FIPS vectors
# 28/08 14:40

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
if 'pub mod sha1;' not in s:
    s=s.replace('pub mod sha512;','pub mod sha1;\npub mod sha512;',1)
open(p,'w').write(s)

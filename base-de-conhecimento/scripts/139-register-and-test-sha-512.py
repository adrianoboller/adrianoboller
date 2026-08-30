# Register and test SHA-512
# 27/08 20:37

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
import re
s=re.sub(r'(pub mod schema;)', r'pub mod schema;\npub mod sha512;', s, count=1)
open(p,'w').write(s)

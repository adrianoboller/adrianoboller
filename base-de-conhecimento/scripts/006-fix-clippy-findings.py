# Fix clippy findings
# 27/08 17:57

p='crates/phxsql-store/src/ndx.rs'
s=open(p).read()
s=s.replace("        p: &mut Vec<u8>,\n        ck: &[u8],","        p: &mut [u8],\n        ck: &[u8],")
s=s.replace("        p: &mut Vec<u8>,\n        pos: usize,","        p: &mut [u8],\n        pos: usize,")
open(p,'w').write(s)

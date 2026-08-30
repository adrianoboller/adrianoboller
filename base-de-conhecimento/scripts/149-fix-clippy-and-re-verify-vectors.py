# Fix clippy and re-verify vectors
# 27/08 20:46

p='crates/phxsql-core/src/ed25519.rs'
s=open(p).read()
velho='''    for i in 1..5 {
        h[i] += c;
        c = h[i] >> 51;
        h[i] &= MASCARA;
    }'''
novo='''    for pedaco in h.iter_mut().skip(1) {
        *pedaco += c;
        c = *pedaco >> 51;
        *pedaco &= MASCARA;
    }'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))

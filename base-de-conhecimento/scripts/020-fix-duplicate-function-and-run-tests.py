# Fix duplicate function and run tests
# 27/08 18:27

p='crates/phxsql-store/src/table.rs'
s=open(p).read()
dup='''    /// Volumes existentes de cada arquivo paginado.
    pub fn volumes(&self) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
        (
            self.reg.volumes(),
            self.bin.volumes(),
            self.memo.volumes(),
            self.log.volumes(),
        )
    }

'''
if dup in s:
    s=s.replace(dup,'',1)
    print("duplicata removida")
open(p,'w').write(s)

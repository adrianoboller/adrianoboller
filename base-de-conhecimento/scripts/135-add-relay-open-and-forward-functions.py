# Add relay open and forward functions
# 27/08 20:35

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''    pub fn destino_permitido(&self, destino: &str) -> bool {''',
'''    /// Ha algum destino configurado? Sem isso a interface so fala consigo.
    pub fn destinos_permitidos_algum(&self) -> bool {
        !self.destinos.is_empty()
    }

    pub fn destino_permitido(&self, destino: &str) -> bool {''')
open(p,'w').write(s)

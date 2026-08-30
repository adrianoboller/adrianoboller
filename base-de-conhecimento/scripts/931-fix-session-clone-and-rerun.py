# Fix session clone and rerun
# 29/08 00:28

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace('''    fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
        let mut ses = sessao.clone();''','''    fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
        let mut ses = Sessao {
            usuario: sessao.usuario.clone(),
            ..Sessao::default()
        };''',1)
p.write_text(s)

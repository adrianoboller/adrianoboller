# Fix the last callers and the shadowed name
# 28/08 13:56

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let t = self.abrir(p, sessao)?;'''
n = '''    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let t = self.abrir_travada(&_trava, p, sessao)?;'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''    fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;'''
n = '''    fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;'''
assert s.count(v) == 1
s = s.replace(v, n)
# o `p` do periodo colide com o `p: &Json` agora que o escopo mudou
s = s.replace('Some(p) => Json::texto_de(p.rotulo(f.chave_periodo)),',
              'Some(per) => Json::texto_de(per.rotulo(f.chave_periodo)),')
s = s.replace('''                            Json::texto_de(match pag.modo.periodo() {
                                None => "quantidade".to_string(),
                                Some(p) => p.nome().to_string(),
                            }),''',
'''                            Json::texto_de(match pag.modo.periodo() {
                                None => "quantidade".to_string(),
                                Some(per) => per.nome().to_string(),
                            }),''')
p.write_text(s)

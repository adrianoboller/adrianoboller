# Filter catalogue ops
# 29/08 00:27

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace('''            "sistabelas" | "systables" => self.op_sistabelas(p),
            "siscolunas" | "syscolumns" => self.op_siscolunas(p),''',
'''            "sistabelas" | "systables" => self.op_sistabelas(p, sessao),
            "siscolunas" | "syscolumns" => self.op_siscolunas(p, sessao),''',1)

s = s.replace('''    fn op_sistabelas(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let t = match db.abrir_qualificada(&nome) {''',
'''    fn op_sistabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            // O catalogo e a mesma lista da arvore por outra porta: se ele nao
            // filtrasse, bastaria pedir `sistabelas` para saber tudo sobre a
            // tabela que a arvore esconde -- nome, colunas, quantas linhas.
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let t = match db.abrir_qualificada(&nome) {''',1)

s = s.replace('''    fn op_siscolunas(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }''',
'''    fn op_siscolunas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }''',1)
p.write_text(s)
print("ok")

# Implement criar_schema op
# 28/08 10:56

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()

OP = '''    /// Cria um schema -- uma pasta dentro do database.
    ///
    /// Estava prometido em dois lugares (a tabela de permissoes e a lista de
    /// operacoes de escrita) e nao existia no despacho: pedir `criar_schema`
    /// pela rede respondia "operacao desconhecida". A biblioteca ja sabia
    /// fazer; faltava a porta.
    fn op_criar_schema(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let schema = p.texto_ou("schema", "").trim();
        if schema.is_empty() {
            return Err(PhxError::Esquema("informe \\"schema\\"".into()));
        }
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        dados.abrir_database(database)?.criar_schema(schema)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("schema", Json::texto_de(schema)),
        ]))
    }

'''
marca = '''    /// Cria uma tabela. Fecha o buraco que estava aberto desde a revisao'''
assert s.count(marca) == 1
s = s.replace(marca, OP + marca, 1)

v = '''            "criar_tabela" => self.op_criar_tabela(p),'''
n = '''            "criar_schema" => self.op_criar_schema(p),
            "criar_tabela" => self.op_criar_tabela(p),'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

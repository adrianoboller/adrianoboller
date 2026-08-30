# Fix criar_tabela to split qualified names
# 28/08 15:37

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''    fn op_criar_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let esquema = crate::valores::esquema_de_json(p)?;
        let nome = esquema.nome().to_string();
        let schema = p.texto_ou("schema", "");
        let schema = (!schema.trim().is_empty()).then_some(schema);

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        if db.existe_tabela(schema, &nome)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {nome} ja existe em {database}"
            )));
        }
        let t = db.criar_tabela(schema, esquema)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabela", Json::texto_de(&nome)),'''
b='''    fn op_criar_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let mut esquema = crate::valores::esquema_de_json(p)?;

        // `filial.clientes` e o schema `filial` mais a tabela `clientes`, e
        // nao uma tabela chamada "filial.clientes".
        //
        // Toda leitura ja separava assim -- `abrir_qualificada` faz isso desde
        // sempre. So a CRIACAO nao fazia, e o resultado eram cinco arquivos
        // chamados `filial.clientes.reg` na raiz do banco, que nenhuma outra
        // operacao conseguia abrir: a tabela nascia inalcancavel, e o servidor
        // respondia "criada".
        let (do_nome, nome) = phxsql_store::catalogo::separar_qualificado(esquema.nome());
        let dito = p.texto_ou("schema", "").trim().to_string();
        let schema = match (do_nome.as_deref(), dito.as_str()) {
            (Some(a), b) if !b.is_empty() && a != b => {
                return Err(PhxError::Esquema(format!(
                    "o nome diz schema {a:?} e o campo \\"schema\\" diz {b:?}: \
                     escolha um dos dois"
                )))
            }
            (Some(a), _) => Some(a.to_string()),
            (None, "") => None,
            (None, b) => Some(b.to_string()),
        };
        if do_nome.is_some() {
            esquema.renomear(&nome);
        }

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        if db.existe_tabela(schema.as_deref(), &nome)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {} ja existe em {database}",
                phxsql_store::catalogo::qualificar(schema.as_deref(), &nome)
            )));
        }
        let t = db.criar_tabela(schema.as_deref(), esquema)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            (
                "schema",
                match &schema {
                    Some(s) => Json::texto_de(s),
                    None => Json::Nulo,
                },
            ),
            (
                "tabela",
                Json::texto_de(phxsql_store::catalogo::qualificar(schema.as_deref(), &nome)),
            ),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

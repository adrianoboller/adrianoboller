# Add copiar, SysTables and SysColumns
# 28/08 11:24

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()

OPS = r'''    /// Copia uma tabela para outro database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. Sao
    /// duas operacoes e nao uma porque a permissao e a mesma mas o alcance
    /// nao: colar num database em que o usuario nao pode criar tem de recusar
    /// no database de DESTINO, e nao no de origem.
    fn op_copiar_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let origem_db = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino_db = match p.texto_ou("destino_database", "").trim() {
            "" => origem_db,
            outro => outro,
        };
        let destino = match p.texto_ou("destino", "").trim() {
            "" => tabela,
            outro => outro,
        };
        // A permissao de criar vale no destino. Sem esta linha, quem pode ler
        // um database e criar noutro conseguiria escrever onde nao devia --
        // ou, pior, o contrario.
        self.exigir(sessao, destino_db, "criar_tabela")?;

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let origem = dados.abrir_database(origem_db)?;
        let alvo = dados.abrir_database(destino_db)?;
        let copiados = origem.copiar_tabela_para(tabela, &alvo, destino)?;
        Ok(Json::objeto(vec![
            ("origem_database", Json::texto_de(origem_db)),
            ("origem", Json::texto_de(tabela)),
            ("destino_database", Json::texto_de(destino_db)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(copiados as u64)),
        ]))
    }

    /// `SysTables`: o catalogo de tabelas como se fosse uma tabela.
    ///
    /// Uma linha por tabela do database, com o que ela pesa. E o mesmo que a
    /// tela de gestao mostra, mas em forma de dado -- para quem quer consultar
    /// o catalogo em vez de olhar para ele.
    fn op_sistabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.tabelas_qualificadas()? {
            let mut t = match db.abrir_qualificada(&nome) {
                Ok(t) => t,
                // Uma tabela ilegivel nao pode derrubar o catalogo inteiro: ela
                // vira uma linha que diz que esta ilegivel, que e exatamente a
                // informacao que alguem foi procurar ali.
                Err(e) => {
                    linhas.push(Json::objeto(vec![
                        ("tabela", Json::texto_de(&nome)),
                        ("erro", Json::texto_de(e.to_string())),
                    ]));
                    continue;
                }
            };
            let e = t.esquema().clone();
            let pag = e.paginacao();
            linhas.push(Json::objeto(vec![
                ("tabela", Json::texto_de(&nome)),
                (
                    "schema",
                    match nome.split_once('.') {
                        Some((sc, _)) => Json::texto_de(sc),
                        None => Json::texto_de(""),
                    },
                ),
                ("registros", Json::de_u64(t.registros())),
                ("slots", Json::de_u64(t.slots())),
                ("colunas", Json::de_u64(e.colunas().len() as u64)),
                ("indices", Json::de_u64(e.indices().len() as u64)),
                (
                    "chave_primaria",
                    match e.chave_primaria() {
                        None => Json::Nulo,
                        Some(k) => Json::texto_de(&k.nome),
                    },
                ),
                (
                    "chaves_estrangeiras",
                    Json::de_u64(e.chaves_estrangeiras().len() as u64),
                ),
                ("bytes_por_linha", Json::de_u64(t.slot_size() as u64)),
                ("paginada", Json::Bool(pag.ligada())),
                (
                    "particao",
                    Json::texto_de(match pag.modo.periodo() {
                        None if pag.ligada() => "quantidade".to_string(),
                        None => "".to_string(),
                        Some(p) => p.nome().to_string(),
                    }),
                ),
                ("volumes", Json::de_u64(t.fronteiras().len() as u64)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("tabelas", Json::Lista(linhas)),
        ]))
    }

    /// `SysColumns`: uma linha por coluna de todas as tabelas do database.
    ///
    /// Aceita `tabela` para filtrar. E aqui que os metadados novos aparecem
    /// juntos -- id, caption, descricao, mascara e o papel nas chaves --, que e
    /// o que um dicionario de dados precisa mostrar.
    fn op_siscolunas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.tabelas_qualificadas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };
            let e = t.esquema();
            for (i, c) in e.colunas().iter().enumerate() {
                let papel = e.papel_da_coluna(i);
                linhas.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("posicao", Json::de_u64(i as u64 + 1)),
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("obrigatoria", Json::Bool(!c.nullable)),
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ]));
            }
        }
        let _ = sessao;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("colunas", Json::Lista(linhas)),
        ]))
    }

'''

marca = '''    /// Cria um schema -- uma pasta dentro do database.'''
assert s.count(marca) == 1
s = s.replace(marca, OPS + marca, 1)

v = '''            "duplicar_tabela" => self.op_duplicar_tabela(p),'''
n = '''            "duplicar_tabela" => self.op_duplicar_tabela(p),
            "copiar_tabela" => self.op_copiar_tabela(p, sessao),
            "sistabelas" | "systables" => self.op_sistabelas(p, sessao),
            "siscolunas" | "syscolumns" => self.op_siscolunas(p, sessao),'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

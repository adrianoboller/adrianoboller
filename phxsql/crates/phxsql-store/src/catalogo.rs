//! Hierarquia de bancos, schemas e tabelas em disco.
//!
//! ```text
//! base/
//! └── Z/                        database Z
//!     ├── cadastroClientes.reg  ] tabelas da raiz
//!     ├── cadastroClientes.ndx  ]  (sem schema)
//!     ├── ...                   ]
//!     ├── X/                    schema X
//!     │   └── pedidos.reg ...   tabelas do schema X
//!     └── Y/                    schema Y
//!         └── notas.reg ...     tabelas do schema Y
//! ```
//!
//! A regra e estrutural, sem arquivo de marcacao: um diretorio dentro da base
//! e um database; um diretorio dentro de um database e um schema; um arquivo
//! `.reg` e uma tabela. Tabelas soltas na raiz do database sao as "tabelas
//! raiz" -- equivalentes ao `public` do Postgres ou ao `dbo` do SQL Server.
//!
//! O nome qualificado de uma tabela e `schema.tabela`, ou so `tabela` quando
//! ela esta na raiz. E o mesmo formato que o catalogo do FraseSQL espera.

use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::BALDES;
use phxsql_core::schema::Schema;
use phxsql_core::EXT_REG;

use crate::table::Table;

/// O nome nao e um engano de digitacao: e uma tentativa de sair do diretorio.
///
/// Separa as duas coisas de proposito. `"minha tabela!"` e um nome ruim --
/// alguem errou. `"../../etc/passwd"` nao e nome nenhum: ninguem digita isso
/// por acidente. Quem chama precisa poder tratar os dois casos de forma
/// diferente, e e por isso que esta funcao existe separada de
/// [`validar_nome`].
/// `esquema.tabela` vira `(Some("esquema"), "tabela")`.
/// O arquivo `precos_002.reg` pertence a tabela `precos` na extensao `reg`?
///
/// A conferencia do sufixo importa: sem ela, `precos_historico.reg` seria
/// dado como volume de `precos` e a exclusao levaria a tabela errada junto.
fn pertence(arquivo: &str, tabela: &str, ext: &str) -> bool {
    let Some(sem_ext) = arquivo.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    let Some(sufixo) = sem_ext.strip_prefix(tabela) else {
        return false;
    };
    // Ou e o nome exato, ou e o nome mais `_` e um sufixo de volume: so
    // digitos, ou uma das 37 letras da particao alfanumerica.
    if sufixo.is_empty() {
        return true;
    }
    let Some(s) = sufixo.strip_prefix('_') else {
        return false;
    };
    !s.is_empty() && (s.bytes().all(|b| b.is_ascii_digit()) || e_balde(s))
}

/// Este sufixo e o nome de um balde da particao alfanumerica?
///
/// Comparacao contra a lista EXATA, e nao "uma letra qualquer": a lista tem 37
/// nomes e nenhum outro serve. Sem isso, `precos_historico.reg` viraria volume
/// de `precos` -- que e o defeito que esta conferencia existe para evitar,
/// agora com um caso a mais.
fn e_balde(s: &str) -> bool {
    BALDES.contains(&s)
}

pub fn nome_hostil(nome: &str) -> bool {
    nome == "."
        || nome == ".."
        || nome.contains("..")
        || nome
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':') || c.is_control())
}

/// Recusa nomes que escapariam do diretorio ou quebrariam o sistema de
/// arquivos. Vale para database, schema e tabela.
pub fn validar_nome(rotulo: &str, nome: &str) -> Result<()> {
    if nome.is_empty() {
        return Err(PhxError::Esquema(format!("{rotulo} sem nome")));
    }
    if nome == "." || nome == ".." {
        return Err(PhxError::Esquema(format!("{rotulo} invalido: {nome}")));
    }
    if nome.chars().any(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(PhxError::Esquema(format!(
            "{rotulo} {nome:?} tem caractere que nao pode entrar em nome de arquivo"
        )));
    }
    Ok(())
}

/// Extrai o nome da tabela de um arquivo `.reg`, tirando o sufixo de volume.
///
/// `cadastroClientes.reg` e `cadastroClientes_007.reg` devolvem os dois
/// `cadastroClientes`.
fn nome_da_tabela(caminho: &Path) -> Option<String> {
    if caminho.extension().and_then(|s| s.to_str()) != Some(EXT_REG) {
        return None;
    }
    let base = caminho.file_stem()?.to_str()?;
    let Some((antes, sufixo)) = base.rsplit_once('_') else {
        return Some(base.to_string());
    };
    if antes.is_empty() || sufixo.is_empty() {
        return Some(base.to_string());
    }
    if sufixo.chars().all(|c| c.is_ascii_digit()) {
        return Some(antes.to_string());
    }
    // Sufixo de LETRA so conta como balde quando o volume 1 esta ali do lado.
    //
    // A conferencia existe por causa de uma ambiguidade real: uma tabela
    // chamada `dados_X` e o balde X de uma tabela `dados` se escrevem igual.
    // O volume 1 (`_A`) nasce com a tabela alfanumerica e nunca falta, entao a
    // presenca dele e o que separa os dois casos -- e uma tabela `dados_X`
    // sozinha continua sendo ela mesma.
    if e_balde(sufixo)
        && caminho
            .with_file_name(format!("{antes}_{}.{EXT_REG}", BALDES[0]))
            .exists()
    {
        return Some(antes.to_string());
    }
    Some(base.to_string())
}

fn tabelas_em(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| nome_da_tabela(&p))
        .collect();
    nomes.sort();
    nomes.dedup();
    Ok(nomes)
}

fn subdiretorios(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    nomes.sort();
    Ok(nomes)
}

/// A raiz que contem varios databases.
pub struct Instancia {
    base: PathBuf,
}

impl Instancia {
    /// Abre (criando se preciso) a raiz de dados.
    pub fn nova(base: impl AsRef<Path>) -> Result<Instancia> {
        let base = base.as_ref().to_path_buf();
        std::fs::create_dir_all(&base)?;
        Ok(Instancia { base })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn criar_database(&self, nome: &str) -> Result<Database> {
        validar_nome("database", nome)?;
        let caminho = self.base.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!("database {nome} ja existe")));
        }
        std::fs::create_dir_all(&caminho)?;
        Ok(Database {
            nome: nome.to_string(),
            caminho,
        })
    }

    pub fn abrir_database(&self, nome: &str) -> Result<Database> {
        validar_nome("database", nome)?;
        let caminho = self.base.join(nome);
        if !caminho.is_dir() {
            return Err(PhxError::NaoEncontrado(format!(
                "database {nome} nao existe em {}",
                self.base.display()
            )));
        }
        Ok(Database {
            nome: nome.to_string(),
            caminho,
        })
    }

    /// Cria o database se ainda nao existir.
    pub fn garantir_database(&self, nome: &str) -> Result<Database> {
        match self.abrir_database(nome) {
            Ok(d) => Ok(d),
            Err(_) => self.criar_database(nome),
        }
    }

    pub fn databases(&self) -> Result<Vec<String>> {
        subdiretorios(&self.base)
    }
}

/// Um database: tabelas na raiz e, opcionalmente, schemas em subdiretorios.
pub struct Database {
    nome: String,
    caminho: PathBuf,
}

impl Database {
    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Diretorio de um schema, ou a raiz do database quando `schema` e `None`.
    pub fn diretorio(&self, schema: Option<&str>) -> Result<PathBuf> {
        match schema {
            None => Ok(self.caminho.clone()),
            Some(s) => {
                validar_nome("schema", s)?;
                Ok(self.caminho.join(s))
            }
        }
    }

    pub fn criar_schema(&self, nome: &str) -> Result<PathBuf> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "schema {nome} ja existe em {}",
                self.nome
            )));
        }
        std::fs::create_dir_all(&caminho)?;
        Ok(caminho)
    }

    pub fn garantir_schema(&self, nome: &str) -> Result<PathBuf> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        std::fs::create_dir_all(&caminho)?;
        Ok(caminho)
    }

    pub fn schemas(&self) -> Result<Vec<String>> {
        subdiretorios(&self.caminho)
    }

    /// Tabelas de um schema, ou da raiz quando `schema` e `None`.
    pub fn tabelas(&self, schema: Option<&str>) -> Result<Vec<String>> {
        tabelas_em(&self.diretorio(schema)?)
    }

    /// Toda tabela do database, com nome qualificado, incluindo as dos schemas.
    pub fn todas_as_tabelas(&self) -> Result<Vec<String>> {
        let mut saida: Vec<String> = self.tabelas(None)?;
        for s in self.schemas()? {
            for t in self.tabelas(Some(&s))? {
                saida.push(format!("{s}.{t}"));
            }
        }
        saida.sort();
        Ok(saida)
    }

    pub fn criar_tabela(&self, schema: Option<&str>, esquema: Schema) -> Result<Table> {
        validar_nome("tabela", esquema.nome())?;
        let dir = match schema {
            None => self.caminho.clone(),
            Some(s) => self.garantir_schema(s)?,
        };
        Table::criar(dir, esquema)
    }

    pub fn abrir_tabela(&self, schema: Option<&str>, nome: &str) -> Result<Table> {
        validar_nome("tabela", nome)?;
        Table::abrir(self.diretorio(schema)?, nome)
    }

    /// Abre por nome qualificado: `schema.tabela` ou so `tabela`.
    pub fn abrir_qualificada(&self, qualificado: &str) -> Result<Table> {
        let (schema, nome) = separar_qualificado(qualificado);
        self.abrir_tabela(schema.as_deref(), &nome)
    }

    /// O que uma CÓPIA leva: os cinco arquivos da tabela mais o espelho
    /// `.bkp`. Nao inclui a lixeira nem os motivos -- uma copia nasce como a
    /// origem esta, e nao herda o que foi excluido dela.
    const EXTENSOES: [&'static str; 6] = ["reg", "ndx", "bin", "memo", "log", "bkp"];

    /// O que uma tabela OCUPA em disco -- tudo que o nome dela carrega.
    ///
    /// Lista separada da de cima, e a diferenca entre as duas e o ponto: o que
    /// uma copia leva e uma decisao (a lixeira da origem nao e da copia), o que
    /// um `excluir_tabela` apaga nao e -- ou apaga tudo, ou nao apagou a tabela.
    ///
    /// Ela nasceu do defeito: a lista de apagar tinha SEIS extensoes e a tabela
    /// ja tinha NOVE. O `.trash`, o `.reason` e o `.pag` entraram depois e
    /// ninguem voltou aqui — a mesma armadilha da peca nova no fim de uma lista
    /// que o `rownum` armou na tela. O estrago era duplo: recriar a tabela com
    /// o mesmo nome passava a ser IMPOSSIVEL (`Table::criar` confere as sete
    /// extensoes e recusa), e se nao fosse, a tabela nova herdaria a lixeira e
    /// os motivos de exclusao de uma tabela alheia — que e conteudo de linha,
    /// e so `administrar` pode ler.
    const EXTENSOES_TODAS: [&'static str; 9] = [
        "reg", "ndx", "bin", "memo", "log", "bkp", "trash", "reason", "pag",
    ];

    /// Apaga os arquivos de uma tabela e devolve o que apagou.
    ///
    /// Inclui o `.bkp`: deixar o espelho para tras faria a tabela "voltar"
    /// pela metade se alguem recriasse uma com o mesmo nome.
    ///
    /// Nao ha desfazer. Quem chama confere antes.
    pub fn excluir_tabela(&self, qualificado: &str) -> Result<Vec<String>> {
        let (schema, nome) = separar_qualificado(qualificado);
        let (schema, nome) = (schema.as_deref(), nome.as_str());
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;
        let mut apagados = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            // Uma tabela paginada tem varios volumes por extensao.
            for arq in std::fs::read_dir(&dir)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome, ext) {
                    std::fs::remove_file(arq.path())?;
                    apagados.push(f.to_string());
                }
            }
        }
        if apagados.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {qualificado} nao existe em {}",
                self.nome()
            )));
        }
        apagados.sort();
        Ok(apagados)
    }

    /// Copia uma tabela inteira para outro nome, byte a byte.
    ///
    /// Copiar os arquivos preserva a ordem de digitacao e os rowids; reinserir
    /// linha a linha nao preservaria nem um nem outro.
    pub fn duplicar_tabela(&self, origem: &str, destino: &str) -> Result<usize> {
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if self.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!("a tabela {destino} ja existe")));
        }
        let dir_o = self.diretorio(schema_o)?;
        let dir_d = self.diretorio(schema_d)?;
        let mut copiados = 0usize;
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    // Preserva o sufixo do volume: `precos_002.reg` vira
                    // `copia_002.reg`, nao `copia.reg`.
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    std::fs::copy(arq.path(), dir_d.join(&novo))?;
                    copiados += 1;
                }
            }
        }
        if copiados == 0 {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copiados)
    }

    /// Copia uma tabela para OUTRO database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. E a
    /// mesma copia byte a byte, e pela mesma razao: a copia nasce com os
    /// mesmos rowids e na mesma ordem de digitacao.
    pub fn copiar_tabela_para(
        &self,
        origem: &str,
        destino_db: &Database,
        destino: &str,
    ) -> Result<usize> {
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if destino_db.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {destino} ja existe em {}",
                destino_db.nome()
            )));
        }
        let dir_o = self.diretorio(schema_o)?;
        // Colar num schema que ainda nao existe cria a pasta -- e o que quem
        // cola espera, e o mesmo que `criar_tabela` faz.
        let dir_d = match schema_d {
            None => destino_db.caminho().to_path_buf(),
            Some(sc) => destino_db.garantir_schema(sc)?,
        };
        if dir_o == dir_d && nome_o == nome_d {
            return Err(PhxError::Duplicado(
                "origem e destino sao a mesma tabela".into(),
            ));
        }

        let mut copiados = 0usize;
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    std::fs::copy(arq.path(), dir_d.join(&novo))?;
                    copiados += 1;
                }
            }
        }
        if copiados == 0 {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copiados)
    }

    pub fn existe_tabela(&self, schema: Option<&str>, nome: &str) -> Result<bool> {
        Ok(self.tabelas(schema)?.iter().any(|t| t == nome))
    }
}

/// Quebra `schema.tabela` em `(Some(schema), tabela)`. Sem ponto, o schema e
/// `None` e a tabela esta na raiz do database.
pub fn separar_qualificado(qualificado: &str) -> (Option<String>, String) {
    match qualificado.split_once('.') {
        Some((s, t)) if !s.is_empty() && !t.is_empty() => (Some(s.to_string()), t.to_string()),
        _ => (None, qualificado.to_string()),
    }
}

/// Monta o nome qualificado a partir das partes.
pub fn qualificar(schema: Option<&str>, tabela: &str) -> String {
    match schema {
        Some(s) => format!("{s}.{tabela}"),
        None => tabela.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-cat-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("descricao", ColumnType::Str(40)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    #[test]
    fn hierarquia_database_schema_tabela() {
        let base = dir_temp("hierarquia");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();

        // Tabela na raiz do database.
        z.criar_tabela(None, esquema("cadastroClientes")).unwrap();
        // Tabelas em dois schemas.
        z.criar_tabela(Some("X"), esquema("pedidos")).unwrap();
        z.criar_tabela(Some("X"), esquema("itens")).unwrap();
        z.criar_tabela(Some("Y"), esquema("notas")).unwrap();

        assert_eq!(inst.databases().unwrap(), vec!["Z"]);
        assert_eq!(z.schemas().unwrap(), vec!["X", "Y"]);
        assert_eq!(z.tabelas(None).unwrap(), vec!["cadastroClientes"]);
        assert_eq!(z.tabelas(Some("X")).unwrap(), vec!["itens", "pedidos"]);
        assert_eq!(z.tabelas(Some("Y")).unwrap(), vec!["notas"]);
        assert_eq!(
            z.todas_as_tabelas().unwrap(),
            vec!["X.itens", "X.pedidos", "Y.notas", "cadastroClientes"]
        );

        // O layout em disco e o do diagrama.
        assert!(base.join("Z/cadastroClientes.reg").exists());
        assert!(base.join("Z/X/pedidos.reg").exists());
        assert!(base.join("Z/Y/notas.reg").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn mesmo_nome_em_schemas_diferentes_nao_colide() {
        let base = dir_temp("homonimas");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();

        let mut a = z.criar_tabela(Some("X"), esquema("pedidos")).unwrap();
        let mut b = z.criar_tabela(Some("Y"), esquema("pedidos")).unwrap();
        a.inserir(&[Value::Int(1), Value::Str("do X".into())])
            .unwrap();
        b.inserir(&[Value::Int(1), Value::Str("do Y".into())])
            .unwrap();
        b.inserir(&[Value::Int(2), Value::Str("so do Y".into())])
            .unwrap();

        assert_eq!(a.registros(), 1);
        assert_eq!(b.registros(), 2);

        let mut lida = z.abrir_qualificada("X.pedidos").unwrap();
        assert_eq!(lida.ler(1).unwrap().unwrap()[1], Value::Str("do X".into()));
        let mut lida = z.abrir_qualificada("Y.pedidos").unwrap();
        assert_eq!(lida.ler(1).unwrap().unwrap()[1], Value::Str("do Y".into()));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn databases_separados_nao_se_enxergam() {
        let base = dir_temp("bancos");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();
        let w = inst.criar_database("W").unwrap();
        z.criar_tabela(None, esquema("clientes")).unwrap();
        w.criar_tabela(None, esquema("fornecedores")).unwrap();

        assert_eq!(z.tabelas(None).unwrap(), vec!["clientes"]);
        assert_eq!(w.tabelas(None).unwrap(), vec!["fornecedores"]);
        assert!(z.abrir_tabela(None, "fornecedores").is_err());
        assert_eq!(inst.databases().unwrap(), vec!["W", "Z"]);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn tabela_paginada_aparece_uma_vez_so_na_listagem() {
        let base = dir_temp("paginada");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();
        let esq = esquema("grande")
            .com_paginacao(phxsql_core::paginacao::Paginacao::nova(2, 99).unwrap())
            .unwrap();
        let mut t = z.criar_tabela(None, esq).unwrap();
        for i in 1..=7i64 {
            t.inserir(&[Value::Int(i), Value::Null]).unwrap();
        }
        // 4 volumes de .reg, mas uma unica tabela na listagem.
        assert!(base.join("Z/grande_004.reg").exists());
        assert_eq!(z.tabelas(None).unwrap(), vec!["grande"]);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nomes_perigosos_sao_recusados() {
        let base = dir_temp("seguranca");
        let inst = Instancia::nova(&base).unwrap();
        assert!(inst.criar_database("..").is_err());
        assert!(inst.criar_database("a/b").is_err());
        assert!(inst.criar_database("a\\b").is_err());
        assert!(inst.criar_database("").is_err());
        let z = inst.criar_database("Z").unwrap();
        assert!(z.criar_schema("../fora").is_err());
        assert!(z.abrir_tabela(Some(".."), "x").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nome_qualificado_vai_e_volta() {
        assert_eq!(
            separar_qualificado("X.pedidos"),
            (Some("X".to_string()), "pedidos".to_string())
        );
        assert_eq!(
            separar_qualificado("cadastroClientes"),
            (None, "cadastroClientes".to_string())
        );
        assert_eq!(qualificar(Some("X"), "pedidos"), "X.pedidos");
        assert_eq!(qualificar(None, "pedidos"), "pedidos");
    }

    #[test]
    fn nome_de_tabela_ignora_sufixo_de_volume() {
        assert_eq!(
            nome_da_tabela(Path::new("cadastroClientes.reg")).as_deref(),
            Some("cadastroClientes")
        );
        assert_eq!(
            nome_da_tabela(Path::new("cadastroClientes_007.reg")).as_deref(),
            Some("cadastroClientes")
        );
        // Sublinhado que nao e sufixo de volume fica no nome.
        assert_eq!(
            nome_da_tabela(Path::new("cadastro_clientes.reg")).as_deref(),
            Some("cadastro_clientes")
        );
        assert_eq!(nome_da_tabela(Path::new("cadastroClientes.ndx")), None);
    }
    #[test]
    fn separa_nome_ruim_de_tentativa_de_travessia() {
        // Engano de digitacao: recusado, mas nao e ataque.
        assert!(!nome_hostil("minha tabela!"));
        assert!(!nome_hostil("cadastro*"));
        assert!(!nome_hostil("aspas\"aqui"));
        assert!(!nome_hostil("cadastroClientes"));
        assert!(!nome_hostil("Comercial"));
        assert!(!nome_hostil("nota.fiscal"));

        // Sondagem: ninguem digita isso por acidente.
        assert!(nome_hostil(".."));
        assert!(nome_hostil("."));
        assert!(nome_hostil("../../etc/passwd"));
        assert!(nome_hostil("..\\..\\windows"));
        assert!(nome_hostil("/etc"));
        assert!(nome_hostil("C:\\dados"));
        assert!(nome_hostil("a/b"));
        assert!(nome_hostil("nome\u{0}nulo"));
        assert!(nome_hostil("quebra\nlinha"));
        // Sem barra, mas ainda saindo do lugar.
        assert!(nome_hostil("tabela..oculta"));
    }

    #[test]
    fn tudo_que_e_hostil_tambem_e_invalido() {
        // O contrario nao vale, e e essa a assimetria que interessa.
        for n in ["..", "/etc", "a/b", "C:\\x", "quebra\nlinha"] {
            assert!(nome_hostil(n));
            assert!(
                validar_nome("tabela", n).is_err(),
                "{n:?} deveria ser invalido"
            );
        }
    }
}

#[cfg(test)]
mod testes_gestao {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};

    #[test]
    fn pertence_nao_confunde_tabela_de_prefixo_igual() {
        // O caso que apagaria a tabela errada: `precos_historico` comeca com
        // `precos`. Sem conferir o sufixo, excluir `precos` levaria as duas.
        assert!(pertence("precos.reg", "precos", "reg"));
        assert!(pertence("precos_001.reg", "precos", "reg"));
        assert!(pertence("precos_00042.reg", "precos", "reg"));

        assert!(!pertence("precos_historico.reg", "precos", "reg"));
        assert!(!pertence("precos2.reg", "precos", "reg"));
        assert!(!pertence("precos_.reg", "precos", "reg"));
        assert!(!pertence("precos_1a.reg", "precos", "reg"));
        assert!(!pertence("precos.ndx", "precos", "reg"), "extensao errada");
        assert!(!pertence("outra.reg", "precos", "reg"));
    }

    #[test]
    fn qualificado_se_parte_em_schema_e_nome() {
        let parte = |q: &str| {
            let (e, n) = separar_qualificado(q);
            (e, n)
        };
        assert_eq!(parte("clientes"), (None, "clientes".into()));
        assert_eq!(
            parte("vendas.pedidos"),
            (Some("vendas".into()), "pedidos".into())
        );
        // Ponto solto nao vira schema vazio: o nome inteiro fica sendo a
        // tabela, e o `validar_nome` recusa depois. E o que impede um
        // ".reg" de virar caminho.
        assert_eq!(parte(".pedidos"), (None, ".pedidos".into()));
        assert_eq!(parte("vendas."), (None, "vendas.".into()));
    }

    fn esquema_simples(nome: &str) -> Schema {
        use phxsql_core::types::ColumnType;
        Schema::new(
            nome,
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    fn base_temp(rotulo: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phxcat-{rotulo}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn excluir_tabela_leva_os_arquivos_dela_e_so_os_dela() {
        let base = base_temp("excluir");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        db.criar_tabela(None, esquema_simples("precos_historico"))
            .unwrap();

        let apagados = db.excluir_tabela("precos").unwrap();
        assert!(!apagados.is_empty());
        assert!(
            apagados.iter().all(|a| a.starts_with("precos.")),
            "levou arquivo que nao era: {apagados:?}"
        );

        assert!(!db.existe_tabela(None, "precos").unwrap());
        assert!(
            db.existe_tabela(None, "precos_historico").unwrap(),
            "a tabela de prefixo igual foi junto"
        );
    }

    /// O defeito: a lista de extensoes do `excluir_tabela` tinha SEIS e a
    /// tabela ja tinha NOVE. O `.trash`, o `.reason` e o `.pag` ficavam para
    /// tras, e o estrago era duplo — recriar a tabela com o mesmo nome passava
    /// a ser IMPOSSIVEL, e o dado de uma tabela excluida sobrevivia num
    /// arquivo que so `administrar` deveria abrir.
    ///
    /// A prova e o CICLO: criar, excluir e criar de novo com o mesmo nome. Ele
    /// falha com a lista velha e passa com a nova, e nao depende de eu lembrar
    /// quais sao as extensoes — quem sabe isso e o `Table::criar`, que confere
    /// todas antes de escrever a primeira.
    #[test]
    fn excluir_tabela_deixa_o_nome_livre_para_a_proxima() {
        let base = base_temp("excluir-ciclo");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        let dir = base.join("Z");

        // A tabela nasce com mais que os quatro arquivos do folclore.
        let antes: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|f| f.starts_with("precos."))
            .collect();
        assert!(
            antes.iter().any(|f| f.ends_with(".trash")),
            "esperava o .trash entre os arquivos da tabela: {antes:?}"
        );

        db.excluir_tabela("precos").unwrap();
        let sobrou: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|f| f.starts_with("precos."))
            .collect();
        assert!(
            sobrou.is_empty(),
            "excluir_tabela deixou para tras: {sobrou:?}"
        );

        // E o que importa para quem usa: o nome volta a estar livre.
        db.criar_tabela(None, esquema_simples("precos"))
            .expect("recriar com o mesmo nome tem de funcionar depois de excluir");
    }

    #[test]
    fn excluir_tabela_que_nao_existe_e_erro() {
        let base = base_temp("excluir-ausente");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        assert!(db.excluir_tabela("naoexiste").is_err());
    }

    #[test]
    fn duplicar_preserva_os_rowids_e_a_ordem() {
        use phxsql_core::value::Value;
        let base = base_temp("duplicar");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        let mut t = db.criar_tabela(None, esquema_simples("precos")).unwrap();
        for i in 1..=5i64 {
            t.inserir(&[Value::Int(i * 10)]).unwrap();
        }
        t.excluir(3).unwrap(); // um buraco no meio
        t.sincronizar().unwrap();
        drop(t);

        let copiados = db.duplicar_tabela("precos", "copia").unwrap();
        assert!(copiados >= 5, "copiou {copiados} arquivos");

        // A copia tem os MESMOS rowids, o mesmo buraco e a mesma ordem. Uma
        // reinsercao linha a linha renumeraria tudo.
        let mut c = db.abrir_tabela(None, "copia").unwrap();
        assert_eq!(c.ler(1).unwrap().unwrap()[0], Value::Int(10));
        assert!(
            c.ler(3).unwrap().is_none(),
            "o slot excluido nao foi copiado"
        );
        assert_eq!(c.ler(5).unwrap().unwrap()[0], Value::Int(50));

        // E o original continua inteiro.
        let mut o = db.abrir_tabela(None, "precos").unwrap();
        assert_eq!(o.ler(5).unwrap().unwrap()[0], Value::Int(50));
    }

    #[test]
    fn duplicar_para_nome_que_ja_existe_e_recusado() {
        let base = base_temp("duplicar-ocupado");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("a")).unwrap();
        db.criar_tabela(None, esquema_simples("b")).unwrap();
        assert!(
            db.duplicar_tabela("a", "b").is_err(),
            "sobrescreveu a tabela b"
        );
    }
}

#[cfg(test)]
mod testes_copia_entre_bancos {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("texto", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
        )
        .unwrap()
    }

    /// A copia entre databases preserva rowid e ordem de digitacao -- que e o
    /// ponto de copiar arquivo em vez de reinserir linha a linha.
    #[test]
    fn colar_em_outro_banco_preserva_rowids_e_ordem() {
        let base = std::env::temp_dir().join(format!("phx-colar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cat = Instancia::nova(&base).unwrap();
        let origem = cat.criar_database("loja").unwrap();
        let destino = cat.criar_database("arquivo").unwrap();

        let mut t = origem.criar_tabela(None, esquema("pedidos")).unwrap();
        for (i, txt) in ["um", "dois", "tres"].iter().enumerate() {
            t.inserir(&[Value::Int(i as i64 + 1), Value::Str((*txt).into())])
                .unwrap();
        }
        // Um buraco no meio: o slot excluido NAO e reaproveitado, e a copia tem
        // de carregar o buraco junto -- senao os rowids andariam.
        t.excluir(2).unwrap();
        t.sincronizar().unwrap();

        let copiados = origem
            .copiar_tabela_para("pedidos", &destino, "pedidos_2026")
            .unwrap();
        assert_eq!(copiados, 5, "os cinco arquivos");

        let mut c = destino.abrir_qualificada("pedidos_2026").unwrap();
        assert_eq!(c.slots(), 3, "o slot excluido continua ocupando lugar");
        assert!(c.ler(2).unwrap().is_none(), "o excluido continua excluido");
        for (rowid, txt) in [(1u64, "um"), (3, "tres")] {
            match &c.ler(rowid).unwrap().unwrap()[1] {
                Value::Str(s) => assert_eq!(s, txt, "rowid {rowid}"),
                outro => panic!("esperava texto, veio {outro:?}"),
            }
        }
        // E a chave primaria atravessou junto.
        assert!(c.esquema().chave_primaria().is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn colar_por_cima_de_tabela_existente_e_recusado() {
        let base = std::env::temp_dir().join(format!("phx-colar2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cat = Instancia::nova(&base).unwrap();
        let a = cat.criar_database("a").unwrap();
        let b = cat.criar_database("b").unwrap();
        a.criar_tabela(None, esquema("t")).unwrap();
        b.criar_tabela(None, esquema("t")).unwrap();

        // Sobrescrever cinco arquivos sem aviso nao tem desfazer.
        assert!(a.copiar_tabela_para("t", &b, "t").is_err());
        // E colar em cima de si mesma tambem nao faz sentido.
        assert!(a.copiar_tabela_para("t", &a, "t").is_err());
        // Mas com outro nome, no mesmo banco, vale.
        assert!(a.copiar_tabela_para("t", &a, "t_copia").is_ok());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn colar_dentro_de_schema_que_ainda_nao_existe_cria_a_pasta() {
        let base = std::env::temp_dir().join(format!("phx-colar3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cat = Instancia::nova(&base).unwrap();
        let a = cat.criar_database("a").unwrap();
        let b = cat.criar_database("b").unwrap();
        a.criar_tabela(None, esquema("t")).unwrap();

        a.copiar_tabela_para("t", &b, "historico.t").unwrap();
        assert!(b.existe_tabela(Some("historico"), "t").unwrap());
        assert!(base.join("b").join("historico").is_dir());

        let _ = std::fs::remove_dir_all(&base);
    }
}

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
    match base.rsplit_once('_') {
        Some((antes, sufixo))
            if !antes.is_empty()
                && !sufixo.is_empty()
                && sufixo.chars().all(|c| c.is_ascii_digit()) =>
        {
            Some(antes.to_string())
        }
        _ => Some(base.to_string()),
    }
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

    /// A tabela existe?
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
            .com_paginacao(phxsql_core::paginacao::Paginacao::nova(2, 99).unwrap());
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

//! DbLink: ligacoes para bancos de fora.
//!
//! O nome vem do Centro de Controle do HFSQL(R), e a ideia e a mesma: guardar
//! um apelido com endereco e credencial, e depois falar com o outro banco por
//! esse apelido, sem repetir endereco em lugar nenhum.
//!
//! # O que uma ligacao NAO carrega
//!
//! Permissao. Uma ligacao guarda UMA credencial, e todo mundo que a usa fala
//! com o outro banco como aquele usuario -- as permissoes por base do PhxSql
//! nao atravessam para o outro lado. Por isso toda operacao de DbLink exige
//! `administrar`: quem usa a ligacao esta usando o poder de quem a criou, e
//! isso nao pode ser um direito de leitor.
//!
//! # Somente leitura vem LIGADO
//!
//! Uma ligacao nasce recusando qualquer coisa que nao seja consulta. Ligar a
//! escrita e uma decisao, e nao um padrao herdado: a mesma tela que lista
//! tabelas de um banco de producao apagaria uma se a escrita viesse ligada por
//! omissao.

pub mod mysql;

use std::path::{Path, PathBuf};
use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Qual banco esta do outro lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motor {
    MySql,
    /// Reconhecido no cadastro, ainda sem cliente. Guardar a definicao antes
    /// do cliente existir e de proposito: o cadastro nao muda quando o cliente
    /// chegar.
    Postgres,
}

impl Motor {
    pub fn de_texto(s: &str) -> Result<Motor> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "mysql" | "mariadb" => Motor::MySql,
            "postgres" | "postgresql" | "pgsql" => Motor::Postgres,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "motor de dblink desconhecido: {outro:?} (use \"mysql\" ou \"postgres\")"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Motor::MySql => "mysql",
            Motor::Postgres => "postgres",
        }
    }

    pub fn porta_padrao(self) -> u16 {
        match self {
            Motor::MySql => 3306,
            Motor::Postgres => 5432,
        }
    }

    /// As operacoes do DbLink ja funcionam com este motor?
    ///
    /// **Nao e "ha cliente escrito"** -- o cliente do PostgreSQL(R) existe em
    /// `crate::pg`, com SCRAM-SHA-256 conferido contra o RFC 7677. O que falta
    /// e o DIALETO: `dblink_tabelas`, `dblink_colunas` e o teste de ligacao
    /// montam SQL de MySQL(R) (crase, `SHOW INDEX`, `current_user()`), e o
    /// `information_schema` do PostgreSQL(R) responde a outras perguntas.
    ///
    /// Devolver `true` aqui acenderia o botao na tela e o botao falharia na
    /// primeira consulta -- que e a mesma armadilha do campo de configuracao
    /// que ninguem le. O sinal diz o que a tela pode fazer, e nao o que o
    /// repositorio tem.
    pub fn conecta(self) -> bool {
        matches!(self, Motor::MySql)
    }
}

/// Uma ligacao cadastrada.
#[derive(Debug, Clone)]
pub struct Definicao {
    /// Apelido, unico. E por ele que os comandos chamam a ligacao.
    pub nome: String,
    pub motor: Motor,
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    /// PRIVADA, como a do rele de e-mail: o servidor precisa apresenta-la ao
    /// outro banco, entao nao da para guardar so o hash -- mas ela nunca sai
    /// em JSON nem em log.
    senha: String,
    /// Nome da variavel de ambiente de onde a senha veio, quando veio de la.
    pub senha_env: String,
    pub database: String,
    pub descricao: String,
    pub somente_leitura: bool,
    pub timeout_s: u64,
    pub max_linhas: u64,
}

impl Default for Definicao {
    fn default() -> Self {
        Definicao {
            nome: String::new(),
            motor: Motor::MySql,
            host: "127.0.0.1".into(),
            porta: 3306,
            usuario: String::new(),
            senha: String::new(),
            senha_env: String::new(),
            database: String::new(),
            descricao: String::new(),
            somente_leitura: true,
            timeout_s: 10,
            max_linhas: 1_000,
        }
    }
}

impl Definicao {
    pub fn de_json(j: &Json) -> Result<Definicao> {
        let padrao = Definicao::default();
        let motor = Motor::de_texto(j.texto_ou("motor", "mysql"))?;
        let nome = j.texto_ou("nome", "").trim().to_string();
        validar_nome(&nome)?;
        let senha_env = j.texto_ou("senha_env", "").trim().to_string();
        let senha = if senha_env.is_empty() {
            j.texto_ou("senha", "").to_string()
        } else {
            std::env::var(&senha_env).unwrap_or_default()
        };
        Ok(Definicao {
            nome,
            motor,
            host: j.texto_ou("host", &padrao.host).trim().to_string(),
            porta: j
                .inteiro_ou("porta", motor.porta_padrao() as i64)
                .clamp(1, 65_535) as u16,
            usuario: j.texto_ou("usuario", "").trim().to_string(),
            senha,
            senha_env,
            database: j.texto_ou("database", "").trim().to_string(),
            descricao: j.texto_ou("descricao", "").trim().to_string(),
            // Sem o campo, somente leitura. Negar por omissao e a regra do
            // projeto, e aqui ela protege o banco DO OUTRO.
            somente_leitura: j.booleano_ou("somente_leitura", true),
            timeout_s: j.inteiro_ou("timeout_s", padrao.timeout_s as i64).max(1) as u64,
            max_linhas: j
                .inteiro_ou("max_linhas", padrao.max_linhas as i64)
                .clamp(1, 100_000) as u64,
        })
    }

    /// Como a definicao vai para o disco: com a senha, quando ela nao veio do
    /// ambiente. E o unico lugar em que ela e escrita.
    fn para_disco(&self) -> Json {
        let mut campos = vec![
            ("nome", Json::texto_de(&self.nome)),
            ("motor", Json::texto_de(self.motor.nome())),
            ("host", Json::texto_de(&self.host)),
            ("porta", Json::de_u64(self.porta as u64)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("database", Json::texto_de(&self.database)),
            ("descricao", Json::texto_de(&self.descricao)),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("timeout_s", Json::de_u64(self.timeout_s)),
            ("max_linhas", Json::de_u64(self.max_linhas)),
        ];
        if self.senha_env.is_empty() {
            campos.push(("senha", Json::texto_de(&self.senha)));
        } else {
            campos.push(("senha_env", Json::texto_de(&self.senha_env)));
        }
        Json::objeto(campos)
    }

    /// Como a definicao aparece na tela e no protocolo: sem a senha, nunca.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(&self.nome)),
            ("motor", Json::texto_de(self.motor.nome())),
            ("conecta", Json::Bool(self.motor.conecta())),
            ("host", Json::texto_de(&self.host)),
            ("porta", Json::de_u64(self.porta as u64)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("database", Json::texto_de(&self.database)),
            ("descricao", Json::texto_de(&self.descricao)),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("timeout_s", Json::de_u64(self.timeout_s)),
            ("max_linhas", Json::de_u64(self.max_linhas)),
            ("senha_env", Json::texto_de(&self.senha_env)),
            (
                "senha",
                Json::texto_de(if self.senha.is_empty() {
                    "(vazia)"
                } else if self.senha_env.is_empty() {
                    "(oculta)"
                } else {
                    "(do ambiente)"
                }),
            ),
        ])
    }

    pub fn senha(&self) -> &str {
        &self.senha
    }

    /// Esta definicao, com a senha de outra.
    ///
    /// Existe para a tela de edicao: ela nunca RECEBE a senha (o `para_json`
    /// nao a manda), entao nao teria como devolve-la, e sem isto mudar a porta
    /// apagaria a credencial.
    pub fn com_a_senha_de(mut self, outra: &Definicao) -> Definicao {
        self.senha = outra.senha.clone();
        self.senha_env = outra.senha_env.clone();
        self
    }

    /// Abre a ligacao pelo cliente MySQL(R).
    ///
    /// Continua devolvendo `mysql::Conexao`, e nao um tipo comum aos dois
    /// motores, de proposito: as operacoes que a usam montam SQL de MySQL(R)
    /// -- crase em volta do nome, `current_user()`, `SHOW INDEX`. Um tipo
    /// comum faria o codigo COMPILAR para o PostgreSQL(R) e falhar na primeira
    /// consulta, que e pior do que nao compilar.
    pub fn conectar(&self) -> Result<mysql::Conexao> {
        match self.motor {
            Motor::MySql => mysql::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                &self.senha,
                &self.database,
                Duration::from_secs(self.timeout_s),
            ),
            Motor::Postgres => Err(PhxError::Esquema(
                "esta ligacao e PostgreSQL(R): o cliente existe (`pg::Conexao`, \
                 com SCRAM-SHA-256), mas as operacoes do DbLink ainda montam SQL \
                 de MySQL(R). Use `conectar_pg` ate o dialeto entrar."
                    .into(),
            )),
        }
    }

    /// Abre a ligacao pelo cliente PostgreSQL(R).
    pub fn conectar_pg(&self) -> Result<crate::pg::Conexao> {
        match self.motor {
            Motor::Postgres => crate::pg::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                &self.senha,
                &self.database,
                Duration::from_secs(self.timeout_s),
            ),
            Motor::MySql => Err(PhxError::Esquema(
                "esta ligacao e MySQL(R); use `conectar`".into(),
            )),
        }
    }
}

/// O apelido vira parte de comando e de caminho, entao nao pode ser qualquer
/// coisa.
fn validar_nome(nome: &str) -> Result<()> {
    if nome.is_empty() {
        return Err(PhxError::Esquema("dblink sem nome".into()));
    }
    if nome.len() > 40 {
        return Err(PhxError::Esquema(format!(
            "nome de dblink longo demais: {nome:?} (maximo 40)"
        )));
    }
    if !nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PhxError::Esquema(format!(
            "nome de dblink com caractere que nao vale: {nome:?} (use letras, numeros, _ e -)"
        )));
    }
    Ok(())
}

/// O cadastro inteiro, lido e gravado num arquivo so.
#[derive(Debug, Default)]
pub struct Registro {
    pub caminho: PathBuf,
    pub ligacoes: Vec<Definicao>,
}

impl Registro {
    /// Le o arquivo. Arquivo que nao existe e cadastro vazio, e nao erro: um
    /// servidor sem nenhuma ligacao e o caso normal.
    pub fn abrir(caminho: &Path) -> Result<Registro> {
        let mut r = Registro {
            caminho: caminho.to_path_buf(),
            ligacoes: Vec::new(),
        };
        let Ok(texto) = std::fs::read_to_string(caminho) else {
            return Ok(r);
        };
        if texto.trim().is_empty() {
            return Ok(r);
        }
        let j = Json::analisar(&texto)?;
        let lista = j
            .campo("dblink")
            .and_then(Json::lista)
            .or_else(|| j.lista())
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{}: esperava uma lista de ligacoes, ou um objeto com \"dblink\"",
                    caminho.display()
                ))
            })?;
        for item in lista {
            r.ligacoes.push(Definicao::de_json(item)?);
        }
        r.conferir_repetidos()?;
        Ok(r)
    }

    fn conferir_repetidos(&self) -> Result<()> {
        let mut vistos = std::collections::HashSet::new();
        for l in &self.ligacoes {
            if !vistos.insert(l.nome.to_lowercase()) {
                return Err(PhxError::Esquema(format!(
                    "duas ligacoes com o nome {:?}: o apelido tem de ser unico",
                    l.nome
                )));
            }
        }
        Ok(())
    }

    pub fn achar(&self, nome: &str) -> Result<&Definicao> {
        self.ligacoes
            .iter()
            .find(|l| l.nome.eq_ignore_ascii_case(nome))
            .ok_or_else(|| PhxError::NaoEncontrado(format!("dblink {nome:?} nao existe")))
    }

    /// Grava ou substitui uma ligacao. Substituir pelo nome e o que faz a tela
    /// de edicao funcionar sem um identificador a mais.
    pub fn salvar(&mut self, d: Definicao) -> Result<()> {
        match self
            .ligacoes
            .iter()
            .position(|l| l.nome.eq_ignore_ascii_case(&d.nome))
        {
            Some(i) => self.ligacoes[i] = d,
            None => self.ligacoes.push(d),
        }
        self.gravar()
    }

    pub fn excluir(&mut self, nome: &str) -> Result<()> {
        let antes = self.ligacoes.len();
        self.ligacoes.retain(|l| !l.nome.eq_ignore_ascii_case(nome));
        if self.ligacoes.len() == antes {
            return Err(PhxError::NaoEncontrado(format!(
                "dblink {nome:?} nao existe"
            )));
        }
        self.gravar()
    }

    /// Grava o arquivo inteiro, com permissao de dono so.
    ///
    /// O arquivo carrega senha de outro banco. Deixa-lo legivel por todo mundo
    /// seria guardar a credencial atras de uma porta aberta.
    fn gravar(&self) -> Result<()> {
        let j = Json::objeto(vec![(
            "dblink",
            Json::Lista(self.ligacoes.iter().map(Definicao::para_disco).collect()),
        )]);
        let temporario = self.caminho.with_extension("tmp");
        std::fs::write(&temporario, j.escrever_identado())
            .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", temporario.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temporario, std::fs::Permissions::from_mode(0o600));
        }
        // Troca atomica: um corte de energia no meio deixa o arquivo antigo
        // inteiro, e nao um cadastro pela metade.
        std::fs::rename(&temporario, &self.caminho)
            .map_err(|e| PhxError::Esquema(format!("nao troquei {}: {e}", self.caminho.display())))
    }
}

/// A instrucao so consulta?
///
/// Usada quando a ligacao esta em somente leitura. Duas coisas seguram o
/// caso, e as duas precisam existir:
///
/// 1. a primeira palavra tem de ser de consulta;
/// 2. `INTO OUTFILE`/`DUMPFILE` estao fora, porque um `SELECT` que escreve
///    arquivo no servidor do outro lado continua sendo um `SELECT`.
///
/// Emendar uma segunda instrucao com `;` nao entra na conta porque nao e
/// possivel: o cliente nao pede `CLIENT_MULTI_STATEMENTS`, e o servidor
/// recusa o pacote com duas.
pub fn so_consulta(sql: &str) -> bool {
    let limpo = sem_comentarios(sql);
    let alto = limpo.trim().to_uppercase();
    let primeira = alto
        .split(|c: char| c.is_whitespace() || c == '(')
        .find(|p| !p.is_empty())
        .unwrap_or("");
    let consulta = matches!(
        primeira,
        "SELECT" | "SHOW" | "DESCRIBE" | "DESC" | "EXPLAIN" | "WITH" | "TABLE" | "VALUES"
    );
    consulta && !alto.contains("INTO OUTFILE") && !alto.contains("INTO DUMPFILE")
}

/// Tira comentario de SQL antes de olhar a primeira palavra.
///
/// Sem isto, `/*x*/ DROP TABLE t` teria como primeira palavra `/*X*/` -- que
/// nao e nenhuma das de consulta, entao o comando seria RECUSADO, e nao
/// aceito. O buraco de verdade e o contrario: `/*x*/SELECT` seria recusado
/// sem motivo. Tirar o comentario acerta os dois.
fn sem_comentarios(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut fora = String::with_capacity(sql.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i += 2;
            fora.push(' ');
        } else if (b[i] == b'-' && b.get(i + 1) == Some(&b'-')) || b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            fora.push(' ');
        } else {
            fora.push(b[i] as char);
            i += 1;
        }
    }
    fora
}

/// Um nome de tabela, coluna ou base vindo da tela, conferido antes de virar
/// SQL.
///
/// A defesa e recusar, e nao escapar. Escapar aspas exige saber em que modo o
/// outro servidor esta -- com `NO_BACKSLASH_ESCAPES` a contrabarra deixa de
/// escapar, e a mesma regra que protegia passa a nao proteger. Nome de objeto
/// nao precisa de aspa, crase, contrabarra nem quebra de linha, entao nada
/// disso entra.
pub fn nome_seguro(nome: &str) -> Result<String> {
    let n = nome.trim();
    if n.is_empty() {
        return Err(PhxError::Esquema("nome vazio".into()));
    }
    if n.len() > 128 {
        return Err(PhxError::Esquema(format!(
            "nome longo demais: {n:?} (o MySQL(R) para em 64)"
        )));
    }
    if n.chars()
        .any(|c| c.is_control() || matches!(c, '`' | '\'' | '"' | '\\'))
    {
        return Err(PhxError::Esquema(format!(
            "nome com caractere que nao vale em identificador: {n:?}"
        )));
    }
    Ok(n.to_string())
}

/// Um nome que precisa virar TEXTO no SQL, como em `TABLE_SCHEMA = '...'`.
///
/// Passa pelo mesmo crivo do identificador e so depois vira literal: sem aspa
/// nem contrabarra dentro, as aspas de fora fecham onde devem em qualquer modo
/// do servidor.
pub fn literal(valor: &str) -> Result<String> {
    Ok(format!("'{}'", nome_seguro(valor)?))
}

/// Um nome de tabela ou coluna vindo da tela, protegido com crase.
///
/// A crase dobrada e como o MySQL(R) escapa uma crase dentro do nome. Sem
/// isto, um nome de tabela escolhido por quem usa a tela emendaria SQL.
pub fn entre_crases(nome: &str) -> String {
    format!("`{}`", nome.replace('`', "``"))
}

/// O resultado da consulta, no formato que a grade da tela espera.
pub fn resultado_para_json(r: &mysql::Resultado) -> Json {
    Json::objeto(vec![
        (
            "colunas",
            Json::Lista(
                r.colunas
                    .iter()
                    .map(|c| {
                        Json::objeto(vec![
                            ("nome", Json::texto_de(&c.nome)),
                            ("tabela", Json::texto_de(&c.tabela)),
                            ("tipo", Json::texto_de(&c.tipo)),
                            ("tamanho", Json::de_u64(c.tamanho as u64)),
                            ("decimais", Json::de_u64(c.decimais as u64)),
                            ("nulavel", Json::Bool(c.nulavel)),
                            ("primaria", Json::Bool(c.primaria)),
                            ("numerico", Json::Bool(c.numerico)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "linhas",
            Json::Lista(
                r.linhas
                    .iter()
                    .map(|l| {
                        Json::Lista(
                            l.iter()
                                .map(|v| match v {
                                    Some(t) => Json::texto_de(t),
                                    None => Json::Nulo,
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        ("quantas", Json::de_u64(r.linhas.len() as u64)),
        ("afetadas", Json::de_u64(r.afetadas)),
        ("truncado", Json::Bool(r.truncado)),
    ])
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_ligacao_nasce_somente_leitura() {
        let d = Definicao::de_json(&Json::analisar(r#"{"nome":"loja"}"#).unwrap()).unwrap();
        assert!(d.somente_leitura, "uma ligacao nova podia escrever");
        assert_eq!(d.porta, 3306);
    }

    #[test]
    fn o_apelido_nao_aceita_qualquer_coisa() {
        for ruim in [
            r#"{"nome":""}"#,
            r#"{"nome":"a b"}"#,
            r#"{"nome":"../etc"}"#,
        ] {
            assert!(
                Definicao::de_json(&Json::analisar(ruim).unwrap()).is_err(),
                "aceitou {ruim}"
            );
        }
        assert!(Definicao::de_json(&Json::analisar(r#"{"nome":"loja-1_A"}"#).unwrap()).is_ok());
    }

    #[test]
    fn a_senha_da_ligacao_nunca_aparece_no_json() {
        let d = Definicao::de_json(
            &Json::analisar(r#"{"nome":"loja","senha":"segredo-do-outro-banco"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(d.senha(), "segredo-do-outro-banco");
        let t = d.para_json().escrever();
        assert!(!t.contains("segredo-do-outro-banco"), "a senha vazou: {t}");
        assert!(t.contains("(oculta)"));
    }

    #[test]
    fn so_consulta_deixa_passar_o_que_le() {
        for bom in [
            "SELECT * FROM t",
            "  select 1",
            "show tables",
            "DESCRIBE clientes",
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "/* comentario */ SELECT 1",
            "-- nota\nSELECT 1",
        ] {
            assert!(so_consulta(bom), "recusou {bom:?}");
        }
    }

    #[test]
    fn so_consulta_barra_o_que_escreve() {
        for ruim in [
            "DELETE FROM t",
            "drop table t",
            "UPDATE t SET a=1",
            "INSERT INTO t VALUES (1)",
            "TRUNCATE t",
            "GRANT ALL ON *.* TO x",
            // Um SELECT que escreve arquivo no servidor do outro lado
            // continua sendo escrita.
            "SELECT * FROM t INTO OUTFILE '/tmp/x'",
            "select a from t into dumpfile '/tmp/x'",
            // A tentativa de esconder o comando atras de comentario.
            "/* SELECT */ DROP TABLE t",
        ] {
            assert!(!so_consulta(ruim), "deixou passar {ruim:?}");
        }
    }

    #[test]
    fn nome_seguro_recusa_o_que_emendaria_sql() {
        for ruim in [
            "cli`entes",
            "cli'entes",
            "cli\"entes",
            "cli\\entes",
            "cli\nentes",
            "  ",
        ] {
            assert!(nome_seguro(ruim).is_err(), "aceitou {ruim:?}");
        }
        assert_eq!(nome_seguro(" clientes ").unwrap(), "clientes");
        // Nome com espaco e acento continua valendo: sao legais no MySQL(R) e
        // a crase de fora resolve.
        assert_eq!(nome_seguro("Notas Fiscais").unwrap(), "Notas Fiscais");
        assert_eq!(literal("loja").unwrap(), "'loja'");
    }

    #[test]
    fn a_crase_no_nome_e_escapada() {
        assert_eq!(entre_crases("clientes"), "`clientes`");
        // O nome que emendaria SQL se entrasse cru.
        assert_eq!(
            entre_crases("a`; DROP TABLE x; --"),
            "`a``; DROP TABLE x; --`"
        );
    }

    #[test]
    fn cadastro_vazio_quando_o_arquivo_nao_existe() {
        let r = Registro::abrir(Path::new("/nao/existe/dblink.json")).unwrap();
        assert!(r.ligacoes.is_empty());
    }

    #[test]
    fn grava_le_e_nao_perde_a_senha() {
        let dir = std::env::temp_dir().join(format!("phx-dblink-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("dblink.json");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(
            Definicao::de_json(
                &Json::analisar(r#"{"nome":"loja","host":"10.0.0.5","senha":"abc","usuario":"u"}"#)
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let lido = Registro::abrir(&caminho).unwrap();
        assert_eq!(lido.ligacoes.len(), 1);
        assert_eq!(lido.achar("LOJA").unwrap().senha(), "abc");
        assert_eq!(lido.achar("loja").unwrap().host, "10.0.0.5");
        // Salvar de novo com o mesmo nome substitui, nao duplica.
        let mut r2 = lido;
        r2.salvar(
            Definicao::de_json(&Json::analisar(r#"{"nome":"loja","host":"10.0.0.9"}"#).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(Registro::abrir(&caminho).unwrap().ligacoes.len(), 1);
        assert_eq!(
            Registro::abrir(&caminho)
                .unwrap()
                .achar("loja")
                .unwrap()
                .host,
            "10.0.0.9"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

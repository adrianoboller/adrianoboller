//! Uma conexao de DbLink, seja qual for o motor do outro lado.
//!
//! # Por que este tipo demorou a existir
//!
//! O comentario que estava em `Definicao::conectar` explicava a demora e
//! estava certo: um tipo comum aos dois motores faria o codigo **compilar**
//! para o PostgreSQL(R) e falhar na primeira consulta, porque o SQL era de
//! MySQL(R). Compilar e falhar em producao e pior do que nao compilar.
//!
//! O que mudou foi o `dialeto`: agora as perguntas do DbLink existem nas duas
//! linguas. Com elas escritas, o tipo comum deixa de esconder um buraco e passa
//! a esconder uma diferenca que ja foi resolvida -- que e o que uma abstracao
//! deve fazer.
//!
//! # O que ele NAO uniformiza
//!
//! O formato do resultado de cada pergunta. `SHOW FULL COLUMNS` e a consulta
//! ao `pg_attribute` devolvem as mesmas seis colunas na mesma ordem porque o
//! `dialeto` as montou assim, e nao porque este tipo as tenha reordenado. A
//! traducao mora no SQL, onde da para ler as duas versoes lado a lado.

use std::time::Duration;

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use super::{mysql, phx, Definicao, Motor};
use crate::pg;

/// Uma coluna do resultado, no formato que a grade da tela espera.
///
/// E o menor denominador HONESTO dos dois: o que o MySQL(R) traz e o
/// PostgreSQL(R) nao (tabela de origem, casas decimais) sai vazio ou zero em
/// vez de inventado.
#[derive(Debug, Clone, Default)]
pub struct Coluna {
    pub nome: String,
    pub tabela: String,
    pub tipo: String,
    pub tamanho: u32,
    pub decimais: u8,
    pub nulavel: bool,
    pub primaria: bool,
    pub numerico: bool,
}

#[derive(Debug, Default)]
pub struct Resultado {
    pub colunas: Vec<Coluna>,
    /// `None` e NULL de verdade, e nao cadeia vazia -- a diferenca importa.
    pub linhas: Vec<Vec<Option<String>>>,
    pub afetadas: u64,
    /// Verdadeiro quando o teto cortou o resultado.
    pub truncado: bool,
}

impl Resultado {
    /// O valor de uma celula, ou `None` quando ela e NULL ou nao existe.
    pub fn celula(&self, linha: usize, coluna: usize) -> Option<String> {
        self.linhas.get(linha)?.get(coluna)?.clone()
    }

    /// Como a grade da tela le o resultado.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            (
                "colunas",
                Json::Lista(
                    self.colunas
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
                    self.linhas
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
            ("quantas", Json::de_u64(self.linhas.len() as u64)),
            ("afetadas", Json::de_u64(self.afetadas)),
            ("truncado", Json::Bool(self.truncado)),
        ])
    }
}

impl From<mysql::Resultado> for Resultado {
    fn from(r: mysql::Resultado) -> Resultado {
        Resultado {
            colunas: r
                .colunas
                .into_iter()
                .map(|c| Coluna {
                    nome: c.nome,
                    tabela: c.tabela,
                    tipo: c.tipo,
                    tamanho: c.tamanho,
                    decimais: c.decimais,
                    nulavel: c.nulavel,
                    primaria: c.primaria,
                    numerico: c.numerico,
                })
                .collect(),
            linhas: r.linhas,
            afetadas: r.afetadas,
            truncado: r.truncado,
        }
    }
}

impl From<pg::Resultado> for Resultado {
    fn from(r: pg::Resultado) -> Resultado {
        Resultado {
            colunas: r
                .colunas
                .into_iter()
                .map(|c| Coluna {
                    nome: c.nome,
                    // O `RowDescription` traz o OID da tabela, e nao o NOME
                    // dela; resolver o nome exigiria uma segunda consulta ao
                    // `pg_class` por coluna. Fica vazio, que e a verdade, em
                    // vez de um numero que a tela mostraria como nome.
                    tabela: String::new(),
                    tipo: c.tipo,
                    tamanho: if c.tamanho > 0 { c.tamanho as u32 } else { 0 },
                    decimais: 0,
                    // O protocolo simples nao diz se a coluna aceita NULO nem
                    // se e chave; quem quer isso pergunta a `dblink_estrutura`,
                    // que consulta o catalogo.
                    nulavel: true,
                    primaria: false,
                    numerico: c.numerico,
                })
                .collect(),
            linhas: r.linhas,
            afetadas: r.afetadas,
            truncado: r.truncado,
        }
    }
}

/// A conexao aberta com o outro banco.
///
/// O terceiro caso NAO fala SQL para o catalogo: `Conexao::Phx` responde as
/// perguntas do DbLink pelo protocolo proprio, e por isso as operacoes o
/// desviam ANTES de montarem instrucao nenhuma (ver `operacoes`). Um metodo
/// aqui que fingisse aceitar SQL de catalogo compilaria e falharia na primeira
/// consulta -- a mesma armadilha que este arquivo ja documenta no cabecalho.
pub enum Conexao {
    MySql(Box<mysql::Conexao>),
    Postgres(Box<pg::Conexao>),
    Phx(Box<phx::Conexao>),
}

impl Conexao {
    /// Uma instrucao SQL contra o outro banco.
    ///
    /// O caso `Phx` precisa de um database, porque a op `sql` do PhxSql o pede
    /// -- e o unico chamador que passa por aqui e o `dblink_consultar`, que
    /// tem o pedido na mao. Ver `Conexao::consultar_em`.
    pub fn consultar(&mut self, sql: &str, teto: u64) -> Result<Resultado> {
        self.consultar_em("", sql, teto)
    }

    /// A mesma consulta, dizendo em que database ela roda.
    ///
    /// O `database` so vale para o motor `phxsql`: nos outros dois a base ja
    /// foi escolhida no aperto de mao, e mandar de novo nao mudaria nada.
    pub fn consultar_em(&mut self, database: &str, sql: &str, teto: u64) -> Result<Resultado> {
        match self {
            Conexao::MySql(c) => Ok(c.consultar(sql, teto)?.into()),
            Conexao::Postgres(c) => Ok(c.consultar(sql, teto)?.into()),
            Conexao::Phx(c) => c.consultar(database, sql, teto),
        }
    }

    pub fn ping(&mut self) -> Result<()> {
        match self {
            Conexao::MySql(c) => c.ping(),
            Conexao::Postgres(c) => c.ping(),
            Conexao::Phx(c) => c.ping().map(|_| ()),
        }
    }

    pub fn encerrar(&mut self) {
        match self {
            Conexao::MySql(c) => c.encerrar(),
            Conexao::Postgres(c) => c.encerrar(),
            // O cliente do protocolo proprio fecha o soquete ao ser largado.
            Conexao::Phx(_) => {}
        }
    }

    /// A versao anunciada pelo outro servidor.
    pub fn versao(&self) -> String {
        match self {
            Conexao::MySql(c) => c.versao.clone(),
            Conexao::Postgres(c) => c.versao.clone(),
            Conexao::Phx(c) => c.versao.clone(),
        }
    }

    /// O identificador da conexao do lado de la: `connection_id()` no
    /// MySQL(R), o PID do processo no PostgreSQL(R).
    ///
    /// O PhxSql nao numera a conexao -- ele conta quantas ha --, entao ali
    /// vale zero. Inventar um numero seria pior que nao ter nenhum.
    pub fn conexao_id(&self) -> u32 {
        match self {
            Conexao::MySql(c) => c.conexao_id,
            Conexao::Postgres(c) => c.conexao_id,
            Conexao::Phx(_) => 0,
        }
    }
}

impl Definicao {
    /// Abre a ligacao pelo cliente do motor que ela declara.
    ///
    /// Substitui o par `conectar`/`conectar_pg`, que continuam existindo para
    /// quem precisa do tipo concreto -- o teste de protocolo, por exemplo.
    pub fn abrir(&self) -> Result<Conexao> {
        let espera = Duration::from_secs(self.timeout_s);
        Ok(match self.motor {
            Motor::MySql => Conexao::MySql(Box::new(mysql::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha(),
                &self.database,
                espera,
            )?)),
            Motor::Postgres => Conexao::Postgres(Box::new(pg::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha(),
                &self.database,
                espera,
            )?)),
            Motor::Phx => Conexao::Phx(Box::new(phx::Conexao::abrir(
                &self.host,
                self.porta,
                self.token(),
                &self.usuario,
                self.senha(),
                espera,
            )?)),
        })
    }
}

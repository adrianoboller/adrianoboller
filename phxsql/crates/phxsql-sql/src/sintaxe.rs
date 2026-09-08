//! Analisador sintatico do `SELECT` simples.
//!
//! A gramatica inteira, e ela cabe aqui:
//!
//! ```text
//! SELECT  ( * | COUNT(*) | coluna [AS apelido] {, coluna [AS apelido]} )
//! FROM    [database.] [schema.] tabela [[AS] apelido]
//! [WHERE  coluna comparador literal]
//! [ORDER BY coluna [ASC|DESC]]
//! [LIMIT  n [OFFSET m]]
//! ```
//!
//! Nao ha `JOIN`, nao ha subconsulta, nao ha expressao e nao ha `AND`. Isso
//! nao e economia de esforco: e o que `docs/SQL.md` mediu. Uma expressao como
//! `WHERE preco * 1.1 > 100` nao tem quem avalie embaixo, e prometer o verbo
//! sem o mecanismo e pior do que nao ter o verbo. Cada coisa que falta sai
//! daqui como recusa escrita, com o nome da clausula -- e nao como sintaxe
//! aceita que quebra depois.

use crate::dml::{Atualizacao, Exclusao, Insercao};
use crate::lexico::{self, Comparador, Simbolo, Token};
use phxsql_core::{PhxError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Guardado como texto pelo mesmo motivo do lexico: decimal nao passa por
    /// `f64` sem perder digito.
    Numero(String),
    Texto(String),
    Bool(bool),
    Nulo,
}

impl Literal {
    pub fn escrever(&self) -> String {
        match self {
            Literal::Numero(n) => n.clone(),
            Literal::Texto(t) => format!("'{}'", t.replace('\'', "''")),
            Literal::Bool(true) => "TRUE".into(),
            Literal::Bool(false) => "FALSE".into(),
            Literal::Nulo => "NULL".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColunaPedida {
    pub nome: String,
    pub apelido: Option<String>,
}

impl ColunaPedida {
    /// O nome com que a coluna sai na resposta.
    pub fn rotulo(&self) -> &str {
        self.apelido.as_deref().unwrap_or(&self.nome)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Projecao {
    /// `SELECT *`
    Tudo,
    /// `SELECT COUNT(*)` -- o unico agregado, e so porque o `varrer` ja
    /// responde a contagem em O(1), lendo dois campos do cabecalho.
    Contagem,
    Colunas(Vec<ColunaPedida>),
}

/// Para onde o `FROM` aponta, ja separado nas tres partes que o motor usa.
///
/// O enderecamento com `schema` ja funciona hoje em toda operacao do
/// protocolo: `tabela: "matriz.estoque"` abre a pasta certa. O que faltava era
/// alguem escrever `FROM matriz.estoque` e chegar la -- e e isto.
#[derive(Debug, Clone, PartialEq)]
pub struct Alvo {
    /// Vazio quando a consulta nao disse -- quem executa poe o banco corrente.
    pub database: String,
    /// Vazio quando a tabela mora na raiz do banco.
    pub schema: String,
    pub tabela: String,
    pub apelido: Option<String>,
}

impl Alvo {
    /// O que vai no campo `"tabela"` do pedido: `estoque` ou `matriz.estoque`.
    pub fn nome_no_protocolo(&self) -> String {
        if self.schema.is_empty() {
            self.tabela.clone()
        } else {
            format!("{}.{}", self.schema, self.tabela)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condicao {
    pub coluna: String,
    pub op: Comparador,
    pub valor: Literal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ordenacao {
    pub coluna: String,
    pub desc: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selecao {
    pub projecao: Projecao,
    pub de: Alvo,
    pub onde: Option<Condicao>,
    pub ordem: Option<Ordenacao>,
    pub limite: Option<u64>,
    pub salto: u64,
}

/// O que a op `sql` recebe como instrucao de DADO: uma consulta, ou um dos
/// tres verbos de escrita por chave (`dml.rs`). Transacao, diretiva, rotina
/// e usuario nao passam por aqui -- sao comandos de sessao ou de catalogo, e
/// cada um tem o proprio detector, consultado antes.
#[derive(Debug, Clone, PartialEq)]
pub enum Comando {
    Selecao(Selecao),
    Insercao(Insercao),
    Atualizacao(Atualizacao),
    Exclusao(Exclusao),
}

impl Comando {
    /// O verbo, para as mensagens.
    pub fn verbo(&self) -> &'static str {
        match self {
            Comando::Selecao(_) => "SELECT",
            Comando::Insercao(_) => "INSERT",
            Comando::Atualizacao(_) => "UPDATE",
            Comando::Exclusao(_) => "DELETE",
        }
    }

    /// A tabela alvo, para quem precisa do esquema antes de traduzir.
    pub fn alvo(&self) -> &Alvo {
        match self {
            Comando::Selecao(s) => &s.de,
            Comando::Insercao(i) => &i.em,
            Comando::Atualizacao(a) => &a.em,
            Comando::Exclusao(e) => &e.de,
        }
    }
}

/// As palavras que ja tem significado no motor e nao podem virar identificador.
///
/// `docs/SQL.md` explica por que a lista mora AQUI e nao no `validar_nome` do
/// catalogo: reservar palavra no motor quebraria banco de quem ja tem a
/// tabela. O parser e novo, entao a reserva so custa a quem escrever SQL.
pub const RESERVADAS_DO_MOTOR: [&str; 1] = ["BULKINSERT"];

/// Palavras da gramatica que nao podem ser lidas como nome de tabela ou de
/// coluna sem aspas.
const CLAUSULAS: [&str; 18] = [
    "SELECT", "FROM", "WHERE", "ORDER", "GROUP", "BY", "LIMIT", "OFFSET", "AS", "HAVING", "JOIN",
    "UNION",
    // Os verbos de escrita e as clausulas deles. SET e VALUES precisam estar
    // aqui por um motivo concreto: o `alvo` aceita apelido SEM `AS`, e um
    // `UPDATE t SET ...` leria SET como apelido da tabela.
    "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
];

/// O texto traz MAIS DE UM comando empilhado -- o `; DROP TABLE ...` classico?
///
/// # Por que ela ANALISA em vez de recortar
///
/// Procurar `";"` no texto acusaria `WHERE nome = '; DROP TABLE clientes'`,
/// que e um DADO legitimo e ja foi gravado assim nesta casa (a bateria
/// `bancada/seguranca/injecao.py` grava exatamente esse valor e o le de
/// volta). Quem decide aqui e o LEXICO do motor -- o mesmo que a consulta
/// usa --, entao o que esta dentro de aspas e um simbolo `Texto` e nunca um
/// separador de comando. E a mesma lei do Profiler: *o que mostra texto cru
/// analisa, nunca recorta*.
///
/// # O que ela responde `false`, e nao e engano
///
/// * texto que o lexico nao consegue ler (aspa aberta, byte estranho): sem
///   simbolos nao ha o que classificar, e chutar aqui seria acusar quem
///   digitou errado. Erro de sintaxe **nao** e injecao;
/// * `SELECT ... ;` com o ponto-e-virgula no fim, que e como todo cliente
///   ODBC manda;
/// * `-- comentario` depois do comando, que o lexico ja descarta.
///
/// Ou seja: so responde `true` quando ha simbolo DEPOIS de um
/// ponto-e-virgula -- que e um segundo comando, e nada mais e.
pub fn comando_empilhado(entrada: &str) -> bool {
    let Ok(simbolos) = lexico::analisar(entrada) else {
        return false;
    };
    if simbolos.is_empty() {
        return false;
    }
    let mut p = Analisador { s: simbolos, i: 0 };
    // Nem chegou a ser um comando desta gramatica -- `CREATE PROCEDURE`,
    // `CALL`, `BEGIN`, ou sintaxe torta. Sai `false`, e a saida e a que mais
    // importa: o CORPO de um procedimento e de um gatilho e cheio de
    // ponto-e-virgula legitimo, e classificar por simbolo solto acusaria todo
    // `CREATE PROCEDURE ... BEGIN a; b; END` que falhasse por qualquer outro
    // motivo.
    if p.comando().is_err() {
        return false;
    }
    while p.aceitar(&Token::PontoEVirgula) {}
    p.espiar().is_some()
}

/// Le um comando inteiro. Um por vez -- lote de comandos e outra rodada.
/// Le uma CONSULTA. Escrita por aqui recusa dizendo que e escrita: a porta
/// dos tres verbos e [`analisar_comando`], que a op `sql` usa.
pub fn analisar(entrada: &str) -> Result<Selecao> {
    match analisar_comando(entrada)? {
        Comando::Selecao(s) => Ok(s),
        outro => Err(PhxError::Esquema(format!(
            "{} e comando de ESCRITA, e esta porta le consultas -- a op `sql` aceita os \
             dois, por `analisar_comando`",
            outro.verbo()
        ))),
    }
}

/// Le qualquer instrucao de dado: `SELECT`, ou `INSERT`/`UPDATE`/`DELETE`
/// por chave.
pub fn analisar_comando(entrada: &str) -> Result<Comando> {
    let simbolos = lexico::analisar(entrada)?;
    if simbolos.is_empty() {
        return Err(PhxError::Esquema("comando SQL vazio".into()));
    }
    let mut p = Analisador { s: simbolos, i: 0 };
    let cmd = p.comando()?;
    p.aceitar(&Token::PontoEVirgula);
    if let Some(sobra) = p.espiar() {
        return Err(lexico::erro(
            sobra.posicao,
            &format!(
                "sobrou {:?} depois do fim do comando; um comando por vez",
                sobra.token.descrever()
            ),
        ));
    }
    Ok(cmd)
}

/// O cursor de simbolos e compartilhado com `rotina`, que analisa os corpos
/// de gatilho e de procedimento: um leitor so, para as mensagens de erro (com
/// a coluna) sairem iguais nas duas gramaticas.
pub(crate) struct Analisador {
    pub(crate) s: Vec<Simbolo>,
    pub(crate) i: usize,
}

impl Analisador {
    pub(crate) fn espiar(&self) -> Option<&Simbolo> {
        self.s.get(self.i)
    }

    pub(crate) fn posicao_atual(&self) -> usize {
        match self.s.get(self.i) {
            Some(s) => s.posicao,
            // Fim do texto: aponta logo depois do ultimo simbolo lido.
            None => self.s.last().map(|s| s.posicao + 1).unwrap_or(0),
        }
    }

    /// Consome o proximo simbolo se ele for exatamente este.
    pub(crate) fn aceitar(&mut self, t: &Token) -> bool {
        if self.espiar().map(|s| &s.token) == Some(t) {
            self.i += 1;
            return true;
        }
        false
    }

    /// Consome a proxima palavra se ela for esta clausula.
    pub(crate) fn aceitar_palavra(&mut self, palavra: &str) -> bool {
        let bate = self
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .is_some_and(|p| p == palavra);
        if bate {
            self.i += 1;
        }
        bate
    }

    pub(crate) fn exigir_palavra(&mut self, palavra: &str) -> Result<()> {
        if self.aceitar_palavra(palavra) {
            return Ok(());
        }
        Err(lexico::erro(
            self.posicao_atual(),
            &format!("esperava {palavra}{}", self.mas_veio()),
        ))
    }

    pub(crate) fn mas_veio(&self) -> String {
        match self.espiar() {
            Some(s) => format!(", e veio {:?}", s.token.descrever()),
            None => ", e o comando acabou".into(),
        }
    }

    /// Le um identificador: palavra que nao seja clausula, ou citada.
    pub(crate) fn identificador(&mut self, papel: &str) -> Result<String> {
        let Some(s) = self.espiar() else {
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava {papel}, e o comando acabou"),
            ));
        };
        let pos = s.posicao;
        match &s.token {
            Token::Palavra { texto, citado } => {
                let nome = texto.clone();
                if !citado {
                    let alto = nome.to_uppercase();
                    if CLAUSULAS.contains(&alto.as_str()) {
                        return Err(lexico::erro(
                            pos,
                            &format!(
                                "{alto} e palavra da linguagem e nao serve de {papel}; \
                                 entre aspas duplas ela vira nome"
                            ),
                        ));
                    }
                    if RESERVADAS_DO_MOTOR.contains(&alto.as_str()) {
                        return Err(lexico::erro(
                            pos,
                            &format!(
                                "{alto} e comando do PhxSql, e nao pode ser {papel}. \
                                 Ele reserva a tabela para carga e vive na CONEXAO, \
                                 nao na instrucao"
                            ),
                        ));
                    }
                }
                self.i += 1;
                Ok(nome)
            }
            outro => Err(lexico::erro(
                pos,
                &format!("esperava {papel}, e veio {:?}", outro.descrever()),
            )),
        }
    }

    fn comando(&mut self) -> Result<Comando> {
        let Some(primeiro) = self.espiar() else {
            return Err(PhxError::Esquema("comando SQL vazio".into()));
        };
        let pos = primeiro.posicao;
        let verbo = primeiro.token.palavra_chave().unwrap_or_default();
        match verbo.as_str() {
            "SELECT" => {
                self.i += 1;
                Ok(Comando::Selecao(self.selecao()?))
            }
            // O passo 2 do roteiro de `docs/SQL.md`: escrita por chave. O
            // parser mora em `dml.rs`, sobre este mesmo cursor.
            "INSERT" => {
                self.i += 1;
                Ok(Comando::Insercao(self.insercao(pos)?))
            }
            "UPDATE" => {
                self.i += 1;
                Ok(Comando::Atualizacao(self.atualizacao(pos)?))
            }
            "DELETE" => {
                self.i += 1;
                Ok(Comando::Exclusao(self.exclusao(pos)?))
            }
            "BULKINSERT" => Err(lexico::erro(
                pos,
                "BULKINSERT e comando de SESSAO, e nao de instrucao: ele reserva a \
                 tabela para carga e a reserva morre com a conexao. Hoje se pede pela \
                 porta de dados, com a operacao bulkinsert",
            )),
            // Chegar AQUI quer dizer que o `transacao::comando` nao
            // reconheceu a forma -- ele e consultado antes, no `op_sql`. Entao
            // a recusa nao e mais "nao ha transacao": e "esta forma nao".
            "BEGIN" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" | "START" => {
                Err(lexico::erro(
                    pos,
                    &format!(
                        "{verbo} e comando de SESSAO e nao de consulta, e esta \
                         forma dele nao foi reconhecida. As aceitas: BEGIN, \
                         BEGIN TRANSACTION, START TRANSACTION, COMMIT, ROLLBACK, \
                         SAVEPOINT <nome>, ROLLBACK TO SAVEPOINT <nome> e \
                         RELEASE SAVEPOINT <nome>"
                    ),
                ))
            }
            "" => Err(lexico::erro(pos, "o comando nao comeca por um verbo")),
            outro => Err(lexico::erro(
                pos,
                &format!("{outro} nao e um comando desta camada"),
            )),
        }
    }

    fn selecao(&mut self) -> Result<Selecao> {
        if self.aceitar_palavra("DISTINCT") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina \
                 repetido numa varredura",
            ));
        }
        let projecao = self.projecao()?;
        self.exigir_palavra("FROM")?;
        let de = self.alvo()?;

        if self.aceitar_palavra("JOIN")
            || self.aceitar_palavra("INNER")
            || self.aceitar_palavra("LEFT")
            || self.aceitar_palavra("RIGHT")
        {
            return Err(lexico::erro(
                self.posicao_atual(),
                "junção ainda nao passa por aqui. O motor ja junta -- e a operacao \
                 juntar, com sete formas -- mas a traducao do JOIN e outra rodada",
            ));
        }

        let onde = if self.aceitar_palavra("WHERE") {
            Some(self.condicao()?)
        } else {
            None
        };

        if self.aceitar_palavra("GROUP") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "GROUP BY geral nao existe embaixo. A tabulacao cruzada e a operacao \
                 pivotar, que e um caso e nao o geral",
            ));
        }

        let ordem = if self.aceitar_palavra("ORDER") {
            self.exigir_palavra("BY")?;
            let coluna = self.identificador("nome de coluna")?;
            let desc = if self.aceitar_palavra("DESC") {
                true
            } else {
                self.aceitar_palavra("ASC");
                false
            };
            if self.aceitar(&Token::Virgula) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "ORDER BY de mais de uma coluna precisa de um indice composto com \
                     essas colunas nessa ordem -- e quem escolhe o indice ainda e quem \
                     chama, porque nao ha planejador",
                ));
            }
            Some(Ordenacao { coluna, desc })
        } else {
            None
        };

        let mut limite = None;
        let mut salto = 0u64;
        if self.aceitar_palavra("LIMIT") {
            limite = Some(self.inteiro("o limite do LIMIT")?);
            if self.aceitar_palavra("OFFSET") {
                salto = self.inteiro("o salto do OFFSET")?;
            }
        } else if self.aceitar_palavra("OFFSET") {
            // OFFSET sem LIMIT e legal em SQL, e o `varrer` sabe pular.
            salto = self.inteiro("o salto do OFFSET")?;
        }

        Ok(Selecao {
            projecao,
            de,
            onde,
            ordem,
            limite,
            salto,
        })
    }

    fn projecao(&mut self) -> Result<Projecao> {
        if self.aceitar(&Token::Asterisco) {
            return Ok(Projecao::Tudo);
        }
        // COUNT(*) -- e so ele. Os outros agregados (SUM, AVG, MIN, MAX) nao
        // tem quem calcule: o `varrer` conta pelo cabecalho, mas nao soma.
        if let Some(nome) = self.espiar().and_then(|s| s.token.palavra_chave()) {
            if ["COUNT", "SUM", "AVG", "MIN", "MAX"].contains(&nome.as_str())
                && self.s.get(self.i + 1).map(|s| &s.token) == Some(&Token::AbreParen)
            {
                let pos = self.posicao_atual();
                if nome != "COUNT" {
                    return Err(lexico::erro(
                        pos,
                        &format!(
                            "{nome}() nao tem quem calcule embaixo. So COUNT(*) passa, \
                             porque a contagem sai do cabecalho da tabela em O(1)"
                        ),
                    ));
                }
                self.i += 2;
                if !self.aceitar(&Token::Asterisco) {
                    return Err(lexico::erro(
                        self.posicao_atual(),
                        "so COUNT(*) -- contar coluna pediria olhar o valor de cada linha",
                    ));
                }
                if !self.aceitar(&Token::FechaParen) {
                    return Err(lexico::erro(self.posicao_atual(), "esperava ) do COUNT(*)"));
                }
                // `COUNT(*) AS quantos` e comum demais para recusar.
                if self.aceitar_palavra("AS") {
                    self.identificador("apelido do COUNT(*)")?;
                }
                return Ok(Projecao::Contagem);
            }
        }

        let mut colunas = Vec::new();
        loop {
            let nome = self.identificador("nome de coluna")?;
            // `t.coluna` -- o qualificador e aceito e descartado, porque so ha
            // uma tabela. Recusa-lo obrigaria a reescrever consulta de cliente
            // que sempre qualifica.
            let nome = if self.aceitar(&Token::Ponto) {
                self.identificador("nome de coluna depois do ponto")?
            } else {
                nome
            };
            let apelido = if self.aceitar_palavra("AS") {
                Some(self.identificador("apelido depois de AS")?)
            } else {
                None
            };
            colunas.push(ColunaPedida { nome, apelido });
            if !self.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok(Projecao::Colunas(colunas))
    }

    /// `tabela`, `schema.tabela` ou `database.schema.tabela`.
    pub(crate) fn alvo(&mut self) -> Result<Alvo> {
        let mut partes = vec![self.identificador("nome de tabela")?];
        while self.aceitar(&Token::Ponto) {
            partes.push(self.identificador("nome depois do ponto")?);
            if partes.len() > 3 {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "o endereco vai ate tres partes: database.schema.tabela",
                ));
            }
        }
        let (database, schema, tabela) = match partes.len() {
            1 => (String::new(), String::new(), partes.remove(0)),
            2 => {
                let t = partes.remove(1);
                (String::new(), partes.remove(0), t)
            }
            _ => {
                let t = partes.remove(2);
                let s = partes.remove(1);
                (partes.remove(0), s, t)
            }
        };
        // O apelido da tabela: com AS ou sem, mas nunca uma clausula solta.
        let apelido = if self.aceitar_palavra("AS") {
            Some(self.identificador("apelido da tabela depois de AS")?)
        } else {
            match self.espiar().map(|s| &s.token) {
                Some(Token::Palavra { texto, citado }) => {
                    let alto = texto.to_uppercase();
                    if *citado || !CLAUSULAS.contains(&alto.as_str()) {
                        let a = texto.clone();
                        self.i += 1;
                        Some(a)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        Ok(Alvo {
            database,
            schema,
            tabela,
            apelido,
        })
    }

    pub(crate) fn condicao(&mut self) -> Result<Condicao> {
        let coluna = self.identificador("nome de coluna no WHERE")?;
        let coluna = if self.aceitar(&Token::Ponto) {
            self.identificador("nome de coluna depois do ponto")?
        } else {
            coluna
        };
        let pos = self.posicao_atual();
        let op = match self.espiar().map(|s| s.token.clone()) {
            Some(Token::Comparador(c)) => {
                self.i += 1;
                c
            }
            _ => {
                // `IS NULL`, `LIKE`, `IN`, `BETWEEN` -- cada um recusado pelo
                // proprio nome, porque cada um falta por um motivo diferente.
                let palavra = self
                    .espiar()
                    .and_then(|s| s.token.palavra_chave())
                    .unwrap_or_default();
                let motivo = match palavra.as_str() {
                    "LIKE" => {
                        "LIKE precisaria varrer comparando texto linha a linha; o \
                               varrer sabe fazer isso (`onde` com `contem`), mas \
                               so dentro da pagina que examina, e um SELECT nao \
                               tem onde dizer isso a quem le a resposta"
                    }
                    "IN" => {
                        "IN e uma lista de buscas; o motor faz cada uma, mas quem \
                             junta os resultados ainda nao existe"
                    }
                    "BETWEEN" => {
                        "BETWEEN e faixa de indice, e a faixa ainda nao esta \
                                  exposta no protocolo"
                    }
                    "IS" => "IS NULL nao tem filtro embaixo: nulo se ve lendo a linha",
                    _ => "esperava um comparador (=, <>, <, <=, >, >=)",
                };
                return Err(lexico::erro(pos, motivo));
            }
        };
        let valor = self.literal()?;
        if self.aceitar_palavra("AND") || self.aceitar_palavra("OR") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, \
                 e nao ha planejador que decida por qual indice comecar",
            ));
        }
        Ok(Condicao { coluna, op, valor })
    }

    pub(crate) fn literal(&mut self) -> Result<Literal> {
        let pos = self.posicao_atual();
        let Some(s) = self.espiar() else {
            return Err(lexico::erro(pos, "esperava um valor, e o comando acabou"));
        };
        let lit = match &s.token {
            Token::Numero(n) => Literal::Numero(n.clone()),
            Token::Texto(t) => Literal::Texto(t.clone()),
            Token::Palavra {
                texto,
                citado: false,
            } => match texto.to_uppercase().as_str() {
                "NULL" => Literal::Nulo,
                "TRUE" => Literal::Bool(true),
                "FALSE" => Literal::Bool(false),
                _ => {
                    return Err(lexico::erro(
                        pos,
                        &format!(
                            "esperava um valor e veio {texto:?}. Comparar coluna com \
                             coluna nao tem quem avalie"
                        ),
                    ))
                }
            },
            outro => {
                return Err(lexico::erro(
                    pos,
                    &format!("esperava um valor e veio {:?}", outro.descrever()),
                ))
            }
        };
        self.i += 1;
        Ok(lit)
    }

    pub(crate) fn inteiro(&mut self, papel: &str) -> Result<u64> {
        let pos = self.posicao_atual();
        match self.espiar().map(|s| s.token.clone()) {
            Some(Token::Numero(n)) if !n.contains('.') => {
                self.i += 1;
                n.parse::<u64>()
                    .map_err(|_| lexico::erro(pos, &format!("{papel} nao cabe: {n}")))
            }
            _ => Err(lexico::erro(
                pos,
                &format!("esperava {papel} como numero inteiro{}", self.mas_veio()),
            )),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn select_estrela() {
        let s = analisar("SELECT * FROM Clientes").unwrap();
        assert_eq!(s.projecao, Projecao::Tudo);
        assert_eq!(s.de.tabela, "Clientes");
        assert!(s.de.schema.is_empty());
        assert!(s.onde.is_none());
    }

    #[test]
    fn colunas_com_apelido() {
        let s = analisar("select id, nome as cliente from Clientes").unwrap();
        let Projecao::Colunas(c) = s.projecao else {
            panic!("esperava lista de colunas")
        };
        assert_eq!(c[0].rotulo(), "id");
        assert_eq!(c[1].rotulo(), "cliente");
        assert_eq!(c[1].nome, "nome");
    }

    #[test]
    fn endereco_com_schema() {
        let s = analisar("SELECT * FROM matriz.estoque").unwrap();
        assert_eq!(s.de.schema, "matriz");
        assert_eq!(s.de.tabela, "estoque");
        assert_eq!(s.de.nome_no_protocolo(), "matriz.estoque");
        assert!(s.de.database.is_empty());
    }

    #[test]
    fn endereco_com_database_e_schema() {
        let s = analisar("SELECT * FROM Comercial.filial.estoque").unwrap();
        assert_eq!(s.de.database, "Comercial");
        assert_eq!(s.de.nome_no_protocolo(), "filial.estoque");
    }

    #[test]
    fn where_ordem_e_limite() {
        let s = analisar("SELECT * FROM t WHERE uf = 'SC' ORDER BY nome DESC LIMIT 10 OFFSET 20")
            .unwrap();
        let o = s.onde.unwrap();
        assert_eq!(o.coluna, "uf");
        assert_eq!(o.op, Comparador::Igual);
        assert_eq!(o.valor, Literal::Texto("SC".into()));
        let ord = s.ordem.unwrap();
        assert_eq!(ord.coluna, "nome");
        assert!(ord.desc);
        assert_eq!(s.limite, Some(10));
        assert_eq!(s.salto, 20);
    }

    #[test]
    fn count_estrela() {
        let s = analisar("SELECT COUNT(*) AS quantos FROM Clientes").unwrap();
        assert_eq!(s.projecao, Projecao::Contagem);
    }

    #[test]
    fn decimal_nao_vira_f64() {
        let s = analisar("SELECT * FROM t WHERE limite = 1500.00").unwrap();
        assert_eq!(
            s.onde.unwrap().valor,
            Literal::Numero("1500.00".into()),
            "o decimal tem de chegar ao motor com os dois zeros"
        );
    }

    #[test]
    fn apelido_de_tabela_sem_as() {
        let s = analisar("SELECT c.nome FROM Clientes c WHERE c.uf = 'SC'").unwrap();
        assert_eq!(s.de.apelido.as_deref(), Some("c"));
        assert_eq!(s.onde.unwrap().coluna, "uf");
    }

    /// A armadilha do apelido sem AS: `FROM t WHERE ...` nao pode ler o WHERE
    /// como apelido da tabela e depois reclamar que falta o WHERE.
    #[test]
    fn clausula_nao_vira_apelido() {
        let s = analisar("SELECT * FROM Clientes WHERE id = 1").unwrap();
        assert_eq!(s.de.apelido, None);
        assert!(s.onde.is_some());
        let s = analisar("SELECT * FROM Clientes LIMIT 5").unwrap();
        assert_eq!(s.de.apelido, None);
        assert_eq!(s.limite, Some(5));
    }

    #[test]
    fn bulkinsert_nao_e_nome_de_tabela() {
        // A regra do docs/SQL.md: e palavra do motor, e o parser tem de dizer
        // isso em vez de tentar abrir uma tabela chamada BULKINSERT.
        let e = analisar("SELECT * FROM BULKINSERT")
            .unwrap_err()
            .to_string();
        assert!(e.contains("BULKINSERT"), "{e}");
        assert!(e.to_lowercase().contains("carga"), "{e}");
        // E entre aspas ela volta a ser um nome, para quem ja tem a tabela.
        assert!(analisar("SELECT * FROM \"BULKINSERT\"").is_ok());
    }

    #[test]
    fn bulkinsert_como_comando_diz_que_e_de_sessao() {
        let e = analisar("BULKINSERT(true)").unwrap_err().to_string();
        assert!(e.contains("SESSAO"), "{e}");
    }

    /// O `analisar` e o tradutor de CONSULTA, e transacao nao e consulta: ela
    /// e reconhecida antes, pelo `crate::transacao::comando`. Quem cai aqui
    /// escreveu uma forma que nenhum dos dois entende, e a recusa diz as
    /// formas que existem em vez de dizer que a transacao nao existe.
    #[test]
    fn transacao_nao_e_consulta_e_a_recusa_lista_as_formas() {
        for c in ["BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT p1"] {
            let e = analisar(c).unwrap_err().to_string();
            assert!(e.contains("SESSAO"), "{c}: {e}");
            assert!(e.contains("START TRANSACTION"), "{c}: {e}");
        }
        // E o caminho de verdade continua reconhecendo os quatro.
        for c in ["BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT p1"] {
            assert!(
                crate::transacao::comando(c).unwrap().is_some(),
                "{c} tem de ser reconhecido como comando de transacao"
            );
        }
    }

    #[test]
    fn o_que_falta_recusa_pelo_nome() {
        for (sql, pedaco) in [
            ("SELECT * FROM t WHERE a = 1 AND b = 2", "UMA comparacao"),
            ("SELECT * FROM t WHERE nome LIKE 'a%'", "LIKE"),
            ("SELECT * FROM t WHERE id IN (1,2)", "IN"),
            ("SELECT * FROM t WHERE id BETWEEN 1 AND 2", "BETWEEN"),
            ("SELECT * FROM t WHERE id IS NULL", "IS NULL"),
            ("SELECT SUM(x) FROM t", "SUM()"),
            ("SELECT DISTINCT a FROM t", "DISTINCT"),
            ("SELECT * FROM t GROUP BY a", "GROUP BY"),
            ("SELECT * FROM a JOIN b", "junção"),
            // INSERT, UPDATE e DELETE sairam daqui no dia em que passaram a
            // existir: as recusas DELES moram em `dml.rs`, uma por falta.
        ] {
            let e = analisar(sql).unwrap_err().to_string();
            assert!(e.contains(pedaco), "{sql} -> {e}");
        }
    }

    #[test]
    fn erro_diz_onde() {
        let e = analisar("SELECT * FRON t").unwrap_err().to_string();
        assert!(e.contains("esperava FROM"), "{e}");
        assert!(e.contains("coluna 10"), "{e}");
    }

    #[test]
    fn sobra_depois_do_comando() {
        let e = analisar("SELECT * FROM t; SELECT * FROM t2")
            .unwrap_err()
            .to_string();
        assert!(e.contains("um comando por vez"), "{e}");
    }

    #[test]
    fn comando_empilhado_acha_o_segundo_comando() {
        for sql in [
            "SELECT * FROM clientes; DROP TABLE clientes; --",
            "SELECT * FROM clientes; DELETE FROM clientes",
            "SELECT * FROM clientes ;;; DROP TABLE clientes",
            "SELECT * FROM clientes WHERE nome = 'x'; EXEC xp_cmdshell('dir')",
        ] {
            assert!(comando_empilhado(sql), "devia acusar: {sql}");
        }
    }

    /// A metade que importa: nada disto e injecao, e acusar aqui bloquearia
    /// quem escreve SQL legitimo.
    #[test]
    fn comando_empilhado_nao_acusa_o_legitimo() {
        for sql in [
            "SELECT * FROM clientes",
            "SELECT * FROM clientes;",
            "SELECT * FROM clientes ; ",
            "SELECT * FROM clientes WHERE nome = 'Alves' -- e o resto",
            "SELECT /* comentario */ * FROM clientes",
            // O veneno como DADO: a aspa fecha, entao e um simbolo Texto so.
            "SELECT * FROM clientes WHERE nome = '; DROP TABLE clientes; --'",
            // Erro de sintaxe nao e injecao -- e o lexico nem le este.
            "SELECT * FROM clientes WHERE nome = '' OR '1'='1",
            // O corpo de um procedimento e cheio de `;` legitimo. Ele nao e
            // desta gramatica, e nao pode ser acusado nem quando falha.
            "CREATE PROCEDURE p(OUT n INT) BEGIN DECLARE i INT DEFAULT 0; \
             SET i = i + 1; SET n = i; END",
            "CREATE TRIGGER t BEFORE INSERT ON clientes FOR EACH ROW \
             BEGIN SET NEW.nome = 'x'; END",
            "CALL somar_ate(100)",
            "BEGIN",
            "COMMIT",
            "SHOW PROCEDURES",
            // Ponto-e-virgula sozinho no fim nao e um segundo comando.
            "SELECT * FROM clientes;;",
        ] {
            assert!(!comando_empilhado(sql), "nao devia acusar: {sql}");
        }
    }

    #[test]
    fn comando_vazio() {
        assert!(analisar("   ").is_err());
        assert!(analisar("-- so um comentario").is_err());
    }
}

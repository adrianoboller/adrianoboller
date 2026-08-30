//! Os comandos de transacao do SQL, traduzidos para as operacoes do
//! protocolo.
//!
//! ```text
//! BEGIN TRANSACTION
//!   SCOPE (clientes, pedidos, pediditens, estoque)
//!   SCOPE MODE STRICT
//!   TIMEOUT 5s
//!   LOCK TIMEOUT 500ms
//!   STATEMENT TIMEOUT 2s
//!   LOCK MODE AUTO;
//!
//! COMMIT;  ROLLBACK;
//! SAVEPOINT p1;  ROLLBACK TO SAVEPOINT p1;  RELEASE SAVEPOINT p1;
//! ```
//!
//! # Por que parametros NOMEADOS, e nao posicionais
//!
//! A forma posicional -- `Transaction(clientes, pedidos, estoque, 5s)` -- nao
//! estende: entrou o segundo prazo, nao ha onde ele caiba sem quebrar quem ja
//! escreveu, e nao ha como dizer QUAL dos tres prazos e aquele. E ela mistura
//! duas coisas de naturezas diferentes -- tabela e duracao -- na mesma lista,
//! onde a quarta posicao so nao e uma tabela porque termina em `s`. Nomeado,
//! acrescentar um campo e acrescentar um campo, e a leitura diz o que cada
//! numero e.
//!
//! **As clausulas nao tem ordem**: quem escreve `TIMEOUT` antes de `SCOPE` nao
//! esta errado. Ordem obrigatoria e uma regra que so existe para facilitar o
//! analisador, e o preco dela e pago por quem digita.
//!
//! # Por que eles nao passam pelo `sintaxe.rs`
//!
//! Porque nao sao consulta: nao tem `FROM`, nao produzem linha e nao dependem
//! de esquema nenhum. Sao comandos de SESSAO, como o `BULKINSERT` -- e a
//! transacao pertence a CONEXAO, nao ao texto do comando.
//!
//! # A armadilha do `BEGIN`, e como ela e evitada
//!
//! `BEGIN` tambem abre o corpo de um `CREATE PROCEDURE p() BEGIN ... END`. Os
//! dois nao se confundem porque quem chama este modulo o chama DEPOIS do
//! [`crate::rotina::comando`], que ja consumiu todo `CREATE`. Um `BEGIN`
//! solto, chegando aqui, so pode ser abertura de transacao -- e ha teste para
//! os dois lados.

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::lexico::{self, Simbolo, Token};

/// Um comando de transacao ja reconhecido.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comando {
    pub op: String,
    /// O nome do `SAVEPOINT`, quando ha um.
    pub nome: Option<String>,
    /// As tabelas do `SCOPE`, como foram escritas.
    pub escopo: Vec<String>,
    pub escopo_modo: Option<String>,
    pub lock_mode: Option<String>,
    /// As tres duracoes, no texto original (`"5s"`, `"500ms"`).
    pub timeout: Option<String>,
    pub lock_timeout: Option<String>,
    pub statement_timeout: Option<String>,
}

impl Comando {
    /// O pedido pronto para o despachar, com os nomes do protocolo.
    pub fn pedido(&self) -> Json {
        let mut pares: Vec<(String, Json)> = Vec::new();
        if let Some(n) = &self.nome {
            pares.push(("nome".into(), Json::texto_de(n)));
        }
        if !self.escopo.is_empty() {
            pares.push((
                "scope".into(),
                Json::Lista(self.escopo.iter().map(Json::texto_de).collect()),
            ));
        }
        for (campo, valor) in [
            ("scope_mode", &self.escopo_modo),
            ("lock_mode", &self.lock_mode),
            ("timeout", &self.timeout),
            ("lock_timeout", &self.lock_timeout),
            ("statement_timeout", &self.statement_timeout),
        ] {
            if let Some(v) = valor {
                pares.push((campo.into(), Json::texto_de(v)));
            }
        }
        Json::Objeto(pares)
    }
}

/// Reconhece um comando de transacao. `None` quando o texto e outra coisa.
pub fn comando(texto: &str) -> Result<Option<Comando>> {
    // Texto que nem passa pelo lexico NAO e assunto deste modulo, e o erro
    // dele nao pode sair daqui: o `SET @x = 1` de um corpo de procedimento
    // tropeca no `@`, e quem sabe explicar isso e o analisador que cuida do
    // corpo -- nao o detector de transacao.
    // O lexico RECUSA numero colado em identificador -- `500ms` -- e a recusa
    // dele esta certa: `SELECT 5x` e engano de digitacao em toda consulta que
    // existe. So que `TIMEOUT 500ms` nao e engano, e obrigar `500 ms` seria
    // fazer quem digita pagar por uma regra do analisador.
    //
    // Entao a unidade e separada AQUI, e so aqui: um comando de transacao e
    // reconhecido pela primeira palavra antes de qualquer analise, e a
    // separacao vale so para os quatro sufixos de tempo. `5x` continua sendo o
    // erro que sempre foi, e o resto da linguagem nao muda um caractere.
    let texto = if primeira_palavra_e_de_transacao(texto) {
        separar_unidade_de_tempo(texto)
    } else {
        texto.to_string()
    };
    let Ok(simbolos) = lexico::analisar(&texto) else {
        return Ok(None);
    };
    let mut p = Passo { s: &simbolos, i: 0 };
    let pos = simbolos.first().map(|x| x.posicao).unwrap_or(0);

    let mut c = Comando::default();
    match p.palavra().as_str() {
        // `BEGIN` sozinho, `BEGIN TRANSACTION` e `BEGIN WORK` sao a mesma
        // coisa em todo banco que os aceita -- e quem digita um espera que o
        // outro tambem valha.
        "BEGIN" => {
            p.i += 1;
            if matches!(p.palavra().as_str(), "TRANSACTION" | "WORK") {
                p.i += 1;
            }
            c.op = "begin".into();
            clausulas(&mut p, &mut c, pos)?;
        }
        "START" => {
            p.i += 1;
            if p.palavra() != "TRANSACTION" {
                return Err(lexico::erro(pos, "esperava START TRANSACTION"));
            }
            p.i += 1;
            c.op = "begin".into();
            clausulas(&mut p, &mut c, pos)?;
        }
        "COMMIT" => {
            p.i += 1;
            if matches!(p.palavra().as_str(), "WORK" | "TRANSACTION") {
                p.i += 1;
            }
            c.op = "commit".into();
        }
        "ROLLBACK" => {
            p.i += 1;
            match p.palavra().as_str() {
                // `ROLLBACK TO SAVEPOINT n` e `ROLLBACK TO n` -- a palavra do
                // meio e facultativa no padrao, e as duas aparecem por ai.
                "TO" => {
                    p.i += 1;
                    if p.palavra() == "SAVEPOINT" {
                        p.i += 1;
                    }
                    c.op = "rollback_para".into();
                    c.nome = Some(p.exigir_nome(pos, "ROLLBACK TO")?);
                }
                _ => {
                    if matches!(p.palavra().as_str(), "WORK" | "TRANSACTION") {
                        p.i += 1;
                    }
                    c.op = "rollback".into();
                }
            }
        }
        "SAVEPOINT" => {
            p.i += 1;
            c.op = "savepoint".into();
            c.nome = Some(p.exigir_nome(pos, "SAVEPOINT")?);
        }
        "RELEASE" => {
            p.i += 1;
            if p.palavra() == "SAVEPOINT" {
                p.i += 1;
            }
            c.op = "release_savepoint".into();
            c.nome = Some(p.exigir_nome(pos, "RELEASE SAVEPOINT")?);
        }
        _ => return Ok(None),
    }
    p.fim(pos, &c.op)?;
    Ok(Some(c))
}

/// O texto comeca por um verbo de transacao?
///
/// Le so a primeira palavra, sem lexico: e o portao que decide se vale a pena
/// mexer no texto, e ele vem antes do trabalho.
fn primeira_palavra_e_de_transacao(texto: &str) -> bool {
    let primeira: String = texto
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    matches!(
        primeira.to_ascii_uppercase().as_str(),
        "BEGIN" | "START" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE"
    )
}

/// `500ms` vira `500 ms`; `5s` vira `5 s`. Nada mais muda.
fn separar_unidade_de_tempo(texto: &str) -> String {
    let b: Vec<char> = texto.chars().collect();
    let mut saida = String::with_capacity(texto.len() + 4);
    let mut i = 0;
    while i < b.len() {
        saida.push(b[i]);
        // So depois de um digito, e so quando o que vem colado e EXATAMENTE
        // uma das quatro unidades. `5x` continua colado, e continua sendo o
        // erro que o lexico ja acusava.
        if b[i].is_ascii_digit() {
            let mut fim = i + 1;
            while fim < b.len() && b[fim].is_ascii_alphabetic() {
                fim += 1;
            }
            let unidade: String = b[i + 1..fim].iter().collect();
            if matches!(
                unidade.to_ascii_lowercase().as_str(),
                "ms" | "s" | "m" | "h"
            ) {
                saida.push(' ');
            }
        }
        i += 1;
    }
    saida
}

/// As clausulas da abertura, em qualquer ordem.
fn clausulas(p: &mut Passo<'_>, c: &mut Comando, pos: usize) -> Result<()> {
    loop {
        match p.palavra().as_str() {
            "SCOPE" => {
                p.i += 1;
                if p.palavra() == "MODE" {
                    p.i += 1;
                    c.escopo_modo = Some(p.exigir_nome(pos, "SCOPE MODE")?);
                } else {
                    c.escopo = p.lista_de_tabelas(pos)?;
                }
            }
            "TIMEOUT" => {
                p.i += 1;
                c.timeout = Some(p.exigir_duracao(pos, "TIMEOUT")?);
            }
            "LOCK" => {
                p.i += 1;
                match p.palavra().as_str() {
                    "TIMEOUT" => {
                        p.i += 1;
                        c.lock_timeout = Some(p.exigir_duracao(pos, "LOCK TIMEOUT")?);
                    }
                    "MODE" => {
                        p.i += 1;
                        c.lock_mode = Some(p.exigir_nome(pos, "LOCK MODE")?);
                    }
                    outro => {
                        return Err(lexico::erro(
                            pos,
                            &format!("depois de LOCK esperava TIMEOUT ou MODE, veio {outro:?}"),
                        ))
                    }
                }
            }
            "STATEMENT" => {
                p.i += 1;
                if p.palavra() != "TIMEOUT" {
                    return Err(lexico::erro(pos, "esperava STATEMENT TIMEOUT"));
                }
                p.i += 1;
                c.statement_timeout = Some(p.exigir_duracao(pos, "STATEMENT TIMEOUT")?);
            }
            _ => return Ok(()),
        }
    }
}

struct Passo<'a> {
    s: &'a [Simbolo],
    i: usize,
}

impl Passo<'_> {
    fn palavra(&self) -> String {
        self.s
            .get(self.i)
            .and_then(|x| x.token.palavra_chave())
            .unwrap_or_default()
    }

    fn nome(&self) -> Option<String> {
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Palavra { texto, .. }) => Some(texto.clone()),
            _ => None,
        }
    }

    fn exigir_nome(&mut self, pos: usize, depois_de: &str) -> Result<String> {
        let n = self
            .nome()
            .ok_or_else(|| lexico::erro(pos, &format!("esperava um nome depois de {depois_de}")))?;
        self.i += 1;
        Ok(n)
    }

    /// `(a, b, schema.c)` -- e tambem `a, b` sem parenteses, porque quem
    /// escreve a mao esquece o parentese e o comando continua sem ambiguidade.
    fn lista_de_tabelas(&mut self, pos: usize) -> Result<Vec<String>> {
        let com_paren = matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::AbreParen));
        if com_paren {
            self.i += 1;
        }
        let mut nomes = Vec::new();
        loop {
            let mut nome = self.exigir_nome(pos, "SCOPE")?;
            // `schema.tabela` chega em tres simbolos, e o nome qualificado e o
            // que o protocolo usa no campo `tabela`.
            while matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::Ponto)) {
                self.i += 1;
                nome.push('.');
                nome.push_str(&self.exigir_nome(pos, "SCOPE")?);
            }
            nomes.push(nome);
            match self.s.get(self.i).map(|x| &x.token) {
                Some(Token::Virgula) => self.i += 1,
                _ => break,
            }
        }
        if com_paren {
            if !matches!(
                self.s.get(self.i).map(|x| &x.token),
                Some(Token::FechaParen)
            ) {
                return Err(lexico::erro(pos, "faltou fechar o parentese do SCOPE"));
            }
            self.i += 1;
        }
        Ok(nomes)
    }

    /// `5s`, `500ms`, `2m` -- o numero e a unidade chegam como dois simbolos.
    fn exigir_duracao(&mut self, pos: usize, depois_de: &str) -> Result<String> {
        let numero = match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Numero(n)) => n.clone(),
            // Entre aspas tambem serve, e e o que um cliente que monta o
            // comando por concatenacao costuma produzir.
            Some(Token::Texto(t)) => {
                let t = t.clone();
                self.i += 1;
                return Ok(t);
            }
            _ => {
                return Err(lexico::erro(
                    pos,
                    &format!("esperava uma duracao depois de {depois_de}, como 5s ou 500ms"),
                ))
            }
        };
        self.i += 1;
        // A unidade e o simbolo seguinte, e ela e facultativa: sem unidade,
        // vale milissegundo, que e a unidade de todo prazo deste servidor.
        let unidade = match self.nome() {
            Some(u) if matches!(u.to_ascii_lowercase().as_str(), "ms" | "s" | "m" | "h") => {
                self.i += 1;
                u
            }
            _ => String::new(),
        };
        Ok(format!("{numero}{unidade}"))
    }

    /// O comando acabou? Sobra de texto e erro, e nao silencio: quem escreveu
    /// `COMMIT AND CHAIN` precisa saber que o `AND CHAIN` nao existe aqui, em
    /// vez de receber um `COMMIT` simples e achar que encadeou.
    fn fim(&self, pos: usize, op: &str) -> Result<()> {
        let sobra: Vec<&Simbolo> = self.s[self.i.min(self.s.len())..]
            .iter()
            .filter(|x| !matches!(x.token, Token::PontoEVirgula))
            .collect();
        if sobra.is_empty() {
            return Ok(());
        }
        Err(lexico::erro(
            pos,
            &format!(
                "sobrou {:?} depois do comando de {op}",
                sobra[0].token.descrever()
            ),
        ))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ok(sql: &str) -> Comando {
        comando(sql)
            .unwrap_or_else(|e| panic!("{sql}: {e}"))
            .unwrap_or_else(|| panic!("{sql}: nao reconheceu"))
    }

    #[test]
    fn os_tres_sinonimos_de_abertura_dao_a_mesma_operacao() {
        for sql in [
            "BEGIN",
            "BEGIN TRANSACTION",
            "START TRANSACTION",
            "begin work",
        ] {
            assert_eq!(ok(sql).op, "begin", "{sql}");
        }
    }

    #[test]
    fn confirmar_e_reverter() {
        assert_eq!(ok("COMMIT").op, "commit");
        assert_eq!(ok("commit work").op, "commit");
        assert_eq!(ok("ROLLBACK").op, "rollback");
        assert_eq!(ok("rollback work").op, "rollback");
    }

    #[test]
    fn o_savepoint_com_e_sem_a_palavra_do_meio() {
        let c = ok("SAVEPOINT antes_do_lote");
        assert_eq!(c.op, "savepoint");
        assert_eq!(c.nome.as_deref(), Some("antes_do_lote"));

        let c = ok("ROLLBACK TO SAVEPOINT antes_do_lote");
        assert_eq!(c.op, "rollback_para");
        assert_eq!(c.nome.as_deref(), Some("antes_do_lote"));

        // Sem a palavra `SAVEPOINT` no meio, que e como o padrao permite.
        let c = ok("ROLLBACK TO antes_do_lote");
        assert_eq!(c.op, "rollback_para");
        assert_eq!(c.nome.as_deref(), Some("antes_do_lote"));

        let c = ok("RELEASE SAVEPOINT antes_do_lote");
        assert_eq!(c.op, "release_savepoint");
        assert_eq!(c.nome.as_deref(), Some("antes_do_lote"));
    }

    /// O comando inteiro do desenho, do jeito que ele foi escrito.
    #[test]
    fn a_abertura_declarada_inteira() {
        let c = ok("BEGIN TRANSACTION \
             SCOPE (clientes, pedidos, pediditens, estoque) \
             TIMEOUT 5s \
             LOCK TIMEOUT 500ms \
             LOCK MODE AUTO");
        assert_eq!(c.op, "begin");
        assert_eq!(
            c.escopo,
            vec!["clientes", "pedidos", "pediditens", "estoque"]
        );
        assert_eq!(c.timeout.as_deref(), Some("5s"));
        assert_eq!(c.lock_timeout.as_deref(), Some("500ms"));
        assert_eq!(c.lock_mode.as_deref(), Some("AUTO"));

        // E o pedido que sai dele.
        let ped = c.pedido();
        assert_eq!(ped.texto_ou("timeout", ""), "5s");
        assert_eq!(ped.texto_ou("lock_timeout", ""), "500ms");
        assert_eq!(ped.texto_ou("lock_mode", ""), "AUTO");
        assert_eq!(
            ped.campo("scope").and_then(Json::lista).map(<[Json]>::len),
            Some(4)
        );
    }

    /// **As clausulas nao tem ordem.** Uma ordem obrigatoria so facilitaria o
    /// analisador, e o preco seria pago por quem digita.
    #[test]
    fn a_ordem_das_clausulas_nao_importa() {
        let a = ok("BEGIN TRANSACTION SCOPE (a, b) TIMEOUT 5s LOCK MODE ROW");
        let b = ok("BEGIN TRANSACTION LOCK MODE ROW TIMEOUT 5s SCOPE (a, b)");
        assert_eq!(a, b);
    }

    #[test]
    fn o_escopo_aceita_tabela_qualificada_e_sem_parentese() {
        let c = ok("BEGIN TRANSACTION SCOPE (filial.pedidos, estoque)");
        assert_eq!(c.escopo, vec!["filial.pedidos", "estoque"]);
        let c = ok("BEGIN TRANSACTION SCOPE clientes, pedidos");
        assert_eq!(c.escopo, vec!["clientes", "pedidos"]);
    }

    #[test]
    fn os_dois_modos_e_o_terceiro_prazo() {
        let c = ok("BEGIN TRANSACTION SCOPE MODE STRICT STATEMENT TIMEOUT 2s");
        assert_eq!(c.escopo_modo.as_deref(), Some("STRICT"));
        assert_eq!(c.statement_timeout.as_deref(), Some("2s"));
    }

    /// `ms` tem de ser reconhecido ANTES de `s`: quem le "500ms" como 500
    /// segundos erra por mil vezes, e erra calado.
    #[test]
    fn a_unidade_vem_inteira_e_o_ms_nao_vira_s() {
        assert_eq!(ok("BEGIN TIMEOUT 500ms").timeout.as_deref(), Some("500ms"));
        assert_eq!(ok("BEGIN TIMEOUT 5s").timeout.as_deref(), Some("5s"));
        // Sem unidade vale milissegundo, e o texto sai sem sufixo.
        assert_eq!(ok("BEGIN TIMEOUT 1500").timeout.as_deref(), Some("1500"));
    }

    /// O que NAO e transacao continua caindo fora daqui -- e e o que garante
    /// que este modulo nao rouba comando de ninguem.
    #[test]
    fn o_resto_do_sql_passa_direto() {
        for sql in [
            "SELECT * FROM t",
            "CREATE PROCEDURE p() BEGIN SET @x = 1; END",
            "DROP TRIGGER t",
            "SHOW TRIGGERS",
            "CALL p()",
            "INSERT INTO t VALUES (1)",
        ] {
            assert!(comando(sql).unwrap().is_none(), "{sql}");
        }
    }

    /// Sobra depois do comando e ERRO. `COMMIT AND CHAIN` nao existe aqui, e
    /// aceitar calado devolveria um `COMMIT` simples a quem pediu encadeamento.
    #[test]
    fn sobra_depois_do_comando_recusa() {
        let e = comando("COMMIT AND CHAIN").unwrap_err().to_string();
        assert!(e.contains("sobrou"), "{e}");
        // E o ponto e virgula do fim nao e sobra.
        assert!(comando("COMMIT;").unwrap().is_some());
    }

    #[test]
    fn o_pedido_leva_o_nome_quando_ha_um() {
        assert_eq!(ok("SAVEPOINT p1").pedido().texto_ou("nome", ""), "p1");
        assert_eq!(ok("COMMIT").pedido().texto_ou("nome", ""), "");
    }
}

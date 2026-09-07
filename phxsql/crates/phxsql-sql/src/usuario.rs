//! `CREATE USER`, `ALTER USER` e `DROP USER` -- os tres traduzidos para as
//! operacoes de cadastro do protocolo.
//!
//! ```text
//! CREATE USER carlos PASSWORD 'a-senha-do-carlos';
//! ALTER  USER carlos PASSWORD 'a-senha-nova';
//! DROP   USER carlos;
//! ```
//!
//! # Por que sao comandos de SESSAO, e nao consulta
//!
//! Pelo mesmo motivo dos de transacao: nao tem `FROM`, nao produzem linha e
//! nao dependem de esquema nenhum. Sao ordens ao SERVIDOR, e viram um pedido
//! de protocolo -- `usuario_criar`, `usuario_alterar`, `usuario_excluir` --
//! que passa pelo portao de sempre, com o poder de quem chamou. **O SQL nao
//! abre porta nenhuma**: quem nao pode chamar `usuario_criar` pelo JSON
//! tambem nao pode por aqui.
//!
//! # Por que este modulo e consultado ANTES do `rotina`
//!
//! Porque `rotina::comando` reclama todo `CREATE` e todo `DROP` para si --
//! gatilho e procedimento. `CREATE USER` chegando la vira erro de sintaxe
//! pedindo `TRIGGER` ou `PROCEDURE`. A ordem resolve, e nao ha ambiguidade
//! nenhuma para desfazer: a segunda palavra decide, e `USER` nao e nome de
//! rotina nenhuma.
//!
//! # O que ele NAO reclama
//!
//! `ALTER` de qualquer outra coisa. O portao le duas palavras e devolve
//! `None` para tudo o que nao seja `<verbo> USER` -- entao `ALTER SERVER`,
//! `ALTER TABLE` e o que mais vier continuam chegando inteiros a quem os
//! atende.
//!
//! # A senha no texto do comando
//!
//! Ela viaja no SQL, e isso e uma escolha de quem digita: o `usuario_criar`
//! em JSON e o caminho em que o Profiler a tapa por NOME de campo. Num texto
//! SQL nao ha campo -- ha uma frase --, entao o pedido inteiro e o que o
//! Profiler enxerga. Por isso o campo `sql` da resposta volta **redigido**:
//! ver [`sem_a_senha`].

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::lexico::{self, Token};

/// Um comando de cadastro ja reconhecido.
#[derive(Debug, Clone, PartialEq)]
pub struct Comando {
    /// A operacao do protocolo: `usuario_criar`, `usuario_alterar` ou
    /// `usuario_excluir`.
    pub op: String,
    pub login: String,
    /// A senha, quando o comando a traz. Nunca vai para log nem para resposta.
    pub senha: Option<String>,
}

impl Comando {
    /// O pedido pronto para o despachar, com os nomes do protocolo.
    pub fn pedido(&self) -> Json {
        let mut pares = vec![("login".to_string(), Json::texto_de(&self.login))];
        if let Some(s) = &self.senha {
            pares.push(("senha".to_string(), Json::texto_de(s)));
        }
        Json::Objeto(pares)
    }
}

/// Reconhece um comando de cadastro. `None` quando o texto e outra coisa.
pub fn comando(texto: &str) -> Result<Option<Comando>> {
    // O portao vem ANTES do trabalho: duas palavras lidas a mao decidem se
    // vale a pena chamar o lexico. Sem ele, TODO texto que passa pela op
    // `sql` -- inclusive um `SELECT` de mil colunas -- pagaria uma analise
    // lexica a mais so para este modulo dizer "nao e comigo".
    if !e_de_cadastro(texto) {
        return Ok(None);
    }
    let simbolos = lexico::analisar(texto)?;
    let pos = simbolos.first().map(|x| x.posicao).unwrap_or(0);
    let mut p = Passo { s: &simbolos, i: 0 };

    let verbo = p.palavra();
    p.i += 1; // o verbo
    p.i += 1; // USER, ja conferido pelo portao

    let login = p.exigir_nome(pos, &format!("{verbo} USER"))?;
    let mut c = Comando {
        op: String::new(),
        login,
        senha: None,
    };
    match verbo.as_str() {
        "CREATE" => {
            c.op = "usuario_criar".into();
            c.senha = Some(p.exigir_senha(pos, "CREATE USER")?);
        }
        "ALTER" => {
            c.op = "usuario_alterar".into();
            c.senha = Some(p.exigir_senha(pos, "ALTER USER")?);
        }
        // `DROP USER` nao tem clausula nenhuma, e a ausencia e deliberada: o
        // `CASCADE`/`RESTRICT` que outros bancos aceitam ali fala dos OBJETOS
        // do usuario, e aqui usuario nao possui objeto -- tabela e do banco,
        // nao de quem a criou.
        _ => c.op = "usuario_excluir".into(),
    }
    p.fim(pos, &c.op)?;
    Ok(Some(c))
}

/// O texto e `<CREATE|ALTER|DROP> USER ...`?
///
/// Le duas palavras sem lexico -- e o portao que decide se vale a pena mexer
/// no texto, e ele vem antes do trabalho.
///
/// Publica porque o Profiler tambem precisa da pergunta: e ela que decide se
/// vale a pena analisar o campo `texto` de um pedido `sql` atras de senha.
pub fn e_de_cadastro(texto: &str) -> bool {
    let mut palavras = texto.split_whitespace();
    let primeira = palavras.next().unwrap_or("").to_ascii_uppercase();
    if !matches!(primeira.as_str(), "CREATE" | "ALTER" | "DROP") {
        return false;
    }
    // O `trim_end_matches(';')` cobre o `DROP USER carlos;` de uma linha so:
    // sem ele a segunda palavra seria `USER` inteira, mas um `DROP USER;`
    // torto passaria adiante. Aparar aqui nao muda o que e reconhecido.
    palavras
        .next()
        .unwrap_or("")
        .trim_end_matches(';')
        .eq_ignore_ascii_case("USER")
}

/// O comando SQL sem a senha dentro -- o que pode ir para log e resposta.
///
/// # Por que ANALISANDO, e nao recortando
///
/// E a regra da casa, e ela vale aqui inteira: recortar dependeria de a senha
/// estar escrita de um jeito -- aspas simples, sem `''` dentro, coladas na
/// palavra `PASSWORD`. Analisar acha o literal de texto onde quer que ele
/// esteja, e reserializa o resto. O que nao se analisa nao vira texto: vira o
/// tamanho em bytes, pelo mesmo motivo que o Profiler ja adota.
pub fn sem_a_senha(texto: &str) -> String {
    let Ok(simbolos) = lexico::analisar(texto) else {
        return format!("<comando invalido, {} bytes>", texto.trim().len());
    };
    let mut saida = String::with_capacity(texto.len());
    for s in &simbolos {
        if !saida.is_empty() && !matches!(s.token, Token::PontoEVirgula) {
            saida.push(' ');
        }
        match &s.token {
            Token::Texto(_) => saida.push_str("'***'"),
            outro => saida.push_str(&outro.descrever()),
        }
    }
    saida
}

struct Passo<'a> {
    s: &'a [lexico::Simbolo],
    i: usize,
}

impl Passo<'_> {
    fn palavra(&self) -> String {
        self.s
            .get(self.i)
            .and_then(|x| x.token.palavra_chave())
            .unwrap_or_default()
    }

    /// Um nome de usuario: palavra ou identificador entre aspas duplas.
    fn exigir_nome(&mut self, pos: usize, onde: &str) -> Result<String> {
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Palavra { texto, .. }) => {
                self.i += 1;
                Ok(texto.clone())
            }
            Some(outro) => Err(lexico::erro(
                pos,
                &format!(
                    "depois de {onde} esperava o login, veio {}",
                    outro.descrever()
                ),
            )),
            None => Err(lexico::erro(
                pos,
                &format!("{onde} sem o login: o comando acabou"),
            )),
        }
    }

    /// `PASSWORD 'texto'`.
    ///
    /// A senha so entra entre aspas SIMPLES, que e como todo dialeto a
    /// escreve. Aceitar uma palavra solta faria `PASSWORD segredo` gravar a
    /// senha `segredo` -- e faria `PASSWORD` sem nada nenhum virar o login do
    /// proximo token, calado.
    fn exigir_senha(&mut self, pos: usize, onde: &str) -> Result<String> {
        if self.palavra() != "PASSWORD" {
            return Err(lexico::erro(
                pos,
                &format!("{onde} exige PASSWORD 'a senha'"),
            ));
        }
        self.i += 1;
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Texto(t)) => {
                self.i += 1;
                Ok(t.clone())
            }
            // A mensagem nao repete o que veio: o que veio ali e a senha, ou
            // quase ela. Um erro de sintaxe que devolve a senha do sujeito
            // ecoa no log de quem quer que esteja lendo.
            _ => Err(lexico::erro(
                pos,
                &format!("{onde}: a senha vai entre aspas simples -- PASSWORD 'a senha'"),
            )),
        }
    }

    /// Acabou mesmo? Sobra depois do comando e engano, e engano calado.
    fn fim(&mut self, pos: usize, op: &str) -> Result<()> {
        if matches!(
            self.s.get(self.i).map(|x| &x.token),
            Some(Token::PontoEVirgula)
        ) {
            self.i += 1;
        }
        match self.s.get(self.i) {
            None => Ok(()),
            Some(x) => Err(lexico::erro(
                pos,
                &format!("{op}: sobrou {:?} depois do comando", x.token.descrever()),
            )),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(texto: &str) -> Comando {
        comando(texto).unwrap().expect("devia reconhecer")
    }

    #[test]
    fn create_user_vira_usuario_criar() {
        let x = c("CREATE USER carlos PASSWORD 'a-senha'");
        assert_eq!(x.op, "usuario_criar");
        assert_eq!(x.login, "carlos");
        assert_eq!(x.senha.as_deref(), Some("a-senha"));
        assert_eq!(
            x.pedido().escrever(),
            r#"{"login":"carlos","senha":"a-senha"}"#
        );
    }

    #[test]
    fn alter_user_vira_usuario_alterar() {
        let x = c("alter user Carlos password 'outra' ;");
        assert_eq!(x.op, "usuario_alterar");
        assert_eq!(x.login, "Carlos", "o caso do login e preservado");
        assert_eq!(x.senha.as_deref(), Some("outra"));
    }

    #[test]
    fn drop_user_vira_usuario_excluir() {
        let x = c("DROP USER carlos;");
        assert_eq!(x.op, "usuario_excluir");
        assert_eq!(x.login, "carlos");
        assert!(x.senha.is_none());
        assert_eq!(x.pedido().escrever(), r#"{"login":"carlos"}"#);
    }

    /// **A guarda do vizinho.** Outra frente atende `ALTER SERVER SET ...` e
    /// `SHOW ... SETTINGS`; este modulo nao pode reclamar nada disso para si.
    #[test]
    fn o_que_nao_e_user_passa_direto() {
        for texto in [
            "ALTER SERVER SET max_linhas = 500",
            "SHOW SERVER SETTINGS",
            "CREATE TRIGGER t AFTER INSERT ON x BEGIN END",
            "DROP TRIGGER t",
            "CREATE PROCEDURE p() BEGIN END",
            "SELECT * FROM clientes",
            "BEGIN TRANSACTION",
        ] {
            assert_eq!(comando(texto).unwrap(), None, "roubou {texto:?}");
        }
    }

    #[test]
    fn senha_sem_aspas_e_recusada_sem_ecoar_o_que_veio() {
        let e = comando("CREATE USER carlos PASSWORD segredo")
            .unwrap_err()
            .to_string();
        assert!(e.contains("aspas simples"), "{e}");
        assert!(
            !e.contains("segredo"),
            "a mensagem de erro devolveu a senha: {e}"
        );
    }

    #[test]
    fn create_sem_password_e_recusado() {
        assert!(comando("CREATE USER carlos").is_err());
    }

    #[test]
    fn sobra_depois_do_comando_e_recusada() {
        assert!(comando("DROP USER carlos EXTRA").is_err());
    }

    /// A redacao e por ANALISE: acha o literal onde ele estiver, em qualquer
    /// forma de escrita -- e o `''` desdobrado nao volta como senha.
    #[test]
    fn a_senha_sai_do_texto_do_comando() {
        for (entrada, senha) in [
            ("CREATE USER c PASSWORD 'segredo1'", "segredo1"),
            ("create   user c   password   'segredo1'", "segredo1"),
            ("ALTER USER c PASSWORD 'O''Brien'", "O'Brien"),
        ] {
            let redigido = sem_a_senha(entrada);
            assert!(!redigido.contains(senha), "{redigido}");
            assert!(redigido.contains("'***'"), "{redigido}");
            assert!(redigido.to_uppercase().contains("USER"), "{redigido}");
        }
    }

    /// Texto que o lexico recusa nao vira texto -- vira o tamanho. E a mesma
    /// decisao do Profiler, pelo mesmo motivo: mostrar o que nao se analisou e
    /// mostrar o que estiver la dentro.
    #[test]
    fn o_que_nao_se_analisa_vira_o_tamanho() {
        let r = sem_a_senha("CREATE USER c PASSWORD 'aberta");
        assert!(r.starts_with("<comando invalido"), "{r}");
        assert!(!r.contains("aberta"), "{r}");
    }
}

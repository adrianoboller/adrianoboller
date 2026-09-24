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
//! `ALTER` de qualquer outra coisa. O portao le os dois primeiros SIMBOLOS
//! (comentario nao conta) e devolve `None` para tudo o que nao seja
//! `<verbo> USER` -- entao `ALTER SERVER`, `ALTER TABLE` e o que mais vier
//! continuam chegando inteiros a quem os atende.
//!
//! A REDACAO da senha tem portao proprio e mais largo, o [`menciona_senha`]:
//! a senha tambem viaja em comando que este modulo nao reclama.
//!
//! # A senha no texto do comando
//!
//! Ela viaja no SQL, e isso e uma escolha de quem digita: o `usuario_criar`
//! em JSON e o caminho em que o Profiler a tapa por NOME de campo. Num texto
//! SQL nao ha campo -- ha uma frase --, entao o pedido inteiro e o que o
//! Profiler enxerga. Por isso o campo `sql` da resposta volta **redigido**:
//! ver [`sem_a_senha`].

use phxsql_core::error::{Result, LITERAL_REDIGIDO};
use phxsql_core::json::Json;

use crate::lexico::{self, Token};

/// Um comando de cadastro ja reconhecido.
#[derive(Clone, PartialEq)]
pub struct Comando {
    /// A operacao do protocolo: `usuario_criar`, `usuario_alterar` ou
    /// `usuario_excluir`.
    pub op: String,
    pub login: String,
    /// A senha, quando o comando a traz. Nunca vai para log nem para resposta.
    pub senha: Option<String>,
}

/// `Debug` a mao: este comando carrega a senha em CLARO, recem-lida do texto
/// SQL. O comentario do campo ja dizia «nunca vai para log nem para resposta»,
/// e quem cumpria era o `sem_a_senha`; o derivado do `Debug` desfazia isso num
/// `dbg!`.
impl std::fmt::Debug for Comando {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Comando {
            op,
            login,
            senha: _,
        } = self;
        f.debug_struct("Comando")
            .field("op", op)
            .field("login", login)
            .field("senha", &"(oculta)")
            .finish()
    }
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
    // O portao vem ANTES do trabalho: sem ele, TODO texto que passa pela op
    // `sql` -- inclusive um `SELECT` de mil colunas -- pagaria uma analise
    // lexica a mais so para este modulo dizer "nao e comigo". Ele so diz
    // NAO, e so quando os bytes provam: ver `pode_ter_a_palavra`.
    if !pode_ter_a_palavra(texto, "USER") {
        return Ok(None);
    }
    // O que nao se analisa nao e daqui: os outros tradutores leem o mesmo
    // texto pelo mesmo lexico e recusam com a mesma frase, so com a coluna.
    let Ok(simbolos) = lexico::analisar(texto) else {
        return Ok(None);
    };
    if !e_de_cadastro(&simbolos) {
        return Ok(None);
    }
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
    p.fim(&c.op)?;
    Ok(Some(c))
}

/// Os simbolos sao `<CREATE|ALTER|DROP> USER ...`?
///
/// Pelos SIMBOLOS do lexico, e nao pelas palavras separadas por espaco --
/// pedido 497, segunda volta do parecer SEC. O portao velho lia
/// `split_whitespace`, e `/* odbc */ CREATE USER`, `-- x\nCREATE USER` e
/// `CREATE/**/USER` nao eram «de cadastro»: o Profiler e a guarda do job
/// deixavam a senha passar em claro. O `lexico.rs` ja dizia que cliente
/// ODBC manda comentario sem avisar; o lexico tira o comentario, e o
/// recorte nao tirava.
fn e_de_cadastro(simbolos: &[lexico::Simbolo]) -> bool {
    let palavra = |i: usize| simbolos.get(i).and_then(|s| s.token.palavra_chave());
    matches!(palavra(0).as_deref(), Some("CREATE" | "ALTER" | "DROP"))
        && palavra(1).as_deref() == Some("USER")
}

/// As letras de `palavra` (em maiusculas) podem estar no texto?
///
/// Num texto ASCII a resposta e exata, e sai sem copiar: as letras estao la,
/// em qualquer caixa, ou nao estao. Fora do ASCII a prova pelos bytes nao
/// vale -- `ſ` (s longo) vira `S` no `to_uppercase` que o lexico usa, e
/// `paſſword` e a palavra-chave `PASSWORD` --, entao a resposta e SIM, e quem
/// pergunta decide depois: o portao do cadastro pelo lexico, o
/// [`menciona_senha`] pela maiuscula do texto inteiro.
fn pode_ter_a_palavra(texto: &str, palavra: &str) -> bool {
    if !texto.is_ascii() {
        return true;
    }
    texto
        .as_bytes()
        .windows(palavra.len())
        .any(|j| j.eq_ignore_ascii_case(palavra.as_bytes()))
}

/// As letras que abrem a redacao, em maiusculas.
const LETRAS_DA_SENHA: [&str; 2] = ["PASSWORD", "IDENTIFIED"];

/// O texto menciona senha -- as LETRAS `PASSWORD` ou `IDENTIFIED` aparecem
/// nele, em qualquer caixa e em qualquer lugar?
///
/// E o portao da redacao do Profiler e a pergunta do job que recusa gravar
/// segredo no `jobs.json`: quando diz SIM, o Profiler mostra o
/// [`sem_a_senha`] e o job recusa. Uma pergunta so, uma funcao so.
///
/// # Por que as letras, e nao os simbolos
///
/// Pedido 497, terceira volta do parecer SEC. O portao que lia os SIMBOLOS
/// dizia «nao» para o que o lexico descarta ou nao liga a `PASSWORD` -- e o
/// Profiler e o job guardam os BYTES, nao os simbolos. Vazavam o roteiro com
/// a linha comentada (`-- ALTER USER c PASSWORD '...'`), o comentario
/// executavel do MySQL(R) (`/*!80000 IDENTIFIED BY '...' */`), a palavra que
/// CONTEM a senha (`MASTER_PASSWORD`, `SOURCE_PASSWORD`), o literal que a
/// carrega (`CONNECTION '... password=...'` do PostgreSQL) e o identificador
/// citado (`"PASSWORD"`). O ramo que nao analisava ja decidia pelas letras e
/// era o seguro; agora e o unico.
///
/// A maiuscula vem ANTES da procura: `ſ` (s longo) vira `S`, e `PAſſWORD` e a
/// palavra-chave. Texto ASCII procura sem copiar -- ali as duas sao a mesma
/// coisa.
///
/// O preco, escrito: `password_hash`, a coluna `password` ou um comentario
/// com a palavra tambem dizem SIM -- o Profiler tapa os literais daquele SQL
/// e o job o recusa. Tapar demais custa um diagnostico; de menos, a senha.
pub fn menciona_senha(texto: &str) -> bool {
    if texto.is_ascii() {
        return LETRAS_DA_SENHA.iter().any(|p| pode_ter_a_palavra(texto, p));
    }
    let alto = texto.to_uppercase();
    LETRAS_DA_SENHA.iter().any(|p| alto.contains(p))
}

/// O texto SQL redigido, quando ele menciona senha; `None` quando pode sair
/// como veio.
///
/// E a decisao inteira de «como um SQL sai do motor», num lugar so: o
/// Profiler a usa para o `perfil.txt` e o anel, e a op `sql` para o campo
/// `sql` que devolve na resposta -- o roteiro com a senha numa linha
/// comentada roda, e a resposta ecoava o texto inteiro (pedido 497, terceira
/// volta).
pub fn sem_a_senha_se_mencionada(texto: &str) -> Option<String> {
    menciona_senha(texto).then(|| sem_a_senha(texto))
}

/// Este simbolo, em maiusculas, contem as letras da senha?
fn tem_letras_da_senha(palavra: &str) -> bool {
    let alto = palavra.to_uppercase();
    LETRAS_DA_SENHA.iter().any(|p| alto.contains(p))
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
    match redigir(texto) {
        Some(saida) => saida,
        None => format!("<comando invalido, {} bytes>", texto.trim().len()),
    }
}

/// A redacao -- `None` quando o texto nao se analisa.
///
/// # A pergunta e «onde esta a senha», e nao «como um literal aparece»
///
/// Pedido 497, B2 do parecer SEC. A primeira volta delegava tudo ao
/// `Token::descrever`, que redige o literal entre aspas SIMPLES -- e a senha
/// escrita `PASSWORD "x"` (o costume do MySQL(R) e do MariaDB, onde aspas
/// duplas sao texto), `PASSWORD x` ou `PASSWORD 12345678` saia inteira no
/// Profiler e no `jobs.json`. Aqui a senha e TUDO o que vem depois da palavra
/// `PASSWORD` ou `IDENTIFIED` (a do MySQL(R) e do MariaDB: `IDENTIFIED BY`,
/// `IDENTIFIED WITH ... AS`), de qualquer tipo, e sai como UM `'***'` so: nem
/// o numero de pedacos de uma senha mal escrita (`'ab'x'cd'`) escapa. O
/// literal de texto em qualquer outro ponto continua tapado, como sempre.
///
/// E a palavra que CONTEM as letras abre a redacao como a propria
/// (`MASTER_PASSWORD="x"`, `SOURCE_PASSWORD=x`) -- terceira volta do parecer
/// SEC. O comentario some porque o lexico o descarta: a linha comentada de um
/// roteiro nao chega a saida.
fn redigir(texto: &str) -> Option<String> {
    let simbolos = lexico::analisar(texto).ok()?;
    let mut saida = String::with_capacity(texto.len());
    let mut depois_do_password = false;
    for s in &simbolos {
        let fim = matches!(s.token, Token::PontoEVirgula);
        if depois_do_password && !fim {
            if !saida.ends_with(LITERAL_REDIGIDO) {
                saida.push(' ');
                saida.push_str(LITERAL_REDIGIDO);
            }
            continue;
        }
        if !saida.is_empty() && !fim {
            saida.push(' ');
        }
        let (texto_do_simbolo, abre) = match &s.token {
            Token::Texto(_) => (LITERAL_REDIGIDO.to_string(), false),
            // Entre aspas duplas com as letras da senha dentro: no MySQL(R)
            // aspas duplas sao texto (`CONNECTION "... password=x"`), e o
            // nome `"PASSWORD"` tambem vale a palavra.
            Token::Palavra {
                texto,
                citado: true,
            } if tem_letras_da_senha(texto) => (LITERAL_REDIGIDO.to_string(), true),
            // O nome entre aspas duplas ANTES do `PASSWORD` e o login, e o
            // Profiler existe para mostra-lo. Nao passa pelo `descrever`, que
            // e a forma do ERRO e tapa o identificador citado.
            Token::Palavra {
                texto,
                citado: true,
            } => (format!("\"{}\"", texto.replace('"', "\"\"")), false),
            Token::Palavra {
                texto,
                citado: false,
            } => (texto.clone(), tem_letras_da_senha(texto)),
            outro => (outro.descrever(), false),
        };
        saida.push_str(&texto_do_simbolo);
        depois_do_password = abre;
    }
    Some(saida)
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

    /// Onde esta o simbolo corrente -- ou `pos`, quando o comando acabou.
    fn posicao(&self, pos: usize) -> usize {
        self.s.get(self.i).map_or(pos, |x| x.posicao)
    }

    /// Um nome de usuario: palavra ou identificador entre aspas duplas.
    ///
    /// A recusa diz ONDE e nao O QUE veio -- pedido 497, B1 do parecer SEC.
    /// Num comando de cadastro todo simbolo pode ser a senha ou um pedaco
    /// dela (`CREATE USER 'minha senha' ...` e o engano mais comum), e a
    /// mensagem vai ao `acessos.log` e ao Profiler. E a regra que o
    /// `exigir_senha` ja seguia.
    fn exigir_nome(&mut self, pos: usize, onde: &str) -> Result<String> {
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Palavra { texto, .. }) => {
                self.i += 1;
                Ok(texto.clone())
            }
            Some(_) => Err(lexico::erro(
                self.posicao(pos),
                &format!(
                    "depois de {onde} esperava o login -- uma palavra ou um nome \
                     entre aspas duplas"
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
    ///
    /// A sobra NAO e citada, so a coluna dela -- pedido 497, B1. Numa senha
    /// com aspas nao dobradas (`PASSWORD 'ab'SEGREDO'cd'`) o que sobra e um
    /// pedaco da senha, e citar o simbolo o mandava ao `acessos.log`.
    fn fim(&mut self, op: &str) -> Result<()> {
        if matches!(
            self.s.get(self.i).map(|x| &x.token),
            Some(Token::PontoEVirgula)
        ) {
            self.i += 1;
        }
        match self.s.get(self.i) {
            None => Ok(()),
            Some(x) => Err(lexico::erro(
                x.posicao,
                &format!(
                    "{op}: sobrou um simbolo depois do comando -- se era a senha, \
                     aspas simples dentro dela vao dobradas ('')"
                ),
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

    /// **Pedido 497, B2 do parecer SEC: a senha e o que vem depois de
    /// `PASSWORD`, de QUALQUER tipo.** A redacao que so tapava literal de
    /// aspas simples deixava sair inteira, no `perfil.txt`, a senha escrita
    /// entre aspas duplas (o costume do MySQL(R)), sem aspas, como numero --
    /// e o pedaco de uma senha de aspas nao dobradas.
    #[test]
    fn a_senha_em_qualquer_forma_sai_do_texto_do_comando() {
        for entrada in [
            "CREATE USER c PASSWORD \"SEGREDO123\"",
            "CREATE USER c PASSWORD SEGREDO123",
            "ALTER USER c PASSWORD 12345678",
            "CREATE USER c PASSWORD 'ab'SEGREDO123'cd';",
        ] {
            let redigido = sem_a_senha(entrada);
            assert!(
                !redigido.contains("SEGREDO123") && !redigido.contains("12345678"),
                "{entrada} -> {redigido}"
            );
            assert!(
                redigido.contains("PASSWORD '***'"),
                "{entrada} -> {redigido}"
            );
            // Um `'***'` so: nem o numero de pedacos de uma senha mal
            // escrita escapa.
            assert_eq!(redigido.matches("'***'").count(), 1, "{redigido}");
            assert!(menciona_senha(entrada), "{entrada}");
        }
        // O login entre aspas duplas continua visivel: o Profiler existe para
        // mostrar o comando, e o nome nao e segredo.
        let r = sem_a_senha("CREATE USER \"Carlos\" PASSWORD 'x'");
        assert_eq!(r, "CREATE USER \"Carlos\" PASSWORD '***'");
        // E quem nao leva senha continua sem acusar -- senao o job recusaria
        // um `DROP USER` legitimo.
        assert!(!menciona_senha("DROP USER c"));
        // O que nao se analisa responde SIM: gravar o texto por nao saber
        // onde a senha esta seria o pior dos dois erros.
        assert!(menciona_senha("CREATE USER c PASSWORD 'SEGREDO123"));
    }

    /// **Pedido 497, segunda volta do parecer SEC: o portao pelos SIMBOLOS, e
    /// a senha tambem depois de `IDENTIFIED`.** Os oito textos levavam a senha
    /// em claro ao `perfil.txt` e ao `jobs.json`: os tres primeiros porque o
    /// portao lia `split_whitespace` e o comentario o enganava; os tres do
    /// meio porque a redacao so olhava `PASSWORD`; os dois ultimos porque o
    /// portao perguntava «e `CREATE USER`?», e a senha viaja em outros
    /// comandos.
    #[test]
    fn o_portao_e_a_redacao_pelos_simbolos() {
        for entrada in [
            "/* odbc */ CREATE USER c PASSWORD 'SEGREDO123'",
            "-- x\nCREATE USER c PASSWORD 'SEGREDO123'",
            "CREATE/**/USER c PASSWORD 'SEGREDO123'",
            "CREATE USER c IDENTIFIED BY \"SEGREDO123\"",
            "CREATE USER c IDENTIFIED BY SEGREDO123",
            "ALTER USER c IDENTIFIED BY \"SEGREDO123\"",
            "ALTER ROLE c PASSWORD 'SEGREDO123'",
            "SET PASSWORD FOR c = 'SEGREDO123'",
            // O s longo vira `S` no `to_uppercase` do lexico: `PAſſWORD` e a
            // palavra-chave, e o texto nao e ASCII.
            "CREATE USER c PAſſWORD \"SEGREDO123\"",
        ] {
            assert!(menciona_senha(entrada), "{entrada:?}");
            let r = sem_a_senha(entrada);
            assert!(!r.contains("SEGREDO123"), "{entrada:?} -> {r}");
        }
        // O comentario nao engana mais o tradutor: o cadastro que o ODBC
        // manda comentado e um cadastro.
        let c = comando("/* odbc */ CREATE USER c PASSWORD 'x'")
            .unwrap()
            .expect("o comentario escondia o cadastro");
        assert_eq!(c.op, "usuario_criar");
        // E o comportamento de sempre: SQL sem senha nao paga nem muda.
        for sem in [
            "SELECT nome FROM clientes WHERE cidade = 'Blumenau'",
            "SELECT nome FROM clientes WHERE cidade = 'São Paulo'",
            "DROP USER c",
        ] {
            assert!(!menciona_senha(sem), "{sem:?}");
        }
        assert!(!menciona_senha(
            "SELECT nome FROM clientes WHERE cidade = 'Blumenau'"
        ));
        // O preco, escrito: a coluna chamada `password` tambem abre a
        // redacao, e o que vem depois dela sai tapado no Profiler. Tapar
        // demais custa um diagnostico; tapar de menos custa a senha.
        assert_eq!(
            sem_a_senha("SELECT password FROM contas"),
            "SELECT password '***'"
        );
    }

    /// **Pedido 497, terceira volta do parecer SEC: o portao sao as LETRAS.**
    /// O portao pelos simbolos dizia «nao» para o que o lexico descarta ou nao
    /// liga a `PASSWORD`, e o Profiler e o job guardam os bytes: o roteiro
    /// com a linha comentada, o comentario executavel do MySQL(R), a palavra
    /// que CONTEM a senha, o literal que a carrega e o `"PASSWORD"` citado.
    #[test]
    fn o_portao_sao_as_letras_e_a_redacao_tapa_o_que_elas_abrem() {
        for entrada in [
            "SELECT n FROM t; -- ALTER USER c PASSWORD 'SEGREDO123'",
            "SELECT n FROM t; /* ALTER USER c PASSWORD 'SEGREDO123' */",
            "CREATE USER c /*!80000 IDENTIFIED BY 'SEGREDO123' */",
            "CHANGE MASTER TO MASTER_PASSWORD='SEGREDO123'",
            "CHANGE REPLICATION SOURCE TO SOURCE_PASSWORD=\"SEGREDO123\"",
            "CHANGE MASTER TO MASTER_PASSWORD=SEGREDO123",
            "CREATE SUBSCRIPTION s CONNECTION 'host=h password=SEGREDO123' PUBLICATION p",
            "CREATE SUBSCRIPTION s CONNECTION \"host=h password=SEGREDO123\" PUBLICATION p",
            "CREATE USER c \"PASSWORD\" 'SEGREDO123'",
            // A maiuscula antes da procura: o s longo nao escapa das letras.
            "SELECT n FROM t; -- paſſword SEGREDO123",
        ] {
            assert!(menciona_senha(entrada), "{entrada:?}");
            let r = sem_a_senha(entrada);
            assert!(!r.contains("SEGREDO123"), "{entrada:?} -> {r}");
        }
        // O comentario some -- e a redacao certa dele e sumir.
        assert_eq!(
            sem_a_senha("SELECT n FROM t; -- ALTER USER c PASSWORD 'x'"),
            "SELECT n FROM t;"
        );
        // O preco, escrito: `password_hash`, ou um comentario com a palavra,
        // tambem dizem SIM -- o Profiler tapa os literais daquele SQL, e o
        // job que o levasse seria recusado.
        assert!(menciona_senha("SELECT password_hash FROM contas"));
        assert!(menciona_senha(
            "SELECT nome FROM t WHERE cidade = 'Blumenau' -- sem password"
        ));
        assert_eq!(
            sem_a_senha("SELECT nome FROM t WHERE cidade = 'Blumenau' -- sem password"),
            "SELECT nome FROM t WHERE cidade = '***'"
        );
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

    /// O `Debug` do comando tambem nao mostra a senha.
    ///
    /// O `sem_a_senha` redige o TEXTO; o `Comando` ja analisado carrega a senha
    /// em claro num campo, e o `derive(Debug)` a imprimia. Redigir a entrada e
    /// deixar a saida aberta protege so metade do caminho.
    #[test]
    fn o_debug_do_comando_nunca_mostra_a_senha() {
        let c = comando("CREATE USER carlos PASSWORD 'segredo1'")
            .unwrap()
            .unwrap();
        assert_eq!(c.senha.as_deref(), Some("segredo1"));
        for texto in [format!("{:?}", c), format!("{c:?}")] {
            assert!(!texto.contains("segredo1"), "a senha vazou: {texto}");
            assert!(texto.contains("carlos"), "o Debug perdeu o login: {texto}");
        }
        // O pedido montado continua levando a senha -- e ele que vai ao
        // servidor. O que se fecha e a saida de DIAGNOSTICO, nao o protocolo.
        assert!(c.pedido().escrever().contains("segredo1"));
    }
}

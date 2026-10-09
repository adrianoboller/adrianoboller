//! O observador de injecao e a digital da consulta (fatia F1 do pedido 495).
//!
//! Desenho em `docs/propostas/ia-495-496-desenho.md` §4 e §7. As duas funcoes
//! trabalham sobre os SIMBOLOS que o lexico do motor ja produziu, e nunca
//! sobre o texto cru: a pergunta «isto esta dentro de um literal?» so o
//! lexico responde certo, e e ela que separa o ataque do dado. Medido em
//! 24/09 (`bancada/seguranca/495/`): as mesmas perguntas feitas RECORTANDO o
//! texto acusavam 30 de 36 consultas que so traziam o ataque como DADO
//! escapado; analisando, 0.
//!
//! # As quatro classes, e so elas
//!
//! O prototipo de 24/09 tinha dez. Ficaram as quatro que o desenho chama de
//! fortes -- as que acusaram os ataques do repositorio sem acusar o corpo
//! legitimo. As outras (`funcao_fora_do_motor`, `catalogo_do_sistema`,
//! `aspa_aberta`...) acusavam 23 a 28 comandos legitimos cada uma: um sinal
//! que dispara no habitual ensina a ignorar o sinal.
//!
//! # Observa, nunca decide
//!
//! Nenhuma das duas recusa nada. O detector nasce ligado PARA OBSERVAR
//! (decisao de 24/09, mantida no desenho): quem decide o que fazer com um
//! sinal e a camada de ocorrencias (F2), e a recusa de verdade de um segundo
//! comando continua onde sempre esteve, na sintaxe.

use crate::lexico::{Simbolo, Token};
use crate::sintaxe::empilhado_nos_simbolos;

/// As classes acusadas numa consulta. Um conjunto de bits, para o laco
/// quente nao alocar uma lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sinais(u8);

impl Sinais {
    /// Nenhuma classe. Constante para caber num `thread_local!` sem
    /// inicializacao preguicosa (o gancho do servidor, F3).
    pub const NENHUM: Sinais = Sinais(0);
    /// Simbolo depois de um `;`: um segundo comando. Decidido pelo MESMO
    /// motor do [`crate::comando_empilhado`] (pedido 501).
    pub const EMPILHADO: Sinais = Sinais(1);
    /// `OR` seguido de um predicado que so tem constante: `OR '1'='1'`,
    /// `OR 1`. A tautologia classica.
    pub const CONSTANTE_SOB_OR: Sinais = Sinais(1 << 1);
    /// `UNION SELECT` de constantes, ou sem `FROM`: a sondagem de quantas
    /// colunas a consulta tem.
    pub const UNIAO_DE_SONDAGEM: Sinais = Sinais(1 << 2);
    /// Comentario com numero IMPAR de aspas simples: engoliu a aspa de
    /// fechamento que o modelo da aplicacao tinha (`admin'--`).
    pub const COMENTARIO_ENGOLE_ASPA: Sinais = Sinais(1 << 3);

    /// As quatro, com o nome que vai ao evento.
    pub const TODAS: [(Sinais, &'static str); 4] = [
        (Sinais::EMPILHADO, "empilhado"),
        (Sinais::CONSTANTE_SOB_OR, "constante_sob_or"),
        (Sinais::UNIAO_DE_SONDAGEM, "uniao_de_sondagem"),
        (Sinais::COMENTARIO_ENGOLE_ASPA, "comentario_engole_aspa"),
    ];

    pub fn vazio(self) -> bool {
        self.0 == 0
    }

    pub fn contem(self, outra: Sinais) -> bool {
        outra.0 != 0 && self.0 & outra.0 == outra.0
    }

    /// Os nomes das classes acusadas, na ordem de [`Sinais::TODAS`].
    pub fn nomes(self) -> impl Iterator<Item = &'static str> {
        Sinais::TODAS
            .into_iter()
            .filter(move |(c, _)| self.contem(*c))
            .map(|(_, n)| n)
    }

    /// As duas juntas. Publica para o gancho do servidor (F3) somar o que
    /// a op `sql` e os campos de expressao do mesmo pedido acusaram.
    pub fn com(self, outra: Sinais) -> Sinais {
        Sinais(self.0 | outra.0)
    }
}

/// As classes de sinal de injecao de uma consulta.
///
/// Os simbolos devem vir de [`crate::lexico::analisar_com_comentarios`]: sem
/// os comentarios na lista, a classe [`Sinais::COMENTARIO_ENGOLE_ASPA`] nao
/// tem o que ver (as outras tres funcionam igual com ou sem eles). Roda no
/// sucesso E no erro da sintaxe: o segundo comando empilhado e justamente o
/// que a sintaxe recusa.
pub fn sinais(s: &[Simbolo]) -> Sinais {
    let mut r = Sinais::default();
    if empilhado(s) {
        r = r.com(Sinais::EMPILHADO);
    }
    for i in 0..s.len() {
        match &s[i].token {
            Token::Comentario { aspas } if aspas % 2 == 1 => {
                r = r.com(Sinais::COMENTARIO_ENGOLE_ASPA);
            }
            Token::Palavra {
                texto,
                citado: false,
            } => {
                if texto.eq_ignore_ascii_case("OR") && predicado_constante(s, i + 1) {
                    r = r.com(Sinais::CONSTANTE_SOB_OR);
                } else if texto.eq_ignore_ascii_case("UNION") && uniao_de_sondagem(s, i) {
                    r = r.com(Sinais::UNIAO_DE_SONDAGEM);
                }
            }
            _ => {}
        }
    }
    r
}

/// O indice do primeiro simbolo a partir de `i` que NAO e comentario.
fn sig(s: &[Simbolo], i: usize) -> Option<usize> {
    (i..s.len()).find(|&k| !matches!(s[k].token, Token::Comentario { .. }))
}

fn palavra(s: &[Simbolo], k: Option<usize>, p: &str) -> bool {
    matches!(
        k.map(|k| &s[k].token),
        Some(Token::Palavra { texto, citado: false }) if texto.eq_ignore_ascii_case(p)
    )
}

fn e_literal(t: &Token) -> bool {
    match t {
        Token::Numero(_) | Token::Texto(_) => true,
        Token::Palavra {
            texto,
            citado: false,
        } => ["TRUE", "FALSE", "NULL"]
            .iter()
            .any(|p| texto.eq_ignore_ascii_case(p)),
        _ => false,
    }
}

/// O que encerra um predicado: o fim, um `)`, um `;`, ou a palavra que abre
/// a proxima clausula. `OR 1=1 AND x = 2` e tautologia tanto quanto
/// `OR 1=1` no fim.
fn fronteira(s: &[Simbolo], k: Option<usize>) -> bool {
    let Some(k) = k else { return true };
    match &s[k].token {
        Token::FechaParen | Token::PontoEVirgula => true,
        Token::Palavra {
            texto,
            citado: false,
        } => [
            "AND", "OR", "ORDER", "GROUP", "LIMIT", "HAVING", "UNION", "OFFSET",
        ]
        .iter()
        .any(|p| texto.eq_ignore_ascii_case(p)),
        _ => false,
    }
}

/// `lit`, ou `lit cmp lit` (`LIKE` conta como comparador), a partir de `i`,
/// com parenteses de abertura opcionais e seguido de fronteira. So
/// constante: `OR nome = 'x'` compara uma COLUNA, e e o legitimo.
fn predicado_constante(s: &[Simbolo], i: usize) -> bool {
    let mut k = sig(s, i);
    while matches!(k.map(|k| &s[k].token), Some(Token::AbreParen)) {
        k = sig(s, k.unwrap_or(0) + 1);
    }
    let Some(a) = k else { return false };
    if !e_literal(&s[a].token) {
        return false;
    }
    let op = sig(s, a + 1);
    if fronteira(s, op) {
        return true;
    }
    let Some(op) = op else { return false };
    if !matches!(s[op].token, Token::Comparador(_)) && !palavra(s, Some(op), "LIKE") {
        return false;
    }
    let Some(b) = sig(s, op + 1) else {
        return false;
    };
    e_literal(&s[b].token) && fronteira(s, sig(s, b + 1))
}

/// `UNION [ALL|DISTINCT] SELECT` cujo braco so projeta constante, ou nao tem
/// `FROM`. O `UNION` legitimo desta casa empilha tabelas inteiras
/// (`SELECT * FROM a UNION SELECT * FROM b`), e esse nao acusa.
fn uniao_de_sondagem(s: &[Simbolo], i: usize) -> bool {
    let mut j = sig(s, i + 1);
    if palavra(s, j, "ALL") || palavra(s, j, "DISTINCT") {
        j = sig(s, j.unwrap_or(0) + 1);
    }
    if !palavra(s, j, "SELECT") {
        return false;
    }
    let (mut so_literal, mut itens, mut tem_from, mut prof) = (true, 0usize, false, 0i32);
    let mut k = sig(s, j.unwrap_or(0) + 1);
    while let Some(x) = k {
        match &s[x].token {
            Token::AbreParen => prof += 1,
            Token::FechaParen if prof == 0 => break,
            Token::FechaParen => prof -= 1,
            Token::PontoEVirgula if prof == 0 => break,
            Token::Virgula if prof == 0 => {}
            t if prof == 0 => {
                if palavra(s, k, "FROM") {
                    tem_from = true;
                    break;
                }
                if ["UNION", "ORDER", "LIMIT", "WHERE"]
                    .iter()
                    .any(|p| palavra(s, k, p))
                {
                    break;
                }
                itens += 1;
                if !e_literal(t) {
                    so_literal = false;
                }
            }
            _ => {}
        }
        k = sig(s, x + 1);
    }
    (itens > 0 && so_literal) || !tem_from
}

/// A classe «empilhado», pelo motor do 501.
///
/// O motor analisa a sintaxe inteira, e isso custa: entao so roda quando
/// pode dar `true` -- ha um `;` com simbolo de verdade depois dele. Sem isso
/// a resposta do motor e `false` de qualquer jeito (ele procura exatamente
/// isso na sobra, que e um pedaco desta lista), e o caso comum nao paga a
/// sintaxe nem a copia.
fn empilhado(s: &[Simbolo]) -> bool {
    let Some(pv) = s.iter().position(|x| x.token == Token::PontoEVirgula) else {
        return false;
    };
    let depois = s[pv + 1..]
        .iter()
        .any(|x| !matches!(x.token, Token::PontoEVirgula | Token::Comentario { .. }));
    if !depois {
        return false;
    }
    let sem_comentario: Vec<Simbolo> = s
        .iter()
        .filter(|x| !matches!(x.token, Token::Comentario { .. }))
        .cloned()
        .collect();
    empilhado_nos_simbolos(sem_comentario, "")
}

// ------------------------------------------------------------------ digital

const FNV_BASE: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIMO: u64 = 0x0000_0100_0000_01b3;
/// Entre dois simbolos. Nao e UTF-8 valido em posicao nenhuma, entao nenhum
/// identificador o contem: `ab c` e `a bc` nao caem na mesma digital.
const SEPARADOR: u8 = 0xFF;
/// Antes do identificador entre aspas duplas, pelo mesmo motivo: `"from"` e
/// uma coluna, e nao pode ter a digital da clausula `FROM`.
const CITADO: u8 = 0xFE;

struct Fnv(u64);

impl Fnv {
    fn byte(&mut self, b: u8) {
        self.0 ^= b as u64;
        self.0 = self.0.wrapping_mul(FNV_PRIMO);
    }

    fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.byte(*x);
        }
    }

    fn unidade(&mut self, b: &[u8]) {
        self.bytes(b);
        self.byte(SEPARADOR);
    }
}

/// A digital da FORMA da consulta: FNV-1a 64 sobre os simbolos, com todo
/// literal virando marcador.
///
/// - literal (numero, texto, `?`, `TRUE`/`FALSE`/`NULL`) vira `?`; uma lista
///   deles separada por virgula vira `...`, e tuplas iguais seguidas
///   (`VALUES (...), (...)`) viram uma -- senao cada tamanho de `IN` e cada
///   lote seriam uma consulta diferente e inundariam a base;
/// - palavra sem aspas entra em maiusculas (`select` e `SELECT` sao a mesma
///   consulta); identificador entre aspas duplas entra como foi escrito;
/// - comentario nao entra: nao muda o que a consulta faz.
///
/// # Sem `String`, e sem cortar o rabo
///
/// O prototipo de 24/09 montava o texto normalizado e o passava ao FNV
/// (741 ns, teto). Aqui o hash e alimentado simbolo a simbolo, e nada se
/// aloca. E nao ha teto de tamanho: o digest do MySQL(R) trunca em
/// `max_digest_length`, e duas consultas que so diferem depois do corte
/// ganham a mesma digital -- numa digital de seguranca, o rabo invisivel e
/// onde o ataque mora (desenho §5).
///
/// Literal nenhum entra no hash, entao a digital nao carrega dado: e isso
/// que permite le-la num log e mandá-la a uma IA (redigir analisando).
pub fn digital(s: &[Simbolo]) -> u64 {
    let mut h = Fnv(FNV_BASE);
    let mut i = 0;
    while let Some(k) = sig(s, i) {
        if let Some((fim, muitos)) = tuplas(s, k) {
            h.unidade(b"(");
            h.unidade(marcador(muitos));
            h.unidade(b")");
            i = fim + 1;
            continue;
        }
        if e_literal(&s[k].token) || matches!(s[k].token, Token::Parametro(_)) {
            let (fim, muitos) = lista_de_literais(s, k);
            h.unidade(marcador(muitos));
            i = fim + 1;
            continue;
        }
        escrever(&mut h, &s[k].token);
        i = k + 1;
    }
    h.0
}

fn marcador(muitos: bool) -> &'static [u8] {
    if muitos {
        b"..."
    } else {
        b"?"
    }
}

fn literal_ou_parametro(s: &[Simbolo], k: Option<usize>) -> bool {
    k.is_some_and(|k| e_literal(&s[k].token) || matches!(s[k].token, Token::Parametro(_)))
}

/// `lit {, lit}` a partir de `k` (que e literal): o indice do ultimo, e se
/// eram dois ou mais.
fn lista_de_literais(s: &[Simbolo], k: usize) -> (usize, bool) {
    let (mut fim, mut n) = (k, 1usize);
    loop {
        let v = sig(s, fim + 1);
        if !matches!(v.map(|v| &s[v].token), Some(Token::Virgula)) {
            break;
        }
        let proximo = sig(s, v.unwrap_or(0) + 1);
        if !literal_ou_parametro(s, proximo) {
            break;
        }
        fim = proximo.unwrap_or(fim);
        n += 1;
    }
    (fim, n > 1)
}

/// `( lits )` a partir de `k`: o indice do `)` e se a lista tinha dois ou
/// mais.
fn tupla(s: &[Simbolo], k: usize) -> Option<(usize, bool)> {
    if s[k].token != Token::AbreParen {
        return None;
    }
    let a = sig(s, k + 1);
    if !literal_ou_parametro(s, a) {
        return None;
    }
    let (fim, muitos) = lista_de_literais(s, a?);
    let f = sig(s, fim + 1)?;
    (s[f].token == Token::FechaParen).then_some((f, muitos))
}

/// Uma tupla de literais e as iguais a ela que a seguem por virgula.
fn tuplas(s: &[Simbolo], k: usize) -> Option<(usize, bool)> {
    let (mut fim, muitos) = tupla(s, k)?;
    while let Some(v) = sig(s, fim + 1).filter(|&v| s[v].token == Token::Virgula) {
        match sig(s, v + 1).and_then(|p| tupla(s, p)) {
            Some((f, m)) if m == muitos => fim = f,
            _ => break,
        }
    }
    Some((fim, muitos))
}

fn escrever(h: &mut Fnv, t: &Token) {
    let fixo: &[u8] = match t {
        Token::Palavra {
            texto,
            citado: false,
        } => {
            let mut buf = [0u8; 4];
            for c in texto.chars().flat_map(char::to_uppercase) {
                h.bytes(c.encode_utf8(&mut buf).as_bytes());
            }
            h.byte(SEPARADOR);
            return;
        }
        Token::Palavra {
            texto,
            citado: true,
        } => {
            h.byte(CITADO);
            h.unidade(texto.as_bytes());
            return;
        }
        Token::Comparador(c) => c.simbolo().as_bytes(),
        Token::Virgula => b",",
        Token::Ponto => b".",
        Token::AbreParen => b"(",
        Token::FechaParen => b")",
        Token::Asterisco => b"*",
        Token::PontoEVirgula => b";",
        Token::Mais => b"+",
        Token::Menos => b"-",
        Token::Barra => b"/",
        // Os literais e o comentario nunca chegam aqui (a `digital` os
        // trata antes); o marcador fica so por exaustividade.
        Token::Numero(_) | Token::Texto(_) | Token::Parametro(_) => b"?",
        Token::Comentario { .. } => return,
    };
    h.unidade(fixo);
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::lexico::analisar_com_comentarios;

    fn de(sql: &str) -> Sinais {
        sinais(&analisar_com_comentarios(sql).expect(sql))
    }

    fn dig(sql: &str) -> u64 {
        digital(&analisar_com_comentarios(sql).expect(sql))
    }

    /// O `ARSENAL` de `bancada/seguranca/injecao.py`, lido do proprio
    /// arquivo: a lista sai do codigo, e um ataque novo la entra aqui sem
    /// ninguem copiar.
    fn arsenal() -> Vec<(String, String)> {
        let fonte = include_str!("../../../bancada/seguranca/injecao.py");
        let ini = fonte.find("ARSENAL = [").expect("ARSENAL no injecao.py");
        let fim = ini + fonte[ini..].find("\n]").expect("fim do ARSENAL");
        fonte[ini..fim]
            .lines()
            .filter_map(|l| {
                let l = l.trim().strip_prefix("(\"")?.strip_suffix("\"),")?;
                let (nome, texto) = l.split_once("\", \"")?;
                Some((nome.to_string(), python(texto)))
            })
            .collect()
    }

    /// Os escapes que o `ARSENAL` usa: `\xNN`, `\\` e `\"`.
    fn python(t: &str) -> String {
        let mut out = String::new();
        let mut c = t.chars();
        while let Some(x) = c.next() {
            if x != '\\' {
                out.push(x);
                continue;
            }
            match c.next() {
                Some('x') => {
                    let h: String = c.by_ref().take(2).collect();
                    out.push(char::from(u8::from_str_radix(&h, 16).expect("\\x")));
                }
                Some(o) => out.push(o),
                None => {}
            }
        }
        out
    }

    /// Os que o repositorio NAO chama de ataque -- o rotulo do
    /// `extrair_deteccao.py` (`NAO_ATAQUE`), pelo mesmo motivo de la.
    const NAO_ATAQUE: [&str; 7] = [
        "UNION SELECT",
        "comentario /* */ no meio",
        "comentario como separador",
        "comentario -- no fim",
        "aspa escapada como DADO",
        "DROP direto",
        "byte nulo no texto",
    ];

    #[test]
    fn os_cinco_ataques_do_repositorio_sao_acusados() {
        let a = arsenal();
        assert_eq!(a.len(), 12, "o ARSENAL mudou de tamanho: {a:?}");
        let mut ataques = 0;
        for (nome, sql) in &a {
            if NAO_ATAQUE.contains(&nome.as_str()) {
                continue;
            }
            ataques += 1;
            assert!(!de(sql).vazio(), "ataque nao acusado: {nome} -- {sql}");
        }
        assert_eq!(ataques, 5);
        // E o que nao e ataque, e o lexico le, nao acusa.
        for (nome, sql) in &a {
            if let (true, Ok(s)) = (
                NAO_ATAQUE.contains(&nome.as_str()),
                analisar_com_comentarios(sql),
            ) {
                assert!(sinais(&s).vazio(), "{nome}: {:?}", sinais(&s));
            }
        }
    }

    /// Cada classe acusa o seu caso, e SO ele -- zerar qualquer uma derruba
    /// este teste.
    #[test]
    fn cada_classe_acusa_o_seu_caso() {
        let casos = [
            (
                "SELECT * FROM clientes; DROP TABLE clientes",
                Sinais::EMPILHADO,
            ),
            (
                "SELECT * FROM clientes WHERE nome = '' OR '1'='1'",
                Sinais::CONSTANTE_SOB_OR,
            ),
            (
                "SELECT * FROM clientes WHERE id = 7 OR 1 ORDER BY nome",
                Sinais::CONSTANTE_SOB_OR,
            ),
            (
                "SELECT nome FROM clientes WHERE id = 1 UNION SELECT 1, 'a'",
                Sinais::UNIAO_DE_SONDAGEM,
            ),
            (
                "SELECT nome FROM clientes UNION ALL SELECT senha, login",
                Sinais::UNIAO_DE_SONDAGEM,
            ),
            (
                "SELECT * FROM usuarios WHERE login = 'admin'--' AND senha = 'x'",
                Sinais::COMENTARIO_ENGOLE_ASPA,
            ),
            (
                "SELECT * FROM usuarios WHERE login = 'admin' /*' AND senha = 'x' */",
                Sinais::COMENTARIO_ENGOLE_ASPA,
            ),
        ];
        for (sql, classe) in casos {
            assert_eq!(de(sql), classe, "{sql}");
        }
    }

    /// O mesmo texto do ataque, gravado como DADO por uma aplicacao segura
    /// (aspa dobrada), nos tres modelos do `extrair_deteccao.py`. Recortar o
    /// texto acusava 30 destes 36.
    #[test]
    fn zero_de_36_no_dado_escapado() {
        let modelos = [
            "SELECT * FROM clientes WHERE obs = '{}'",
            "INSERT INTO comentarios (autor, texto) VALUES ('ana', '{}')",
            "UPDATE clientes SET obs = '{}' WHERE id = 7",
        ];
        let mut total = 0;
        let mut acusados = Vec::new();
        for (nome, texto) in arsenal() {
            let v = texto.replace('\0', "").replace('\'', "''");
            for m in modelos {
                total += 1;
                let sql = m.replace("{}", &v);
                if !de(&sql).vazio() {
                    acusados.push(format!("{nome}: {sql}"));
                }
            }
        }
        assert_eq!(total, 36);
        assert!(acusados.is_empty(), "{acusados:#?}");
    }

    #[test]
    fn o_legitimo_de_perto_nao_acusa() {
        for sql in [
            "SELECT * FROM clientes WHERE nome = 'x' OR cidade = 'y'",
            "SELECT * FROM clientes WHERE (a = 1 OR b = 2)",
            "SELECT * FROM a UNION SELECT * FROM b",
            "SELECT * FROM clientes;",
            "SELECT * FROM clientes ; -- fim",
            "SELECT * FROM clientes WHERE nome = 'O''Brien' -- comentario",
            "SELECT * FROM clientes WHERE nome = 'Alves' /* sem aspa */",
            "SELECT 'a;b'",
            "CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END",
        ] {
            assert!(
                de(sql).vazio(),
                "{sql}: {:?}",
                de(sql).nomes().collect::<Vec<_>>()
            );
        }
    }

    /// Comentario entre os simbolos nao esconde a tautologia.
    #[test]
    fn comentario_no_meio_nao_esconde() {
        assert!(
            de("SELECT * FROM t WHERE a = 'x' OR/**/1/**/=/**/1").contem(Sinais::CONSTANTE_SOB_OR)
        );
        assert!(de("SELECT * FROM t;/**/DROP TABLE t").contem(Sinais::EMPILHADO));
    }

    #[test]
    fn os_nomes_saem_na_ordem() {
        let s = de("SELECT * FROM t WHERE a = '' OR 1=1; DROP TABLE t --'");
        assert_eq!(
            s.nomes().collect::<Vec<_>>(),
            ["empilhado", "constante_sob_or", "comentario_engole_aspa"]
        );
    }

    #[test]
    fn a_digital_ignora_o_literal() {
        let pares = [
            (
                "SELECT * FROM clientes WHERE nome = 'Alves'",
                "select * from clientes where nome = 'Souza'",
            ),
            (
                "SELECT * FROM t WHERE id = 1 AND ativo = TRUE",
                "SELECT * FROM t WHERE id = 99 AND ativo = FALSE",
            ),
            (
                "SELECT * FROM t WHERE id IN (1, 2, 3)",
                "SELECT * FROM t WHERE id IN (4, 5)",
            ),
            (
                "INSERT INTO t (a, b) VALUES (1, 'x')",
                "INSERT INTO t (a, b) VALUES (2, 'y'), (3, 'z'), (4, 'w')",
            ),
            (
                "SELECT * FROM t WHERE id = ?",
                "SELECT * FROM t WHERE id = 5 -- ola",
            ),
        ];
        for (a, b) in pares {
            assert_eq!(dig(a), dig(b), "{a} / {b}");
        }
    }

    #[test]
    fn formas_diferentes_tem_digitais_diferentes() {
        let formas = [
            "SELECT * FROM clientes WHERE nome = 'x'",
            "SELECT * FROM clientes WHERE cidade = 'x'",
            "SELECT * FROM clientes WHERE nome = 'x' OR '1' = '1'",
            "SELECT nome FROM clientes WHERE nome = 'x'",
            "SELECT * FROM \"clientes\" WHERE nome = 'x'",
            "SELECT * FROM clientes WHERE nome < 'x'",
            "SELECT * FROM cliente WHERE snome = 'x'",
            "SELECT * FROM clientes WHERE id IN (1)",
            "SELECT * FROM clientes WHERE id IN (1, 2)",
        ];
        let d: Vec<u64> = formas.iter().map(|f| dig(f)).collect();
        for i in 0..d.len() {
            for j in i + 1..d.len() {
                assert_ne!(d[i], d[j], "{} / {}", formas[i], formas[j]);
            }
        }
    }

    /// O teste do RABO: duas consultas iguais por mais de 4 KiB e diferentes
    /// so no fim. Uma digital que corta (o `max_digest_length` de 1024 do
    /// MySQL(R)) da a mesma para as duas.
    #[test]
    fn a_digital_ve_o_rabo() {
        let colunas: Vec<String> = (0..600).map(|i| format!("coluna_{i}")).collect();
        let cabeca = format!("SELECT {} FROM t WHERE ", colunas.join(", "));
        assert!(cabeca.len() > 4096);
        let a = dig(&format!("{cabeca}id = 1"));
        let b = dig(&format!("{cabeca}id = 1 OR 1 = 1"));
        let c = dig(&format!("{cabeca}nome = 1"));
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, dig(&format!("{cabeca}id = 2")));
    }
}

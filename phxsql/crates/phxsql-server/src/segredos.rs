//! Os nomes de campo que carregam segredo -- UM lugar so, e a regua que
//! impede a lista de envelhecer.
//!
//! # Por que um modulo, e nao a constante dentro do profiler
//!
//! Porque a lista ja envelheceu uma vez por morar num lugar que so o profiler
//! lia. `token_remoto` nasceu no `dblink/mod.rs` em 03/09/2026 (`948e153`)
//! com o nome escolhido de proposito -- `token` ja e o portao 1 DESTE servidor
//! e o portao o leria primeiro --, e a lista foi retocada em 05/09 (`70c5382`)
//! sem ganha-lo. Resultado, com o profiler ligado: `dblink_salvar` e
//! `replicacao_testar` escreviam o token de servico do OUTRO PhxSql em texto
//! puro no `perfil.txt` e o devolviam pela op `profiler` (revisao SEC de
//! 17/09/2026, achado A1). O job tinha a mesma lei por outra porta: a guarda
//! recusava `token` -- um nome -- e deixava `senha`, `senha_hash`,
//! `token_remoto` e `prova` irem para o `jobs.json` e voltarem na ficha (A2).
//!
//! Duas listas em dois arquivos seria o mesmo numero em dois lugares, e
//! numero em dois lugares diverge. Entao a lista mora aqui; o profiler
//! REDIGE por ela e o job RECUSA por ela. Por que um redige e o outro recusa:
//! o profiler mostra o pedido e pode tapar o valor sem perder nada; o job
//! EXECUTA o pedido, e um `usuario_alterar` com a senha tapada trocaria a
//! senha da pessoa por `***`. O que nao pode ficar em arquivo nao entra.
//!
//! # A regua que impede a lista de envelhecer de novo
//!
//! Por `token_remoto` na lista fecha o vazamento de hoje e nao o de amanha: o
//! proximo campo de segredo com nome novo volta pelo mesmo caminho. O que
//! segura isso e `todo_parametro_com_cara_de_segredo_esta_na_lista`, nos
//! testes deste arquivo: cruza os parametros do `catalogo.rs` -- o inventario
//! das operacoes do protocolo, que ja tem teste amarrando-o ao `despachar` --
//! com esta lista, usando o lexico de `bancada/guardas/debug-com-segredo.py`
//! LIDO do proprio arquivo, e nao copiado. Parametro cujo nome casa o lexico e
//! nao esta aqui reprova NOMEANDO a operacao e o parametro; falso positivo
//! fica declarado com o motivo lido no fonte, e entrada morta reprova, porque
//! chave morta e pior que chave faltando.
//!
//! O que a regua NAO ve, declarado: campo que o `despachar` le e o catalogo
//! nao declara (medido em 17/09/2026: `senha_env` e `token_remoto_env` do
//! `dblink_salvar` sao lidos e nao declarados -- os dois sao NOME de variavel
//! de ambiente, nao valor). Catalogo incompleto e assunto do teste do
//! catalogo, nao desta regua.

use phxsql_core::json::Json;

/// Campos cujo valor nunca e guardado nem escrito.
///
/// A lista e por NOME e nao por heuristica: adivinhar o que e sensivel pelo
/// formato do valor erra nos dois sentidos, e errar para o lado de mostrar e
/// irreversivel -- o texto ja saiu.
pub const SEGREDOS: &[&str] = &[
    "senha",
    "senha_b64",
    "senha_hash",
    "nova_senha",
    // A senha do BANCO, quando ela passar a entrar pelo login.
    //
    // Ela entra na lista ANTES do caminho que a usa, e nao depois, e o motivo
    // e a assimetria que a distingue da senha da conta: a senha da CONTA pode
    // ser provada sem viajar (o desafio-resposta manda uma `prova`, nunca a
    // senha), mas a senha do BANCO nao pode -- o servidor precisa dela em
    // claro para derivar a chave do PBKDF2. Ela viaja, e o unico lugar em que
    // se pode tapar e este.
    //
    // Campo redigido que ninguem ainda manda nao custa nada; campo que se
    // esquece de redigir no dia em que alguem passa a manda-lo custa o
    // segredo, e custa em silencio -- o `perfil.txt` nao acusa nada.
    "senha_banco",
    "senha_banco_b64",
    "prova",
    "token",
    // O token de servico do OUTRO PhxSql (`dblink_salvar`, `replicacao_testar`).
    //
    // Chama-se assim de proposito, porque `token` e o portao 1 daqui -- e foi
    // exatamente o nome escolhido de proposito que ficou dois dias fora desta
    // lista enquanto ela era retocada. A regua nos testes deste arquivo e o
    // que impede o proximo nome novo de repetir isso.
    "token_remoto",
    "chave",
    "chave_privada",
    "assinatura",
];

/// O nome de campo carrega segredo?
///
/// `nome.trim()`: a chave `"senha "` -- com espaco DENTRO das aspas -- nao e
/// a chave que o servidor le, entao ela nunca autentica ninguem; mas um
/// cliente desastrado que a mande poe uma senha de verdade no fio, e quem
/// comparasse sem aparar a mostraria inteira. Comparar aparado nao perde nada
/// e fecha a porta.
pub fn e_nome_de_segredo(nome: &str) -> bool {
    SEGREDOS.iter().any(|s| nome.trim().eq_ignore_ascii_case(s))
}

/// O campo que carrega texto SQL -- onde uma senha mora DENTRO da frase.
///
/// E o caso que a lista de nomes nao alcanca: `{"op":"sql","texto":"CREATE
/// USER c PASSWORD 'x'"}` nao tem campo chamado `senha`. Quem redige (o
/// profiler) analisa a frase e tapa o literal; quem recusa (o job) so precisa
/// saber que ha um literal para tapar.
pub fn e_campo_de_sql(nome: &str) -> bool {
    matches!(nome.trim().to_ascii_lowercase().as_str(), "texto" | "sql")
}

/// O primeiro segredo da arvore, em qualquer profundidade -- ou `None`.
///
/// Devolve a FRASE que nomeia o achado (`o campo "senha"`, `a senha dentro
/// do SQL do campo "texto"`), pronta para entrar numa mensagem de recusa: quem
/// recebe o erro precisa saber o que tirar, e nao so que algo foi recusado.
///
/// Toda a arvore, e nao o primeiro nivel: `{"op":"lote","linhas":[{"senha":
/// ...}]}` e um pedido legitimo, e a senha esta dois niveis abaixo.
pub fn achar_segredo(j: &Json) -> Option<String> {
    match j {
        Json::Objeto(pares) => {
            for (k, v) in pares {
                if e_nome_de_segredo(k) {
                    return Some(format!("o campo {:?}", k.trim()));
                }
                if e_campo_de_sql(k) {
                    if let Some(t) = v.texto() {
                        // `'***'` so aparece quando havia um literal de texto
                        // para tapar: `DROP USER c` e de cadastro e nao leva
                        // senha nenhuma.
                        if phxsql_sql::usuario::e_de_cadastro(t)
                            && phxsql_sql::usuario::sem_a_senha(t).contains("'***'")
                        {
                            return Some(format!("a senha dentro do SQL do campo {:?}", k.trim()));
                        }
                    }
                }
                if let Some(achado) = achar_segredo(v) {
                    return Some(achado);
                }
            }
            None
        }
        Json::Lista(itens) => itens.iter().find_map(achar_segredo),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::Path;

    /// O lexico da guarda `debug-com-segredo.py`, lido do proprio arquivo.
    ///
    /// Lido, e nao copiado: copiado seria o mesmo lexico em dois lugares, e
    /// o dia em que a guarda aprendesse uma palavra nova esta regua nao a
    /// veria. Se o arquivo mudar de lugar, este teste para com o caminho --
    /// que e o comportamento certo para uma regua que perdeu a fonte.
    fn guarda_debug_com_segredo() -> String {
        let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bancada/guardas/debug-com-segredo.py");
        std::fs::read_to_string(&caminho).unwrap_or_else(|e| {
            panic!(
                "nao li {}: {e}. O lexico desta regua mora la; se o arquivo \
                 mudou de lugar, aponte o caminho novo aqui",
                caminho.display()
            )
        })
    }

    /// As strings de um bloco `NOME = (` ... `)` do Python, na ordem.
    ///
    /// So o que esta entre aspas: `LEXICO = ("senha", "token", ...)` vira
    /// `["senha", "token", ...]`. Basta para os dois blocos que se leem aqui,
    /// e um bloco que nao se acha e parada com o nome dele.
    fn strings_do_bloco(python: &str, nome: &str) -> Vec<String> {
        let marca = format!("\n{nome} = (");
        let ini = python
            .find(&marca)
            .unwrap_or_else(|| panic!("`{nome} = (` nao esta no debug-com-segredo.py"));
        // O bloco fecha no `)` que devolve a profundidade a zero -- e nao no
        // primeiro `)`, porque o `NEGA_NO_NOME` e uma tupla de tuplas e o
        // primeiro `)` dele fecha a tupla de dentro.
        let mut strings = Vec::new();
        let mut atual = String::new();
        let mut dentro = false;
        let mut profundidade = 1usize;
        for c in python[ini + marca.len()..].chars() {
            match (dentro, c) {
                (true, '"') => {
                    strings.push(std::mem::take(&mut atual));
                    dentro = false;
                }
                (true, c) => atual.push(c),
                (false, '"') => dentro = true,
                (false, '(') => profundidade += 1,
                (false, ')') => {
                    profundidade -= 1;
                    if profundidade == 0 {
                        return strings;
                    }
                }
                _ => {}
            }
        }
        panic!("`{nome}` nao fecha")
    }

    /// Casa como a guarda casa: PREFIXO do nome inteiro ou de um componente
    /// (`senha_do_rele`, `token_remoto`, `usa_senha`).
    fn casa_o_lexico(nome: &str, lexico: &[String]) -> bool {
        std::iter::once(nome)
            .chain(nome.split('_'))
            .any(|parte| lexico.iter().any(|l| parte.starts_with(l.as_str())))
    }

    /// **A regua da lista: todo parametro do protocolo com cara de segredo
    /// esta em [`SEGREDOS`], ou esta declarado aqui como falso positivo com o
    /// motivo lido no fonte.**
    ///
    /// E o que faltou em 05/09/2026: a lista foi retocada e `token_remoto`,
    /// que ja existia no `dblink_salvar`, ficou fora. Com esta regua, tirar
    /// qualquer nome que o catalogo conhece reprova nomeando a operacao e o
    /// parametro -- e nome novo com cara de segredo reprova no dia em que
    /// entrar no catalogo, que e antes de alguem manda-lo pelo fio.
    #[test]
    fn todo_parametro_com_cara_de_segredo_esta_na_lista() {
        let python = guarda_debug_com_segredo();
        let lexico = strings_do_bloco(&python, "LEXICO");
        assert!(
            lexico.iter().any(|l| l == "senha") && lexico.iter().any(|l| l == "token"),
            "o lexico lido nao parece o da guarda: {lexico:?}"
        );
        // O nome que casa o lexico e NEGA o segredo por si -- `senha_env` e o
        // NOME da variavel de ambiente; `chave_publica` e publica. Vem do
        // mesmo arquivo, pela mesma razao.
        let negadores: Vec<String> = strings_do_bloco(&python, "NEGA_NO_NOME")
            .into_iter()
            .step_by(2)
            .collect();
        assert!(
            negadores.iter().any(|n| n == "_env"),
            "os negadores lidos nao parecem os da guarda: {negadores:?}"
        );

        // Os falsos positivos, DECLARADOS -- `(parametro, motivo lido no
        // fonte)`. Lista visivel e uma linha que alguem le; lista escondida
        // e vazamento futuro com cara de verde. Cada entrada tem de casar um
        // parametro do catalogo: entrada morta reprova, porque parece que
        // protege.
        let isentos: &[(&str, &str)] = &[
            (
                "nonce_cliente",
                "o nonce que o cliente sorteia para o desafio-resposta: viaja em \
                 claro por desenho, e o que se prova e o HMAC dele (`prova`)",
            ),
            (
                "chaves_estrangeiras",
                "NOMES das chaves estrangeiras da tabela (`criar_tabela`), nao \
                 valor de credencial",
            ),
            (
                "salto",
                "o passo de uma sequencia; casa `salt` por prefixo e nao tem \
                 nada de sal",
            ),
        ];

        let mut fora = Vec::new();
        let mut casados = std::collections::HashSet::new();
        for op in crate::catalogo::OPERACOES {
            for p in op.parametros {
                let nome = p.nome;
                if !casa_o_lexico(nome, &lexico) {
                    continue;
                }
                if negadores.iter().any(|n| nome.contains(n.as_str())) {
                    continue;
                }
                if e_nome_de_segredo(nome) {
                    continue;
                }
                if let Some((isento, _)) = isentos.iter().find(|(i, _)| *i == nome) {
                    casados.insert(*isento);
                    continue;
                }
                fora.push(format!("{}.{nome}", op.nome));
            }
        }
        assert!(
            fora.is_empty(),
            "parametro(s) do catalogo com cara de segredo FORA de `SEGREDOS` -- a \
             lista envelheceu. Ou o nome entra na lista, ou entra em `isentos` \
             com o motivo lido no fonte:\n  {}",
            fora.join("\n  ")
        );
        let mortos: Vec<&str> = isentos
            .iter()
            .map(|(i, _)| *i)
            .filter(|i| !casados.contains(i))
            .collect();
        assert!(
            mortos.is_empty(),
            "isencao morta -- nao casa parametro nenhum do catalogo: {mortos:?}"
        );
    }

    /// E o outro sentido da mesma regua: nada que esta na lista deixou de
    /// ser reconhecido, com ou sem espaco, com ou sem maiuscula.
    #[test]
    fn a_lista_reconhece_os_seus_nomes_aparados_e_sem_caixa() {
        for s in SEGREDOS {
            assert!(e_nome_de_segredo(s), "{s}");
            assert!(e_nome_de_segredo(&format!(" {s} ")), "{s} com espaco");
            assert!(
                e_nome_de_segredo(&s.to_ascii_uppercase()),
                "{s} em caixa alta"
            );
        }
        assert!(!e_nome_de_segredo("senha_env"));
        assert!(!e_nome_de_segredo("token_remoto_env"));
        assert!(!e_nome_de_segredo("nome"));
    }

    /// O achado nomeia o campo, em qualquer profundidade -- e a frase SQL
    /// conta como segredo so quando ha literal para tapar.
    #[test]
    fn o_achado_nomeia_o_campo_em_qualquer_profundidade() {
        let j = |s: &str| Json::analisar(s).unwrap();
        assert_eq!(
            achar_segredo(&j(r#"{"op":"usuario_alterar","login":"a","senha":"x"}"#)).as_deref(),
            Some("o campo \"senha\"")
        );
        assert_eq!(
            achar_segredo(&j(
                r#"{"op":"lote","linhas":[{"nome":"ok"},{"token_remoto":"x"}]}"#
            ))
            .as_deref(),
            Some("o campo \"token_remoto\"")
        );
        assert_eq!(
            achar_segredo(&j(r#"{"op":"sql","texto":"CREATE USER c PASSWORD 'x'"}"#)).as_deref(),
            Some("a senha dentro do SQL do campo \"texto\"")
        );
        assert_eq!(
            achar_segredo(&j(r#"{"op":"sql","texto":"DROP USER c"}"#)),
            None,
            "DROP USER e de cadastro e nao leva senha"
        );
        assert_eq!(
            achar_segredo(&j(
                r#"{"op":"sql","texto":"SELECT nome FROM clientes WHERE cidade = 'Blumenau'"}"#
            )),
            None,
            "literal que nao e senha continua sendo dado"
        );
        assert_eq!(
            achar_segredo(&j(r#"{"op":"backup","database":"loja","senha_env":"X"}"#)),
            None,
            "o nome da variavel de ambiente nao e segredo"
        );
    }
}

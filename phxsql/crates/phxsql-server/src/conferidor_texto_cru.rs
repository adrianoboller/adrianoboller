//! O conferidor do TEXTO DE TELA que chega cru ao `innerHTML`.
//!
//! # O defeito que o motivou
//!
//! Pedido 347. O texto de tela nao e constante do programa: ele vem da tabela
//! `phxsys.mensagens`, que e tabela COMUM do motor e se grava com direito de
//! `alterar` naquele database -- privilegio estritamente menor que o
//! `administrar` que o `idiomas_importar` exige. Quem altera essa tabela
//! executa script no navegador de qualquer pessoa que abrir a tela, inclusive
//! de quem tem mais poder que ele. Nao e auto-XSS.
//!
//! O caminho: linha de `phxsys.mensagens` -> `GET /idiomas` -> `aplicarIdioma`
//! guarda em `est.textos` -> `txt(nome, padrao)` devolve
//! `est.textos[nome] || padrao` **sem tratamento** -> literal de gabarito ->
//! `innerHTML`.
//!
//! A CSP nao salva: `<script>` injetado por `innerHTML` nao roda, mas
//! `script-src 'unsafe-inline'` deixa um manipulador de evento rodar, e
//! `connect-src 'self'` permite o `fetch` para o proprio servidor -- a
//! exfiltracao e 100% conforme a CSP.
//!
//! # Por que uma catraca, e nao so o conserto
//!
//! Porque a lei ja estava escrita e ja estava incompleta. O `index.html`
//! dizia, em comentario, que o texto do `phxsys.mensagens` e entrada de
//! usuario e que se usa «`textContent`, nunca `innerHTML`»; e a docstring de
//! `nenhum_texto_da_fabrica_traz_etiqueta_crua` afirmava que «os dois
//! caminhos escapam antes de escrever». **Existiam quatro.** Lei que lista
//! menos casos do que existem nao protege menos hoje -- protege menos no dia
//! em que alguem usar a lista como inventario.
//!
//! E a guarda que ja existia varre a `FABRICA_TELA` (os `texto!` do FONTE),
//! que e o que o programador escreveu. O que vaza e o que o BANCO devolve.
//!
//! # O que ele acha, e o que NAO acha
//!
//! Ele conta a forma `${txt(...)}` dentro de literal de gabarito **sem** o
//! `esc(` em volta. E a forma que oito dos nove sitios do 347 tinham.
//!
//! Ele **nao** acha o nono: no `phx-grid.js` o que vazava era
//! `cA.titulo`, um titulo de coluna que VIAJOU -- saiu de `txt()` em outro
//! lugar, foi guardado num objeto de configuracao e so depois virou HTML.
//! Nenhuma varredura de texto liga as duas pontas, e fingir que liga seria
//! pior que nao ter guarda. Esse lado e segurado pela prova de navegador
//! `testes-web/prova-xss-do-texto-de-tela.mjs`, que repoe o defeito num clone
//! do componente e exige que o veneno DISPARE nele.
//!
//! Dizer o que a guarda nao cobre e parte da guarda: foi a omissao disso que
//! deixou o 347 nascer.

use crate::conferidor::FONTES;

/// Uma interpolacao de texto de tela achada sem escape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cru {
    pub arquivo: &'static str,
    /// Linha 1-based, para o relatorio poder ser aberto no editor.
    pub linha: usize,
    /// A chave pedida, quando da para ler entre aspas.
    pub chave: String,
}

/// Acha `${txt(` que nao esteja dentro de um `esc(`.
///
/// O crivo e deliberadamente simples e LOCAL: ele olha os poucos caracteres
/// antes do `${`. Um analisador de JavaScript acharia mais, e tambem teria de
/// ser mantido; esta forma pega a classe inteira que o 347 nomeou, e o que
/// ela nao pega esta dito na documentacao do modulo em vez de ficar implicito.
pub fn varrer(arquivo: &'static str, fonte: &str) -> Vec<Cru> {
    let mut achados = Vec::new();
    for (i, linha) in fonte.lines().enumerate() {
        let mut de = 0usize;
        while let Some(p) = linha[de..].find("${txt(") {
            let abs = de + p;
            // `${esc(txt(` e a forma certa: o `${` vem antes do `esc`, entao
            // a busca por `${txt(` nao casa com ela. Mas um `esc(` pode estar
            // ANTES na mesma linha envolvendo tudo -- e o caso de quem escreve
            // `esc(`\u{60}...${txt(...)}...`\u{60})`. Por isso a conferencia olha
            // se ha um `esc(` aberto antes, sem fechar.
            if !dentro_de_esc(&linha[..abs]) {
                achados.push(Cru {
                    arquivo,
                    linha: i + 1,
                    chave: chave_de(&linha[abs..]),
                });
            }
            de = abs + 6;
        }
    }
    achados
}

/// Ha um `esc(` aberto (parenteses sem fechar) no trecho antes?
fn dentro_de_esc(antes: &str) -> bool {
    let bytes = antes.as_bytes();
    let mut i = 0usize;
    let mut abertos: Vec<bool> = Vec::new(); // true = este parentese e de um `esc(`
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let e = i >= 3 && &antes[i - 3..i] == "esc";
            abertos.push(e);
        } else if bytes[i] == b')' {
            abertos.pop();
        }
        i += 1;
    }
    abertos.iter().any(|e| *e)
}

/// A chave entre as primeiras aspas depois do `txt(`, para o relatorio.
fn chave_de(trecho: &str) -> String {
    let dentro = &trecho[6.min(trecho.len())..];
    match (
        dentro.find('"'),
        dentro.find('"').and_then(|a| dentro[a + 1..].find('"')),
    ) {
        (Some(a), Some(b)) => dentro[a + 1..a + 1 + b].to_string(),
        _ => String::from("(sem chave legivel)"),
    }
}

/// Todas as ocorrencias, nos mesmos fontes que o resto dos conferidores le.
pub fn conferir() -> Vec<Cru> {
    FONTES
        .iter()
        .flat_map(|(arq, fonte)| varrer(arq, fonte))
        .collect()
}

/// A catraca. **So desce.**
///
/// Nasceu em **1** e nao em zero, e o 1 tem nome: `ui/index.html`, a
/// interpolacao de `tela.st_ms_ou_mais` dentro do `rot` do grafico de
/// distribuicao. Ali o `txt()` entra cru num pedaco que e escapado UM NIVEL
/// ACIMA, no `esc(rot)` que vai para o SVG -- conferido linha a linha em
/// 18/09/2026. Nao e furo.
///
/// Zero seria comprado caro: exigiria escapar dentro do `rot` e tirar o
/// `esc(rot)` de fora, criando uma string «ja escapada» que o proximo a
/// concatenar nao saberia que e. Trocar um furo real por uma armadilha futura
/// nao e conserto.
pub const TETO_TXT_CRU_EM_HTML: usize = 1;

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nenhum_texto_de_tela_novo_chega_cru_ao_html() {
        let crus = conferir();
        assert!(
            crus.len() <= TETO_TXT_CRU_EM_HTML,
            "{} interpolacoes `${{txt(...)}}` sem `esc`, e a catraca esta em \
             {TETO_TXT_CRU_EM_HTML}.\nO texto de tela vem de `phxsys.mensagens`, \
             que se grava com `alterar` -- e entrada de usuario, nao constante \
             do programa. Escreva `${{esc(txt(...))}}`.\nAchados: {:?}",
            crus.len(),
            crus
        );
    }

    /// A regua contra o zero por engano: se o crivo parar de achar, esta
    /// prova cai antes de a catraca declarar tudo limpo.
    #[test]
    fn o_conferidor_acha_o_que_promete() {
        let doente = "const h = `<b>${txt(\"tela.x\", \"x\")}</b>`;";
        let achados = varrer("sintetico", doente);
        assert_eq!(achados.len(), 1, "nao achou a forma crua: {achados:?}");
        assert_eq!(achados[0].chave, "tela.x");
        assert_eq!(achados[0].linha, 1);

        let sao = "const h = `<b>${esc(txt(\"tela.x\", \"x\"))}</b>`;";
        assert!(
            varrer("sintetico", sao).is_empty(),
            "acusou a forma JA escapada -- a catraca reprovaria codigo certo"
        );

        // O escape um nivel acima, que e o caso do teto 1: o `esc(` abre
        // antes e ainda nao fechou quando o `${txt(` aparece.
        let acima = "const r = esc(`${txt(\"tela.y\", \"y\")} ms`);";
        assert!(
            varrer("sintetico", acima).is_empty(),
            "acusou o escape um nivel acima, que e seguro"
        );
    }

    /// O teto de 1 nao pode virar desculpa: se a unica ocorrencia sumir, a
    /// catraca desce -- e esta prova diz que ela precisa descer.
    #[test]
    fn o_teto_de_um_ainda_tem_dono() {
        let crus = conferir();
        assert_eq!(
            crus.len(),
            TETO_TXT_CRU_EM_HTML,
            "a catraca esta em {TETO_TXT_CRU_EM_HTML} e ha {} ocorrencia(s). \
             Se desceu, BAIXE O TETO no mesmo commit -- catraca frouxa nao \
             segura nada. Achados: {:?}",
            crus.len(),
            crus
        );
    }
}

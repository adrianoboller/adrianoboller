//! O conferidor das GRADES: quais tabelas da tela ainda sao montadas na mao.
//!
//! # Por que ele existe
//!
//! Palavra do dono: *«todas as table sao phxgrid com agrupamento dinamico»* --
//! o agrupamento pela barra de cima que recebe os cabecalhos arrastados, no
//! molde do DevExpress e do Janus. Uma tela que monta `<table>` na mao nao tem
//! isso, e nao tem filtro por coluna, nem congelar, nem exportar a vista.
//!
//! A padronizacao sem catraca dura ate a proxima frente: quem acrescenta tela
//! escreve `<table>` porque e o que ele conhece, e ninguem percebe. Foi o que
//! ja aconteceu com os textos fora da fabrica de idiomas -- a maquina existia
//! desde a 0.17.0 e a tela tinha 16 `data-txt` em 11.987 linhas. Maquina que
//! funciona e que ninguem usa e promessa, nao garantia.
//!
//! # O que ele NAO faz, e de proposito
//!
//! Ele nao manda converter tudo. Nem toda `<table>` e grade: a janela de
//! conflito de escrita e um formulario coluna a coluna, o cartao do diagrama
//! ER e um no de desenho, os creditos sao uma lista de duas colunas que
//! ninguem vai ordenar. Converter essas seria estrago, nao padronizacao.
//!
//! Por isso a saida nao e um numero so: e um numero **com catalogo**. Quem
//! monta tabela na mao com motivo entra em [`ISENTAS`] com o motivo escrito
//! ao lado; quem monta sem motivo conta para a catraca. Dispensa registrada e
//! decisao; dispensa silenciosa e esquecimento -- e a diferenca entre as duas
//! e a unica coisa que este modulo cobra.
//!
//! # A lista de arquivos sai do codigo
//!
//! Ele varre [`crate::conferidor::FONTES`], que e a mesma lista que o
//! conferidor de idiomas usa e que o teste `a_lista_cobre_tudo_que_o_http_serve`
//! confere contra o `http.rs`. Lista digitada envelhece calado: o
//! `multitela.js` ja entrou no `http.rs` sem entrar na lista, e ficou 1.474
//! linhas invisiveis para a catraca.
//!
//! ```bash
//! cargo run --example grades-fora-do-padrao -p phxsql-server
//! ```

use crate::conferidor::FONTES;

/// Uma tabela montada na mao: onde esta e de quem e.
#[derive(Debug, Clone)]
pub struct Tabela {
    pub arquivo: &'static str,
    pub linha: usize,
    /// A funcao que a monta. `(topo do arquivo)` quando esta fora de funcao --
    /// que e o caso do HTML estatico.
    pub funcao: String,
    /// `Some(motivo)` quando esta em [`ISENTAS`]; `None` quando conta.
    pub isenta: Option<&'static str>,
}

/// Quem monta tabela na mao COM MOTIVO, e o motivo.
///
/// Cada linha e uma decisao registrada. Entrada que sobra aqui depois que a
/// funcao virou grade e pior que entrada faltando: o proximo leitor confia
/// nela e nao converte o que ja daria para converter. Por isso
/// `nenhuma_isencao_morta` reprova a entrada que nao corresponde mais a
/// tabela nenhuma -- e a mesma licao da chave morta da fabrica de idiomas.
pub const ISENTAS: &[(&str, &str, &str)] = &[
    (
        "ui/grid/phx-grid.js",
        "*",
        "e a propria grade: e ela quem monta a `<table>` que todo mundo usa",
    ),
    (
        "ui/claude.js",
        "desenharRevisao",
        "pre-visualizacao das colunas que a IA PROPOE criar, dentro do cartao \
         de cada tabela na tela de revisao. Ordenar aqui esconderia a ordem em \
         que as colunas vao nascer -- e a ordem de digitacao e sagrada, entao \
         uma grade que deixa reordenar a vista mentiria sobre o que vai gravar",
    ),
    (
        "ui/multitela.js",
        "telaAjuda",
        "os monitores fisicos desta area de trabalho, na tela de ajuda. Sao de \
         um a quatro e a ordem e a do sistema operacional; agrupar monitor por \
         monitor nao responde pergunta nenhuma",
    ),
    (
        "ui/explorador.js",
        "blocoDaOperacao",
        "os campos de UMA operacao da API, no explorador do OpenAPI. E \
         documentacao: a tabela existe para ser lida junto do texto ao lado, e \
         a ordem e a do contrato -- filtrar a definicao pela propria definicao \
         nao e uso que exista",
    ),
];

/// O nome da funcao declarada nesta linha, se ela declara alguma.
///
/// A pagina declara todas do mesmo jeito (`function nome(` ou
/// `async function nome(`). Metodo de objeto e funcao anonima nao entram --
/// e nenhuma tabela da tela nasce dentro de uma.
///
/// **Limite declarado**: vale a declaracao mais recente, entao numa funcao
/// aninhada o dono sai o nome de dentro e nao o de fora. Em `index.html` isso
/// nao acontece (as telas sao todas de primeiro nivel), e num arquivo bem
/// aninhado como o `phx-grid.js` o nome fica impreciso -- por isso a isencao
/// dele e por arquivo. A CONTA nao depende disso: o que a catraca conta e
/// ocorrencia de `<table>`, e a linha impressa no relatorio e exata.
fn declara_funcao(linha: &str) -> Option<&str> {
    let t = linha.trim_start();
    let t = t.strip_prefix("async ").unwrap_or(t);
    let t = t.strip_prefix("function ")?;
    let fim = t.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')?;
    let nome = &t[..fim];
    if nome.is_empty() || !t[fim..].trim_start().starts_with('(') {
        return None;
    }
    Some(nome)
}

/// Varre um arquivo e devolve toda `<table>` montada na mao que houver nele.
pub fn varrer(arquivo: &'static str, fonte: &str) -> Vec<Tabela> {
    let mut achados = Vec::new();
    let mut dono = String::from("(topo do arquivo)");
    for (i, linha) in fonte.lines().enumerate() {
        if let Some(n) = declara_funcao(linha) {
            dono = n.to_string();
        }
        // Uma linha pode abrir mais de uma tabela; cada uma conta.
        let mut resto = linha;
        while let Some(p) = resto.find("<table") {
            let isenta = ISENTAS
                .iter()
                .find(|(a, f, _)| *a == arquivo && (*f == dono || *f == "*"))
                .map(|(_, _, porque)| *porque);
            achados.push(Tabela {
                arquivo,
                linha: i + 1,
                funcao: dono.clone(),
                isenta,
            });
            resto = &resto[p + "<table".len()..];
        }
    }
    achados
}

/// Toda tabela na mao de toda a interface servida.
pub fn conferir() -> Vec<Tabela> {
    FONTES
        .iter()
        .flat_map(|(arq, fonte)| varrer(arq, fonte))
        .collect()
}

/// So as que contam para a catraca -- isto e, as sem motivo registrado.
pub fn sem_motivo() -> Vec<Tabela> {
    conferir()
        .into_iter()
        .filter(|t| t.isenta.is_none())
        .collect()
}

/// Quantas chamadas de `PhxGrid.criar(` a interface faz.
///
/// E o outro lado do placar: a catraca sozinha diz o que falta e nao diz o
/// que ja andou. Numero medido, nunca digitado.
pub fn no_padrao() -> usize {
    FONTES
        .iter()
        .filter(|(a, _)| *a != "ui/grid/phx-grid.js")
        .map(|(_, f)| f.matches("PhxGrid.criar(").count())
        .sum()
}

/// A catraca. **So desce.**
///
/// Converteu uma tela, baixe o TETO no mesmo commit: catraca frouxa nao
/// segura nada. Se a tabela nao devia mesmo virar grade, ela entra em
/// [`ISENTAS`] com o motivo -- e ai o TETO desce do mesmo jeito.
pub const TETO: usize = 24;

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nenhuma_tabela_nova_fora_do_padrao() {
        let na_mao = sem_motivo();
        assert!(
            na_mao.len() <= TETO,
            "{} tabelas montadas na mao, e a catraca esta em {TETO}.\n\
             Tela nova usa `PhxGrid.criar` -- e se esta tabela nao e grade \
             (formulario, cartao de diagrama, legenda), ponha em ISENTAS com \
             o motivo.\nRelatorio: cargo run --example grades-fora-do-padrao \
             -p phxsql-server\nAs primeiras: {:?}",
            na_mao.len(),
            na_mao.iter().take(5).collect::<Vec<_>>()
        );
        assert!(
            na_mao.len() >= TETO.saturating_sub(10),
            "sobraram {} e a catraca esta em {TETO}: baixe a catraca no mesmo \
             commit da conversao, senao ela para de segurar",
            na_mao.len()
        );
    }

    /// Isencao morta e pior que isencao faltando: o proximo leitor confia
    /// nela e deixa de converter o que ja daria para converter.
    #[test]
    fn nenhuma_isencao_morta() {
        let todas = conferir();
        for (arq, funcao, porque) in ISENTAS {
            assert!(
                todas
                    .iter()
                    .any(|t| t.arquivo == *arq && (t.funcao == *funcao || *funcao == "*")),
                "ISENTAS diz que `{funcao}` de `{arq}` monta tabela na mao \
                 ({porque}), e ela nao monta mais nenhuma. Tire a linha"
            );
        }
    }

    /// A prova de que o conferidor PEGA -- sem ela ele pode estar medindo zero
    /// por estar quebrado, e um zero por engano e pior que um numero alto.
    #[test]
    fn o_conferidor_acha_o_que_promete() {
        const FONTE: &str = "\
function telaQualquer() {\n\
  return `<table class=\"grade\"><thead><tr><th>Nome</th></tr></thead></table>`;\n\
}\n\
async function telaCerta() {\n\
  PhxGrid.criar('#alvo', { colunas: cols, agrupavel: true });\n\
}\n";
        let achados = varrer("ui/teste.html", FONTE);
        assert_eq!(achados.len(), 1, "achou: {achados:?}");
        assert_eq!(achados[0].funcao, "telaQualquer");
        assert_eq!(achados[0].linha, 2);
        assert!(achados[0].isenta.is_none());
    }

    /// Duas tabelas na mesma linha contam duas vezes. O laco que corta so a
    /// primeira ocorrencia por linha subcontaria em silencio.
    #[test]
    fn duas_na_mesma_linha_contam_duas() {
        let achados = varrer(
            "ui/teste.html",
            "function t(){return `<table></table><table></table>`;}",
        );
        assert_eq!(achados.len(), 2, "achou: {achados:?}");
    }
}

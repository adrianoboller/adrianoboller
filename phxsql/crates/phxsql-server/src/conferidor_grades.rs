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
//! # As duas formas de montar tabela na mao
//!
//! Contar so `<table>` cru subcontava, e feio: a pagina tem um AJUDANTE
//! (`tabela(cabecas, linhas, montar)`) que monta a marcacao por dentro, entao
//! dezoito telas construidas a mao apareciam como **uma** ocorrencia -- a do
//! proprio ajudante. Uma catraca que subconta e pior que catraca nenhuma:
//! ela deixa declarar a padronizacao terminada com dezoito telas de fora.
//!
//! Por isso [`varrer`] conta as duas formas, e o relatorio as separa:
//!
//! - **marcacao crua** -- um `<table>` escrito na tela;
//! - **ajudante** -- uma chamada a `tabela(`, que e a mesma tabela a mao com
//!   menos letras.
//!
//! O dia em que o ultimo chamador sair, o ajudante vira codigo morto e sai
//! junto -- e a ocorrencia dele desce sozinha.
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

/// Como a tabela foi montada a mao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forma {
    /// `<table>` escrito na tela.
    Marcacao,
    /// Chamada ao ajudante `tabela(`, que monta a mesma marcacao por dentro.
    Ajudante,
}

/// Uma tabela montada na mao: onde esta, de quem e, e de que jeito.
#[derive(Debug, Clone)]
pub struct Tabela {
    pub arquivo: &'static str,
    pub linha: usize,
    /// A funcao que a monta. `(topo do arquivo)` quando esta fora de funcao --
    /// que e o caso do HTML estatico.
    pub funcao: String,
    /// `Some(motivo)` quando esta em [`ISENTAS`]; `None` quando conta.
    pub isenta: Option<&'static str>,
    pub forma: Forma,
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
    // ---------------------------------------------------------------- 03/09
    // As 20 que sobraram, classificadas uma a uma. Nenhuma delas e lista de
    // dado: sao FORMULARIO (input por celula), FICHA TECNICA (campo -> valor
    // -> o que faz, em que a ORDEM e a informacao), PREVIA ilustrativa, o
    // PIVOT (que ja e o resultado de agrupar) ou desenho. Converter qualquer
    // uma seria estrago com cara de padronizacao -- e e a mesma frase que o
    // cabecalho deste modulo ja dizia antes de alguem ter classificado.
    (
        "ui/index.html",
        "desenharNovaTabela",
        "as duas sao FORMULARIO: `input` por celula em campos e em indices. E \
         a de campos e a ordem de digitacao sendo montada -- ordenar a vista \
         ali mentiria sobre a ordem em que as colunas vao nascer, que e o \
         mesmo motivo da `desenharRevisao`",
    ),
    (
        "ui/index.html",
        "cartaoNovaTabelaER",
        "formulario, como a `desenharNovaTabela`, dentro do cartao do \
         diagrama: `input` de nome, `select` de tipo e as caixas de obrigatorio \
         e PK, com a linha vazia no fim para acrescentar campo",
    ),
    (
        "ui/index.html",
        "editorDeMenu",
        "formulario: uma linha por rotulo, com o `input` do nome que a pessoa \
         quer. O agrupamento que faria sentido -- por grupo de menu -- ja \
         existe, e e um `<h3>` por grupo, decidido pelo codigo e nao arrastavel",
    ),
    (
        "ui/index.html",
        "dialogoConflito",
        "a janela de conflito de escrita, e ela e o contrario de uma grade: \
         cada linha e uma COLUNA em disputa e a pessoa escolhe um radio por \
         linha. Ordenar ou agrupar aqui reordenaria as escolhas no meio da \
         decisao, que e o estrago que esta janela existe para impedir",
    ),
    (
        "ui/index.html",
        "assistenteDbLink",
        "passo 4 do assistente: caixa de marcar, `select` de sentido e `select` \
         de conflito por tabela de la. Formulario com cara de tabela, e o que \
         a pessoa faz nele e marcar, nao consultar",
    ),
    (
        "ui/index.html",
        "verConfigTabela",
        "as tres sao FICHA TECNICA -- geometria, diretiva desta tabela, \
         diretiva herdada --, no molde campo -> valor -> por que e assim. A \
         ordem e a do raciocinio e nao a alfabetica: ordenar transformaria uma \
         explicacao em lista",
    ),
    (
        "ui/index.html",
        "verConfigBanco",
        "ficha tecnica de um database: onde ele mora e o que herda do \
         servidor. Duas colunas, sem cabecalho, lida junto do texto ao lado",
    ),
    (
        "ui/index.html",
        "grupoDeAjustes",
        "o AJUDANTE das fichas de configuracao (campo do `config.json` -> \
         valor agora -> para que serve), usado por varias telas. Mesma \
         natureza da `verConfigTabela`, num lugar so",
    ),
    (
        "ui/index.html",
        "verDiretivasDoBanco",
        "os portoes NA ORDEM EM QUE FECHAM, e a ordem E a informacao: o \
         portao 1 recusa antes de o 2 existir. Uma grade que deixa reordenar \
         essa tabela apaga justamente o que ela ensina",
    ),
    (
        "ui/index.html",
        "verCreditos",
        "sobre o que este motor se apoia: duas colunas escritas a mao, com RFC \
         e norma ao lado de cada peca. Nao vem de dado nenhum e ninguem vai \
         agrupar por «de onde vem»",
    ),
    (
        "ui/index.html",
        "gradeDeParticoes",
        "as duas sao PREVIA ilustrativa, e nao dado: tres volumes de exemplo, \
         uma linha de reticencias e a ultima, para mostrar como o nome do \
         arquivo e o rowid vao ser cortados. Ordenar uma reticencia nao \
         significa nada",
    ),
    (
        "ui/index.html",
        "pivotPasso1",
        "as duas sao do assistente: a escolha da tabela de fatos (clique \
         simples marca, e a grade abre linha no clique DUPLO -- trocar isso \
         piora o passo) e a lista de juncoes ja declaradas, um conjunto de \
         trabalho de zero a tres linhas com botao de tirar",
    ),
    (
        "ui/index.html",
        "pivotPasso3",
        "o RESULTADO do pivot, e ele ja e o agrupamento: as colunas nascem do \
         dado (`rotulos_coluna`), ha canto, total de linha e total de coluna. \
         Agrupar um pivot e agrupar o que ja foi agrupado, e uma grade de \
         colunas fixas nao expressa colunas que vem da consulta",
    ),
    (
        "ui/index.html",
        "cartaoTabelaER",
        "as colunas da tabela dentro do cartao do diagrama ER -- um no de \
         desenho que se arrasta, com 220px de altura. Filtro, paginacao e \
         barra de agrupamento nao cabem num cartao que a pessoa esta \
         arrastando pela tela",
    ),
    (
        "ui/index.html",
        "telaExportar",
        "NAO E TABELA, e limite medido da regua: a ocorrencia e a palavra \
         `<table>` DENTRO do texto que explica a importacao de HTML («a \
         primeira <table> do documento»). A varredura casa `<table` em \
         qualquer lugar da linha, e distinguir marcacao de prosa por padrao \
         de texto seria heuristica fragil escondendo tabela de verdade \
         amanha. E a unica em prosa nas 20 -- conferido linha a linha",
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

/// As posicoes em que esta linha CHAMA o ajudante `tabela(`.
///
/// Duas armadilhas, as duas encontradas medindo e nao imaginando:
///
/// - **`tabela(s)`** aparece em texto de tela o tempo todo («3 tabela(s) têm
///   coluna Sequence»). Nao e chamada, e sem esta recusa o numero inflava.
/// - **`function tabela(`** e a DECLARACAO do ajudante, nao um uso dele. Quem
///   conta a declaracao pede para o ajudante se converter a si mesmo.
///
/// Tambem sai fora `algo.tabela(`, que e metodo de outro objeto.
///
/// **Limite declarado**: comentario no MEIO da linha nao e reconhecido -- so
/// a linha que COMECA comentada sai fora. Separar codigo de comentario no
/// meio de uma linha pede um analisador que saiba de aspas, crase e barra de
/// expressao regular, e um analisador frouxo erraria calado. Medido nos seis
/// arquivos servidos, isso hoje nao acontece nenhuma vez: o reconhecedor acha
/// exatamente as mesmas 18 chamadas que uma varredura independente achou.
/// E o erro, se um dia vier, pesa para o lado seguro: conta a mais, e uma
/// contagem a mais so obriga alguem a registrar a dispensa com o motivo.
fn chamadas_ao_ajudante(linha: &str) -> Vec<usize> {
    const ALVO: &str = "tabela(";
    let b = linha.as_bytes();
    let mut fora = Vec::new();
    let mut de = 0;
    while let Some(rel) = linha[de..].find(ALVO) {
        let p = de + rel;
        de = p + ALVO.len();
        let antes = if p == 0 { None } else { Some(b[p - 1]) };
        let e_identificador = matches!(antes, Some(c)
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.');
        let plural = linha[de..].starts_with("s)");
        let declaracao = p >= 9 && &linha[p - 9..p] == "function ";
        if !e_identificador && !plural && !declaracao {
            fora.push(p);
        }
    }
    fora
}

/// Varre um arquivo e devolve toda `<table>` montada na mao que houver nele.
pub fn varrer(arquivo: &'static str, fonte: &str) -> Vec<Tabela> {
    let mut achados = Vec::new();
    let mut dono = String::from("(topo do arquivo)");
    for (i, linha) in fonte.lines().enumerate() {
        if let Some(n) = declara_funcao(linha) {
            dono = n.to_string();
        }
        let isenta = ISENTAS
            .iter()
            .find(|(a, f, _)| *a == arquivo && (*f == dono || *f == "*"))
            .map(|(_, _, porque)| *porque);
        let mut poe = |linha_n: usize, forma: Forma| {
            achados.push(Tabela {
                arquivo,
                linha: linha_n,
                funcao: dono.clone(),
                isenta,
                forma,
            });
        };

        // Uma linha pode abrir mais de uma tabela; cada uma conta.
        let mut resto = linha;
        while let Some(p) = resto.find("<table") {
            poe(i + 1, Forma::Marcacao);
            resto = &resto[p + "<table".len()..];
        }

        // Comentario nao e codigo, e a palavra «tabela» aparece muito nos
        // comentarios desta base.
        let t = linha.trim_start();
        if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
            continue;
        }
        for p in chamadas_ao_ajudante(linha) {
            let _ = p;
            poe(i + 1, Forma::Ajudante);
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

/// A catraca das tabelas montadas a mao. **So desce, e NUNCA sobe.**
///
/// Converteu uma tela, baixe o teto no mesmo commit: catraca frouxa nao segura
/// nada. Se a tabela nao devia mesmo virar grade, ela entra em [`ISENTAS`] com
/// o motivo -- e ai o teto desce do mesmo jeito.
///
/// # Esta catraca SUBSTITUI outra, e e por isso que ela se chama assim
///
/// Houve antes um teto que contava so `<table>` cru. Quando o conferidor
/// aprendeu a enxergar tambem a chamada ao ajudante `tabela(`, o numero pulou
/// de 24 para 43 -- e eu subi o teto, com o motivo escrito. **Decisao do dono:
/// isso nao se faz.** Regua que passa a medir mais nao levanta a catraca
/// existente; ela **aposenta** a antiga e nasce uma nova, no numero medido do
/// dia, dizendo que substitui a outra.
///
/// E o que este nome registra: `TETO_TABELA_NA_MAO` conta as DUAS formas e
/// substituiu o teto de `<table>` cru, que esta aposentado. A comparacao com o
/// numero antigo se perde de proposito -- perder a serie e mais barato que
/// deixar «mudei a regua» virar porta para afrouxar, que foi exatamente o
/// risco que eu corri.
/// 03/09, 20h: **0**, e este e o numero em que o pedido 158 fecha.
///
/// Nao houve conversao em massa: houve CLASSIFICACAO. Quatro viraram grade
/// porque sao lista de dado de verdade -- o Profiler, as transacoes abertas,
/// o resultado de consulta da tela da Claude, e o ajudante `tabela()` que
/// morreu junto com o ultimo chamador dele. As outras 20 entraram em
/// [`ISENTAS`] com o motivo: formulario, ficha tecnica, previa, o pivot, e
/// uma que nem tabela e.
///
/// **Zero nao quer dizer «acabou a tela»**: quer dizer que nao ha mais tabela
/// a mao SEM MOTIVO. E a partir daqui a catraca e a mais dura que ja houve
/// aqui -- tabela nova sem grade e sem linha em `ISENTAS` reprova na hora.
///
/// Quem guarda a regua contra o zero por engano nao e esta catraca e sim
/// `o_conferidor_acha_o_que_promete`, que a exercita com fonte sintetica; e
/// quem guarda a isencao contra virar desculpa e `nenhuma_isencao_morta`.
pub const TETO_TABELA_NA_MAO: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nenhuma_tabela_nova_fora_do_padrao() {
        let na_mao = sem_motivo();
        // `is_empty()` e nao `len() <= TETO`: com a catraca em ZERO os dois
        // dizem a mesma coisa, e o clippy reprova o segundo por ser sempre
        // verdadeiro ou sempre falso. O TETO continua existindo porque e ele
        // que a mensagem cita e que a lei da casa proibe de subir.
        assert!(
            na_mao.is_empty(),
            "{} tabelas montadas na mao, e a catraca esta em {TETO_TABELA_NA_MAO}.\n\
             Tela nova usa `PhxGrid.criar` -- e se esta tabela nao e grade \
             (formulario, cartao de diagrama, legenda), ponha em ISENTAS com \
             o motivo.\nRelatorio: cargo run --example grades-fora-do-padrao \
             -p phxsql-server\nAs primeiras: {:?}",
            na_mao.len(),
            na_mao.iter().take(5).collect::<Vec<_>>()
        );
        // A guarda de PISO que existia aqui -- «sobraram muito menos que a
        // catraca, baixe-a no mesmo commit» -- foi APOSENTADA com a catraca em
        // zero, e nao esquecida: `>= 0.saturating_sub(10)` e sempre verdadeiro,
        // e o clippy a reprovou por isso. Ela existia para forcar a catraca a
        // descer junto da conversao, e em zero nao ha para onde descer.
        //
        // Quem guarda contra o zero POR ENGANO -- a regua quebrada medindo
        // nada -- e `o_conferidor_acha_o_que_promete`, que a exercita com
        // fonte sintetica e nao depende deste numero.
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

    /// O reconhecedor da chamada ao ajudante, nas duas armadilhas que ele
    /// existe para nao cair -- e que so apareceram medindo o arquivo de
    /// verdade, nao imaginando.
    #[test]
    fn o_ajudante_nao_conta_texto_nem_a_propria_declaracao() {
        const FONTE: &str = "\
function tabela(cabecas, linhas, montar) { return \"<table>\"; }\n\
function telaA() { return tabela([{t:\"nome\"}], linhas, montar); }\n\
function telaB() { avisar(`${n} tabela(s) têm coluna Sequence`); }\n\
function telaC() { return obj.tabela(x) + minhaTabela(y); }\n\
function telaD() { /* a tabela(...) daqui e comentario */ return 1; }\n";
        let achados = varrer("ui/teste.html", FONTE);
        let ajudante: Vec<_> = achados
            .iter()
            .filter(|t| t.forma == Forma::Ajudante)
            .collect();
        // DUAS, e nao uma: o `telaD` esta aqui para travar o limite declarado
        // em `chamadas_ao_ajudante` -- comentario no MEIO da linha nao e
        // reconhecido, e conta. O caso existe para que o limite seja uma
        // decisao escrita, e nao uma surpresa; no dia em que alguem ensinar o
        // reconhecedor a ver comentario inline, este numero cai para 1 e o
        // caso avisa que a promessa mudou.
        assert_eq!(ajudante.len(), 2, "achou: {ajudante:?}");
        assert_eq!(ajudante[0].funcao, "telaA");
        assert_eq!(ajudante[1].funcao, "telaD", "o limite declarado mudou");
        // A declaracao do ajudante monta `<table>` -- isso conta como
        // marcacao, e uma so.
        assert_eq!(
            achados
                .iter()
                .filter(|t| t.forma == Forma::Marcacao)
                .count(),
            1
        );
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

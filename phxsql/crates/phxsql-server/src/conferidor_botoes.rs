//! O conferidor dos BOTOES: quantos a tela tem, e quais a bateria EXERCITA.
//!
//! # Por que ele existe
//!
//! Ordem do dono, 05/09/2026: *«bateria de testes de todos os botoes»*. Para
//! cumprir isso e preciso primeiro saber quantos sao -- e esse numero nunca
//! tinha sido medido. A varredura ingenua diz um numero e erra: parte dos
//! botoes desta tela nasce em tempo de execucao, dentro de template literal,
//! e outra parte nem e `<button>` (e um `<span role="button">`).
//!
//! Esta casa ja mediu metade uma vez pelo mesmo motivo: o conferidor de
//! grades contava so `<table>` cru, nao via o ajudante `tabela(`, e o numero
//! pulou de 24 para 43 quando aprendeu a ver os dois. Numero de regua curta
//! nao e numero baixo -- e numero errado.
//!
//! # As duas formas de botao
//!
//! - **marcacao** -- um `<button>` escrito na tela (estatico ou dentro de
//!   crase);
//! - **papel** -- um elemento qualquer com `role="button"`, que para quem usa
//!   teclado e leitor de tela E um botao, e que uma varredura por `<button`
//!   nao ve.
//!
//! # A CHAVE, e por que nunca a frase
//!
//! O que identifica um botao aqui e a **chave**: o gancho pelo qual a bateria
//! consegue clica-lo. Nesta ordem:
//!
//! 1. `#id` -- o `id` literal;
//! 2. `[data-x="v"]` -- o primeiro `data-*` que nao seja da fabrica de
//!    idiomas (`data-txt*` e rotulo, nao gancho);
//! 3. `.classe` -- **so** a classe que o proprio codigo da interface usa como
//!    gancho (`querySelector`, `closest`, `matches`, `classList.contains`).
//!
//! O texto do botao **nunca** entra. Ele passa pelos seis idiomas da
//! [`crate::idiomas::FABRICA_TELA`], entao quem casa por frase quebra calado
//! no dia em que alguem melhorar a redacao -- ou quando a tela abre em ingles.
//! E a mesma lei que o [`crate::conferidor`] ja aplica aos textos.
//!
//! # A lista das classes-gancho sai do CODIGO, nao de uma lista digitada
//!
//! `botao`, `secundario`, `mini`, `incluir`, `alterar` e `excluir` sao
//! ESTILO -- a convencao das cinco cores da acao --, e nao identidade: casar
//! por elas daria por provado todo botao verde do sistema no dia em que
//! alguem clicasse um. Distinguir estilo de gancho por lista digitada
//! envelheceria calado, entao [`ganchos_de_classe`] deriva a lista do proprio
//! fonte: uma classe e gancho quando a interface a usa para ACHAR o elemento.
//! No dia em que uma classe nova virar gancho, ela entra sozinha.
//!
//! E a receita de um numero tambem envelhece: foi uma lista de arquivos
//! digitada que fez o rodape do dossie publicar 780 KiB quando eram 1.032.
//!
//! # O cruzamento: quem CLICOU, e nao quem mencionou
//!
//! A segunda leitura vem de [`EVIDENCIA`], um arquivo que a **bateria
//! escreve** numa corrida inteira: um ouvinte de captura no navegador anota a
//! chave de cada botao que recebeu clique de verdade. Nao e a lista dos
//! seletores que aparecem no fonte dos casos -- mencionar nao e clicar, e o
//! `passeio` clica ~112 botoes que nenhum seletor do fonte nomeia.
//!
//! Evidencia digitada a mao seria a porta dos fundos desta catraca, e
//! `nenhuma_chave_morta_na_evidencia` e quem a fecha: chave que a tela nao tem
//! mais reprova, do mesmo jeito que chave morta reprova na fabrica de idiomas.
//!
//! ```bash
//! cargo run --example botoes-sem-prova -p phxsql-server
//! ```

use crate::conferidor::FONTES;
use crate::conferidor_grades::declara_funcao;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Como o botao aparece no fonte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forma {
    /// `<button>` escrito na tela.
    Marcacao,
    /// Outro elemento com `role="button"` -- botao para quem usa teclado.
    Papel,
}

/// De onde saiu a chave. E a ordem da especificidade, da melhor para a pior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// `id` literal.
    Id,
    /// `data-*` que nao e da fabrica de idiomas.
    Dado,
    /// Classe que o proprio codigo usa como gancho.
    Classe,
}

/// Um botao da tela: onde esta, de quem e, por onde se clica.
#[derive(Debug, Clone)]
pub struct Botao {
    pub arquivo: &'static str,
    pub linha: usize,
    /// A funcao que o monta -- a MESMA regua do conferidor de grades, de
    /// proposito: duas copias divergiriam calado.
    pub funcao: String,
    pub forma: Forma,
    /// O seletor pelo qual a bateria o alcanca. `None` = botao sem gancho
    /// estavel, e isso e um achado por si so.
    pub chave: Option<String>,
    pub tipo: Option<Tipo>,
    /// `Some(motivo)` quando esta em [`DISPENSADOS`].
    pub dispensa: Option<&'static str>,
    /// TODOS os ganchos que este botao carrega -- `#id`, `[data-x]`,
    /// `[data-x="v"]` e cada `.classe`, inclusive as de estilo.
    ///
    /// A [`Botao::chave`] e um deles, escolhido pela especificidade. Esta
    /// lista existe para o outro lado: a evidencia grava o que o navegador
    /// viu no elemento clicado, e ali `.botao` e `.mini` vem junto. Sem saber
    /// que esses ganchos EXISTEM, a guarda de chave morta acusaria a
    /// evidencia inteira de estar velha.
    pub ganchos: Vec<String>,
}

/// Onde a bateria deposita o que ela realmente clicou.
///
/// Relativo a raiz do repositorio. O arquivo e GERADO -- ver o cabecalho
/// dele.
pub const EVIDENCIA: &str = "testes-web/botoes-exercitados.txt";

/// Botoes que NAO se exercitam, e o motivo de cada um.
///
/// Regra da casa: dispensa registrada e decisao, dispensa silenciosa e
/// esquecimento. Nada entra aqui por ser chato -- «derruba o servico» sozinho
/// nao basta, porque o caso `40` ja derruba a porta de dados e a levanta pela
/// web. Entra o que se PROVOU que nao da.
///
/// A chave e a mesma do [`Botao::chave`]. Para o botao SEM chave -- que nao
/// tem outro endereco -- use `fn:<nome da funcao>`, e essa forma alcanca **so**
/// os sem chave daquela funcao: uma dispensa por funcao varreria junto os
/// irmaos que tem chave e que ninguem dispensou, e dispensa que abrange mais
/// do que se leu e dispensa silenciosa com outro nome.
pub const DISPENSADOS: &[(&str, &str, &str)] = &[
    (
        "ui/telemetria.js",
        "fn:desenharCartao",
        "e o GEMEO DESLIGADO do `#tlmEncerrar`: a tela desenha um ou outro, e \
         este nasce `disabled` com o `title` dizendo por que. Botao que nasce \
         desligado nao recebe clique nenhum -- e por isso ele nunca teve id, e \
         e o unico botao sem chave da tela inteira",
    ),
    (
        "ui/multitela.js",
        "[data-acao=\"devolver\"]",
        "so existe DENTRO de uma janela do sistema destacada (`W.destacada`), \
         e essa janela a bateria nao consegue abrir: ela depende da permissao \
         `window-management`, que o Playwright 1.56 nao sabe conceder -- a \
         mesma limitacao que o caso `monitores` ja carrega escrita",
    ),
    (
        "ui/multitela.js",
        "[data-acao=\"pinar-janela\"]",
        "irmao do `devolver`: so nasce no ramo `W.destacada` do `pintarTira`, \
         pelo mesmo motivo e com a mesma limitacao do navegador",
    ),
    (
        "ui/index.html",
        "#btSair",
        "derruba a sessao. Ele TEM prova -- o caso `entrada` sai e volta --, e \
         esta aqui porque o `passeio` o tira do laco pelo mesmo motivo: \
         clicado no meio de uma varredura, o resto dela nao teria onde \
         acontecer",
    ),
];

/// As classes que a interface usa para ACHAR elemento -- e que por isso valem
/// como chave.
///
/// Sai do fonte, nunca de lista digitada: `querySelector`, `querySelectorAll`,
/// `closest`, `matches`, os atalhos `$`/`$$` da pagina, e as comparacoes
/// diretas de classe (`classList.contains`, `className ===`, `indexOf`) que a
/// grade usa no despacho por delegacao.
pub fn ganchos_de_classe() -> BTreeSet<String> {
    const ACHADORES: &[&str] = &[
        "querySelectorAll(",
        "querySelector(",
        "closest(",
        "matches(",
        "$$(",
        "$(",
    ];
    const COMPARADORES: &[&str] = &["classList.contains(", "className === ", "indexOf("];
    let mut fora = BTreeSet::new();
    for (_, fonte) in FONTES {
        for achador in ACHADORES {
            for pedaco in trechos_apos(fonte, achador) {
                for t in classes_do_seletor(&pedaco) {
                    fora.insert(t);
                }
            }
        }
        for comp in COMPARADORES {
            for pedaco in trechos_apos(fonte, comp) {
                for t in pedaco.split_whitespace() {
                    if let Some(t) = token_css(t) {
                        fora.insert(t.to_string());
                    }
                }
            }
        }
    }
    fora
}

/// O primeiro literal de string que vem depois de cada ocorrencia de `marca`.
///
/// Nao e analisador de JavaScript: e o bastante para ler o argumento de um
/// achador, que nesta base e sempre um literal simples. Quando nao e (o
/// seletor montado por concatenacao), o trecho sai truncado -- e truncado
/// pesa para o lado seguro: uma classe a MENOS na lista de ganchos vira um
/// botao a MAIS na conta de quem falta prova.
fn trechos_apos(fonte: &str, marca: &str) -> Vec<String> {
    let mut fora = Vec::new();
    let mut de = 0;
    while let Some(rel) = fonte[de..].find(marca) {
        let p = de + rel + marca.len();
        de = p;
        let resto = &fonte[p..];
        let mut cs = resto.char_indices().skip_while(|(_, c)| *c == ' ');
        let Some((i0, aspa)) = cs.next() else {
            continue;
        };
        if aspa != '"' && aspa != '\'' && aspa != '`' {
            continue;
        }
        let corpo = &resto[i0 + aspa.len_utf8()..];
        if let Some(fim) = corpo.find(aspa) {
            fora.push(corpo[..fim].to_string());
        }
    }
    fora
}

/// As classes citadas num seletor CSS.
fn classes_do_seletor(seletor: &str) -> Vec<String> {
    let mut fora = Vec::new();
    let mut de = 0;
    while let Some(rel) = seletor[de..].find('.') {
        let p = de + rel + 1;
        de = p;
        if let Some(t) = token_css(&seletor[p..]) {
            fora.push(t.to_string());
        }
    }
    fora
}

/// O pedaco inicial que ainda e nome de classe CSS.
///
/// Existe por causa da grade, que monta a marcacao por concatenacao de
/// strings: `class="phx-pg' + (ativo ? ...` deixa `phx-pg'` e `(ativo` como
/// "classes". Cortar no primeiro caractere invalido devolve `phx-pg` e joga
/// `(ativo` fora, que e o certo nos dois casos.
fn token_css(t: &str) -> Option<&str> {
    let fim = t
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        .unwrap_or(t.len());
    if fim == 0 {
        None
    } else {
        Some(&t[..fim])
    }
}

/// Os pedacos da etiqueta que sao APENAS nome de classe.
///
/// Existe so para [`Botao::ganchos`] -- ver o comentario la.
///
/// Corta a etiqueta nas aspas (as tres: dupla, simples e crase) e fica com os
/// pedacos formados so por letra, digito, `-`, `_` e espaco. Sem parar na
/// PARIDADE das aspas, de proposito: o texto de uma etiqueta montada em
/// JavaScript nao alterna literal/codigo de um jeito que se possa contar --
/// `class="tira-aba${sel ? " sel" : ""}"` deixa o « sel» exatamente na posicao
/// que um contador de paridade chamaria de codigo. O crivo de FORMA acerta os
/// dois casos e joga fora o resto: `: `, `}" draggable=` e
/// `tira-aba${` nao passam.
fn classes_soltas_da_etiqueta(tag: &str) -> Vec<String> {
    let mut fora = Vec::new();
    for pedaco in tag.split(['"', '\'', '`']) {
        let t = pedaco.trim();
        if t.is_empty()
            || !t
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ')
        {
            continue;
        }
        for tok in t.split_whitespace() {
            fora.push(tok.to_string());
        }
    }
    fora
}

/// Onde termina a etiqueta que comeca em `de`.
///
/// Precisa saber de aspas e de `${…}`: o `>` de uma seta (`x => y`) dentro de
/// uma interpolacao fecharia a etiqueta cedo demais e cortaria os atributos
/// que vem depois -- e o `id` desta base costuma vir DEPOIS do `class`.
fn fim_da_etiqueta(fonte: &str, de: usize) -> usize {
    let mut aspa: Option<char> = None;
    let mut chaves = 0usize;
    let mut anterior = '\0';
    for (i, c) in fonte[de..].char_indices() {
        let i = de + i;
        if let Some(a) = aspa {
            if c == a {
                aspa = None;
            }
        } else if c == '"' || c == '\'' {
            aspa = Some(c);
        } else if c == '{' && (anterior == '$' || chaves > 0) {
            // `${` ABRE a interpolacao; `{` de dentro dela so conta para
            // achar o `}` que a fecha. As duas somam no mesmo contador, e por
            // isso a condicao e uma so -- o clippy reprova, e com razao, as
            // duas escritas separadas com o mesmo corpo.
            chaves += 1;
        } else if c == '}' && chaves > 0 {
            chaves -= 1;
        } else if c == '>' && chaves == 0 {
            return i + 1;
        }
        anterior = c;
    }
    fonte.len()
}

/// Os atributos de uma etiqueta, na ordem em que foram escritos.
fn atributos(tag: &str) -> Vec<(String, String)> {
    let mut fora = Vec::new();
    let b: Vec<char> = tag.chars().collect();
    let mut i = 0;
    // Pula o `<nome`.
    while i < b.len() && !b[i].is_whitespace() {
        i += 1;
    }
    while i < b.len() {
        while i < b.len() && !(b[i].is_ascii_alphabetic() || b[i] == '_') {
            if b[i] == '>' {
                return fora;
            }
            i += 1;
        }
        let ini = i;
        while i < b.len()
            && (b[i].is_ascii_alphanumeric() || b[i] == '-' || b[i] == '_' || b[i] == ':')
        {
            i += 1;
        }
        if ini == i {
            i += 1;
            continue;
        }
        let nome: String = b[ini..i].iter().collect();
        while i < b.len() && b[i] == ' ' {
            i += 1;
        }
        if i >= b.len() || b[i] != '=' {
            fora.push((nome, String::new()));
            continue;
        }
        i += 1;
        while i < b.len() && b[i] == ' ' {
            i += 1;
        }
        if i >= b.len() || (b[i] != '"' && b[i] != '\'') {
            continue;
        }
        let aspa = b[i];
        i += 1;
        let ini = i;
        while i < b.len() && b[i] != aspa {
            i += 1;
        }
        fora.push((nome, b[ini..i].iter().collect()));
        i += 1;
    }
    fora
}

/// O valor sem interpolacao, ou `None` quando ele DEPENDE do dado.
///
/// `id="btSalvar"` e chave; `data-rowid="${l.rowid}"` nao e -- o valor muda a
/// cada linha. O gancho, nesse caso, e o ATRIBUTO (`[data-rowid]`), e e assim
/// que [`chave_de`] o trata.
fn literal(valor: &str) -> Option<String> {
    // `${…}` e a interpolacao de template literal. A aspa simples e a crase
    // sao o OUTRO jeito de o valor depender do dado, e ele so aparece na
    // grade, que monta a marcacao por concatenacao de strings simples:
    // `data-p="' + alvo + '"` nao e o valor `' + alvo + '`, e um valor que
    // muda a cada pagina. Sem esta recusa a chave sairia com pedaco de codigo
    // dentro -- e uma chave assim nao casa com clique nenhum, entao o botao
    // ficaria eternamente sem prova sem ninguem entender por que.
    if valor.contains("${")
        || valor.contains('\u{1}')
        || valor.contains('\'')
        || valor.contains('`')
    {
        return None;
    }
    let v = valor.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// A chave de um botao, na ordem de especificidade.
fn chave_de(ats: &[(String, String)], ganchos: &BTreeSet<String>) -> Option<(String, Tipo)> {
    if let Some((_, v)) = ats.iter().find(|(n, _)| n == "id") {
        if let Some(v) = literal(v) {
            return Some((format!("#{v}"), Tipo::Id));
        }
    }
    for (n, v) in ats {
        // `data-txt*` e a fabrica de idiomas: e ROTULO, nao gancho. Casar por
        // ele seria casar pela frase por um caminho torto.
        if !n.starts_with("data-") || n.starts_with("data-txt") {
            continue;
        }
        return Some(match literal(v) {
            Some(v) => (format!("[{n}=\"{v}\"]"), Tipo::Dado),
            None => (format!("[{n}]"), Tipo::Dado),
        });
    }
    if let Some((_, v)) = ats.iter().find(|(n, _)| n == "class") {
        for bruto in v.split_whitespace() {
            if let Some(t) = token_css(bruto) {
                if ganchos.contains(t) {
                    return Some((format!(".{t}"), Tipo::Classe));
                }
            }
        }
    }
    None
}

/// Varre um fonte e devolve todo botao que houver nele.
pub fn varrer(arquivo: &'static str, fonte: &str, ganchos: &BTreeSet<String>) -> Vec<Botao> {
    let mut fora: Vec<Botao> = Vec::new();
    let mut donos: Vec<(usize, String)> = vec![(0, String::from("(topo do arquivo)"))];
    let mut inicios: Vec<usize> = Vec::with_capacity(fonte.lines().count() + 1);
    {
        let mut pos = 0usize;
        for (n, linha) in fonte.lines().enumerate() {
            inicios.push(pos);
            if let Some(f) = declara_funcao(linha) {
                donos.push((n + 1, f.to_string()));
            }
            pos += linha.len() + 1;
        }
        inicios.push(pos);
    }
    let linha_de = |p: usize| match inicios.binary_search(&p) {
        Ok(i) => i + 1,
        Err(i) => i,
    };
    let dono_de = |l: usize| {
        donos
            .iter()
            .rev()
            .find(|(n, _)| *n <= l)
            .map(|(_, f)| f.clone())
            .unwrap_or_default()
    };

    let mut poe = |p: usize, forma: Forma, tag: &str| {
        let l = linha_de(p);
        // Comentario nao e tela. A mesma recusa do conferidor de grades, e
        // pelo mesmo motivo: a palavra aparece muito na prosa desta base.
        let inicio_linha = inicios[l - 1];
        let t = fonte[inicio_linha..p].trim_start();
        if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
            return;
        }
        let ats = atributos(tag);
        let mut crus = Vec::new();
        // A classe CONDICIONAL nao esta em atributo nenhum: ela mora dentro
        // de um literal solto, ora numa interpolacao
        // (`class="tira-aba${sel ? " sel" : ""}"`), ora numa concatenacao de
        // strings (`class="phx-fbtn' + (f ? " phx-fbtn-on" : "")`). Nenhum
        // analisador curto a alcanca sem virar analisador de JavaScript.
        //
        // Entao ela entra pelos LITERAIS da etiqueta -- e essa frouxidao e a
        // direcao segura AQUI, e so aqui: esta lista nao escolhe chave
        // nenhuma, ela so responde «este gancho existe na tela?» para a
        // guarda de evidencia velha. Um gancho a mais nela nunca da um botao
        // por provado; um a MENOS faria a guarda acusar de velha uma
        // evidencia recem-gravada, e guarda que grita sempre ninguem le.
        for t in classes_soltas_da_etiqueta(tag) {
            crus.push(format!(".{t}"));
        }
        for (n, v) in &ats {
            if n == "id" {
                if let Some(v) = literal(v) {
                    crus.push(format!("#{v}"));
                }
            } else if n == "class" {
                for bruto in v.split_whitespace() {
                    if let Some(t) = token_css(bruto) {
                        crus.push(format!(".{t}"));
                    }
                }
            } else if n.starts_with("data-") && !n.starts_with("data-txt") {
                crus.push(format!("[{n}]"));
                if let Some(v) = literal(v) {
                    crus.push(format!("[{n}=\"{v}\"]"));
                }
            }
        }
        let (chave, tipo) = match chave_de(&ats, ganchos) {
            Some((c, t)) => (Some(c), Some(t)),
            None => (None, None),
        };
        let funcao = dono_de(l);
        let dispensa = DISPENSADOS
            .iter()
            .find(|(a, alvo, _)| {
                *a == arquivo
                    && match alvo.strip_prefix("fn:") {
                        Some(f) => chave.is_none() && f == funcao,
                        None => Some(*alvo) == chave.as_deref(),
                    }
            })
            .map(|(_, _, porque)| *porque);
        fora.push(Botao {
            arquivo,
            linha: l,
            funcao,
            forma,
            chave,
            tipo,
            dispensa,
            ganchos: crus,
        });
    };

    // Via 1: `<button`.
    let mut de = 0;
    while let Some(rel) = fonte[de..].find("<button") {
        let p = de + rel;
        de = p + "<button".len();
        let depois = fonte[de..].chars().next().unwrap_or(' ');
        if depois.is_ascii_alphanumeric() || depois == '-' {
            continue; // `<buttonzinho` nao existe, mas a regua nao adivinha
        }
        let fim = fim_da_etiqueta(fonte, p);
        poe(p, Forma::Marcacao, &fonte[p..fim]);
    }

    // Via 2: `role="button"` em qualquer outro elemento. Anda para TRAS ate o
    // `<` que abre a etiqueta -- e o unico jeito de achar o elemento sem
    // varrer todo `<` de um arquivo que tem `a < b` em JavaScript.
    for aspa in ["\"", "'"] {
        let marca = format!("role={aspa}button{aspa}");
        let mut de = 0;
        while let Some(rel) = fonte[de..].find(&marca) {
            let p = de + rel;
            de = p + marca.len();
            let Some(ini) = fonte[..p].rfind('<') else {
                continue;
            };
            let nome: String = fonte[ini + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if nome.is_empty() || nome == "button" {
                continue;
            }
            let fim = fim_da_etiqueta(fonte, ini);
            if fim <= p {
                continue; // o `role` ficou fora da etiqueta: nao era ela
            }
            poe(ini, Forma::Papel, &fonte[ini..fim]);
        }
    }

    fora.sort_by_key(|b| b.linha);
    fora
}

/// Todo botao de toda a interface servida.
pub fn conferir() -> Vec<Botao> {
    let ganchos = ganchos_de_classe();
    FONTES
        .iter()
        .flat_map(|(arq, fonte)| varrer(arq, fonte, &ganchos))
        .collect()
}

/// A raiz do repositorio, a partir do `Cargo.toml` deste pacote.
fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// As chaves que a bateria REALMENTE clicou, lidas de [`EVIDENCIA`].
///
/// Vazio quando o arquivo nao existe -- e ai a catraca reprova tudo, que e o
/// certo: sem evidencia nao ha prova.
pub fn exercitados() -> BTreeSet<String> {
    let Ok(texto) = std::fs::read_to_string(raiz().join(EVIDENCIA)) else {
        return BTreeSet::new();
    };
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .map(str::to_string)
        .collect()
}

/// Um botao esta provado quando a bateria clicou a chave dele.
pub fn provado(b: &Botao, clicados: &BTreeSet<String>) -> bool {
    let Some(chave) = &b.chave else {
        return false;
    };
    if clicados.contains(chave) {
        return true;
    }
    // `[data-jan="fechar"]` tambem conta quando a bateria clicou algum
    // `[data-jan]` -- a gravacao guarda o valor, entao isto so acontece com
    // valor interpolado, em que o valor E o dado e nao o botao.
    match chave.split_once("=\"") {
        Some((atr, _)) => clicados.contains(&format!("{atr}]")),
        None => false,
    }
}

/// Todo gancho que algum botao da tela carrega hoje.
///
/// E o outro lado da [`Botao::chave`]: a chave e o gancho ESCOLHIDO, esta e a
/// lista inteira. Serve para julgar a evidencia, que grava o que o navegador
/// viu no elemento e nao o que esta regua teria escolhido.
pub fn ganchos_vivos() -> BTreeSet<String> {
    conferir().into_iter().flat_map(|b| b.ganchos).collect()
}

/// Os que faltam: nem clicados, nem dispensados com motivo.
pub fn sem_prova() -> Vec<Botao> {
    let clicados = exercitados();
    conferir()
        .into_iter()
        .filter(|b| b.dispensa.is_none() && !provado(b, &clicados))
        .collect()
}

/// A catraca dos botoes sem prova. **So desce, e NUNCA sobe.**
///
/// Nasceu em 05/09/2026 valendo **211**, o numero medido do dia pelo
/// `--example botoes-sem-prova` depois das tres primeiras levas de casos. O
/// dia comecou em **268** de 298 botoes: a bateria inteira clicava 28, e hoje
/// clica 85.
///
/// Escreveu caso novo? Rode a bateria INTEIRA -- so ela reescreve a evidencia
/// -- e baixe o teto no mesmo commit: catraca frouxa nao segura nada. Botao
/// que nao se exercita entra em [`DISPENSADOS`] com o motivo, e ai o teto
/// desce do mesmo jeito.
///
/// # Se a regua passar a enxergar mais, esta catraca APOSENTA
///
/// Nao se sobe um teto com o motivo escrito ao lado: isso ja foi decidido e
/// recusado pelo dono quando o conferidor de grades aprendeu a ver o ajudante
/// `tabela(` e o numero pulou de 24 para 43. Regua que passa a medir mais faz
/// nascer uma catraca NOVA (`..._V2`), no numero medido do dia, dizendo no
/// nome e no comentario que substitui esta. A serie com o passado se perde de
/// proposito -- perder a comparacao e mais barato que deixar «mudei a regua»
/// virar a porta pela qual se afrouxa uma catraca.
pub const TETO_BOTAO_SEM_PROVA: usize = 211;

#[cfg(test)]
mod testes {
    use super::*;

    fn ganchos_de_teste(quais: &[&str]) -> BTreeSet<String> {
        quais.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nenhum_botao_novo_sem_prova() {
        let faltam = sem_prova();
        assert!(
            faltam.len() <= TETO_BOTAO_SEM_PROVA,
            "{} botoes sem prova, e a catraca esta em {TETO_BOTAO_SEM_PROVA}.\n\
             Botao novo entra na bateria (`testes-web/casos/`) -- e se ele nao \
             se exercita, entra em DISPENSADOS com o motivo.\n\
             Relatorio: cargo run --example botoes-sem-prova -p phxsql-server\n\
             Os primeiros: {:?}",
            faltam.len(),
            faltam
                .iter()
                .take(5)
                .map(|b| format!("{}:{} {:?}", b.arquivo, b.linha, b.chave))
                .collect::<Vec<_>>()
        );
        // O PISO, que e o que obriga a catraca a descer junto da conversao.
        // Sem ele quem escreve dez casos deixa o teto onde estava, e a catraca
        // volta a nao segurar nada.
        assert!(
            faltam.len() + 15 >= TETO_BOTAO_SEM_PROVA,
            "sobraram {} botoes sem prova e a catraca esta em \
             {TETO_BOTAO_SEM_PROVA}: baixe o teto no mesmo commit",
            faltam.len()
        );
    }

    /// A prova de que o conferidor PEGA. Sem ela ele pode estar medindo zero
    /// por estar quebrado, e um zero por engano e pior que um numero alto.
    #[test]
    fn o_conferidor_acha_o_que_promete() {
        const FONTE: &str = "\
function telaQualquer() {\n\
  return `<button class=\"botao incluir\" id=\"btNovo\">Novo</button>\n\
    <button class=\"botao mini\">sem gancho</button>\n\
    <span class=\"tira-x\" role=\"button\" data-x=\"${i}\">×</span>`;\n\
}\n";
        let g = ganchos_de_teste(&["tira-x"]);
        let achados = varrer("ui/teste.html", FONTE, &g);
        assert_eq!(achados.len(), 3, "achou: {achados:?}");
        assert_eq!(achados[0].chave.as_deref(), Some("#btNovo"));
        assert_eq!(achados[0].tipo, Some(Tipo::Id));
        assert_eq!(achados[0].funcao, "telaQualquer");
        // `botao mini` e ESTILO, nao gancho: sem chave, e isso e um achado.
        assert_eq!(achados[1].chave, None);
        // O `role="button"` num `<span>` conta, e a chave vem do atributo
        // porque o VALOR e dado interpolado.
        assert_eq!(achados[2].forma, Forma::Papel);
        assert_eq!(achados[2].chave.as_deref(), Some("[data-x]"));
        // Os ganchos CRUS trazem tambem a classe de estilo, que nao e chave
        // de ninguem e mesmo assim aparece na evidencia do navegador.
        assert!(achados[1].ganchos.contains(&".mini".to_string()));
    }

    /// A lei que decide se este conferidor serve ou engana: a identidade e a
    /// CHAVE, nunca a frase. Dois botoes com o mesmo texto e ids diferentes
    /// sao dois; o mesmo id com o texto traduzido continua sendo um.
    #[test]
    fn a_chave_nao_e_a_frase() {
        let g = ganchos_de_teste(&[]);
        let dois = varrer(
            "ui/teste.html",
            "<button id=\"btA\">Salvar</button><button id=\"btB\">Salvar</button>",
            &g,
        );
        assert_eq!(dois.len(), 2);
        assert_ne!(dois[0].chave, dois[1].chave);

        let pt = varrer("ui/teste.html", "<button id=\"btA\">Salvar</button>", &g);
        let de = varrer("ui/teste.html", "<button id=\"btA\">Speichern</button>", &g);
        assert_eq!(pt[0].chave, de[0].chave, "a traducao mudou a identidade");
    }

    /// O `id` desta base costuma vir DEPOIS do `class`, e o `class` costuma
    /// carregar `${…}` com seta dentro. Fechar a etiqueta no primeiro `>`
    /// perderia o `id` e o botao viraria «sem chave» calado.
    #[test]
    fn a_interpolacao_nao_fecha_a_etiqueta_cedo() {
        let g = ganchos_de_teste(&[]);
        let a = varrer(
            "ui/teste.html",
            "`<button class=\"${l.map(x => x > 1 ? \"a\" : \"b\")}\" id=\"btTarde\">ok</button>`",
            &g,
        );
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].chave.as_deref(), Some("#btTarde"));
    }

    /// A classe so vale como chave quando o CODIGO a usa para achar elemento.
    /// Sem isso, `class="botao"` daria por provado todo botao do sistema.
    #[test]
    fn classe_de_estilo_nao_e_chave() {
        let fonte = "<button class=\"botao secundario\">x</button>";
        assert_eq!(
            varrer("ui/teste.html", fonte, &ganchos_de_teste(&[]))[0].chave,
            None
        );
        assert_eq!(
            varrer("ui/teste.html", fonte, &ganchos_de_teste(&["secundario"]))[0]
                .chave
                .as_deref(),
            Some(".secundario")
        );
    }

    /// A lista de ganchos sai do codigo: `.phx-fbtn` e gancho porque a grade
    /// o procura, e `botao` nao e porque ninguem procura por ele.
    #[test]
    fn a_lista_de_ganchos_sai_do_codigo() {
        let g = ganchos_de_classe();
        assert!(g.contains("phx-fbtn"), "a grade procura `.phx-fbtn`");
        assert!(g.contains("item"), "o menu procura `.menubar .item`");
        assert!(
            !g.contains("secundario"),
            "`secundario` e estilo: ninguem procura por ele"
        );
    }

    /// Dispensa morta e pior que dispensa faltando: o proximo leitor confia
    /// nela e deixa de escrever o caso que ja daria para escrever.
    #[test]
    fn nenhuma_dispensa_morta() {
        let todos = conferir();
        for (arq, alvo, porque) in DISPENSADOS {
            assert!(
                todos.iter().any(|b| b.arquivo == *arq
                    && match alvo.strip_prefix("fn:") {
                        Some(f) => b.chave.is_none() && f == b.funcao,
                        None => Some(*alvo) == b.chave.as_deref(),
                    }),
                "DISPENSADOS diz que `{alvo}` de `{arq}` existe ({porque}), e \
                 nao existe mais botao nenhum ali. Tire a linha"
            );
        }
    }

    /// Chave morta na evidencia e a porta dos fundos desta catraca: bastaria
    /// digitar uma linha no arquivo para dar um botao por provado. Aqui todo
    /// gancho gravado tem de existir num botao que a tela AINDA tem.
    ///
    /// Contra os GANCHOS e nao contra as chaves, e o motivo e medido: a
    /// evidencia grava o que o navegador viu no elemento clicado, e ali vem
    /// `.botao`, `.mini` e o `data-i="7"` do item de menu -- ganchos
    /// legitimos que nao sao a chave de ninguem. Comparar so com as chaves
    /// acusava 105 linhas boas de estarem velhas, e uma guarda que grita
    /// sempre e uma guarda que ninguem le. O buraco que ela fecha continua
    /// fechado: `#btInventado` nao e gancho de botao nenhum, e o `#btAntigo`
    /// de um botao renomeado deixa de ser no mesmo commit.
    #[test]
    fn nenhuma_chave_morta_na_evidencia() {
        let vivas = ganchos_vivos();
        let mortas: Vec<_> = exercitados()
            .into_iter()
            .filter(|c| {
                !vivas.contains(c)
                    && match c.split_once("=\"") {
                        // `[data-i="7"]` e o item 7 do menu: o valor nasce do
                        // dado, entao o fonte so tem `[data-i]`.
                        Some((atr, _)) => !vivas.contains(&format!("{atr}]")),
                        None => true,
                    }
            })
            .collect();
        assert!(
            mortas.is_empty(),
            "a evidencia grava clique em botao que a tela nao tem mais: {mortas:?}.\n\
             Rode a bateria inteira de novo -- ela reescreve o arquivo"
        );
    }
}

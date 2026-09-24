//! O conferidor da TELA: simbolo Unicode no lugar de icone, cor fora de token,
//! icone pedido sem desenho e o vermelho da marca virando texto.
//!
//! # Por que ele existe
//!
//! Quatro perguntas de designer que ler o codigo nao responde e que a tela so
//! mostra depois do estrago:
//!
//! - **Simbolo pictografico.** O Centro de Controle desenhava icone com o
//!   caractere Unicode (o banco `⛁`, o escudo `⛨`, a corrente `⛓`, a lixeira
//!   `🗑`...). Quem desenha esse caractere e a FONTE de quem abre, nao nos: no
//!   Windows varios viram quadradinho, noutros viram emoji colorido, e nenhum
//!   segue o traco nem a cor da marca. Medido em 24/09/2026, antes da troca:
//!   **346 ocorrencias de 69 simbolos** na `ui/` (comentario incluido), e o
//!   conjunto de icones da casa (`<svg class="sprite">` no `index.html`,
//!   chamado por `icone("nome")`) nasceu para substitui-los.
//! - **Cor solta.** A paleta da marca vive em tokens (`--fundo`, `--laranja`,
//!   `--acao-incluir`...), e o tema claro troca os tokens. Uma cor escrita
//!   direto na regra nao troca com o tema -- e foi exatamente isso que a
//!   primeira captura da grade mostrou: um `#eceff2` fixo na folha da PhxGrid
//!   desenhava um fio BRANCO entre cada linha no meio da pagina preta.
//! - **Icone sem desenho.** `icone("nome")` com um nome que o sprite nao tem
//!   nao da erro: desenha um quadrado vazio, calado.
//! - **Vermelho da marca como texto.** `#D71A1A` da 3,9:1 sobre o `#010418`, e
//!   o vinho `#8B0D0D` da 2,1:1 -- abaixo dos 4,5:1 de texto. Regra da marca:
//!   os dois so preenchem ou contornam, nunca pintam letra.
//!
//! # O que conta, e o que NAO conta -- as decisoes
//!
//! - **Comentario nao conta.** Seta num comentario (`campo → valor`) e prosa
//!   de programador, nao tela. O descasque e de linha (`//` precedido de
//!   espaco, nunca `://`, que e URL), de bloco (`/* */`) e de HTML (`<!-- -->`).
//! - **Simbolo que continua contando, e por que ficou** (a catraca inclui
//!   estes, de proposito, porque sao a lista do que ainda se pode trocar):
//!   - dentro de TEXTO DA FABRICA (`txt("tela.voltar", "← Voltar")` e as seis
//!     linguas no `idiomas.rs`): tirar a seta do texto de fabrica nao tira a
//!     seta da linha ja semeada na `phxsys.mensagens` de quem instalou, e a
//!     tela sairia com icone E seta. Trocar pede migracao do texto, nao CSS;
//!   - FORMA DE ESTADO (`● ▲ ✕ ○ ■`): e o que o daltonico le sem a cor, dentro
//!     de texto e de `<text>` de SVG, onde `<use>` nao entra;
//!   - SETA EM FRASE (`Arquivo → Novo database`, `Primary → Replica`, `A → B`),
//!     e o `↓`/`↑` das taxas: e pontuacao da frase, nao icone;
//!   - a estrela do titulo de coluna (`nome ★`): o titulo e texto puro da
//!     PhxGrid, e texto nao carrega `<svg>`.
//! - **Cor que nao conta:** a que nasce num token (`--nome: #hex`, onde quer
//!   que esteja -- `:root`, tema claro, bloco `.phx-grid`, `@media print`) e as
//!   cores das BANDEIRAS de idioma, que sao dado de terceiro (a bandeira do
//!   Brasil e verde e amarela em qualquer tema).
//!
//! ```bash
//! cargo run --example simbolos-e-cores -p phxsql-server
//! ```

use std::collections::BTreeSet;

/// Os arquivos de interface, pelo mesmo `include_str!` que o servidor usa --
/// o conferidor mede a pagina que o binario serve, nao uma copia.
///
/// Lista PROPRIA, e nao a `conferidor::FONTES`: esta pergunta alcanca tambem
/// as folhas de estilo, e ampliar a `FONTES` moveria a catraca dos idiomas.
pub const FONTES: &[(&str, &str)] = &[
    ("ui/index.html", include_str!("../ui/index.html")),
    ("ui/claude.js", include_str!("../ui/claude.js")),
    ("ui/telemetria.js", include_str!("../ui/telemetria.js")),
    ("ui/telemetria.css", include_str!("../ui/telemetria.css")),
    ("ui/diagrama-er.js", include_str!("../ui/diagrama-er.js")),
    ("ui/multitela.js", include_str!("../ui/multitela.js")),
    ("ui/multitela.css", include_str!("../ui/multitela.css")),
    ("ui/explorador.html", include_str!("../ui/explorador.html")),
    ("ui/explorador.js", include_str!("../ui/explorador.js")),
    ("ui/explorador.css", include_str!("../ui/explorador.css")),
    (
        "ui/grid/phx-grid.js",
        include_str!("../ui/grid/phx-grid.js"),
    ),
    (
        "ui/grid/phx-grid.css",
        include_str!("../ui/grid/phx-grid.css"),
    ),
];

/// Um achado: arquivo, linha e o trecho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    pub arquivo: &'static str,
    pub linha: usize,
    pub trecho: String,
}

/// O simbolo e pictografico -- desenho, nao letra?
///
/// Faixas do Unicode, e nao lista de caracteres: um simbolo novo da mesma
/// familia (outra seta, outro dingbat, outro emoji) cai aqui sem ninguem
/// lembrar de acrescenta-lo. Setas (2190-21FF e 2900-297F), tecnicos
/// (2300-23FF), OCR (2440-245F), formas geometricas, simbolos diversos e
/// dingbats (25A0-27BF), matematicos diversos B (2980-29FF), setas e simbolos
/// diversos (2B00-2BFF) e todo o plano dos emoji (1F000+). Dos operadores
/// matematicos so os circulados e quadrados (2295-22BF), o `∿` e o `№`, que
/// eram icone de menu -- o resto da faixa (`×`, `≤`, `−`) e texto de verdade.
pub fn pictografico(c: char) -> bool {
    let o = c as u32;
    matches!(o,
        0x2190..=0x21FF
        | 0x2295..=0x22BF
        | 0x2300..=0x23FF
        | 0x2440..=0x245F
        | 0x25A0..=0x27BF
        | 0x2900..=0x29FF
        | 0x2B00..=0x2BFF
        | 0x1F000..)
        || o == 0x223F
        || o == 0x2116
}

/// Troca todo comentario por espaco, guardando as quebras de linha -- assim a
/// linha do achado continua a do arquivo.
///
/// Nao e um analisador de JavaScript, e nao precisa ser: os tres formatos de
/// comentario desta pasta sao estes, e o `//` so vale precedido de espaco ou
/// comeco de linha (o `https://` de uma URL fica). String com `/*` dentro
/// faria o descasque errar para MENOS, nunca para mais -- e a prova
/// `o_descasque_nao_come_url_nem_texto` trava o caso que existe.
pub fn sem_comentarios(fonte: &str) -> String {
    let b: Vec<char> = fonte.chars().collect();
    let abre = |i: usize, s: &str| s.chars().enumerate().all(|(k, c)| b.get(i + k) == Some(&c));
    let apaga = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut saida = String::with_capacity(fonte.len());
    let mut i = 0;
    while i < b.len() {
        let fecho = if abre(i, "<!--") {
            Some("-->")
        } else if abre(i, "/*") {
            Some("*/")
        } else {
            None
        };
        if let Some(fecho) = fecho {
            while i < b.len() && !abre(i, fecho) {
                saida.push(apaga(b[i]));
                i += 1;
            }
            for _ in 0..fecho.len().min(b.len() - i) {
                saida.push(' ');
                i += 1;
            }
        } else if abre(i, "//") && (i == 0 || b[i - 1].is_whitespace()) {
            while i < b.len() && b[i] != '\n' {
                saida.push(' ');
                i += 1;
            }
        } else {
            saida.push(b[i]);
            i += 1;
        }
    }
    saida
}

/// Os simbolos pictograficos de UM fonte, fora de comentario. Conta tambem a
/// entidade numerica (`&#128274;`), que e o mesmo caractere escrito de outro
/// jeito -- foi assim que o cadeado da tela de entrada escapou de uma busca
/// por caractere.
pub fn simbolos_de(arquivo: &'static str, fonte: &str) -> Vec<Achado> {
    let limpo = sem_comentarios(fonte);
    let mut achados = Vec::new();
    for (n, linha) in limpo.lines().enumerate() {
        for c in linha.chars().filter(|c| pictografico(*c)) {
            achados.push(Achado {
                arquivo,
                linha: n + 1,
                trecho: c.to_string(),
            });
        }
        let mut resto = linha;
        while let Some(i) = resto.find("&#") {
            let dep = &resto[i + 2..];
            let fim = dep.find(';').unwrap_or(0);
            if let Some(c) = dep[..fim].parse::<u32>().ok().and_then(char::from_u32) {
                if pictografico(c) {
                    achados.push(Achado {
                        arquivo,
                        linha: n + 1,
                        trecho: format!("&#{};", &dep[..fim]),
                    });
                }
            }
            resto = &resto[i + 2..];
        }
    }
    achados
}

/// Todos os simbolos pictograficos da `ui/`.
pub fn simbolos() -> Vec<Achado> {
    FONTES.iter().flat_map(|(a, f)| simbolos_de(a, f)).collect()
}

/// Onde a cor e dado de terceiro, e por isso nao vira token: o objeto das
/// bandeiras de idioma. Do marcador ate o `};` que o fecha.
pub const ISENCAO_DE_COR: (&str, &str) = (
    "const BANDEIRAS = {",
    "a bandeira e dado de terceiro: verde e amarela em qualquer tema",
);

/// As cores escritas FORA de token num fonte: `#hex` que nao esta numa
/// declaracao `--nome: ...`, fora de comentario e fora da isencao.
///
/// O casador pede o hexadecimal inteiro numa caixa so (`#fff`, `#C63C0A`),
/// porque `#cDb` -- o seletor do campo de banco no JavaScript -- tambem e
/// hexadecimal valido, e nenhuma cor desta pasta mistura caixa.
pub fn cores_soltas_de(arquivo: &'static str, fonte: &str) -> Vec<Achado> {
    let mut limpo = sem_comentarios(fonte);
    if let Some(i) = limpo.find(ISENCAO_DE_COR.0) {
        let fim = limpo[i..].find("\n};").map_or(limpo.len(), |f| i + f);
        let branco: String = limpo[i..fim]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect();
        limpo.replace_range(i..fim, &branco);
    }
    let b = limpo.as_bytes();
    let mut achados = Vec::new();
    let mut linha = 1;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\n' {
            linha += 1;
        }
        if b[i] == b'#' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'&')) {
            let n = b[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            let depois = b.get(i + 1 + n).copied().unwrap_or(b' ');
            let hex = &limpo[i + 1..i + 1 + n];
            let uma_caixa = hex == hex.to_lowercase() || hex == hex.to_uppercase();
            if matches!(n, 3 | 4 | 6 | 8)
                && !depois.is_ascii_alphanumeric()
                && depois != b'_'
                && depois != b'-'
                && uma_caixa
                && !em_token(&limpo[..i])
            {
                achados.push(Achado {
                    arquivo,
                    linha,
                    trecho: format!("#{hex}"),
                });
            }
            i += 1 + n;
            continue;
        }
        i += 1;
    }
    achados
}

/// A cor que termina em `antes` esta sendo DEFINIDA num token? Volta ate o
/// comeco da declaracao (`;`, `{` ou `}`) e pergunta se ela abre com `--`.
fn em_token(antes: &str) -> bool {
    let comeco = antes.rfind([';', '{', '}']).map_or(0, |p| p + 1);
    antes[comeco..].trim_start().starts_with("--")
}

/// Todas as cores soltas da `ui/`.
pub fn cores_soltas() -> Vec<Achado> {
    FONTES
        .iter()
        .flat_map(|(a, f)| cores_soltas_de(a, f))
        .collect()
}

/// Os desenhos do sprite: cada `<symbol id="i-nome">` do `index.html`.
pub fn desenhos() -> BTreeSet<String> {
    let mut nomes = BTreeSet::new();
    let (_, pagina) = FONTES[0];
    let mut resto = pagina;
    while let Some(i) = resto.find("<symbol id=\"i-") {
        let dep = &resto[i + 14..];
        if let Some(f) = dep.find('"') {
            nomes.insert(dep[..f].to_string());
        }
        resto = dep;
    }
    nomes
}

/// Os nomes de icone pedidos por LITERAL: `icone("nome")`, `ico:"nome"` e
/// `href="#i-nome"`. O nome montado em tempo de execucao
/// (`icone(\`regioes-${k}\`)`) nao aparece aqui -- esse se prova exercitando.
pub fn pedidos() -> Vec<Achado> {
    let mut achados = Vec::new();
    for (arquivo, fonte) in FONTES {
        let limpo = sem_comentarios(fonte);
        for (n, linha) in limpo.lines().enumerate() {
            for marca in ["icone(\"", "ico:\"", "ico: \"", "href=\"#i-"] {
                let mut resto = linha;
                while let Some(i) = resto.find(marca) {
                    let dep = &resto[i + marca.len()..];
                    let fim = dep.find('"').unwrap_or(0);
                    let nome = &dep[..fim];
                    // `ico:"…"` com nome que nao e de icone (vazio, `${…}`)
                    // nao e pedido ao sprite.
                    if !nome.is_empty()
                        && nome
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    {
                        achados.push(Achado {
                            arquivo,
                            linha: n + 1,
                            trecho: nome.to_string(),
                        });
                    }
                    resto = dep;
                }
            }
        }
    }
    achados
}

/// Pedidos cujo nome o sprite nao desenha.
pub fn icones_sem_desenho() -> Vec<Achado> {
    let tem = desenhos();
    pedidos()
        .into_iter()
        .filter(|p| !tem.contains(&p.trecho))
        .collect()
}

/// O vermelho e o vinho da marca, que nao passam de 4,5:1 sobre o fundo escuro.
pub const VERMELHOS_DA_MARCA: &[&str] = &["#d71a1a", "#8b0d0d"];

/// Declaracoes `color:` (a propriedade de TEXTO, e nao `background-color` nem
/// `border-color`) que usam o vermelho ou o vinho da marca -- direto ou por um
/// token cujo valor e um deles.
pub fn vermelho_da_marca_em_texto() -> Vec<Achado> {
    let mut tokens = BTreeSet::new();
    for (_, fonte) in FONTES {
        let limpo = sem_comentarios(fonte).to_lowercase();
        for decl in limpo.split([';', '{', '}']) {
            let d = decl.trim();
            if let Some((nome, valor)) = d.split_once(':') {
                if nome.starts_with("--") && VERMELHOS_DA_MARCA.iter().any(|v| valor.contains(v)) {
                    tokens.insert(nome.trim().to_string());
                }
            }
        }
    }
    let mut achados = Vec::new();
    for (arquivo, fonte) in FONTES {
        let limpo = sem_comentarios(fonte).to_lowercase();
        for (n, linha) in limpo.lines().enumerate() {
            for decl in linha.split([';', '{', '}', '"']) {
                let Some((nome, valor)) = decl.split_once(':') else {
                    continue;
                };
                if nome.trim() != "color" {
                    continue;
                }
                let direto = VERMELHOS_DA_MARCA.iter().any(|v| valor.contains(v));
                let por_token = tokens.iter().any(|t| valor.contains(&format!("var({t}")));
                if direto || por_token {
                    achados.push(Achado {
                        arquivo,
                        linha: n + 1,
                        trecho: decl.trim().to_string(),
                    });
                }
            }
        }
    }
    achados
}

/// Simbolos pictograficos que sobraram na `ui/`. **So desce.**
///
/// Nasceu em 24/09/2026 no numero medido DEPOIS da troca pelo conjunto de
/// icones da casa: o que sobrou e a lista do cabecalho deste arquivo -- texto
/// de fabrica, forma de estado, seta em frase. Trocou um? Baixe no mesmo commit.
pub const TETO_SIMBOLOS_PICTOGRAFICOS: usize = 80;

/// Cores escritas fora de token na `ui/`. **So desce.** Nasceu em zero, no dia
/// em que a paleta fechou: cor nova entra como token, ou nao entra.
pub const TETO_CORES_SOLTAS: usize = 0;

/// `icone("nome")` sem desenho no sprite. Zero, e zero e o numero certo: o
/// icone que falta nao da erro, desenha um quadrado vazio.
pub const TETO_ICONE_SEM_DESENHO: usize = 0;

/// O vermelho ou o vinho da marca pintando texto. Zero: regra da marca.
pub const TETO_VERMELHO_DA_MARCA_EM_TEXTO: usize = 0;

#[cfg(test)]
fn listar(achados: &[Achado]) -> String {
    achados
        .iter()
        .map(|a| format!("  {}:{}  {}", a.arquivo, a.linha, a.trecho))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn simbolo_pictografico_novo_reprova() {
        let s = simbolos();
        assert!(
            s.len() <= TETO_SIMBOLOS_PICTOGRAFICOS,
            "{} simbolo(s) pictografico(s) na ui/ (teto {TETO_SIMBOLOS_PICTOGRAFICOS}). Icone \
             se desenha pelo conjunto da casa: `icone(\"nome\")`, com o desenho no \
             `<svg class=\"sprite\">` do index.html.\n{}",
            s.len(),
            listar(&s)
        );
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn cor_solta_nova_reprova() {
        let c = cores_soltas();
        assert!(
            c.len() <= TETO_CORES_SOLTAS,
            "{} cor(es) fora de token na ui/ (teto {TETO_CORES_SOLTAS}). Cor escrita na regra \
             nao troca com o tema: crie ou reaproveite um token (`--nome: #hex`).\n{}",
            c.len(),
            listar(&c)
        );
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn todo_icone_pedido_tem_desenho() {
        let f = icones_sem_desenho();
        assert!(
            f.len() <= TETO_ICONE_SEM_DESENHO,
            "icone pedido sem `<symbol id=\"i-…\">` no sprite -- desenharia um quadrado \
             vazio, calado:\n{}",
            listar(&f)
        );
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn vermelho_da_marca_nao_pinta_texto() {
        let v = vermelho_da_marca_em_texto();
        assert!(
            v.len() <= TETO_VERMELHO_DA_MARCA_EM_TEXTO,
            "#D71A1A (3,9:1) e #8B0D0D (2,1:1) sobre o #010418 nao passam de 4,5:1 -- so \
             preenchem ou contornam:\n{}",
            listar(&v)
        );
    }

    // ------------------------------------------------ prova real dos casadores
    // Cada casador acusa o defeito reposto num texto SINTETICO. Sem isto, um
    // casador quebrado deixaria as quatro catracas verdes para sempre.

    #[test]
    fn o_casador_de_simbolo_acusa_caractere_entidade_e_poupa_comentario() {
        let f = "<button>\u{26C1} banco</button>\n<span>&#128274;</span>\n\
                 // \u{2192} seta de comentario\n/* \u{2605} */ texto × 2 ≤ 3";
        let a = simbolos_de("x", f);
        let t: Vec<_> = a.iter().map(|a| a.trecho.as_str()).collect();
        assert_eq!(t, vec!["\u{26C1}", "&#128274;"], "{a:?}");
        assert_eq!(a[1].linha, 2);
    }

    #[test]
    fn o_casador_de_cor_acusa_a_solta_e_poupa_token_seletor_e_isencao() {
        let f = ".a{color:#fff;--x:#123456}\n:root{--y: #ABCDEF; --z:var(--y)}\n\
                 $(\"#cDb\").value; /* #abc */ &#9998;\nconst BANDEIRAS = {\n  br:`fill=\"#009b3a\"`,\n};\n\
                 b{border:1px solid #C63C0A}";
        let a = cores_soltas_de("x", f);
        let t: Vec<_> = a.iter().map(|a| (a.linha, a.trecho.as_str())).collect();
        assert_eq!(t, vec![(1, "#fff"), (7, "#C63C0A")], "{a:?}");
    }

    #[test]
    fn o_descasque_nao_come_url_nem_texto() {
        let f = "a = \"https://x.y/z\"; // fora\nb <!-- c\nd --> e";
        let l = sem_comentarios(f);
        assert!(l.contains("https://x.y/z"), "{l}");
        assert!(!l.contains("fora") && !l.contains('c') && l.contains(" e"));
        assert_eq!(
            l.lines().count(),
            3,
            "a linha do achado tem de ser a do arquivo"
        );
    }

    #[test]
    fn o_sprite_e_os_pedidos_sao_lidos_de_verdade() {
        // Um casador que lesse zero desenhos e zero pedidos passaria o
        // `todo_icone_pedido_tem_desenho` sem medir nada.
        assert!(desenhos().len() >= 90, "{}", desenhos().len());
        assert!(pedidos().len() >= 150, "{}", pedidos().len());
        assert!(desenhos().contains("imprimir"));
    }
}

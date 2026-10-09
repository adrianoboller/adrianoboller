//! As regioes dobraveis de um arquivo, para o painel de leitura do IDE.
//!
//! Por que aqui e nao no editor: o editor do IDE e o Helix 25.07.1 num PTY, e ele nao tem
//! dobra (0 comandos `fold`, `foldingRange` ausente do cliente LSP dele -- pesquisa em
//! `docs/propostas/sp32-r5-r1-pesquisa.md`, hipotese (c)). Quem dobra e o painel de leitura da
//! tela, que mostra o arquivo EM DISCO em HTML; o Helix continua sendo o editor.
//!
//! A fonte das regioes, nesta ordem, e o que a resposta diz em `fonte`:
//! 1. o servidor de linguagem (`textDocument/foldingRange`), quando ha um para a extensao e
//!    ele responde no prazo -- e a regiao que o VS Code mostraria;
//! 2. as chaves (`{`/`[`), para as linguagens de chave, pulando texto e comentario;
//! 3. a indentacao, para o resto (Python, YAML, Markdown com lista...).
//!
//! A reserva segue a convencao do rust-analyzer com `lineFoldingOnly` (lida no fonte dele,
//! `to_proto::folding_range`, e medida no `tests/desktop/ide_dobra.mjs`: a funcao de 3 a 14
//! volta com fim 14, a linha da `}`): a regiao vai da linha que abre ate a linha que fecha,
//! MENOS quando ha texto depois do fechamento na mesma linha (`};`, `} else {`) -- ai para na
//! de cima, para nao esconder o `else`. Trocar de fonte nao muda onde a dobra acaba. Linhas a
//! partir de 1.

use serde::Serialize;
use serde_json::Value;

/// Teto de regioes devolvidas: arquivo gerado de milhares de blocos nao e leitura de gente, e
/// a lista inteira so pesaria na tela.
pub const TETO_DE_REGIOES: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Regiao {
    /// Linha que abre (a que continua visivel quando dobra), a partir de 1.
    pub inicio: u32,
    /// Ultima linha escondida quando dobra, a partir de 1; sempre maior que `inicio`.
    pub fim: u32,
    /// `comment`, `imports` ou `region` quando o servidor diz; a reserva nao diz.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tipo: Option<String>,
}

/// As regioes da resposta do servidor (`FoldingRange[]`, linhas a partir de 0), cortadas ao
/// tamanho do texto que a tela mostra: o servidor le o disco por conta propria e um arquivo
/// que encolheu entre as duas leituras nao pode dar faixa fora do texto.
pub fn do_lsp(v: &Value, total_de_linhas: u32) -> Vec<Regiao> {
    let lista = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let regioes = lista.iter().filter_map(|r| {
        let inicio = r["startLine"].as_u64()? as u32 + 1;
        let fim = (r["endLine"].as_u64()? as u32 + 1).min(total_de_linhas);
        (fim > inicio).then(|| Regiao {
            inicio,
            fim,
            tipo: r["kind"].as_str().map(str::to_string),
        })
    });
    arrumar(regioes.collect())
}

/// Uma regiao por linha que abre (a maior, como o VS Code faz), em ordem, ate o teto.
fn arrumar(mut v: Vec<Regiao>) -> Vec<Regiao> {
    v.sort_by(|a, b| a.inicio.cmp(&b.inicio).then(b.fim.cmp(&a.fim)));
    v.dedup_by(|b, a| a.inicio == b.inicio);
    v.truncate(TETO_DE_REGIOES);
    v
}

/// Extensoes em que bloco e chave. O resto cai na indentacao.
const DE_CHAVE: &[&str] = &[
    "rs", "c", "h", "cc", "cpp", "hpp", "cs", "java", "kt", "kts", "scala", "go", "swift", "js",
    "mjs", "cjs", "jsx", "ts", "tsx", "json", "jsonc", "css", "scss", "less", "php", "dart", "zig",
    "proto", "gradle", "groovy",
];

/// A reserva quando o servidor nao responde: chaves para linguagem de chave, indentacao para
/// o resto -- e indentacao tambem quando a de chave nao achou bloco nenhum (um `.json` de uma
/// linha so, um `.c` so de declaracoes). Devolve a fonte usada, que vai na resposta.
pub fn reserva(caminho: &str, texto: &str) -> (&'static str, Vec<Regiao>) {
    let ext = std::path::Path::new(caminho)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if DE_CHAVE.contains(&ext.as_str()) {
        let r = por_chaves(texto, ext != "rs");
        if !r.is_empty() {
            return ("chaves", r);
        }
    }
    ("indentacao", por_indentacao(texto))
}

/// Blocos `{ }` e `[ ]` que abrem numa linha e fecham noutra. Pula texto entre aspas (com
/// escape), `//` e `/* */`. A regiao inclui a linha que fecha quando o fechamento e o fim do
/// que ha nela (a convencao do rust-analyzer, no topo do modulo). `aspas_simples_sao_texto`: em Rust `'a` e tempo de vida, entao
/// so `'x'` e `'\n'` contam como caractere; nas outras, `'...'` e texto.
pub fn por_chaves(texto: &str, aspas_simples_sao_texto: bool) -> Vec<Regiao> {
    let cs: Vec<char> = texto.chars().collect();
    let mut linha: u32 = 1;
    // (caractere que abriu, linha onde abriu)
    let mut pilha: Vec<(char, u32)> = Vec::new();
    let mut saida = Vec::new();
    let mut i = 0;
    // Pula ate o fim do texto entre `fim`, contando as linhas que atravessa.
    let pular_texto = |i: &mut usize, linha: &mut u32, fim: char| {
        *i += 1;
        while *i < cs.len() {
            match cs[*i] {
                '\\' => *i += 1,
                '\n' => *linha += 1,
                c if c == fim => break,
                _ => {}
            }
            *i += 1;
        }
    };
    while i < cs.len() {
        let c = cs[i];
        match c {
            '\n' => linha += 1,
            '/' if cs.get(i + 1) == Some(&'/') => {
                while i < cs.len() && cs[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if cs.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < cs.len() && !(cs[i] == '*' && cs.get(i + 1) == Some(&'/')) {
                    if cs[i] == '\n' {
                        linha += 1;
                    }
                    i += 1;
                }
                i += 2;
                continue;
            }
            // Texto cru do Rust: r"..." e r#"..."# -- sem escape, fecha com o mesmo numero de #.
            'r' if !aspas_simples_sao_texto
                && (i == 0 || !(cs[i - 1].is_alphanumeric() || cs[i - 1] == '_'))
                && matches!(cs.get(i + 1), Some('"') | Some('#')) =>
            {
                let mut j = i + 1;
                let mut cerquilhas = 0;
                while cs.get(j) == Some(&'#') {
                    cerquilhas += 1;
                    j += 1;
                }
                if cs.get(j) == Some(&'"') {
                    j += 1;
                    while j < cs.len() {
                        if cs[j] == '\n' {
                            linha += 1;
                        }
                        if cs[j] == '"' && (1..=cerquilhas).all(|k| cs.get(j + k) == Some(&'#')) {
                            j += cerquilhas;
                            break;
                        }
                        j += 1;
                    }
                    i = j + 1;
                    continue;
                }
            }
            '"' | '`' => pular_texto(&mut i, &mut linha, c),
            '\'' if aspas_simples_sao_texto => pular_texto(&mut i, &mut linha, '\''),
            '\'' => {
                // Rust: caractere ('x', '\n', '\u{1F600}') ou tempo de vida ('a).
                if cs.get(i + 1) == Some(&'\\') {
                    pular_texto(&mut i, &mut linha, '\'');
                } else if cs.get(i + 2) == Some(&'\'') {
                    i += 2;
                }
            }
            '{' | '[' => pilha.push((c, linha)),
            '}' | ']' => {
                let par = if c == '}' { '{' } else { '[' };
                // Fecha o abridor do mesmo tipo mais recente; um desequilibrio (codigo pela
                // metade) perde so aquela regiao, nao o resto do arquivo.
                if let Some(p) = pilha.iter().rposition(|(a, _)| *a == par) {
                    let (_, abriu) = pilha.remove(p);
                    // Texto depois do fechamento (`};`, `} else {`): a linha fica visivel.
                    let resto = cs[i + 1..].iter().take_while(|c| **c != '\n');
                    let fim = if resto.clone().any(|c| !c.is_whitespace()) {
                        linha.saturating_sub(1)
                    } else {
                        linha
                    };
                    if fim > abriu {
                        saida.push(Regiao {
                            inicio: abriu,
                            fim,
                            tipo: None,
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    arrumar(saida)
}

/// Bloco por indentacao: uma linha abre regiao quando a proxima linha com texto esta mais
/// para dentro, e a regiao vai ate a ultima linha com texto antes de alguma voltar a mesma
/// coluna (ou menos). Linha em branco no meio nao fecha; em branco no fim fica de fora.
/// Tabulacao conta como 4 colunas, a mesma regra do minimapa.
pub fn por_indentacao(texto: &str) -> Vec<Regiao> {
    let recuo: Vec<Option<usize>> = texto
        .lines()
        .map(|l| {
            let l = l.replace('\t', "    ");
            let t = l.trim_start();
            (!t.is_empty()).then(|| l.len() - t.len())
        })
        .collect();
    let mut saida = Vec::new();
    for (i, r) in recuo.iter().enumerate() {
        let Some(r) = *r else { continue };
        let Some(prox) = recuo[i + 1..].iter().flatten().next() else {
            continue;
        };
        if *prox <= r {
            continue;
        }
        let mut ultima = i;
        for (j, rj) in recuo.iter().enumerate().skip(i + 1) {
            match rj {
                Some(x) if *x <= r => break,
                Some(_) => ultima = j,
                None => {}
            }
        }
        if ultima > i {
            saida.push(Regiao {
                inicio: i as u32 + 1,
                fim: ultima as u32 + 1,
                tipo: None,
            });
        }
    }
    arrumar(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn faixas(v: &[Regiao]) -> Vec<(u32, u32)> {
        v.iter().map(|r| (r.inicio, r.fim)).collect()
    }

    #[test]
    fn chaves_dobram_ate_a_chave_que_fecha_menos_quando_ha_texto_depois() {
        let t = "fn a() {\n    let x = 1;\n    if x > 0 {\n        x;\n    }\n}\nfn b() {}\n";
        assert_eq!(faixas(&por_chaves(t, false)), vec![(1, 6), (3, 5)]);
        // `} else {`: o if para na linha de cima, para o `else` nao sumir junto.
        let t = "if a {\n    1\n} else {\n    2\n}\n";
        assert_eq!(faixas(&por_chaves(t, false)), vec![(1, 2), (3, 5)]);
    }

    /// Chave dentro de texto, comentario, caractere e texto cru nao abre nem fecha bloco; o
    /// tempo de vida `'a` nao e caractere.
    ///
    /// RED medido: sem o braco `'"' | '`' => pular_texto` (`// REPOSTO`), a chave do texto
    /// abriu bloco e a asserção caiu.
    #[test]
    fn chaves_em_texto_e_comentario_nao_contam() {
        let t = "fn a<'a>(s: &'a str) {\n    let c = '{';\n    let s = \"}{\";\n    // }\n    /* { */\n    let r = r#\"}\"#;\n    let e = '\\'';\n}\n";
        assert_eq!(faixas(&por_chaves(t, false)), vec![(1, 8)]);
        let js = "const o = {\n  a: '}',\n  b: `{\n`,\n};\n";
        assert_eq!(faixas(&por_chaves(js, true)), vec![(1, 4)]);
    }

    #[test]
    fn colchete_multilinha_dobra_e_bloco_de_uma_linha_nao() {
        let t = "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {}\n}\n";
        assert_eq!(faixas(&por_chaves(t, true)), vec![(1, 7), (2, 4)]);
    }

    #[test]
    fn indentacao_abre_onde_a_proxima_entra_e_fecha_onde_volta() {
        let t = "def a():\n    x = 1\n\n    if x:\n        y = 2\n\nb = 3\n";
        assert_eq!(faixas(&por_indentacao(t)), vec![(1, 5), (4, 5)]);
        // Sem nada mais para dentro, nada dobra.
        assert!(por_indentacao("a\nb\nc\n").is_empty());
    }

    #[test]
    fn a_reserva_escolhe_pela_extensao_e_cai_na_indentacao_sem_bloco() {
        let rs = "fn a() {\n    1\n}\n";
        assert_eq!(
            reserva("src/a.rs", rs),
            (
                "chaves",
                vec![Regiao {
                    inicio: 1,
                    fim: 3,
                    tipo: None
                }]
            )
        );
        let py = "def a():\n    return 1\n";
        assert_eq!(
            reserva("a.py", py),
            (
                "indentacao",
                vec![Regiao {
                    inicio: 1,
                    fim: 2,
                    tipo: None
                }]
            )
        );
        // .c sem bloco nenhum, mas com recuo: a indentacao responde.
        assert_eq!(reserva("a.c", "int a;\n  int b;\n").0, "indentacao");
    }

    /// O servidor fala em linhas a partir de 0; faixa vazia some, a que passa do texto e
    /// cortada, e duas no mesmo inicio viram a maior.
    #[test]
    fn a_resposta_do_lsp_vira_linhas_a_partir_de_um() {
        let v = json!([
            {"startLine": 0, "endLine": 3, "kind": "region"},
            {"startLine": 0, "endLine": 1},
            {"startLine": 2, "endLine": 2},
            {"startLine": 5, "endLine": 90, "kind": "comment"}
        ]);
        let r = do_lsp(&v, 10);
        assert_eq!(faixas(&r), vec![(1, 4), (6, 10)]);
        assert_eq!(r[0].tipo.as_deref(), Some("region"));
        assert!(do_lsp(&Value::Null, 10).is_empty());
    }
}

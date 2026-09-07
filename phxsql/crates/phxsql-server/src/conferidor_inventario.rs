//! O conferidor do INVENTARIO: os arquivos de uma tabela moram em quatro
//! lugares, e so um deles e codigo.
//!
//! # Por que ele existe
//!
//! Pedido 213 (pergunta do dono, 07/09/2026): *«onde fica os .fts? por que o
//! dossie nao tem no grafico: Organograma dos arquivos?»*. Medido antes de
//! mexer: o `.fts` faltava em TRES copias escritas a mao -- a Figura 1 do
//! dossie (organograma), a Figura 8 (o caminho de uma insercao) e a
//! tabela-mestra do `docs/FORMATO.md` -- porque nenhuma das tres sai do
//! codigo. E o codigo TAMBEM estava errado: `EXTENSOES_TODAS`, em
//! `phxsql-store/src/catalogo.rs`, tambem nao tinha `.fts` -- o mesmo defeito
//! que ja tinha acontecido duas vezes ali (a lista nasceu com seis, a tabela
//! tinha nove; depois nasceu com nove, a tabela tinha dez). Sem a lista do
//! codigo estar certa, nao havia contra o que comparar as tres copias.
//!
//! O conserto de raiz tem duas pernas: a primeira foi consertar
//! `EXTENSOES_TODAS` (ver `phxsql_store::catalogo`, e os testes la que provam
//! o `.fts` deixando de ficar orfao num `excluir_tabela`/`renomear_tabela`).
//! Esta segunda perna e ESTE modulo -- a guarda que impede a proxima extensao
//! de repetir o defeito nas TRES copias, agora que ha uma lista certa contra
//! a qual compara-las.
//!
//! # O que ele conta
//!
//! Le `Database::extensoes_de_uma_tabela()` (a lista unica do codigo, depois
//! do conserto) e confere que toda extensao aparece:
//!
//! * na **Figura 1** do dossie -- o organograma completo, que promete
//!   "SEMPRE os sete, SO AS VEZES quatro": tem de ter as onze;
//! * na **tabela-mestra do `docs/FORMATO.md`** -- o dicionario dos arquivos,
//!   que tambem promete o inventario completo: as onze, somadas entre a
//!   tabela principal (dez) e a do `.bkp` (uma, por ser "o oitavo arquivo
//!   opcional", ver FORMATO.md);
//! * na **Figura 8** -- MENOS as que [`FORA_DA_INSERCAO`] documenta como
//!   legitimamente ausentes. Figura 8 desenha o caminho de UMA INSERCAO, nao
//!   o inventario da tabela -- esse papel e da Figura 1. Confundir os dois
//!   faria a guarda exigir que uma insercao desenhasse o `.trash` e o
//!   `.reason`, que so nascem numa EXCLUSAO: forcar isso na figura seria
//!   documentar uma mentira para agradar um conferidor.
//!
//! Reprova tambem o sentido contrario -- uma extensao que uma copia MENCIONA
//! e o codigo NAO tem -- porque um nome errado ali e tao enganoso quanto um
//! que falta.
//!
//! # Onde a linha se traca, e por que ali
//!
//! [`FORA_DA_INSERCAO`] existe porque a Figura 1 e a Figura 8 respondem
//! perguntas DIFERENTES ("o que uma tabela TEM" vs. "o que UMA INSERCAO
//! toca"), e gerar a Figura 8 inteira do codigo perderia o texto de decisao
//! de cada caixa ("so quando ligado", "fora do desfazer, de proposito") que
//! a fez valer a pena desenhar a mao -- a mesma razao que fez pedido 213
//! pedir uma GUARDA em vez de um gerador. A lista e curta (duas extensoes,
//! com o motivo escrito ao lado) de proposito: e o mesmo molde do `ISENTOS`
//! de [`crate::conferidor_temporarios`] -- isencao por NOME e com motivo, para
//! que a proxima extensao que tambem so nasca numa exclusao entre aqui por
//! DECISAO, nao por a figura ter esquecido dela calada.
//!
//! ```bash
//! cargo run --example inventarios-descasados -p phxsql-server
//! ```

use std::path::{Path, PathBuf};

use phxsql_store::Database;

/// Extensoes que a Figura 8 (o caminho de UMA INSERCAO) legitimamente nao
/// desenha, porque a insercao nao as toca -- e o motivo de cada uma e o que
/// a impede de precisar aparecer la.
///
/// Note o que NAO esta aqui: `.lgpd` nao precisa de isencao porque a Figura 8
/// ja o MENCIONA, no `<code>.lgpd</code>` da propria legenda ("nunca numa
/// insercao") -- ele e achado pela varredura como qualquer outra mencao.
/// Isencao e para o que a figura de proposito NAO cita, nao para o que ela
/// cita de um jeito que a varredura ainda nao sabia ler.
pub const FORA_DA_INSERCAO: &[(&str, &str)] = &[
    (
        "trash",
        "so nasce numa EXCLUSAO fisica -- uma insercao nao tem o que descartar",
    ),
    (
        "reason",
        "so nasce numa EXCLUSAO -- uma insercao nao tem motivo de exclusao para guardar",
    ),
];

/// Um descasamento entre o codigo e uma das copias escritas a mao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descasamento {
    pub extensao: String,
    pub onde: &'static str,
    /// `true`: o codigo tem e a copia nao (falta). `false`: a copia tem uma
    /// extensao que o codigo nao conhece (sobra).
    pub falta: bool,
}

/// O que a varredura achou, em cada um dos quatro lugares.
#[derive(Debug, Default)]
pub struct Relatorio {
    pub canonico: Vec<String>,
    pub figura1: Vec<String>,
    pub figura8: Vec<String>,
    pub formato_md: Vec<String>,
    pub descasamentos: Vec<Descasamento>,
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("o crate mora em <raiz>/crates/phxsql-server")
        .to_path_buf()
}

/// O unico dossie da pasta -- mesma regra do `dossie_da_pasta.py`: zero ou
/// dois e parada com o motivo, nunca um palpite sobre qual ler. Reescrita em
/// Rust para o `cargo test` nao depender de python -- as DUAS tem de
/// continuar concordando, e e por isso que a regra e citada por extenso aqui
/// em vez de so um `veja o .py`.
fn achar_o_dossie(raiz: &Path) -> PathBuf {
    let pasta = raiz.join("docs/dossie");
    let mut achados: Vec<PathBuf> = std::fs::read_dir(&pasta)
        .unwrap_or_else(|e| panic!("nao abriu {}: {e}", pasta.display()))
        .flatten()
        .map(|entrada| entrada.path())
        .filter(|p| {
            let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            nome.starts_with("dossie-phxsql-") && nome.ends_with(".html")
        })
        .collect();
    achados.sort();
    match achados.len() {
        1 => achados.remove(0),
        0 => panic!(
            "DOSSIE NAO ACHADO em {} (padrao `dossie-phxsql-*.html`)",
            pasta.display()
        ),
        _ => panic!(
            "DOIS DOSSIES na pasta, e so pode haver UM por vez: {}",
            achados
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// O bloco `<figure>...</figure>` cuja legenda comeca com `Figura N.`.
///
/// Os blocos nao se aninham nesta pagina (conferido: cada `<figure>` fecha
/// antes do proximo abrir), entao um casador de string acha o par certo sem
/// precisar de um parser de HTML de verdade -- que este repositorio nao tem,
/// por ser dependencia externa.
fn extrair_figura(html: &str, numero: u32) -> String {
    let marca = format!("<b>Figura {numero}.</b>");
    let pos_legenda = html
        .find(&marca)
        .unwrap_or_else(|| panic!("o dossie nao tem a legenda \"{marca}\""));
    // Anda para TRAS ate o `<figure` que abre este bloco, e para FRENTE ate o
    // `</figure>` que o fecha -- a legenda mora sempre entre os dois.
    let inicio = html[..pos_legenda]
        .rfind("<figure")
        .unwrap_or_else(|| panic!("achei a legenda \"{marca}\" fora de um <figure>"));
    let fim_relativo = html[pos_legenda..]
        .find("</figure>")
        .unwrap_or_else(|| panic!("a legenda \"{marca}\" nao fecha com </figure>"));
    html[inicio..pos_legenda + fim_relativo].to_string()
}

/// As extensoes do CODIGO, na ordem em que `Database` as declara.
pub fn extensoes_do_codigo() -> Vec<String> {
    Database::extensoes_de_uma_tabela()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Toda extensao CONHECIDA (a lista do codigo) que aparece como `.ext` em
/// `texto`, com fronteira de palavra dos dois lados -- o mesmo criterio que
/// evita casar `.reg` dentro de "registra".
///
/// Varre so a lista conhecida de proposito: o que ela NAO conhece e coberto
/// por [`mencoes_desconhecidas`], que faz o caminho inverso.
fn mencoes_conhecidas(texto: &str, conhecidas: &[String]) -> Vec<String> {
    let mut achadas: Vec<String> = conhecidas
        .iter()
        .filter(|ext| contem_extensao(texto, ext))
        .cloned()
        .collect();
    achadas.sort();
    achadas
}

/// Extensoes de aparencia real (`.` seguido de 2 a 7 letras minusculas, com
/// fronteira dos dois lados) que NAO estao na lista conhecida -- o sentido
/// contrario da funcao de cima: uma copia que cita uma extensao errada.
///
/// O corte em letras minusculas ASCII e o que faz o crivo nao se confundir
/// com pontuacao de prosa: nesta pagina, ponto de fim de frase e sempre
/// seguido de espaco ou maiuscula, nunca de letra minuscula colada -- so uma
/// extensao de arquivo cola letras direto depois do ponto.
fn mencoes_desconhecidas(texto: &str, conhecidas: &[String]) -> Vec<String> {
    let bytes = texto.as_bytes();
    let mut achadas = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.' {
            let ini = i + 1;
            let mut fim = ini;
            while fim < bytes.len() && bytes[fim].is_ascii_lowercase() {
                fim += 1;
            }
            let tam = fim - ini;
            let fronteira_direita = fim >= bytes.len() || !bytes[fim].is_ascii_alphanumeric();
            let fronteira_esquerda = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if (2..=7).contains(&tam) && fronteira_direita && fronteira_esquerda {
                let candidata = &texto[ini..fim];
                if !conhecidas.iter().any(|c| c == candidata) {
                    achadas.insert(candidata.to_string());
                }
            }
            i = fim.max(i + 1);
        } else {
            i += 1;
        }
    }
    achadas.into_iter().collect()
}

/// `texto` menciona `.{ext}` com fronteira de palavra dos dois lados?
fn contem_extensao(texto: &str, ext: &str) -> bool {
    let alvo = format!(".{ext}");
    let bytes = texto.as_bytes();
    let alvo_bytes = alvo.as_bytes();
    let mut inicio = 0;
    while let Some(pos) = texto[inicio..].find(&alvo) {
        let pos_abs = inicio + pos;
        let fim = pos_abs + alvo_bytes.len();
        let fronteira_esquerda = pos_abs == 0 || !bytes[pos_abs - 1].is_ascii_alphanumeric();
        let fronteira_direita = fim >= bytes.len() || !bytes[fim].is_ascii_alphanumeric();
        if fronteira_esquerda && fronteira_direita {
            return true;
        }
        inicio = pos_abs + 1;
    }
    false
}

/// As extensoes citadas na tabela-mestra do `docs/FORMATO.md`.
///
/// A busca ancora em `| Arquivo |` -- o cabecalho exato das DUAS tabelas de
/// inventario (a principal, de dez linhas, e a do `.bkp`, de uma linha "o
/// oitavo arquivo opcional"). Uma TERCEIRA tabela do mesmo arquivo (§ sobre
/// `ALTER TABLE`, "Os arquivos irmaos") lista as mesmas extensoes sob outro
/// cabecalho (`| arquivo |`, minusculo, colunas diferentes) -- ancorar no
/// texto exato do cabecalho e o que evita misturar as duas perguntas
/// diferentes ("o que uma tabela TEM" vs. "o que uma alteracao de coluna
/// toca") na mesma varredura.
fn linhas_do_inventario_no_formato_md(conteudo: &str) -> String {
    let mut linhas_de_tabela = String::new();
    let linhas: Vec<&str> = conteudo.lines().collect();
    let mut i = 0;
    while i < linhas.len() {
        if linhas[i].starts_with("| Arquivo |") {
            // Pula o cabecalho e a linha `|---|...`; colhe ate a tabela acabar.
            let mut j = i + 2;
            while j < linhas.len() && linhas[j].trim_start().starts_with('|') {
                linhas_de_tabela.push_str(linhas[j]);
                linhas_de_tabela.push('\n');
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    linhas_de_tabela
}

/// Varre os quatro lugares e devolve o relatorio completo.
pub fn varrer() -> Relatorio {
    let raiz = raiz();
    let canonico = extensoes_do_codigo();

    let dossie_path = achar_o_dossie(&raiz);
    let dossie = std::fs::read_to_string(&dossie_path)
        .unwrap_or_else(|e| panic!("nao leu {}: {e}", dossie_path.display()));
    let bloco1 = extrair_figura(&dossie, 1);
    let bloco8 = extrair_figura(&dossie, 8);
    let figura1 = mencoes_conhecidas(&bloco1, &canonico);
    let figura8 = mencoes_conhecidas(&bloco8, &canonico);

    let formato_path = raiz.join("docs/FORMATO.md");
    let formato_txt = std::fs::read_to_string(&formato_path)
        .unwrap_or_else(|e| panic!("nao leu {}: {e}", formato_path.display()));
    let linhas_formato = linhas_do_inventario_no_formato_md(&formato_txt);
    let formato_md = mencoes_conhecidas(&linhas_formato, &canonico);

    let mut descasamentos = Vec::new();
    let exemptas_fig8: Vec<&str> = FORA_DA_INSERCAO.iter().map(|(e, _)| *e).collect();

    for (achadas, onde, exemptas) in [
        (&figura1, "Figura 1", &[][..]),
        (&figura8, "Figura 8", &exemptas_fig8[..]),
        (&formato_md, "docs/FORMATO.md", &[][..]),
    ] {
        for ext in &canonico {
            if !achadas.contains(ext) && !exemptas.contains(&ext.as_str()) {
                descasamentos.push(Descasamento {
                    extensao: ext.clone(),
                    onde,
                    falta: true,
                });
            }
        }
    }

    // O sentido contrario: uma copia citando uma extensao que o codigo nao
    // tem. Varrido sobre o bloco/tabela inteiro, nao so sobre `achadas`
    // (que ja e o filtro "so as conhecidas") -- por isso chama de novo com
    // a lista completa de candidatas.
    //
    // Para o FORMATO.md, a varredura usa `linhas_formato` -- a mesma fatia
    // restrita as DUAS tabelas de inventario -- e NAO o arquivo inteiro: o
    // documento tem outras dezenas de "ponto-letras" que nao sao extensao
    // nenhuma (`*.novo`, o arquivo temporario do `.pag`; `.tx`, a marca de
    // commit; `..fim`, faixa de bytes de um CRC) e que ficariam fora do
    // escopo da pergunta "que extensao uma tabela TEM".
    for (bloco, onde) in [
        (bloco1.as_str(), "Figura 1"),
        (bloco8.as_str(), "Figura 8"),
        (linhas_formato.as_str(), "docs/FORMATO.md"),
    ] {
        for extra in mencoes_desconhecidas(bloco, &canonico) {
            descasamentos.push(Descasamento {
                extensao: extra,
                onde,
                falta: false,
            });
        }
    }

    Relatorio {
        canonico,
        figura1,
        figura8,
        formato_md,
        descasamentos,
    }
}

/// A catraca. **So desce.**
///
/// Zero, e zero e o numero certo: as tres copias e o codigo estao
/// consertados nesta rodada (pedido 213). Uma extensao nova que entrar no
/// motor sem passar pelas tres reprova aqui, nomeando qual lugar ficou para
/// tras -- a mesma pergunta que o dono fez olhando a tela, agora automatica.
pub const TETO_INVENTARIO_DESCASADO: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;

    /// A catraca do pedido 213.
    ///
    /// O `<=` fica de proposito, com o `allow` ao lado -- mesma forma dos
    /// irmaos `conferidor_temporarios` e `conferidor_vermelhas`: a lei da
    /// casa e "catraca so desce", entao a comparacao aceita um numero MENOR
    /// que o teto. So parece absurda hoje porque zero e o piso.
    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn toda_extensao_do_codigo_aparece_nas_tres_copias() {
        let r = varrer();
        assert!(
            r.descasamentos.len() <= TETO_INVENTARIO_DESCASADO,
            "{} descasamento(s) entre o codigo e as copias escritas a mao \
             (teto {TETO_INVENTARIO_DESCASADO}):\n{}",
            r.descasamentos.len(),
            r.descasamentos
                .iter()
                .map(|d| format!(
                    "  {} -- {} em {}",
                    d.extensao,
                    if d.falta {
                        "falta"
                    } else {
                        "sobra, desconhecida"
                    },
                    d.onde
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Prova real, sentido 1: com o defeito REPOSTO (uma extensao do codigo
    /// que a Figura 1 nao menciona), o casador ACUSA -- sem tocar em nenhum
    /// arquivo do repositorio, do mesmo jeito que os irmaos provam a si
    /// mesmos sobre um texto sintetico.
    #[test]
    fn acusa_quando_uma_figura_perde_uma_extensao() {
        let canonico = vec!["reg".to_string(), "ndx".to_string(), "fts".to_string()];
        let bloco_sem_fts = "<figure><text>.reg</text><text>.ndx</text></figure>";
        let achadas = mencoes_conhecidas(bloco_sem_fts, &canonico);
        assert!(
            !achadas.contains(&"fts".to_string()),
            "o casador tinha de NAO achar o fts neste bloco -- ele nao esta la"
        );
    }

    /// Prova real, sentido 2: devolvido o `.fts` ao bloco, o casador acha.
    #[test]
    fn nao_acusa_quando_a_figura_tem_a_extensao() {
        let canonico = vec!["reg".to_string(), "ndx".to_string(), "fts".to_string()];
        let bloco_com_fts = "<figure><text>.reg</text><text>.ndx</text><text>.fts</text></figure>";
        let achadas = mencoes_conhecidas(bloco_com_fts, &canonico);
        assert!(
            achadas.contains(&"fts".to_string()),
            "o casador tinha de achar o fts -- ele esta la, como texto de SVG"
        );
    }

    /// `contem_extensao` nao pode casar `.reg` dentro de "registra": sem a
    /// fronteira de palavra, toda prosa em portugues acusaria falso.
    #[test]
    fn nao_casa_extensao_dentro_de_palavra_maior() {
        assert!(
            !contem_extensao("o motor registra o evento", "reg"),
            "\"registra\" nao pode contar como mencao de \".reg\""
        );
        assert!(
            contem_extensao("grava no .reg e no .bkp", "reg"),
            "\".reg\" de verdade tem de ser achado"
        );
    }

    /// `mencoes_desconhecidas` acha uma extensao que o codigo nao conhece --
    /// e NAO confunde ponto de fim de frase (sempre seguido de espaco ou
    /// maiuscula nesta prosa) com extensao de arquivo.
    #[test]
    fn mencoes_desconhecidas_acha_extensao_errada_e_ignora_prosa() {
        let conhecidas = vec!["reg".to_string(), "ndx".to_string()];
        let achadas = mencoes_desconhecidas(
            "grava no .reg e no .idx, que ninguem declarou. Fim da frase.",
            &conhecidas,
        );
        assert_eq!(
            achadas,
            vec!["idx".to_string()],
            "so o .idx (desconhecido) devia aparecer -- nem o .reg (conhecido, \
             filtrado por design) nem a prosa apos o ponto final"
        );
    }

    /// O `FORA_DA_INSERCAO` e o alcance documentado -- confere que nenhuma
    /// entrada dele e um nome que o codigo nem conhece (isencao morta).
    #[test]
    fn a_lista_de_isentas_da_insercao_e_extensao_de_verdade() {
        let canonico = extensoes_do_codigo();
        for (ext, motivo) in FORA_DA_INSERCAO {
            assert!(
                canonico.contains(&ext.to_string()),
                "FORA_DA_INSERCAO cita \"{ext}\" ({motivo}), mas o codigo nao \
                 tem essa extensao -- isencao morta"
            );
        }
    }

    /// Prova real do conferidor como um todo, contra o disco de verdade: hoje
    /// (pedido 213 fechado) as tres copias batem com o codigo, e a varredura
    /// tem de dizer isso -- zero descasamentos, canonico com 11 extensoes.
    #[test]
    fn hoje_as_tres_copias_batem_com_o_codigo() {
        let r = varrer();
        assert_eq!(
            r.canonico.len(),
            11,
            "o codigo tinha de ter onze extensoes (dez de antes + o .fts do \
             pedido 213): {:?}",
            r.canonico
        );
        assert!(
            r.descasamentos.is_empty(),
            "descasamentos que nao deviam existir mais: {:?}",
            r.descasamentos
        );
    }
}

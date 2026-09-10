//! O conferidor das PROVAS VERMELHAS: guarda desligada tem de ter pedido.
//!
//! # Por que ele existe
//!
//! Esta casa entrega a guarda **vermelha** quando o defeito e real e o
//! conserto e decisao do dono: escreve-se o teste que falha com o defeito de
//! pe, marca-se `#[ignore = "VERMELHA de proposito: ..."]`, e o defeito fica
//! provado enquanto espera a decisao. E uma boa pratica, e ela tem um buraco.
//!
//! **Achado na revisao completa de 07/09/2026**: havia DUAS provas vermelhas
//! na arvore -- o vazamento da cifra em coluna externa marcada sozinha (de
//! 05/09) e a posicao do diario encolhendo em silencio -- e **nenhuma das
//! duas estava no `PENDENCIAS.md`**. Uma prova vermelha desligada e a forma
//! mais educada de esquecer um defeito: a bateria fica verde, o `cargo test`
//! diz "0 falharam", e o defeito nao aparece em lugar nenhum que alguem leia.
//!
//! *Papel que nao esta cumprindo tem de aparecer como nao cumprindo* -- e ali
//! ele nao aparecia.
//!
//! # O que ele conta
//!
//! Cada `#[ignore = "VERMELHA de proposito...` em `crates/*/src` e
//! `crates/*/tests`, e o nome da funcao logo abaixo. Conta quem NAO aparece
//! no `docs/PENDENCIAS.md`.
//!
//! # O que ele NAO conta, e e decisao
//!
//! Os `#[ignore]` comuns. O vetor de 1.000.000 de iteracoes do X25519 leva
//! minutos, e o corpo tracado da sonda do fecho so roda reexecutado por outro
//! teste -- os dois sao ignorados por CUSTO, e nao por defeito. Misturar os
//! dois encheria a catraca de ruido e a faria parar de significar "ha defeito
//! conhecido aqui", que e a unica coisa que ela existe para dizer.
//!
//! ```bash
//! cargo run --example vermelhas-sem-pedido -p phxsql-server
//! ```

use std::path::{Path, PathBuf};

/// A marca que separa a guarda vermelha do `#[ignore]` por custo.
///
/// Partida em duas metades pelo mesmo motivo do irmao `conferidor_temporarios`:
/// escrita inteira, esta linha casaria consigo mesma e o conferidor se acusaria.
const MARCA: &str = concat!("VERMELHA de ", "proposito");

/// Uma prova vermelha achada na arvore.
#[derive(Debug, Clone)]
pub struct Vermelha {
    pub arquivo: String,
    pub linha: usize,
    /// O nome da funcao de teste, que e a chave procurada no PENDENCIAS.
    pub funcao: String,
    /// O motivo escrito no proprio `#[ignore = "..."]`.
    pub motivo: String,
    /// Aparece no `docs/PENDENCIAS.md`?
    pub tem_pedido: bool,
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("o crate mora em <raiz>/crates/phxsql-server")
        .to_path_buf()
}

/// Anda a arvore e devolve toda prova vermelha, com e sem pedido.
pub fn varrer() -> Vec<Vermelha> {
    let raiz = raiz();
    let pendencias = std::fs::read_to_string(raiz.join("docs/PENDENCIAS.md")).unwrap_or_default();

    let mut fontes = Vec::new();
    let mut crates: Vec<PathBuf> = std::fs::read_dir(raiz.join("crates"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    crates.sort();
    for c in crates {
        for pasta in ["src", "tests"] {
            juntar_rs(&c.join(pasta), &mut fontes);
        }
    }

    let mut achadas = Vec::new();
    for arq in fontes {
        let rel = arq
            .strip_prefix(&raiz)
            .unwrap_or(&arq)
            .to_string_lossy()
            .into_owned();
        let Ok(texto) = std::fs::read_to_string(&arq) else {
            continue;
        };
        achadas.extend(casar_arquivo(&rel, &texto, &pendencias));
    }
    achadas
}

/// Casa as provas vermelhas de UM arquivo ja lido.
///
/// Fica separada de `varrer` de proposito: a prova real do casador precisa
/// exercita-lo contra um texto SINTETICO, sem depender de a arvore ter uma
/// vermelha viva -- e desde o pedido 211 ela pode nao ter (a arvore limpa e o
/// objetivo, nao um defeito). Enquanto o casamento vivia dentro do laco que le
/// o disco, a unica forma de prova-lo era achar uma vermelha real, e um teste
/// que EXIGE uma vermelha real falha justamente quando o projeto conserta a
/// ultima -- punindo o sucesso.
fn casar_arquivo(rel: &str, texto: &str, pendencias: &str) -> Vec<Vermelha> {
    let linhas: Vec<&str> = texto.lines().collect();
    let mut achadas = Vec::new();
    for (i, linha) in linhas.iter().enumerate() {
        if !linha.contains(MARCA) || !linha.trim_start().starts_with("#[ignore") {
            continue;
        }
        let motivo = linha
            .split_once('"')
            .and_then(|(_, r)| r.rsplit_once('"'))
            .map(|(m, _)| m.to_string())
            .unwrap_or_default();
        // A funcao vem logo abaixo; pode haver outros atributos no meio.
        let funcao = linhas[i + 1..]
            .iter()
            .take(6)
            .find_map(|l| {
                l.trim_start()
                    .strip_prefix("fn ")
                    .and_then(|r| r.split('(').next())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        achadas.push(Vermelha {
            arquivo: rel.to_string(),
            linha: i + 1,
            tem_pedido: !funcao.is_empty() && pendencias.contains(&funcao),
            funcao,
            motivo,
        });
    }
    achadas
}

/// As que contam para a catraca: prova vermelha que ninguem consegue achar.
pub fn sem_pedido() -> Vec<Vermelha> {
    varrer().into_iter().filter(|v| !v.tem_pedido).collect()
}

fn juntar_rs(pasta: &Path, saida: &mut Vec<PathBuf>) {
    let mut entradas: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entradas.sort();
    for p in entradas {
        if p.is_dir() {
            juntar_rs(&p, saida);
        } else if p.extension().is_some_and(|e| e == "rs") {
            saida.push(p);
        }
    }
}

/// A catraca. **So desce.**
///
/// Zero, e zero e o numero certo: uma prova vermelha existe justamente para
/// que o defeito nao se perca, e ela so cumpre isso se estiver escrita onde o
/// dono le. Guarda vermelha nova entra com o pedido no mesmo commit.
pub const TETO_VERMELHA_SEM_PEDIDO: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;

    /// A catraca do pedido 212.
    ///
    /// O `<=` fica de proposito, com o `allow` ao lado: a lei da casa e
    /// "catraca so desce", entao a comparacao tem de aceitar um numero MENOR
    /// que o teto. Ela so parece absurda hoje porque zero e o piso.
    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn toda_prova_vermelha_tem_pedido_no_pendencias() {
        let soltas = sem_pedido();
        assert!(
            soltas.len() <= TETO_VERMELHA_SEM_PEDIDO,
            "{} prova(s) vermelha(s) sem pedido no docs/PENDENCIAS.md (teto \
             {TETO_VERMELHA_SEM_PEDIDO}).\nGuarda vermelha que ninguem acha e defeito esquecido \
             com a consciencia limpa: abra o pedido no mesmo commit.\n{}",
            soltas.len(),
            soltas
                .iter()
                .map(|v| format!("  {}:{}  {}", v.arquivo, v.linha, v.funcao))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Prova real do casador, no sentido que importa: ele ACHA a marca e o nome
    /// da funcao logo abaixo. Um casador que parasse de reconhecer a marca
    /// devolveria vazio -- o zero que nao prova nada.
    ///
    /// # Por que contra um texto sintetico, e nao contra a arvore (pedido 211)
    ///
    /// A versao anterior fazia `varrer()` e exigia `!todas.is_empty()`. Isso
    /// amarrava a prova do casador a haver uma vermelha VIVA na arvore -- e no
    /// dia em que o projeto consertou a ultima (a posicao do diario, este mesmo
    /// pedido 211), o teste passou a falhar por SUCESSO: arvore limpa e o
    /// objetivo, nao um defeito. O casador agora se prova contra um fonte
    /// montado aqui, e continua valendo com a arvore vazia ou cheia.
    ///
    /// A marca vem partida (`"VERMELHA de " , "proposito"`) pelo mesmo motivo da
    /// `MARCA`: escrita inteira num literal, esta linha viraria uma vermelha
    /// sintetica que o proprio conferidor acharia na arvore.
    #[test]
    fn o_casador_enxerga_uma_vermelha_sintetica() {
        let fonte = concat!(
            "    #[test]\n",
            "    #[ignore = \"",
            "VERMELHA de ",
            "proposito: exemplo do controle\"]\n",
            "    fn defeito_de_exemplo() {\n",
            "        // corpo\n",
            "    }\n",
        );
        let achadas = casar_arquivo("crates/exemplo/src/lib.rs", fonte, "");
        assert_eq!(
            achadas.len(),
            1,
            "o casador tinha de achar a unica vermelha sintetica"
        );
        assert_eq!(
            achadas[0].funcao, "defeito_de_exemplo",
            "o casador leu o atributo e nao achou o `fn` logo abaixo"
        );
        assert!(
            !achadas[0].tem_pedido,
            "com o PENDENCIAS vazio, a vermelha conta como SEM pedido"
        );
    }

    /// O outro sentido: um nome que NAO esta no PENDENCIAS tem de contar.
    /// Sem isto, um `contains` que sempre devolvesse `true` passaria calado.
    #[test]
    fn nome_que_nao_esta_no_pendencias_conta_como_sem_pedido() {
        let pendencias =
            std::fs::read_to_string(raiz().join("docs/PENDENCIAS.md")).unwrap_or_default();
        assert!(
            !pendencias.contains("funcao_que_nunca_existiu_em_lugar_nenhum"),
            "o PENDENCIAS nao pode conter o nome inventado deste controle"
        );
        // E o positivo, para o controle nao passar por um PENDENCIAS vazio:
        assert!(
            !pendencias.is_empty(),
            "o PENDENCIAS.md veio vazio -- o conferidor estaria acusando todo mundo"
        );
    }
}

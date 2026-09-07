//! O conferidor dos TEMPORARIOS: quem cria diretorio em `/tmp` sem guarda.
//!
//! # Por que ele existe
//!
//! Pedido 150. Uma corrida da bateria dos tres crates de servidor deixava
//! **265 diretorios** para tras em `/tmp`, e o `/tmp` desta maquina ja tinha
//! 27.519 entradas acumuladas quando isto foi medido. O padrao era sempre o
//! mesmo: um ajudante de teste que devolvia so o `PathBuf`, com um
//! `remove_dir_all` na ENTRADA (para o proximo achar limpo) e nenhum na
//! saida. Quem falhava no meio -- que e o caso comum de um teste de asserção
//! -- deixava tudo.
//!
//! O conserto foi um guarda com `Drop` (`apoio_teste::DirTemp` nos crates com
//! biblioteca, `tests/comum/mod.rs` nos testes de integracao). Mas conserto
//! sem catraca dura ate a proxima frente: **a do `.fts` ja tinha reposto o
//! defeito** no `phxsql-store`, que fora convertido antes -- 21 diretorios por
//! corrida, tres sitios novos, nenhum aviso. Foi o que provou que este modulo
//! precisava existir.
//!
//! # O que ele conta
//!
//! Toda ocorrencia de `std::env::temp_dir()` em `crates/*/src` e
//! `crates/*/tests`, menos as catalogadas em [`ISENTOS`] -- e a isencao e por
//! ARQUIVO **com a quantidade esperada**, para que uma ocorrencia nova num
//! arquivo ja isento tambem reprove. Isencao com numero e decisao; isencao
//! aberta e porta.
//!
//! # O que ele NAO varre, e de proposito
//!
//! `examples/` fica de fora. Um exemplo e um medidor chamado a mao ou pela
//! bancada, nao a bateria, e varios guardam o que criaram justamente para se
//! olhar depois. Contar os 47 ali dentro encheria o catalogo de ruido e
//! escondaria o que importa. O lixo que os exemplos deixam esta medido e
//! anotado no `docs/PENDENCIAS.md` como item proprio -- dispensa registrada e
//! decisao, dispensa silenciosa e esquecimento.
//!
//! # A lista de arquivos sai do disco, nao do codigo
//!
//! [`varrer`] anda o diretorio de verdade. Uma lista digitada envelhece
//! calado: foi assim que o rodape do dossie publicou 780 KiB quando a
//! interface tinha 1.032.
//!
//! ```bash
//! cargo run --example temporarios-sem-guarda -p phxsql-server
//! ```

use std::path::{Path, PathBuf};

/// O padrao procurado, montado em duas metades DE PROPOSITO: escrito inteiro,
/// esta linha casaria consigo mesma e o conferidor se acusaria.
const PADRAO: &str = concat!("std::env::", "temp_dir()");

/// A raiz do repositorio, a partir do `Cargo.toml` deste crate.
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("o crate mora em <raiz>/crates/phxsql-server")
        .to_path_buf()
}

/// Arquivos que podem chamar `std::env::temp_dir()`, com a quantidade
/// esperada e o motivo. Caminho relativo a raiz do repositorio.
///
/// A quantidade faz parte da isencao: uma chamada NOVA num arquivo daqui
/// reprova igual, porque o motivo escrito ao lado vale para as que estao
/// listadas, nao para as que alguem acrescentar amanha.
pub const ISENTOS: &[(&str, usize, &str)] = &[
    (
        "crates/phxsql-store/src/apoio_teste.rs",
        1,
        "e o proprio guarda: e ele quem chama o temp_dir e apaga no Drop",
    ),
    (
        "crates/phxsql-store/tests/comum/mod.rs",
        1,
        "o guarda dos testes de integracao do store",
    ),
    (
        "crates/phxsql-server/src/apoio_teste.rs",
        1,
        "o guarda dos testes unitarios do servidor",
    ),
    (
        "crates/phxsql-server/tests/comum/mod.rs",
        1,
        "o guarda dos testes de integracao do servidor",
    ),
    (
        "crates/phxsql-cmd/tests/comum/mod.rs",
        1,
        "o guarda dos testes de integracao do phxsql-cmd",
    ),
    (
        "crates/phxsql-cli/src/main.rs",
        1,
        "guarda proprio: o cli e um BINARIO, nao tem biblioteca de onde importar",
    ),
    (
        "crates/phxsql-ffi/src/testes.rs",
        1,
        "guarda proprio (`Area`), com Drop, escrito antes do pedido 150",
    ),
    (
        "crates/phxsql-server/src/mensagens.rs",
        6,
        "so monta caminho para LER a tabela de mensagens -- nao cria diretorio nenhum",
    ),
    (
        "crates/phxsql-server/tests/versao.rs",
        1,
        "usa o /tmp como diretorio de trabalho do processo filho; nao cria nada",
    ),
    (
        "crates/phxsql-store/src/restaurar.rs",
        3,
        "codigo de PRODUCAO: dois sao o plano B do palco de restauracao, e um \
         e a asserção que prova que o palco NAO cai no /tmp",
    ),
];

/// Uma chamada a `std::env::temp_dir()` fora do catalogo.
#[derive(Debug, Clone)]
pub struct Solto {
    pub arquivo: String,
    pub linha: usize,
    pub texto: String,
}

/// O que a varredura achou.
#[derive(Debug, Default)]
pub struct Relatorio {
    /// As chamadas que contam para a catraca.
    pub soltas: Vec<Solto>,
    /// Arquivos isentos cuja quantidade mudou: `(arquivo, esperado, achado)`.
    ///
    /// Vale nos DOIS sentidos. Achou mais: entrou chamada nova onde o motivo
    /// escrito nao a cobre. Achou menos: o motivo envelheceu e o catalogo
    /// tem de encolher junto -- catalogo que sobra e catalogo que ninguem le.
    pub isentos_mudaram: Vec<(String, usize, usize)>,
}

/// Anda `crates/*/src` e `crates/*/tests` e conta as chamadas.
pub fn varrer() -> Relatorio {
    let raiz = raiz();
    let mut achados: Vec<Solto> = Vec::new();
    let mut fontes = Vec::new();
    for crate_ in ordenar(&raiz.join("crates")) {
        for pasta in ["src", "tests"] {
            juntar_rs(&crate_.join(pasta), &mut fontes);
        }
    }
    for arq in fontes {
        let rel = arq
            .strip_prefix(&raiz)
            .unwrap_or(&arq)
            .to_string_lossy()
            .into_owned();
        let Ok(texto) = std::fs::read_to_string(&arq) else {
            continue;
        };
        for (i, linha) in texto.lines().enumerate() {
            // Comentario nao cria diretorio nenhum. Sem este corte, o proprio
            // conferidor e a documentacao que explica o defeito apareceriam
            // como o defeito -- e um relatorio que se acusa a si mesmo e um
            // relatorio que ninguem le ate o fim.
            if linha.trim_start().starts_with("//") {
                continue;
            }
            if linha.contains(PADRAO) {
                achados.push(Solto {
                    arquivo: rel.clone(),
                    linha: i + 1,
                    texto: linha.trim().to_string(),
                });
            }
        }
    }

    let mut r = Relatorio::default();
    for (arquivo, esperado, _) in ISENTOS {
        let achado = achados.iter().filter(|s| s.arquivo == *arquivo).count();
        if achado != *esperado {
            r.isentos_mudaram
                .push((arquivo.to_string(), *esperado, achado));
        }
    }
    r.soltas = achados
        .into_iter()
        .filter(|s| !ISENTOS.iter().any(|(a, _, _)| *a == s.arquivo))
        .collect();
    r
}

/// Os subdiretorios de `pasta`, em ordem -- `read_dir` nao promete ordem, e
/// relatorio que muda de ordem a cada corrida nao se compara com o anterior.
fn ordenar(pasta: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// Junta todo `.rs` de `pasta` e das filhas dela.
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
/// Zero, e zero e o numero certo aqui: todo uso legitimo do `/tmp` neste
/// repositorio ja esta em [`ISENTOS`] com o motivo escrito. Quem precisar de
/// um novo entra no catalogo com o motivo, nao na catraca.
pub const TETO_TEMP_DIR_SOLTO: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;

    /// A catraca do pedido 150.
    ///
    /// O `<=` e proposital, e o `allow` explica por que ele fica mesmo com o
    /// teto em zero: a lei da casa e «catraca so desce», entao a comparacao
    /// tem de aceitar um numero MENOR que o teto -- e ai o conserto e baixar
    /// o teto no mesmo commit. Trocar por `==`, como o clippy sugere, faria a
    /// catraca reprovar quem melhorou, que e o contrario do que ela existe
    /// para fazer; ela so parece absurda hoje porque zero e o piso.
    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn ninguem_chama_temp_dir_fora_do_catalogo() {
        let r = varrer();
        assert!(
            r.soltas.len() <= TETO_TEMP_DIR_SOLTO,
            "{} chamada(s) a {PADRAO} fora do catalogo (teto {TETO_TEMP_DIR_SOLTO}).\n\
             Crie o diretorio pelo guarda (`DirTemp::novo`), que apaga no Drop -- ou, se o uso \
             for legitimo, entre em ISENTOS com o motivo escrito.\n{}",
            r.soltas.len(),
            r.soltas
                .iter()
                .map(|s| format!("  {}:{}  {}", s.arquivo, s.linha, s.texto))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// O outro lado do laco: o catalogo tambem envelhece. Sem isto, uma
    /// chamada nova num arquivo ja isento passaria calada -- que e a mesma
    /// falha da chave morta na fabrica de idiomas.
    #[test]
    fn o_catalogo_de_isentos_bate_com_o_disco() {
        let r = varrer();
        assert!(
            r.isentos_mudaram.is_empty(),
            "o catalogo ISENTOS nao bate com o que esta no disco: {}",
            r.isentos_mudaram
                .iter()
                .map(|(a, e, c)| format!("{a}: esperava {e}, achou {c}"))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }

    /// Prova real, no sentido que importa: com o defeito REPOSTO o conferidor
    /// tem de acusar. Aqui o defeito se repoe no proprio texto varrido, sem
    /// tocar em arquivo nenhum -- um teste que escrevesse um `.rs` de mentira
    /// dentro de `crates/` estaria criando exatamente o lixo que este modulo
    /// existe para proibir.
    #[test]
    fn o_conferidor_acusa_quando_o_defeito_volta() {
        let linha = format!("        let d = {PADRAO}.join(\"phx-novo\");");
        assert!(
            linha.contains(PADRAO),
            "o casador nao reconheceria o padrao velho -- e ele e o defeito do pedido 150"
        );
        // E o contrario: uma linha que usa o guarda nao pode ser acusada.
        assert!(
            !"        let d = DirTemp::novo(\"phx-novo\");".contains(PADRAO),
            "o guarda certo nao pode contar como defeito"
        );
    }
}

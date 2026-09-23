//! O conferidor dos SEGREDOS SOLTOS: material de chave dentro da arvore que o
//! `git add` alcanca.
//!
//! # Por que ele existe
//!
//! Pedido 402, medido em 23/09/2026. Um `crates/phxsql-server/chave-do-fio.hex`
//! de 65 bytes (0600, uma privada de 256 bits em hexadecimal) nasceu numa
//! corrida de teste e apareceu entre os 57 arquivos de um commit de
//! integracao. Nao estava no `.gitignore`, e nenhum processo o usava: estava a
//! um `git add -A` de virar segredo publicado num repositorio remoto, de onde
//! nao se tira mais.
//!
//! O pedido 196 desta casa ja se chamava *«a chave privada volta a nascer
//! solta»*. Entao o alcance e o aprendizado: **consertar onde ela nasce nao
//! impede a proxima** -- o defeito nao era o lugar, era **nada impedir de ser
//! versionada**.
//!
//! # Por que `.gitignore` nao basta, e por isso isto existe ALEM dele
//!
//! O `.gitignore` foi posto no mesmo passo (`chave-do-fio.hex`, `*.chave`,
//! `*.key`) e protege do `git add -A`. Ele **nao** protege de um `git add -f`
//! distraido, nem diz nada quando o arquivo existe -- ignorar e calar, e calar
//! e exatamente o que deixou a chave viver o dia inteiro no disco. Este modulo
//! faz o contrario: **reprova** enquanto o arquivo estiver la, ignorado ou
//! nao.
//!
//! # Onde ele roda, e por que aqui
//!
//! No `cargo test --workspace`, pelo `mod testes` do fim deste arquivo, no
//! molde dos outros sete `conferidor_*` do crate. Tres razoes:
//!
//! 1. **E o portao que antecede o commit.** A lei da casa manda `fmt`,
//!    `clippy` e `cargo test --workspace` antes de commitar; guarda que mora
//!    fora desse caminho e script que so roda quando alguem lembra -- e a
//!    licao do zelador e que ninguem lembra.
//! 2. **Ele acha a raiz sem `git`.** `CARGO_MANIFEST_DIR` sobe ate o `.git`;
//!    nao depende de o binario do git existir, nem do que ele acha que esta
//!    ignorado.
//! 3. **E aqui que a chave nasce** (`config::CifraFio::estatica`) e e aqui que
//!    moram os outros conferidores. Um catalogo, um lugar.
//!
//! O limite, escrito porque a guarda sem ele mente: a chave que nascer DURANTE
//! esta mesma corrida so e vista na corrida SEGUINTE, porque a ordem dos
//! binarios de teste nao se escolhe. Isso basta para o defeito que motivou o
//! pedido -- o arquivo fica no disco ate alguem apaga-lo, e o commit veio
//! horas depois --, mas nao transforma a guarda num detetor instantaneo.
//!
//! # Guarda nova entra PEDIDA, nao imposta
//!
//! Ela nao muda comportamento nenhum de producao: nao le `config.json`, nao
//! olha diretorio de servidor instalado, nao roda no `phxsqld`. Quem tem a sua
//! `chave-do-fio.hex` ao lado do `config.json` de uma instalacao continua
//! exatamente como estava -- a varredura so alcanca a arvore do repositorio.
//! E quem precisar de material com cara de chave DENTRO do repositorio (um
//! vetor de prova, por exemplo) entra em [`ISENTOS`] com o motivo escrito.
//!
//! # O que ele NUNCA faz
//!
//! Nao imprime, nao copia e nao guarda um unico byte do arquivo. O relatorio
//! carrega **caminho, tamanho e o motivo do crivo** -- e so. Guarda de segredo
//! que mostra o segredo no erro e o vazamento com outro nome.
//!
//! ```bash
//! cargo run --example segredos-soltos -p phxsql-server
//! ```

use std::path::{Path, PathBuf};

/// Diretorios pulados pelo NOME, em qualquer profundidade, e o motivo.
///
/// Sao os dois que nao sao arvore de trabalho: o `target` e saida de
/// compilacao (12 GB medidos nesta maquina, e ninguem faz `git add -f` dentro
/// dele), e o `.git` e o deposito de objetos -- la o segredo JA foi commitado,
/// e esta guarda existe para agir ANTES disso.
pub const PULADOS: &[(&str, &str)] = &[
    (
        ".git",
        "deposito de objetos: la o segredo ja esta commitado",
    ),
    ("target", "saida de compilacao, fora da arvore de trabalho"),
];

/// Acima disto o arquivo nao se le -- so se mede. Uma privada em hexadecimal
/// cabe em 65 bytes; um PEM de chave, em algumas centenas.
const TETO_DE_LEITURA: u64 = 4096;

/// Menos de 64 digitos nao e chave de 256 bits. E o piso que separa um
/// "cafe" escrito num arquivo de um segredo de verdade.
const MINIMO_DE_HEX: usize = 64;

/// As duas metades do marcador PEM, separadas DE PROPOSITO: escrito inteiro,
/// este fonte casaria consigo mesmo e o conferidor se acusaria -- o mesmo
/// cuidado do `PADRAO` do conferidor de temporarios.
const PEM_ABRE: &str = concat!("-----", "BEGIN");
const PEM_PRIVADA: &str = concat!("PRIVATE", " KEY");

/// Nomes inteiros que sao material de chave, independente do conteudo.
const NOMES: &[&str] = &[
    "chave-do-fio.hex",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
];

/// Extensoes que sao material de chave. As tres primeiras sao as mesmas que o
/// `.gitignore` ganhou no pedido 402 -- a lista viaja junto de proposito, para
/// que quem acrescentar uma la lembre de acrescentar aqui, onde ela REPROVA.
const EXTENSOES: &[&str] = &["chave", "key", "pem", "p12", "pfx", "jks", "keystore"];

/// Por que o arquivo entrou no relatorio.
///
/// Note o que NAO ha aqui: conteudo. O motivo e uma etiqueta fechada, nao um
/// trecho do arquivo -- assim nenhum caminho de formatacao consegue vazar
/// byte de chave por descuido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// O nome ou a extensao dizem "chave".
    Nome,
    /// O arquivo inteiro e hexadecimal, e da para 256 bits ou mais.
    TudoHex,
    /// Tem cabecalho PEM de chave privada.
    Pem,
}

impl Motivo {
    pub fn dizer(&self) -> &'static str {
        match self {
            Motivo::Nome => "o nome e de material de chave",
            Motivo::TudoHex => "o arquivo inteiro e hexadecimal, 256 bits ou mais",
            Motivo::Pem => "cabecalho PEM de chave privada",
        }
    }
}

/// Um arquivo com cara de chave dentro da arvore. **Caminho e tamanho, nunca
/// bytes.**
#[derive(Debug, Clone)]
pub struct Solto {
    /// Relativo a raiz varrida, com `/`.
    pub caminho: String,
    pub bytes: u64,
    pub motivo: Motivo,
}

/// Arquivos com cara de chave que PODEM viver na arvore, com o motivo.
///
/// Esta vazio hoje, e o numero e medido: 7.305 arquivos varridos da raiz do
/// repositorio, **zero** casando o crivo. Vazio nao quer dizer que a porta nao
/// exista -- quer dizer que ninguem precisou dela ainda, e que a primeira
/// entrada vai ter de escrever por que.
///
/// Um arquivo com um SHA-256 solto em hexadecimal casaria o crivo
/// `TudoHex`, porque de fora nao ha como distinguir um resumo de uma chave. O
/// vies e proposital: catalogar um resumo custa uma linha aqui, e deixar uma
/// chave passar custa a chave.
pub const ISENTOS: &[(&str, &str)] = &[];

/// O que a varredura achou.
#[derive(Debug, Default)]
pub struct Relatorio {
    /// Os que contam para a catraca.
    pub soltos: Vec<Solto>,
    /// Isentos catalogados que nao existem mais no disco.
    ///
    /// Isencao que sobra e isencao que ninguem le -- e a mesma falha da chave
    /// morta na fabrica de idiomas, vista do outro lado.
    pub isentos_sumiram: Vec<String>,
    /// Quantos arquivos a varredura olhou. E o denominador do "zero falso
    /// positivo": sem ele, zero achados tanto pode ser crivo certo quanto
    /// varredura que nao saiu do lugar.
    pub arquivos: usize,
}

/// A raiz do repositorio: sobe do `Cargo.toml` deste crate ate achar `.git`.
///
/// Sobe ate o `.git` -- e nao para no `phxsql/` -- porque a fronteira que
/// importa e a do `git add`, e ela e a do repositorio inteiro. Sem `.git`
/// (fonte extraido de um pacote, por exemplo), cai no `phxsql/`, que e o que
/// esta casa escreve.
pub fn raiz() -> PathBuf {
    let projeto = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("o crate mora em <raiz>/crates/phxsql-server")
        .to_path_buf();
    let mut p: &Path = &projeto;
    loop {
        if p.join(".git").exists() {
            return p.to_path_buf();
        }
        match p.parent() {
            Some(pai) => p = pai,
            None => return projeto,
        }
    }
}

/// O crivo, separado da varredura DE PROPOSITO: e uma funcao pura de nome,
/// tamanho e conteudo, entao a prova dos dois sentidos nao precisa escrever
/// chave nenhuma dentro do repositorio para exercita-lo.
///
/// `conteudo` e `None` quando o arquivo e grande demais para se ler -- ai so o
/// nome decide. Quem decide ler e a varredura, nao o crivo.
pub fn crivo(nome: &str, conteudo: Option<&[u8]>) -> Option<Motivo> {
    let minusculo = nome.to_ascii_lowercase();
    if NOMES.iter().any(|n| *n == minusculo) {
        return Some(Motivo::Nome);
    }
    if let Some((_, ext)) = minusculo.rsplit_once('.') {
        if EXTENSOES.contains(&ext) {
            return Some(Motivo::Nome);
        }
    }
    let b = conteudo?;
    let limpo: Vec<u8> = b
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if limpo.len() >= MINIMO_DE_HEX
        && limpo.len() % 2 == 0
        && limpo.iter().all(|c| c.is_ascii_hexdigit())
    {
        return Some(Motivo::TudoHex);
    }
    let texto = String::from_utf8_lossy(b);
    if texto.contains(PEM_ABRE) && texto.contains(PEM_PRIVADA) {
        return Some(Motivo::Pem);
    }
    None
}

/// Varre a raiz do repositorio.
pub fn varrer() -> Relatorio {
    varrer_em(&raiz())
}

/// Varre uma raiz qualquer. Publica porque a prova dos dois sentidos monta uma
/// arvore de mentira num diretorio temporario: repor o defeito DENTRO do
/// repositorio seria criar exatamente o lixo que este modulo proibe.
pub fn varrer_em(raiz: &Path) -> Relatorio {
    let mut r = Relatorio::default();
    andar(raiz, raiz, &mut r);
    r.soltos
        .retain(|s| !ISENTOS.iter().any(|(a, _)| *a == s.caminho));
    for (arquivo, _) in ISENTOS {
        if !raiz.join(arquivo).exists() {
            r.isentos_sumiram.push((*arquivo).to_string());
        }
    }
    r
}

fn andar(raiz: &Path, pasta: &Path, r: &mut Relatorio) {
    let mut entradas: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    // `read_dir` nao promete ordem, e relatorio que muda de ordem a cada
    // corrida nao se compara com o anterior.
    entradas.sort();
    for p in entradas {
        // Ligacao simbolica nao se segue: ela sai da arvore, e o que esta
        // fora da arvore nao entra em commit nenhum.
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            let nome = p.file_name().unwrap_or_default().to_string_lossy();
            if PULADOS.iter().any(|(d, _)| *d == nome) {
                continue;
            }
            andar(raiz, &p, r);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        r.arquivos += 1;
        let bytes = meta.len();
        let nome = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let conteudo = if (MINIMO_DE_HEX as u64..=TETO_DE_LEITURA).contains(&bytes) {
            std::fs::read(&p).ok()
        } else {
            None
        };
        if let Some(motivo) = crivo(&nome, conteudo.as_deref()) {
            let caminho = p
                .strip_prefix(raiz)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            r.soltos.push(Solto {
                caminho,
                bytes,
                motivo,
            });
        }
    }
}

/// A catraca. **So desce**, e zero e o numero certo: nenhum arquivo do
/// repositorio casa o crivo hoje (7.304 varridos, 23/09/2026). Quem precisar
/// de um entra em [`ISENTOS`] com o motivo, nao na catraca.
pub const TETO_SEGREDO_SOLTO: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    /// Hexadecimal de mentira, do tamanho de uma privada de 256 bits. Nao e
    /// chave de coisa nenhuma: e o FORMATO que o crivo procura, e um teste que
    /// gerasse material de verdade seria o proprio defeito.
    const HEX_DE_MENTIRA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// A catraca do pedido 402.
    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn nenhum_segredo_solto_na_arvore() {
        let r = varrer();
        assert!(
            r.arquivos > 100,
            "a varredura olhou {} arquivo(s) -- nao saiu do lugar, e zero achado nao valeria nada",
            r.arquivos
        );
        assert!(
            r.soltos.len() <= TETO_SEGREDO_SOLTO,
            "{} arquivo(s) com cara de chave na arvore (teto {TETO_SEGREDO_SOLTO}, \
             de {} varridos).\n\
             Apague o arquivo -- se ele nasceu de uma corrida de teste, o teste tem de \
             cria-lo dentro do DirTemp dele (veja `cifra_fio.arquivo` em \
             tests/laco-do-unico-secundario.rs). Se o arquivo for legitimo, entre em \
             ISENTOS com o motivo escrito.\n{}",
            r.soltos.len(),
            r.arquivos,
            r.soltos
                .iter()
                .map(|s| format!(
                    "  {}  ({} bytes) -- {}",
                    s.caminho,
                    s.bytes,
                    s.motivo.dizer()
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// O outro lado do laco: isencao catalogada que nao existe mais no disco.
    #[test]
    fn o_catalogo_de_isentos_bate_com_o_disco() {
        let r = varrer();
        assert!(
            r.isentos_sumiram.is_empty(),
            "ISENTOS cataloga arquivo que nao existe mais: {}",
            r.isentos_sumiram.join(", ")
        );
    }

    /// **Prova real, contra o SISTEMA DE ARQUIVOS e nos dois sentidos.**
    ///
    /// Monta uma arvore de mentira num `DirTemp` -- repor o defeito dentro do
    /// repositorio seria criar o lixo que este modulo existe para proibir --,
    /// e mede: com a chave solta a varredura acusa; sem ela, nao acusa.
    #[test]
    fn a_varredura_acusa_a_chave_solta_e_cala_quando_ela_sai() {
        let d = DirTemp::novo("segredo-solto");
        std::fs::create_dir_all(d.join("crates/phxsql-server")).unwrap();
        std::fs::write(d.join("crates/phxsql-server/Cargo.toml"), "[package]\n").unwrap();
        std::fs::write(d.join("README.md"), "um texto qualquer\n").unwrap();

        // VERDE antes: a arvore limpa nao acusa nada.
        let antes = varrer_em(&d);
        assert_eq!(antes.soltos.len(), 0, "arvore limpa nao pode acusar");
        assert_eq!(antes.arquivos, 2);

        // VERMELHO: o defeito do pedido 402, do tamanho exato que foi medido.
        let alvo = d.join("crates/phxsql-server/chave-do-fio.hex");
        std::fs::write(&alvo, format!("{HEX_DE_MENTIRA}\n")).unwrap();
        assert_eq!(std::fs::metadata(&alvo).unwrap().len(), 65);
        let com = varrer_em(&d);
        assert_eq!(com.soltos.len(), 1, "a chave solta TEM de acusar");
        assert_eq!(
            com.soltos[0].caminho,
            "crates/phxsql-server/chave-do-fio.hex"
        );
        assert_eq!(com.soltos[0].bytes, 65);

        // O renomeado tambem: e o furo que o `.gitignore` tem e este nao.
        std::fs::rename(&alvo, d.join("crates/phxsql-server/anotacao.txt")).unwrap();
        let renomeado = varrer_em(&d);
        assert_eq!(renomeado.soltos.len(), 1, "renomear nao esconde o conteudo");
        assert_eq!(renomeado.soltos[0].motivo, Motivo::TudoHex);

        // VERDE depois: o conserto e apagar.
        std::fs::remove_file(d.join("crates/phxsql-server/anotacao.txt")).unwrap();
        assert_eq!(varrer_em(&d).soltos.len(), 0);
    }

    /// O relatorio NUNCA carrega bytes do arquivo -- nem por acidente de
    /// formatacao. Se um dia alguem puser um campo `conteudo` no [`Solto`],
    /// este teste cai.
    #[test]
    fn o_relatorio_nao_vaza_um_unico_byte() {
        let d = DirTemp::novo("segredo-nao-vaza");
        std::fs::write(d.join("chave-do-fio.hex"), format!("{HEX_DE_MENTIRA}\n")).unwrap();
        let r = varrer_em(&d);
        let impresso = format!(
            "{:?} {} {} {}",
            r.soltos[0],
            r.soltos[0].caminho,
            r.soltos[0].bytes,
            r.soltos[0].motivo.dizer()
        );
        assert!(
            !impresso.contains(HEX_DE_MENTIRA),
            "o relatorio imprimiu o conteudo do arquivo"
        );
    }

    /// O crivo, nos dois sentidos, sem tocar em disco.
    #[test]
    fn o_crivo_pega_a_chave_e_deixa_o_legitimo_passar() {
        // Pega.
        assert_eq!(crivo("chave-do-fio.hex", None), Some(Motivo::Nome));
        assert_eq!(crivo("do-fio.chave", None), Some(Motivo::Nome));
        assert_eq!(crivo("servidor.key", None), Some(Motivo::Nome));
        assert_eq!(crivo("id_ed25519", None), Some(Motivo::Nome));
        assert_eq!(
            crivo(
                "anotacao.txt",
                Some(format!("{HEX_DE_MENTIRA}\n").as_bytes())
            ),
            Some(Motivo::TudoHex)
        );
        let pem = format!("{PEM_ABRE} {PEM_PRIVADA}-----\nQUJD\n");
        assert_eq!(crivo("nota.txt", Some(pem.as_bytes())), Some(Motivo::Pem));

        // Deixa passar. Estes sao o estrago que a guarda nao pode causar.
        assert_eq!(crivo("config.json", Some(br#"{"token":"t"}"#)), None);
        assert_eq!(crivo("servidor.rs", Some(b"fn main() {}\n")), None);
        // Curto demais para ser 256 bits: um "deadbeef" nao e chave.
        assert_eq!(crivo("nota.txt", Some(b"deadbeef\n")), None);
        // Hexadecimal SOBRANDO no meio de texto nao conta: o crivo exige o
        // arquivo INTEIRO, senao todo documento com um hash dentro reprovaria.
        let prosa = format!("o resumo e {HEX_DE_MENTIRA}, e ele nao e chave\n");
        assert_eq!(crivo("doc.md", Some(prosa.as_bytes())), None);
        // Arquivo grande nao se le: so o nome decide.
        assert_eq!(crivo("base.reg", None), None);
    }

    /// Este fonte nao pode se acusar. Sem as metades separadas, o marcador PEM
    /// escrito inteiro faria o conferidor reprovar a si mesmo -- e relatorio
    /// que se acusa e relatorio que ninguem le ate o fim.
    #[test]
    fn o_conferidor_nao_se_acusa() {
        let meu = include_str!("conferidor_segredos.rs");
        assert_eq!(crivo("conferidor_segredos.rs", Some(meu.as_bytes())), None);
    }
}

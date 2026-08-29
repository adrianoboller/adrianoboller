//! Restauracao de um backup: o caminho de volta.
//!
//! # O desenho, e por que este e nao outro
//!
//! Copiar de volta e mais do que copiar: e decidir o que fazer com o que ja
//! esta la. As tres saidas possiveis eram parar o processo e trocar por cima,
//! restaurar ao lado e trocar os diretorios, ou restaurar com OUTRO nome. A
//! escolhida e a terceira como caminho principal, e a segunda como o
//! "restaurar por cima":
//!
//! * **Com outro nome** (o padrao) nao destroi nada, nao precisa parar o
//!   servico e nao precisa segurar a trava de dados durante a copia -- o
//!   database de destino ainda nao existe, entao ninguem esta lendo dele. Erra
//!   sem estrago: quem restaurou o backup errado apaga o database e refaz.
//!   Depois de olhar o dado restaurado, renomear e um `excluir_tabela` a menos
//!   do que teria sido restaurar por cima do original.
//!
//! * **Por cima** (`por_cima`) e a mesma mecanica com um passo a mais: o
//!   database antigo e movido para o lado ANTES de o novo entrar, e nao
//!   apagado. Continua no disco, com o caminho na resposta, ate quem
//!   restaurou conferir e apagar. Restauracao que apaga o que substituiu nao
//!   tem volta -- e a hora em que se descobre que o backup era do mes errado e
//!   sempre depois.
//!
//! Parar o processo inteiro nao entrou porque nao e preciso: o `servico_parar`
//! ja para a porta de dados sem derrubar o processo, e a interface web
//! continua no ar para religar. O `por_cima` exige justamente isso -- porta de
//! dados parada -- e quem exige e o servidor, que e quem sabe o estado dela.
//!
//! # A ordem que faz a conferencia valer
//!
//! ```text
//! 1. le o manifesto do backup
//! 2. extrai para um PALCO fora da raiz de dados
//! 3. confere o SHA-256 de cada arquivo contra o manifesto  <-- aqui recusa
//! 4. so entao troca, com um rename
//! ```
//!
//! O passo 3 acontece antes de o destino ser tocado, e essa e a regra do
//! modulo: **backup corrompido nao vira database restaurado pela metade**. O
//! palco fica FORA da raiz de dados de proposito -- um diretorio dentro da
//! raiz seria listado como database enquanto a copia acontece, e o `bancos`
//! mostraria um banco meio escrito.
//!
//! # O que a restauracao NAO faz
//!
//! Nao junta, nao mescla e nao migra formato: o database que sai e byte a byte
//! o que entrou no backup. Nao restaura a raiz inteira de uma vez -- um
//! database por vez, porque restaurar seis por cima com um clique e seis
//! estragos com um clique. E nao confere assinatura: o manifesto prova que o
//! backup nao APODRECEU, nao que ninguem o reescreveu de proposito (ver
//! `docs/RESTAURACAO.md`).

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::json::Json;
use phxsql_core::zip::LeitorZip;

use crate::backup::{listar, relativo, MANIFESTO};

/// De que o backup e copia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Escopo {
    /// A raiz de dados inteira: cada diretorio de primeiro nivel e um database.
    Raiz,
    /// Um database so, e os caminhos de dentro ja sao relativos a ele.
    Database(String),
}

/// O que um backup traz dentro -- lido do manifesto, sem abrir o resto.
#[derive(Debug, Clone)]
pub struct Conteudo {
    pub origem: String,
    /// Arquivo unico (`.zip`) ou arvore de diretorios copiada.
    pub zip: bool,
    /// A versao do PhxSql que gravou o backup.
    pub versao: String,
    pub quando: String,
    pub arquivos: usize,
    pub bytes: u64,
    pub escopo: Escopo,
    /// O que da para restaurar deste backup.
    pub databases: Vec<String>,
    /// O escopo veio ESCRITO no manifesto, ou foi deduzido dos caminhos?
    ///
    /// Backup gravado antes de a restauracao existir nao traz o campo. A
    /// deducao acerta em todo caso comum, e a tela diz que deduziu em vez de
    /// afirmar.
    pub declarado: bool,
}

/// O que aconteceu na restauracao.
#[derive(Debug, Clone)]
pub struct Restaurado {
    /// O database que passou a existir.
    pub database: String,
    /// O que veio de dentro do backup.
    pub de: String,
    pub arquivos: usize,
    pub bytes: u64,
    pub tabelas: Vec<String>,
    /// Havia um database ali antes?
    pub substituiu: bool,
    /// Onde o anterior foi parar, quando havia um.
    pub anterior_em: Option<String>,
}

// ------------------------------------------------------------------- a fonte

/// De onde os bytes saem: uma pasta copiada ou um ZIP.
enum Fonte {
    Pasta(PathBuf),
    Zip(Box<LeitorZip<File>>),
}

impl Fonte {
    fn abrir(origem: &Path) -> Result<Fonte> {
        if origem.is_dir() {
            if !origem.join(MANIFESTO).is_file() {
                return Err(PhxError::NaoEncontrado(format!(
                    "{} nao tem {MANIFESTO}: nao e um backup do PhxSql",
                    origem.display()
                )));
            }
            return Ok(Fonte::Pasta(origem.to_path_buf()));
        }
        if !origem.is_file() {
            return Err(PhxError::NaoEncontrado(format!(
                "{} nao existe",
                origem.display()
            )));
        }
        let leitor = LeitorZip::abrir(File::open(origem)?)?;
        Ok(Fonte::Zip(Box::new(leitor)))
    }

    /// Todo arquivo de dentro, com caminho relativo e barra normal.
    fn nomes(&self) -> Result<Vec<String>> {
        Ok(match self {
            Fonte::Pasta(raiz) => listar(raiz)?.iter().map(|c| relativo(raiz, c)).collect(),
            Fonte::Zip(z) => z.entradas().iter().map(|e| e.nome.clone()).collect(),
        })
    }

    fn ler(&mut self, caminho: &str) -> Result<Vec<u8>> {
        match self {
            Fonte::Pasta(raiz) => Ok(std::fs::read(raiz.join(caminho))?),
            Fonte::Zip(z) => z.ler(caminho),
        }
    }
}

/// Este caminho, vindo de dentro de um backup, pode virar arquivo?
///
/// # Por que a pergunta existe
///
/// O que esta escrito num manifesto e num ZIP e texto que veio de FORA. Um
/// backup com `../../etc/cron.d/tarefa` la dentro escreveria fora da raiz de
/// dados se a restauracao apenas juntasse os pedacos -- e o backup e
/// justamente o arquivo que anda por pen drive, anexo e nuvem, longe de quem
/// o gerou. E a mesma conferencia que o `despachar` ja faz com nome de tabela,
/// pelo mesmo motivo, num caminho que ninguem digita.
fn caminho_seguro(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.contains(':') {
        return false;
    }
    rel.split('/')
        .all(|parte| !parte.is_empty() && parte != "." && parte != ".." && !parte.contains('\0'))
}

/// O manifesto lido: o que cada arquivo tem de ter.
struct Manifesto {
    esperados: BTreeMap<String, (u64, String)>,
    versao: String,
    quando: String,
    escopo: Escopo,
    declarado: bool,
}

fn ler_manifesto(fonte: &mut Fonte, origem: &Path) -> Result<Manifesto> {
    let texto = String::from_utf8_lossy(&fonte.ler(MANIFESTO).map_err(|e| {
        PhxError::NaoEncontrado(format!(
            "{} nao tem {MANIFESTO}: nao e um backup do PhxSql ({e})",
            origem.display()
        ))
    })?)
    .into_owned();
    let json = Json::analisar(&texto)?;

    let mut esperados = BTreeMap::new();
    for a in json.campo("conteudo").and_then(Json::lista).unwrap_or(&[]) {
        let caminho = a.texto_ou("caminho", "").to_string();
        if !caminho_seguro(&caminho) {
            return Err(PhxError::Corrompido(format!(
                "o manifesto traz o caminho {caminho:?}, que sairia da pasta de destino"
            )));
        }
        esperados.insert(
            caminho,
            (
                a.campo("bytes").and_then(Json::inteiro).unwrap_or(0) as u64,
                a.texto_ou("sha256", "").to_string(),
            ),
        );
    }
    if esperados.is_empty() {
        return Err(PhxError::Corrompido(format!(
            "o {MANIFESTO} de {} nao lista arquivo nenhum",
            origem.display()
        )));
    }

    // O escopo ESCRITO manda. Sem ele -- backup mais velho que a
    // restauracao --, deduz.
    let (escopo, declarado) = match json.campo("escopo").and_then(Json::texto) {
        Some("database") => (
            Escopo::Database(nome_do_database(&json, origem, &esperados)),
            true,
        ),
        Some("raiz") => (Escopo::Raiz, true),
        _ => (deduzir_escopo(origem, &esperados), false),
    };

    Ok(Manifesto {
        esperados,
        versao: json.texto_ou("phxsql", "?").to_string(),
        quando: json.texto_ou("quando", "").to_string(),
        escopo,
        declarado,
    })
}

/// O nome do database quando o manifesto diz que o escopo e um so.
fn nome_do_database(
    json: &Json,
    origem: &Path,
    esperados: &BTreeMap<String, (u64, String)>,
) -> String {
    match json.campo("database").and_then(Json::texto) {
        Some(n) if !n.is_empty() => n.to_string(),
        // Escopo declarado sem nome nao deveria acontecer; cair na deducao e
        // melhor do que devolver vazio e a tela mostrar um banco sem nome.
        _ => match deduzir_escopo(origem, esperados) {
            Escopo::Database(n) => n,
            Escopo::Raiz => nome_pelo_arquivo(origem),
        },
    }
}

/// Deduz o escopo pelos caminhos, para backup gravado antes do campo existir.
///
/// A regra: numa copia da RAIZ todo arquivo mora dentro de um diretorio de
/// database, entao nenhum caminho fica solto no primeiro nivel. Um `.reg`
/// solto no primeiro nivel so acontece em copia de UM database.
fn deduzir_escopo(origem: &Path, esperados: &BTreeMap<String, (u64, String)>) -> Escopo {
    if esperados.keys().any(|c| !c.contains('/')) {
        Escopo::Database(nome_pelo_arquivo(origem))
    } else {
        Escopo::Raiz
    }
}

/// O nome do banco tirado do nome do arquivo, `Banco_Admin_Data_HoraMin.zip`.
///
/// So serve para MOSTRAR de que banco a copia era: o conteudo nao depende
/// disso, e quem restaura escolhe o nome de destino de qualquer jeito.
fn nome_pelo_arquivo(origem: &Path) -> String {
    let nome = origem
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match nome.split('_').next() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => nome,
    }
}

/// Os databases que este backup contem.
fn databases_de(escopo: &Escopo, esperados: &BTreeMap<String, (u64, String)>) -> Vec<String> {
    match escopo {
        Escopo::Database(n) => vec![n.clone()],
        Escopo::Raiz => {
            let mut nomes: Vec<String> = esperados
                .keys()
                .filter_map(|c| c.split_once('/').map(|(db, _)| db.to_string()))
                .collect();
            nomes.sort();
            nomes.dedup();
            nomes
        }
    }
}

/// Os nomes de tabela que os caminhos descrevem, qualificados por schema.
///
/// Aproximacao de VITRINE, e nao catalogo: sai dos nomes de arquivo, sem abrir
/// nenhum `.reg`. Serve para a tela dizer o que tem dentro antes de restaurar;
/// depois de restaurado, quem responde e o catalogo de verdade.
pub fn tabelas_dos_caminhos(caminhos: &[String]) -> Vec<String> {
    let mut nomes: Vec<String> = caminhos
        .iter()
        .filter_map(|c| {
            let sem_ext = c.strip_suffix(".reg")?;
            let (dir, arquivo) = match sem_ext.rsplit_once('/') {
                Some((d, a)) => (Some(d), a),
                None => (None, sem_ext),
            };
            // `precos_002` e volume de `precos`; `precos_historico` nao e.
            let tabela = match arquivo.rsplit_once('_') {
                Some((antes, sufixo))
                    if !antes.is_empty()
                        && !sufixo.is_empty()
                        && sufixo.bytes().all(|b| b.is_ascii_digit()) =>
                {
                    antes
                }
                _ => arquivo,
            };
            Some(match dir {
                Some(d) => format!("{d}.{tabela}"),
                None => tabela.to_string(),
            })
        })
        .collect();
    nomes.sort();
    nomes.dedup();
    nomes
}

/// Le o que um backup contem, sem extrair nada.
///
/// De um ZIP le so o fim do arquivo e o manifesto: da para listar dez backups
/// de um gigabyte sem ler um gigabyte.
pub fn conteudo(origem: &Path) -> Result<Conteudo> {
    let mut fonte = Fonte::abrir(origem)?;
    let m = ler_manifesto(&mut fonte, origem)?;
    Ok(Conteudo {
        origem: origem.display().to_string(),
        zip: matches!(fonte, Fonte::Zip(_)),
        versao: m.versao,
        quando: m.quando,
        arquivos: m.esperados.len(),
        bytes: m.esperados.values().map(|(b, _)| *b).sum(),
        databases: databases_de(&m.escopo, &m.esperados),
        escopo: m.escopo,
        declarado: m.declarado,
    })
}

/// As tabelas de um database dentro do backup, para a tela mostrar.
pub fn tabelas_de(origem: &Path, de: &str) -> Result<Vec<String>> {
    let mut fonte = Fonte::abrir(origem)?;
    let m = ler_manifesto(&mut fonte, origem)?;
    let (_, prefixo) = escolher(&m.escopo, de)?;
    let caminhos: Vec<String> = m
        .esperados
        .keys()
        .filter_map(|c| c.strip_prefix(&prefixo).map(str::to_string))
        .collect();
    Ok(tabelas_dos_caminhos(&caminhos))
}

/// Decide QUAL database do backup sera restaurado, e com que prefixo ele mora
/// la dentro.
fn escolher(escopo: &Escopo, de: &str) -> Result<(String, String)> {
    let de = de.trim();
    match escopo {
        Escopo::Database(n) => {
            if !de.is_empty() && de != n {
                return Err(PhxError::NaoEncontrado(format!(
                    "este backup e do database {n}, e nao tem {de} dentro"
                )));
            }
            Ok((n.clone(), String::new()))
        }
        Escopo::Raiz => {
            if de.is_empty() {
                return Err(PhxError::Esquema(
                    "este backup e da raiz inteira: diga em \"de\" qual database restaurar".into(),
                ));
            }
            crate::catalogo::validar_nome("database", de)?;
            Ok((de.to_string(), format!("{de}/")))
        }
    }
}

// ------------------------------------------------------------- a restauracao

/// Um backup ja extraido e conferido, esperando so a troca.
///
/// # Por que em duas etapas
///
/// A extracao e a conferencia sao o pedaco caro -- le o backup inteiro e
/// calcula um SHA-256 por arquivo -- e acontecem FORA da raiz de dados, sem a
/// trava. A troca e um `rename` e acontece com a trava na mao. Fazer tudo
/// travado pararia o servidor pelo tempo da copia; fazer tudo destravado
/// deixaria dois pedidos criarem o mesmo database ao mesmo tempo.
pub struct Preparada {
    palco: PathBuf,
    de: String,
    arquivos: usize,
    bytes: u64,
    tabelas: Vec<String>,
}

impl Drop for Preparada {
    /// Palco que nao virou database e lixo, e lixo do tamanho do backup.
    ///
    /// No `Drop` e nao no fim de `preparar`: entre preparar e confirmar ha um
    /// portao de permissao e uma trava, e qualquer um dos dois pode recusar.
    fn drop(&mut self) {
        if !self.palco.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.palco);
        }
    }
}

impl Preparada {
    pub fn de(&self) -> &str {
        &self.de
    }

    pub fn arquivos(&self) -> usize {
        self.arquivos
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn tabelas(&self) -> &[String] {
        &self.tabelas
    }

    /// Extrai o database escolhido para um palco fora da raiz e confere tudo.
    ///
    /// Nao toca no destino. Backup que nao confere para AQUI, e o unico
    /// estrago possivel e um diretorio temporario, que o `Drop` apaga.
    pub fn preparar(origem: &Path, base: &Path, de: &str) -> Result<Preparada> {
        let mut fonte = Fonte::abrir(origem)?;
        let m = ler_manifesto(&mut fonte, origem)?;
        let (de, prefixo) = escolher(&m.escopo, de)?;

        let meus: Vec<(String, u64, String)> = m
            .esperados
            .iter()
            .filter_map(|(c, (bytes, sha))| {
                let dentro = c.strip_prefix(&prefixo)?;
                (!dentro.is_empty()).then(|| (c.clone(), *bytes, sha.clone()))
            })
            .collect();
        if meus.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "o backup nao tem arquivo nenhum do database {de}"
            )));
        }

        // Arquivo que existe na copia e nao esta no manifesto e a terceira
        // forma de um backup estragar -- as outras duas sao sumir e mudar --,
        // e a unica que restauraria dado que ninguem conferiu. O `conferir`
        // ja a acha; aqui ela RECUSA, porque aqui o arquivo viraria tabela.
        for nome in fonte.nomes()? {
            if nome == MANIFESTO {
                continue;
            }
            if !caminho_seguro(&nome) {
                return Err(PhxError::Corrompido(format!(
                    "o backup traz o caminho {nome:?}, que sairia da pasta de destino"
                )));
            }
            if nome.starts_with(&prefixo)
                && nome.len() > prefixo.len()
                && !m.esperados.contains_key(&nome)
            {
                return Err(PhxError::Corrompido(format!(
                    "{nome} esta no backup e nao esta no {MANIFESTO}: \
                     a copia foi mexida depois de gravada"
                )));
            }
        }

        let palco = palco_para(base)?;
        let mut escrita = 0u64;
        for (caminho, bytes, sha) in &meus {
            let dados = fonte.ler(caminho)?;
            if dados.len() as u64 != *bytes {
                return Err(PhxError::Corrompido(format!(
                    "{caminho}: o {MANIFESTO} diz {bytes} bytes e o backup tem {}",
                    dados.len()
                )));
            }
            let confere = para_hex(&sha256(&dados));
            if &confere != sha {
                return Err(PhxError::Corrompido(format!(
                    "{caminho}: o SHA-256 nao bate com o {MANIFESTO} -- \
                     este backup nao esta integro e NADA foi restaurado"
                )));
            }
            let dentro = caminho.strip_prefix(&prefixo).unwrap_or(caminho);
            let alvo = palco.join(dentro);
            if let Some(pai) = alvo.parent() {
                std::fs::create_dir_all(pai)?;
            }
            // Ao disco de verdade, e nao ao cache: o proximo passo e um
            // `rename`, e um rename e instantaneo mesmo quando o conteudo
            // ainda nao chegou no prato. Restaurar e raro; pagar o `fsync`
            // aqui e barato perto de descobrir depois que o database novo
            // ficou com um arquivo vazio.
            let arquivo = File::create(&alvo)?;
            {
                use std::io::Write;
                let mut w = std::io::BufWriter::new(&arquivo);
                w.write_all(&dados)?;
                w.flush()?;
            }
            arquivo.sync_all()?;
            escrita += dados.len() as u64;
        }

        Ok(Preparada {
            tabelas: tabelas_dos_caminhos(
                &meus
                    .iter()
                    .map(|(c, _, _)| c.strip_prefix(&prefixo).unwrap_or(c).to_string())
                    .collect::<Vec<_>>(),
            ),
            palco,
            de,
            arquivos: meus.len(),
            bytes: escrita,
        })
    }

    /// A troca. Quem chama segura a trava de dados.
    ///
    /// Com `por_cima`, o database que estava ali e movido para fora da raiz --
    /// nunca apagado -- e o caminho volta na resposta.
    pub fn confirmar(mut self, base: &Path, destino: &str, por_cima: bool) -> Result<Restaurado> {
        crate::catalogo::validar_nome("database", destino)?;
        let alvo = base.join(destino);
        let existia = alvo.exists();
        if existia && !por_cima {
            return Err(PhxError::Esquema(format!(
                "o database {destino} ja existe. Escolha outro nome, ou peca a \
                 restauracao POR CIMA -- que substitui o que esta la"
            )));
        }
        std::fs::create_dir_all(base)?;

        // O antigo sai da raiz ANTES de o novo entrar, e vai para fora dela:
        // um "banco.antigo" ao lado seria listado como database e apareceria
        // na arvore de quem nem sabe que houve restauracao.
        let mut anterior_em = None;
        if existia {
            let guardado = vizinho_da_base(base, &format!("substituido-{destino}"))?;
            renomear_ou_copiar(&alvo, &guardado)?;
            anterior_em = Some(guardado.display().to_string());
        }

        let palco = std::mem::take(&mut self.palco);
        if let Err(e) = renomear_ou_copiar(&palco, &alvo) {
            // Falhou na troca: o antigo volta para o lugar. Sem isto, uma
            // falha aqui deixaria a raiz SEM o database -- o unico jeito de a
            // restauracao piorar o que ja existia.
            if let Some(guardado) = &anterior_em {
                let _ = renomear_ou_copiar(Path::new(guardado), &alvo);
            }
            let _ = std::fs::remove_dir_all(&palco);
            return Err(e);
        }

        Ok(Restaurado {
            database: destino.to_string(),
            de: std::mem::take(&mut self.de),
            arquivos: self.arquivos,
            bytes: self.bytes,
            tabelas: std::mem::take(&mut self.tabelas),
            substituiu: existia,
            anterior_em,
        })
    }
}

/// Um diretorio de trabalho VIZINHO da raiz de dados, nunca dentro dela.
///
/// Vizinho, e nao em `/tmp`: o `rename` que troca o database no fim so e
/// instantaneo -- e atomico -- dentro do mesmo sistema de arquivos. Com a
/// raiz de dados num disco proprio, `/tmp` seria outro sistema de arquivos e a
/// troca viraria uma copia demorada no meio da trava.
fn vizinho_da_base(base: &Path, para_que: &str) -> Result<PathBuf> {
    let carimbo = format!(
        ".phxsql-{para_que}-{}-{}",
        std::process::id(),
        crate::util::agora_ms()
    );
    let pai = match base.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        // `base: "dados"` -- o padrao do config.json -- tem pai VAZIO. O
        // vizinho ali e o diretorio de trabalho, que e onde `dados` esta de
        // verdade. Cair no `/tmp` seria quase sempre cair em OUTRO sistema de
        // arquivos (num `tmpfs`, inclusive), e ai a troca deixa de ser um
        // rename e vira uma copia do database inteiro -- para dentro da RAM.
        _ => std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
    };
    Ok(pai.join(carimbo))
}

fn palco_para(base: &Path) -> Result<PathBuf> {
    let palco = vizinho_da_base(base, "restaurando")?;
    // Quando o vizinho nao aceita escrita, o temporario do sistema ainda
    // resolve -- so custa uma copia em vez do rename.
    match std::fs::create_dir_all(&palco) {
        Ok(()) => Ok(palco),
        Err(_) => {
            let alternativo = std::env::temp_dir().join(
                palco
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "phxsql-restaurando".into()),
            );
            std::fs::create_dir_all(&alternativo)?;
            Ok(alternativo)
        }
    }
}

/// Move; se o `rename` nao servir -- outro sistema de arquivos --, copia.
fn renomear_ou_copiar(de: &Path, para: &Path) -> Result<()> {
    if std::fs::rename(de, para).is_ok() {
        return Ok(());
    }
    copiar_arvore(de, para)?;
    std::fs::remove_dir_all(de)?;
    Ok(())
}

fn copiar_arvore(de: &Path, para: &Path) -> Result<()> {
    std::fs::create_dir_all(para)?;
    for entrada in std::fs::read_dir(de)? {
        let entrada = entrada?;
        let origem = entrada.path();
        let Some(nome) = origem.file_name() else {
            continue;
        };
        let alvo = para.join(nome);
        if origem.is_dir() {
            copiar_arvore(&origem, &alvo)?;
        } else {
            std::fs::copy(&origem, &alvo)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "phxrst-{nome}-{}-{}",
            std::process::id(),
            crate::util::agora_ms()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Uma raiz com dois databases, um deles com schema e com volume.
    fn dados_de_exemplo(raiz: &Path) {
        std::fs::create_dir_all(raiz.join("Z/matriz")).unwrap();
        std::fs::create_dir_all(raiz.join("Financeiro")).unwrap();
        std::fs::write(raiz.join("Z/clientes.reg"), b"registros de clientes").unwrap();
        std::fs::write(raiz.join("Z/clientes.ndx"), b"indices de clientes").unwrap();
        std::fs::write(raiz.join("Z/clientes_002.reg"), b"volume dois").unwrap();
        std::fs::write(raiz.join("Z/matriz/pedidos.reg"), b"pedidos do schema").unwrap();
        std::fs::write(raiz.join("Financeiro/contas.reg"), b"nao e para vir").unwrap();
    }

    fn zip_de(base: &Path, raiz: &Path, banco: &str) -> PathBuf {
        crate::backup::executar_zip(raiz, &base.join("copias"), banco, "ana", 1_787_000_000_000)
            .unwrap()
            .0
    }

    #[test]
    fn restaura_um_banco_do_zip_com_outro_nome() {
        let base = temp("zipnovo");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let zip = zip_de(&base, &raiz, "Z");

        let c = conteudo(&zip).unwrap();
        assert!(c.zip);
        assert_eq!(c.escopo, Escopo::Database("Z".into()));
        assert!(c.declarado, "o manifesto novo DIZ o escopo");
        assert_eq!(c.arquivos, 4);

        let p = Preparada::preparar(&zip, &raiz, "").unwrap();
        assert_eq!(p.de(), "Z");
        assert_eq!(p.arquivos(), 4);
        assert_eq!(
            p.tabelas(),
            ["clientes".to_string(), "matriz.pedidos".to_string()]
        );
        let r = p.confirmar(&raiz, "Z_restaurado", false).unwrap();
        assert!(!r.substituiu);
        assert!(r.anterior_em.is_none());

        // Byte a byte, e a hierarquia junto.
        for (arquivo, esperado) in [
            ("clientes.reg", "registros de clientes"),
            ("clientes.ndx", "indices de clientes"),
            ("clientes_002.reg", "volume dois"),
            ("matriz/pedidos.reg", "pedidos do schema"),
        ] {
            let lido = std::fs::read(raiz.join("Z_restaurado").join(arquivo)).unwrap();
            assert_eq!(String::from_utf8_lossy(&lido), esperado, "{arquivo}");
        }
        // O original continua onde estava, intocado.
        assert!(raiz.join("Z/clientes.reg").is_file());
        // E o manifesto NAO virou arquivo do database.
        assert!(!raiz.join("Z_restaurado/backup.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn restaura_da_arvore_copiada_escolhendo_o_banco() {
        let base = temp("pasta");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let copia = base.join("copia");
        crate::backup::executar(&raiz, &copia, "2026-08-29 10:00:00").unwrap();

        let c = conteudo(&copia).unwrap();
        assert!(!c.zip);
        assert_eq!(c.escopo, Escopo::Raiz);
        assert_eq!(c.databases, vec!["Financeiro".to_string(), "Z".to_string()]);

        // Sem dizer qual, a copia da raiz recusa -- e diz o que fazer.
        let Err(e) = Preparada::preparar(&copia, &raiz, "") else {
            panic!("copia da raiz restaurou sem ninguem dizer qual database");
        };
        assert!(e.to_string().contains("qual database"), "{e}");

        let p = Preparada::preparar(&copia, &raiz, "Financeiro").unwrap();
        let r = p.confirmar(&raiz, "Fin2", false).unwrap();
        assert_eq!(r.arquivos, 1);
        assert_eq!(r.tabelas, vec!["contas".to_string()]);
        assert_eq!(
            std::fs::read(raiz.join("Fin2/contas.reg")).unwrap(),
            b"nao e para vir"
        );
        // O outro banco do backup NAO veio junto.
        assert!(!raiz.join("Fin2/clientes.reg").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// **O teste do enunciado.** Um byte trocado dentro do backup e o destino
    /// nao chega a existir.
    #[test]
    fn manifesto_que_nao_confere_e_recusado_e_nada_e_escrito() {
        let base = temp("adulterado");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let copia = base.join("copia");
        crate::backup::executar(&raiz, &copia, "agora").unwrap();

        // MESMO TAMANHO, conteudo diferente: so o SHA-256 pega. Trocar o
        // tamanho junto deixaria a conferencia de bytes -- que e mais fraca --
        // passar por conferencia de conteudo.
        std::fs::write(copia.join("Z/clientes.reg"), b"registros de CLIENTES").unwrap();

        let Err(e) = Preparada::preparar(&copia, &raiz, "Z") else {
            panic!("o backup adulterado passou pela conferencia");
        };
        assert_eq!(e.nome(), "CORROMPIDO", "veio {e}");
        assert!(e.to_string().contains("SHA-256"), "{e}");
        assert!(
            !raiz.join("Zbis").exists(),
            "o destino nao pode nem comecar a existir"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Arquivo acrescentado a copia depois de gravada tambem recusa: ele
    /// viraria tabela sem ninguem ter conferido nada.
    #[test]
    fn arquivo_que_nao_esta_no_manifesto_recusa() {
        let base = temp("intruso");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let copia = base.join("copia");
        crate::backup::executar(&raiz, &copia, "agora").unwrap();
        std::fs::write(copia.join("Z/intruso.reg"), b"entrei depois").unwrap();

        let Err(e) = Preparada::preparar(&copia, &raiz, "Z") else {
            panic!("o arquivo intruso passou pela conferencia");
        };
        assert!(e.to_string().contains("nao esta no backup.json"), "{e}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Arquivo do manifesto que sumiu da copia recusa na leitura.
    #[test]
    fn arquivo_sumido_recusa() {
        let base = temp("sumido");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let copia = base.join("copia");
        crate::backup::executar(&raiz, &copia, "agora").unwrap();
        std::fs::remove_file(copia.join("Z/clientes.ndx")).unwrap();

        assert!(Preparada::preparar(&copia, &raiz, "Z").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Por cima: o antigo sai da raiz e continua no disco.
    #[test]
    fn por_cima_guarda_o_antigo_fora_da_raiz() {
        let base = temp("porcima");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let zip = zip_de(&base, &raiz, "Z");
        // O banco muda DEPOIS do backup: e o que a restauracao tem de desfazer.
        std::fs::write(raiz.join("Z/clientes.reg"), b"mudou depois do backup").unwrap();

        // Sem `por_cima`, recusa e diz o caminho.
        let e = Preparada::preparar(&zip, &raiz, "")
            .unwrap()
            .confirmar(&raiz, "Z", false)
            .unwrap_err();
        assert!(e.to_string().contains("POR CIMA"), "{e}");

        let r = Preparada::preparar(&zip, &raiz, "")
            .unwrap()
            .confirmar(&raiz, "Z", true)
            .unwrap();
        assert!(r.substituiu);
        let guardado = PathBuf::from(r.anterior_em.unwrap());
        assert_eq!(
            std::fs::read(raiz.join("Z/clientes.reg")).unwrap(),
            b"registros de clientes",
            "o backup nao voltou"
        );
        assert_eq!(
            std::fs::read(guardado.join("clientes.reg")).unwrap(),
            b"mudou depois do backup",
            "o que estava la tem de continuar existindo"
        );
        assert!(
            !guardado.starts_with(&raiz),
            "o antigo nao pode ficar dentro da raiz: viraria um database"
        );
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&guardado);
    }

    /// Caminho que sai da pasta e recusado ANTES de escrever qualquer coisa.
    /// Backup e arquivo que anda pelo mundo: o que vem escrito dentro dele nao
    /// e confiavel so por estar num ZIP.
    #[test]
    fn caminho_que_escapa_da_pasta_e_recusado() {
        assert!(caminho_seguro("Z/clientes.reg"));
        assert!(caminho_seguro("clientes.reg"));
        for mau in [
            "../fora.reg",
            "Z/../../etc/passwd",
            "/etc/passwd",
            "C:/janela.reg",
            "Z\\clientes.reg",
            "",
            "Z//clientes.reg",
            ".",
        ] {
            assert!(!caminho_seguro(mau), "{mau:?} passou");
        }

        let base = temp("escape");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        let copia = base.join("copia");
        std::fs::create_dir_all(&copia).unwrap();
        std::fs::write(
            copia.join(MANIFESTO),
            r#"{"phxsql":"9.9.9","quando":"agora","escopo":"raiz",
                "conteudo":[{"caminho":"../../fora.reg","bytes":3,"sha256":"00"}]}"#,
        )
        .unwrap();
        let e = conteudo(&copia).unwrap_err();
        assert!(e.to_string().contains("sairia da pasta"), "{e}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// **O comportamento VELHO.** Backup gravado antes de o manifesto dizer o
    /// escopo continua restaurando -- e por deducao, nao por adivinhacao
    /// otimista: um `.reg` solto no primeiro nivel so existe em copia de um
    /// database.
    #[test]
    fn backup_antigo_sem_escopo_no_manifesto_ainda_restaura() {
        let base = temp("antigo");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let copia = base.join("copia");
        crate::backup::executar(&raiz, &copia, "2026-01-01 03:00:00").unwrap();

        // Reescreve o manifesto SEM os campos novos, como a 0.18.0 gravava.
        let texto = std::fs::read_to_string(copia.join(MANIFESTO)).unwrap();
        let j = Json::analisar(&texto).unwrap();
        let velho = Json::objeto(vec![
            ("phxsql", Json::texto_de("0.18.0")),
            ("quando", Json::texto_de(j.texto_ou("quando", ""))),
            ("arquivos", Json::de_u64(j.inteiro_ou("arquivos", 0) as u64)),
            ("bytes", Json::de_u64(j.inteiro_ou("bytes", 0) as u64)),
            ("conteudo", j.campo("conteudo").cloned().unwrap()),
        ]);
        std::fs::write(copia.join(MANIFESTO), velho.escrever()).unwrap();

        let c = conteudo(&copia).unwrap();
        assert!(!c.declarado, "manifesto sem o campo nao pode dizer que tem");
        assert_eq!(c.escopo, Escopo::Raiz);
        assert_eq!(c.versao, "0.18.0");

        let r = Preparada::preparar(&copia, &raiz, "Z")
            .unwrap()
            .confirmar(&raiz, "Zvelho", false)
            .unwrap();
        assert_eq!(r.arquivos, 4);
        assert_eq!(
            std::fs::read(raiz.join("Zvelho/matriz/pedidos.reg")).unwrap(),
            b"pedidos do schema"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// O mesmo, para o ZIP de um banco so: la os caminhos vem soltos no
    /// primeiro nivel, e a deducao tem de dizer "database", nao "raiz".
    #[test]
    fn zip_antigo_de_um_banco_e_deduzido_como_database() {
        let base = temp("antigozip");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let zip = zip_de(&base, &raiz, "Z");

        // Refaz o ZIP com o manifesto da 0.18.0 dentro.
        let mut leitor = LeitorZip::abrir(File::open(&zip).unwrap()).unwrap();
        let nomes: Vec<String> = leitor.entradas().iter().map(|e| e.nome.clone()).collect();
        let mut novo = phxsql_core::zip::Zip::novo(1_787_000_000_000);
        for nome in &nomes {
            let dados = leitor.ler(nome).unwrap();
            if nome == MANIFESTO {
                let j = Json::analisar(&String::from_utf8_lossy(&dados)).unwrap();
                let velho = Json::objeto(vec![
                    ("phxsql", Json::texto_de("0.18.0")),
                    ("quando", Json::texto_de(j.texto_ou("quando", ""))),
                    ("conteudo", j.campo("conteudo").cloned().unwrap()),
                ]);
                novo.acrescentar(nome, velho.escrever().as_bytes());
            } else {
                novo.acrescentar(nome, &dados);
            }
        }
        let velho_zip = base.join("copias/Z_ana_2026-01-01_0300.zip");
        std::fs::write(&velho_zip, novo.terminar()).unwrap();

        let c = conteudo(&velho_zip).unwrap();
        assert!(!c.declarado);
        assert_eq!(
            c.escopo,
            Escopo::Database("Z".into()),
            "o nome do banco sai do nome do arquivo"
        );
        let r = Preparada::preparar(&velho_zip, &raiz, "")
            .unwrap()
            .confirmar(&raiz, "Zzip", false)
            .unwrap();
        assert_eq!(r.arquivos, 4);
        assert_eq!(
            std::fs::read(raiz.join("Zzip/clientes.reg")).unwrap(),
            b"registros de clientes"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Preparar e nao confirmar nao deixa lixo: o palco morre com o `Drop`.
    #[test]
    fn palco_abandonado_nao_fica_no_disco() {
        let base = temp("palco");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let zip = zip_de(&base, &raiz, "Z");

        let caminho = {
            let p = Preparada::preparar(&zip, &raiz, "").unwrap();
            let onde = p.palco.clone();
            assert!(onde.is_dir());
            assert!(
                !onde.starts_with(&raiz),
                "o palco dentro da raiz apareceria como database no meio da copia"
            );
            onde
        };
        assert!(!caminho.exists(), "o palco ficou para tras");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Nome de destino hostil nao vira caminho -- a mesma regra do catalogo.
    #[test]
    fn nome_de_destino_hostil_e_recusado() {
        let base = temp("hostil");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let zip = zip_de(&base, &raiz, "Z");
        for mau in ["..", "../fora", "/etc", "a/b", ""] {
            let p = Preparada::preparar(&zip, &raiz, "").unwrap();
            assert!(p.confirmar(&raiz, mau, false).is_err(), "{mau:?} passou");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// O palco e o guardado ficam ao lado da raiz de dados, e nao no `/tmp`.
    ///
    /// Nao e detalhe: a troca final e um `rename`, que so e instantaneo dentro
    /// do mesmo sistema de arquivos. Com `base: "dados"` -- o padrao do
    /// `config.json` --, o pai do caminho e VAZIO, e o codigo tem de entender
    /// isso como "o diretorio de trabalho", nunca como "nao sei, use o /tmp".
    #[test]
    fn o_vizinho_da_base_relativa_e_o_diretorio_de_trabalho() {
        let aqui = std::env::current_dir().unwrap();
        let v = vizinho_da_base(Path::new("dados"), "restaurando").unwrap();
        assert_eq!(v.parent().unwrap(), aqui, "o palco saiu do lado da base");
        assert!(!v.starts_with(std::env::temp_dir()));

        // Com caminho absoluto, o vizinho e o pai mesmo.
        let v = vizinho_da_base(Path::new("/srv/phxsql/dados"), "x").unwrap();
        assert_eq!(v.parent().unwrap(), Path::new("/srv/phxsql"));
    }

    #[test]
    fn o_que_nao_e_backup_nao_restaura() {
        let base = temp("naobackup");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::create_dir_all(base.join("qualquer")).unwrap();
        std::fs::write(base.join("qualquer/coisa.txt"), b"nada a ver").unwrap();
        assert!(conteudo(&base.join("qualquer")).is_err());
        assert!(conteudo(&base.join("qualquer/coisa.txt")).is_err());
        assert!(conteudo(&base.join("nem existe")).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}

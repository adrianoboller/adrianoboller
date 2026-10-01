//! **Pedido 635: a TRAVA DE INSTANCIA** -- um processo so gravando cada pasta
//! de tabelas.
//!
//! # O defeito
//!
//! O congelamento (`crate::congelamento`), o registro do que nasce
//! (`crate::nascendo`) e a trava global de dados do servidor sao todos
//! `static` do PROCESSO. Entre processos nao havia nada: a CLI `phxsql`, um
//! app pela `phxsql-ffi` ou um segundo `phxsqld` gravavam na mesma pasta sem
//! recusa. E isso ja perde dado sem reescrita nenhuma: o `atualizar` grava o
//! cabecalho inteiro do volume 1 a partir da RAM do processo
//! (`gravar_contadores`), e o segundo processo sobrescreve `slot_count`,
//! `live_count` e `marcadas` do primeiro com os valores velhos dele.
//!
//! # O que os maduros fazem -- aceite automatico
//!
//! Os tres convergem: o PostgreSQL recusa o segundo `postmaster` pelo
//! `postmaster.pid` (com trava), o InnoDB do MySQL e do MariaDB trava o
//! `ibdata1` («Unable to lock ./ibdata1»), e o SQLite coordena por trava de
//! arquivo. Nenhuma petrea se opoe: o meio e da `std`.
//!
//! # O meio: `File::try_lock` (H1), e as hipoteses que morreram
//!
//! * **H1, a que ficou** -- a trava do NUCLEO sobre um arquivo da pasta
//!   (`flock` no Unix, `LockFileEx` no Windows), pela `std` desde 1.89. Um
//!   `syscall`, atomica, e o nucleo a solta quando o processo morre, inclusive
//!   por `kill -9`: nao existe trava orfa. Custou subir a versao minima de
//!   1.75 para 1.89, e o custo foi medido: 19 avisos novos do `clippy` em
//!   cinco crates, todos mecanicos (`is_multiple_of`, `is_none_or`).
//! * **H2, morta** -- arquivo de pid por `create_new` com prova de vida. Morta
//!   por dois motivos que nao se consertam: no Windows nao ha prova de vida
//!   sem FFI, entao a queda deixaria a trava ETERNA (o criterio de aceite
//!   proibe); e a tomada da trava velha e uma corrida -- dois processos julgam
//!   o pid morto, os dois apagam e os dois recriam.
//! * **H3, morta** -- `flock` declarado a mao (`extern "C"`). Compraria a
//!   mesma trava sem subir a versao, ao preco de `unsafe` num crate que ate
//!   hoje recusou FFI duas vezes para nao te-lo (`util.rs`), e sem como
//!   conferir o lado Windows nesta maquina.
//!
//! # O ponto unico, e o alcance
//!
//! A trava se toma onde TODA tabela gravavel nasce -- [`crate::table::Table`]
//! com `escrever` (o mesmo ponto do congelamento) e o `Table::criar` --, e
//! nos dois caminhos que mexem nos arquivos sem abrir a tabela
//! (`excluir_tabela` e `renomear_tabela`). Quem le nao toma nada: ler
//! continua livre, como nos tres maduros.
//!
//! A chave e a PASTA, e nao a tabela: uma trava por tabela seria um
//! descritor aberto por tabela, e o servidor com mil tabelas estouraria o
//! teto de descritores padrao (1.024). A pasta e o que um processo serve.
//!
//! # Quanto tempo ela dura
//!
//! Enquanto alguem do processo segurar uma [`Posse`]. A tabela gravavel
//! segura a dela; a [`crate::catalogo::Instancia`] FIXA a de cada pasta em
//! que gravou e so a solta quando morre -- e e isso que faz o `phxsqld`
//! ocioso, entre um pedido e outro, continuar recusando a CLI. A CLI e o app
//! embutido soltam quando fecham.
//!
//! # O que nao e
//!
//! Nao e trava de registro, e nao substitui nenhuma trava de dentro do
//! processo: entre as threads de um processo quem manda continua sendo a
//! trava global e o congelamento. Tambem nao serve de prova de que ninguem
//! grava: o processo que nao usa este motor (um `cp`, um editor) nao pergunta.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use phxsql_core::error::{PhxError, Result};

/// O arquivo da trava, dentro da pasta. Ponto na frente porque nao e tabela
/// e nao deve aparecer para quem lista a pasta a procura de uma; e nenhum
/// nome de tabela comeca por ponto (`validar_nome`).
///
/// O CONTEUDO e so o pid de quem segura, para a recusa dizer quem e. A trava
/// mora no nucleo, e nao no arquivo: por isso ele nunca se apaga -- apagar
/// abriria a corrida de dois processos travando dois arquivos diferentes com
/// o mesmo nome.
pub const NOME_DO_ARQUIVO: &str = ".phxsql.trava";

struct Entrada {
    caminho: PathBuf,
    /// `None` depois de [`soltar_sob`]: a pasta saiu do lugar e a trava foi
    /// junto com ela.
    arquivo: Mutex<Option<File>>,
}

/// Solta a trava ANTES de fechar o descritor -- e nao so fecha.
///
/// # Por que o `unlock` explicito
///
/// A trava do nucleo e da DESCRICAO aberta, nao do processo. Entre o `fork`
/// e o `exec` de qualquer `Command` o filho carrega uma copia de todo
/// descritor do pai, inclusive o da trava; se o pai so fechar o dele, a
/// descricao continua viva (e travada) no filho por esse instante, e a
/// proxima abertura do pai e recusada pelo PROPRIO processo. Medido nesta
/// casa, com tres filhos subindo em paralelo: 3 corridas em 20 recusaram o
/// proprio pid. O `unlock` solta a descricao inteira, com copia ou sem.
///
/// Esperar e tentar de novo tambem fechava a janela, e foi recusado: a
/// abertura gravavel acontece com a trava global de dados na mao, e dormir
/// ali e a classe `rede-ou-espera` do mapa da trava.
fn soltar_o_arquivo(f: File) {
    let _ = f.unlock();
}

impl Drop for Entrada {
    fn drop(&mut self) {
        if let Some(f) = self.arquivo().take() {
            soltar_o_arquivo(f);
        }
    }
}

impl Entrada {
    fn arquivo(&self) -> std::sync::MutexGuard<'_, Option<File>> {
        self.arquivo.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Ainda trava a pasta deste caminho? Falso quando foi solta, ou quando
    /// o arquivo no caminho ja nao e o que esta travado (a pasta foi trocada
    /// por outra com o mesmo nome -- a restauracao por cima).
    fn vale(&self) -> bool {
        let guarda = self.arquivo();
        let Some(f) = guarda.as_ref() else {
            return false;
        };
        mesmo_arquivo(f, &self.caminho)
    }
}

#[cfg(unix)]
fn mesmo_arquivo(f: &File, caminho: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (f.metadata(), std::fs::metadata(caminho)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Fora do Unix a `std` nao da a identidade do arquivo; e no Windows a pasta
/// com um arquivo aberto dentro nem se renomeia -- quem a tira do lugar
/// chama [`soltar_sob`] antes, e e so por la que a troca acontece.
#[cfg(not(unix))]
fn mesmo_arquivo(_f: &File, caminho: &Path) -> bool {
    caminho.exists()
}

/// A posse da trava de uma pasta. Clonar e barato (um `Arc`); a trava sai
/// quando a ultima posse do processo cai.
#[derive(Clone)]
pub struct Posse(Arc<Entrada>);

impl Posse {
    /// A pasta desta trava, como chave -- a mesma de quem a fixa.
    pub(crate) fn chave(&self) -> PathBuf {
        self.0
            .caminho
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

impl std::fmt::Debug for Posse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Posse({})", self.0.caminho.display())
    }
}

/// Pasta -> a entrada viva. `Weak`: o registro nao segura trava nenhuma, so
/// lembra de quem segura, para a segunda tabela da mesma pasta no mesmo
/// processo nao tentar travar de novo -- o `flock` e por descritor, e o
/// processo se recusaria a si mesmo.
static REGISTRO: Mutex<BTreeMap<PathBuf, Weak<Entrada>>> = Mutex::new(BTreeMap::new());

fn registro() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, Weak<Entrada>>> {
    REGISTRO.lock().unwrap_or_else(|e| e.into_inner())
}

fn chave(diretorio: &Path) -> PathBuf {
    crate::volume::absoluto_lexico(diretorio).unwrap_or_else(|| diretorio.to_path_buf())
}

/// Toma (ou reaproveita) a trava da pasta para GRAVAR.
///
/// `Ok(None)` quando a pasta nao existe: nao ha o que proteger, e quem
/// chamou recusa logo adiante com a frase certa («a tabela nao existe»), em
/// vez de um erro de E/S do arquivo da trava.
///
/// O custo no caminho de sempre (a pasta ja travada por este processo) e um
/// `BTreeMap` e um `stat`; o `open` e o `flock` so na primeira vez.
pub fn tomar(diretorio: &Path) -> Result<Option<Posse>> {
    let chave = chave(diretorio);
    let mut reg = registro();
    if let Some(viva) = reg.get(&chave).and_then(Weak::upgrade) {
        if viva.vale() {
            return Ok(Some(Posse(viva)));
        }
    }
    reg.retain(|_, w| w.strong_count() > 0);
    let caminho = chave.join(NOME_DO_ARQUIVO);
    // 0600 pelo motor da permissao (pedido 542): o pid e pouco, mas arquivo
    // do banco legivel pela maquina inteira e o que aquele pedido fechou.
    let mut arquivo = match crate::util::opcoes_do_banco()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&caminho)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !chave.is_dir() => return Ok(None),
        Err(e) => return Err(PhxError::from(e)),
    };
    match arquivo.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            // A mesma pasta por outra grafia (um link) ja travada AQUI: o
            // `flock` e por descritor, e sem isto o processo se recusaria.
            if let Some(viva) = reg
                .values()
                .filter_map(Weak::upgrade)
                .find(|e| e.arquivo().as_ref().is_some_and(|f| mesmo_de(f, &arquivo)))
            {
                reg.insert(chave, Arc::downgrade(&viva));
                return Ok(Some(Posse(viva)));
            }
            return Err(recusa(&chave, &mut arquivo));
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(PhxError::from(e)),
    }
    // O pid e so diagnostico: falhar em escreve-lo nao solta a trava.
    let _ = arquivo
        .set_len(0)
        .and_then(|_| arquivo.seek(SeekFrom::Start(0)))
        .and_then(|_| writeln!(arquivo, "{}", std::process::id()));
    let viva = Arc::new(Entrada {
        caminho,
        arquivo: Mutex::new(Some(arquivo)),
    });
    reg.insert(chave, Arc::downgrade(&viva));
    Ok(Some(Posse(viva)))
}

#[cfg(unix)]
fn mesmo_de(a: &File, b: &File) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (a.metadata(), b.metadata()) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn mesmo_de(_a: &File, _b: &File) -> bool {
    false
}

/// A recusa: so o NOME da pasta, nunca o caminho -- a frase vai crua ao
/// cliente (pedido 428) -- e o pid de quem segura, quando se le. No Windows
/// a trava do nucleo tranca tambem a leitura, e o pid sai como desconhecido.
fn recusa(chave: &Path, arquivo: &mut File) -> PhxError {
    let mut texto = String::new();
    let _ = arquivo.read_to_string(&mut texto);
    let pid = texto.trim();
    let quem = if !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()) {
        format!("o processo {pid}")
    } else {
        "outro processo".to_string()
    };
    let pasta = chave
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    PhxError::InstanciaOcupada(format!(
        "a pasta {pasta} esta aberta para gravar por {quem}. Dois processos \
         gravando a mesma pasta destroem os contadores das tabelas; feche o \
         outro (ou pare o servidor que a serve) e tente de novo. Ler continua \
         livre"
    ))
}

/// Solta a trava de toda pasta debaixo de `prefixo` -- para quem vai TIRAR a
/// pasta do lugar (a restauracao por cima). No Windows a pasta com o arquivo
/// da trava aberto nem se renomearia; no Unix a trava iria junto com a pasta
/// velha e a nova ficaria sem.
///
/// Quem ainda segura posse dela fica com uma posse oca, e a proxima
/// abertura gravavel trava a pasta nova.
pub fn soltar_sob(prefixo: &Path) {
    let prefixo = chave(prefixo);
    let reg = registro();
    for (k, w) in reg.iter() {
        if !k.starts_with(&prefixo) {
            continue;
        }
        if let Some(viva) = w.upgrade() {
            if let Some(f) = viva.arquivo().take() {
                soltar_o_arquivo(f);
            }
        }
    }
}

/// As posses que um dono de longa vida (a [`crate::catalogo::Instancia`])
/// FIXA: uma por pasta em que gravou, ate ele morrer.
#[derive(Default)]
pub struct Fixadas(Mutex<BTreeMap<PathBuf, Posse>>);

impl Fixadas {
    /// Fixa a posse. Barata quando a pasta ja esta fixada pela mesma
    /// entrada: e o caso de todo pedido do servidor depois do primeiro.
    pub fn fixar(&self, posse: &Posse) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let k = posse.chave();
        match m.get(&k) {
            Some(p) if Arc::ptr_eq(&p.0, &posse.0) => {}
            _ => {
                m.insert(k, posse.clone());
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_mesma_pasta_no_mesmo_processo_reaproveita_a_trava() {
        let d = crate::apoio_teste::DirTemp::novo("trava-mesma");
        let a = tomar(&d.0).unwrap().unwrap();
        let b = tomar(&d.0).unwrap().unwrap();
        assert!(Arc::ptr_eq(&a.0, &b.0), "o processo travou duas vezes");
        let texto = std::fs::read_to_string(d.0.join(NOME_DO_ARQUIVO)).unwrap();
        assert_eq!(texto.trim(), std::process::id().to_string());
    }

    #[test]
    fn pasta_que_nao_existe_nao_trava_nem_recusa() {
        let d = crate::apoio_teste::DirTemp::novo("trava-ausente");
        assert!(tomar(&d.0.join("nao-existe")).unwrap().is_none());
    }

    /// A segunda abertura do MESMO arquivo, por outro descritor, e o que um
    /// segundo processo faria: a trava do nucleo a recusa. E o pedaco do
    /// comportamento que se prova sem processo; o de dois processos reais
    /// esta em `tests/trava-de-instancia.rs`.
    #[test]
    fn outro_descritor_e_recusado_enquanto_a_posse_vive() {
        let d = crate::apoio_teste::DirTemp::novo("trava-outro");
        let posse = tomar(&d.0).unwrap().unwrap();
        let outro = File::options()
            .read(true)
            .write(true)
            .open(d.0.join(NOME_DO_ARQUIVO))
            .unwrap();
        assert!(matches!(
            outro.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(posse);
        outro
            .try_lock()
            .expect("a trava nao saiu com a ultima posse");
    }

    #[test]
    fn soltar_sob_deixa_a_posse_oca_e_a_proxima_trava_de_novo() {
        let d = crate::apoio_teste::DirTemp::novo("trava-soltar");
        let posse = tomar(&d.0).unwrap().unwrap();
        soltar_sob(&d.0);
        assert!(!posse.0.vale());
        let nova = tomar(&d.0).unwrap().unwrap();
        assert!(!Arc::ptr_eq(&posse.0, &nova.0));
        assert!(nova.0.vale());
    }
}

//! O `fsync` do motor, e o que vale DEPOIS de um que foi recusado -- pedido 509.
//!
//! # O defeito, medido contra o sistema operacional
//!
//! No Linux, o `fsync` que falha pode ter custado as paginas que nao foram ao
//! disco: o nucleo as marca LIMPAS, e o proximo `fsync` responde Ok sem elas
//! estarem la. Medido pelo papel C no 6.18 (ext4 sobre loop com
//! provisionamento fino, 3/3): o 1o `fsync` deu EIO, o 2o no mesmo descritor
//! e o 3o num descritor novo responderam Ok, e depois de remontar o dado nao
//! existia. No motor, o fecho 1 deu ENOSPC, o fecho 2 deu Ok, e o servidor
//! apagava as marcas dos commits: o `.log` perdeu 5.000 de 5.000 eventos
//! confirmados (`docs/propostas/parecer-dba-496-catastrofes-2026-09-24.md`,
//! C1). Quem contava com a repeticao era o proprio motor: o
//! `Volumes::sincronizar` devolvia a lista ao registro «para o fecho tentar de
//! novo».
//!
//! # A regra, e de onde ela vem
//!
//! **Depois de um `fsync` recusado, nada naquele diretorio se confirma mais
//! sem recuperacao.** Os quatro motores convergem no COMPORTAMENTO -- nunca
//! tratar a repeticao como sucesso, 10 x 0 --, e isso e aceite automatico:
//! PostgreSQL (`data_sync_retry = off` -> PANIC), MariaDB e MySQL
//! (`os_file_sync_posix` -> `ib::fatal`), SQLite (`unixSync` ->
//! `SQLITE_IOERR_FSYNC`, e o pager fica em erro ate a conexao se refazer).
//!
//! O MEIO diverge, e a regua o decide: o processo CAI nos tres servidores
//! (PG 4 + MariaDB 3 + MySQL 2 = 9) e o erro FICA na biblioteca (SQLite 1).
//! Este motor e as duas coisas, entao:
//!
//! * o **servidor** cai. Ele registra [`ao_recusar`] com um `abort` antes da
//!   recuperacao do arranque, e o gancho roda AQUI, no instante da recusa --
//!   antes de qualquer `Drop` atestar um `.ndx` para este processo, e antes
//!   de qualquer laco repetir o `fsync`. (Baixar o byte 52 o `Drop` nao baixa
//!   mais desde o pedido 522: so o `sincronizar`, depois dos `fsync`.);
//! * a **biblioteca** embutida (FFI, exemplos, ferramentas) nao derruba o
//!   processo de quem a embute -- o caso do SQLite, e a resposta dele: o
//!   diretorio passa a recusar TODO `fsync` com o erro de [`conferir`] ate o
//!   processo reiniciar, e o `.ndx` dali nao baixa mais a marca de sujo nem
//!   se atesta. A proxima abertura -- ate neste processo -- manda
//!   reconstruir.
//!
//! # Por que POR DIRETORIO, e nao por arquivo nem pelo processo
//!
//! Nao por arquivo: na prova do papel C quem recusou foi o `.log`, e quem
//! perdeu pagina foi o `.ndx` da mesma tabela, que nunca viu erro nenhum -- o
//! `Drop` dele gravou o cabecalho limpo por cima. Nao pelo processo: o `cargo
//! test` roda os testes em threads do mesmo processo, e um estado global
//! envenenaria o vizinho -- o mesmo motivo de
//! [`crate::volume::familias_devendo_em`]. No servidor a pergunta nem se poe:
//! ele cai.
//!
//! # E o diretorio e o do DISCO no instante da recusa, e nao o texto do caminho (pedido 523)
//!
//! A chave era a absoluta LEXICA: a relativa e a absoluta casavam, o symlink e
//! o `dir/../dir` nao. Com a recusa forjada, 5 dos 6 pares cruzados de grafia
//! (`real/`, `link/`, `real/../real/`) sincronizavam Ok, e a tabela aberta pelo
//! symlink fechava baixando o byte 52 no proprio processo da recusa. A recusa
//! agora se grava pelas duas chaves de `volume::chaves_reais` -- a
//! lexica e a resolvida no disco -- e a consulta so resolve a grafia DEPOIS do
//! `HA_RECUSADO`: o caminho de sempre continua sem syscall nenhuma. O que se
//! resolve e o DIRETORIO, e no instante da recusa: renomear o diretorio depois
//! escapa dela, como escapava antes do 523.
//!
//! # O que isto NAO compra, dito
//!
//! Sem diario de refazer, parar compra menos que no PostgreSQL: o que o nucleo
//! ja descartou nao volta. Compra nao confirmar mais nada sobre um disco que
//! mente. O bilhete -- a marca do commit, o byte 52 -- fica, mas NAO salva
//! quem sobe no mesmo boot: o cache do nucleo devolve o que o disco perdeu, e
//! a marca sairia (medido pelo papel C, parecer 509+512, P1). Esse arranque e
//! a metade do 509 que continua aberta.
//!
//! # Quem passa por aqui, e quem nao passa
//!
//! Todo `fsync` de arquivo de dado do `phxsql-store` (`Volumes`, `.ndx`, as
//! copias do `.reg` e da restauracao), e o `fdatasync` da subida do byte 52
//! ([`sync_data`], pedido 533): uma decisao so, e a que alguem esquecesse
//! seria a que repete. O nome da funcao e `sync_all` de proposito
//! -- o `bancada/concorrencia/mapa-da-trava.py` acha o `fsync` pelo texto
//! `sync_all`, e um nome novo o esconderia um salto mais fundo sem nada ter
//! mudado. Ficam fora, no servidor, os tres que nao sao dado de tabela: a
//! marca do `COMMIT` (a resposta dela e o pedido 503), o `config.json` (se
//! reescreve inteiro) e a sonda da saude do disco (existe para VER a recusa).
//!
//! # O `abort` e do disco do BANCO, e o backup escreve num disco diferente
//!
//! Pedido 524, condicao C1 do parecer do DBA: o DESTINO de um backup (um
//! disco USB, uma pasta de rede, um provisionamento fino que o operador ainda
//! nao ampliou) nao e o disco que este arquivo protege -- o `.reg`, o `.ndx`,
//! a trilha continuam la, intactos. Um `fsync` recusado no destino nao pode
//! `abort()`ar um servidor que segue servindo o banco de verdade: viraria
//! produção fora do ar por causa de um disco de backup cheio.
//! [`sync_all_sem_abortar`] faz a MESMA conta de [`sync_all`] -- inclusive a
//! marca em [`RECUSADOS`], que continua impedindo uma repeticao de responder
//! Ok sem o dado -- e so' NAO chama o [`GANCHO`]. `phxsql_store::backup` e o
//! unico chamador dela hoje.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use phxsql_core::error::{PhxError, Result};

/// Os diretorios com `fsync` recusado neste processo, com a primeira recusa.
static RECUSADOS: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

/// Ha algum? E o que poupa a trava no caminho de sempre: o `Drop` de todo
/// `.ndx` pergunta, a abertura do `.ndx` marcado tambem (pedido 522), e quase
/// nunca ha nada a achar.
static HA_RECUSADO: AtomicBool = AtomicBool::new(false);

/// O que o processo faz no instante da recusa. Ver [`ao_recusar`].
static GANCHO: OnceLock<fn(&Path, &io::Error)> = OnceLock::new();

/// Registra o que ESTE PROCESSO faz quando um `fsync` for recusado.
///
/// O servidor registra um `abort`; a biblioteca nao registra nada e fica com o
/// erro que nao sai mais. Vale uma vez por processo: a segunda chamada nao
/// troca a primeira, porque a politica e do processo e nao de quem chamou por
/// ultimo -- um teste que sobe dois servidores nao pode trocar a do outro.
pub fn ao_recusar(gancho: fn(&Path, &io::Error)) {
    let _ = GANCHO.set(gancho);
}

/// O `fsync` do motor: `File::sync_all`, e a regra de depois.
///
/// Recusa sem tocar no disco quando o diretorio ja teve um `fsync` recusado --
/// repetir e exatamente o que responde Ok sem o dado. Na primeira recusa,
/// marca o diretorio e chama o gancho do processo (o servidor cai ali mesmo).
///
/// So para dado do BANCO. Quem escreve num disco que nao e o do banco --
/// hoje so o [`crate::backup`] -- usa [`sync_all_sem_abortar`].
pub fn sync_all(arquivo: &File, caminho: &Path) -> Result<()> {
    sync_all_interno(arquivo, caminho, true, false)
}

/// O `fdatasync` do motor: a MESMA conta de [`sync_all`] -- a recusa anterior
/// que impede repetir, a marca, o gancho do processo e o `falha_de_teste` de
/// prova --, so' sem levar os metadados que a leitura nao precisa.
///
/// Existe para a subida do byte 52 do `.ndx` (pedido 533). Basta o
/// `fdatasync`: em regime a pagina 0 ja existe e o tamanho nao muda; na
/// criacao do `.ndx` o tamanho muda, e o `fdatasync` leva junto o metadado que
/// a leitura precisa (POSIX; no Windows a `std` o faz igual ao `sync_all`).
/// Passar pelo `File` cru
/// pularia o gancho de teste e a trava do 509/523 -- e um `fsync` recusado ali
/// deixaria o proximo responder Ok sem o dado, que e o que o 509 fecha. O nome
/// termina em `sync_data` de proposito: o `mapa-da-trava.py` acha o `fsync`
/// pelo texto (`sync_all` e `sync_data`), e um nome novo o esconderia.
pub fn sync_data(arquivo: &File, caminho: &Path) -> Result<()> {
    sync_all_interno(arquivo, caminho, true, true)
}

/// A MESMA conta de [`sync_all`] -- inclusive a marca em [`recusar`], que
/// continua impedindo uma repeticao de responder Ok sem o dado --, so' sem
/// chamar o [`GANCHO`] do processo na recusa.
///
/// Pedido 524, condicao C1: o destino de um backup nao e o disco que o
/// gancho protege (ver a nota do modulo), e uma recusa ali tem de virar
/// `Err` para quem chamou decidir, nao um `abort()` do servidor inteiro.
/// Um motor so: a conferencia contra recusa anterior, a marca e o
/// `falha_de_teste` de prova continuam os MESMOS de [`sync_all`] -- so o
/// gancho fica de fora.
///
/// `pub(crate)`: quem decide gravar num disco que nao e o do banco e o
/// proprio phxsql-store (hoje so o [`crate::backup`]) -- o servidor e o
/// FFI nao tem por que escolher pular o gancho de protecao do 509.
pub(crate) fn sync_all_sem_abortar(arquivo: &File, caminho: &Path) -> Result<()> {
    sync_all_interno(arquivo, caminho, false, false)
}

fn sync_all_interno(
    arquivo: &File,
    caminho: &Path,
    com_gancho: bool,
    so_dados: bool,
) -> Result<()> {
    let descarregar = || {
        if so_dados {
            arquivo.sync_data()
        } else {
            arquivo.sync_all()
        }
    };
    conferir(caminho)?;
    #[cfg(debug_assertions)]
    let feito = match falha_de_teste::disparar(caminho, falha_de_teste::Onde::Fsync) {
        Some(e) => Err(e),
        None => descarregar(),
    };
    #[cfg(not(debug_assertions))]
    let feito = descarregar();
    if let Err(e) = feito {
        recusar(caminho, &e);
        if com_gancho {
            if let Some(gancho) = GANCHO.get() {
                gancho(caminho, &e);
            }
        }
        return Err(PhxError::Io(e));
    }
    Ok(())
}

/// `Err` se o diretorio de `caminho` (ou um acima dele) ja teve um `fsync`
/// recusado neste processo.
///
/// Existe fora do [`sync_all`] para quem responde «sincronizado» sem chegar a
/// `fsync` nenhum -- o `NdxFile::sincronizar` com a arvore que nao presta
/// voltava `Ok` sem tocar no disco, e seria por ali que um fecho repetido
/// responderia Ok.
pub fn conferir(caminho: &Path) -> Result<()> {
    match recusado_em(caminho) {
        None => Ok(()),
        Some(primeira) => Err(PhxError::Io(io::Error::other(format!(
            "um fsync anterior foi recusado ({primeira}), e depois disso o \
             nucleo pode ter descartado o que nao foi ao disco: um fsync novo \
             responderia Ok sem o dado estar la. Nada neste diretorio se \
             confirma mais ate o processo reiniciar (pedido 509)"
        )))),
    }
}

/// A primeira recusa que alcanca `caminho`, se houve.
///
/// A grafia se resolve contra o disco -- symlink e `..` (pedido 523) -- so
/// DEPOIS do atomico: o `Drop` de todo `.ndx` pergunta, e o caminho de sempre
/// nao paga syscall nenhuma.
pub fn recusado_em(caminho: &Path) -> Option<String> {
    if !HA_RECUSADO.load(Ordering::Acquire) {
        return None;
    }
    let alvos = crate::volume::chaves_reais(caminho);
    trava(&RECUSADOS)
        .iter()
        .find(|(d, _)| alvos.iter().any(|a| a.starts_with(d)))
        .map(|(_, primeira)| primeira.clone())
}

/// Grava o diretorio por TODAS as grafias que ele tem agora -- a lexica e a
/// real. So a lexica, e a recusa gravada por `dir/` nao alcancava o mesmo
/// diretorio aberto por `link/` nem por `dir/../dir` (pedido 523).
fn recusar(caminho: &Path, e: &io::Error) {
    let primeira = format!("{}: {e}", caminho.display());
    let chaves = crate::volume::chaves_reais(caminho);
    let mut r = trava(&RECUSADOS);
    for chave in chaves {
        let dir = chave.parent().map(Path::to_path_buf).unwrap_or(chave);
        if !r.iter().any(|(d, _)| dir.starts_with(d)) {
            r.push((dir, primeira.clone()));
        }
    }
    HA_RECUSADO.store(true, Ordering::Release);
}

/// A chave lexica das familias do `Volumes`, e so para a ARMA de teste: a
/// recusa de verdade usa `volume::chaves_reais`, que resolve symlink
/// e `..` (pedido 523). A arma continua lexica porque o que ela forja e
/// prefixo de TEXTO por desenho -- ver `falha_de_teste`.
fn absoluto(caminho: &Path) -> PathBuf {
    crate::volume::absoluto_lexico(caminho).unwrap_or_else(|| caminho.to_path_buf())
}

/// Envenenamento aqui nao engana ninguem: o que a trava protege e uma lista
/// que so cresce. Propagar tiraria do ar justamente a recusa.
fn trava<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Recusas FORJADAS, para as provas que nao podem montar um disco.
///
/// A prova de verdade e contra o sistema operacional
/// (`bancada/catastrofes/`): tmpfs cheio e ext4 sobre loop com provisionamento
/// fino, que exigem `CAP_SYS_ADMIN`. Estas forjam o MESMO retorno -- o
/// `fsync` que devolve EIO, a pagina do `.ndx` que devolve ENOSPC -- para a
/// suite provar a conduta de depois sem privilegio nenhum.
///
/// # Por que `debug_assertions`, e nao `cfg(test)`
///
/// O mesmo corte do `ndx::panico_de_teste`: `cfg(test)` so vale dentro do
/// proprio crate, e as provas do servidor compilam este crate como dependencia
/// comum. Em `release` nada disto existe -- `armar` e `desarmar` viram funcoes
/// vazias, e o `fsync` nao paga nem a leitura de um atomico.
///
/// # Por que por PREFIXO de caminho
///
/// Os testes rodam em paralelo no mesmo processo, cada um no seu diretorio
/// temporario: uma arma global derrubaria o `fsync` do vizinho. E prefixo de
/// TEXTO, e nao de componente, para alcancar uma tabela so (`.../loja/alvo.`).
#[doc(hidden)]
pub mod falha_de_teste {
    use std::path::Path;

    /// Onde a falha acontece.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Onde {
        /// O `fsync` de [`super::sync_all`] -- e o `fdatasync` de
        /// [`super::sync_data`], que passa pelo mesmo ponto -- devolve EIO.
        Fsync,
        /// A gravacao de uma pagina do `.ndx` devolve ENOSPC -- o `write` do
        /// disco cheio, no despejo e no `descarregar`.
        PaginaDoIndice,
    }

    #[cfg(debug_assertions)]
    static ARMADAS: std::sync::Mutex<Vec<(String, Onde, u32)>> = std::sync::Mutex::new(Vec::new());

    #[cfg(debug_assertions)]
    static ALGUMA: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// As proximas `vezes` operacoes `onde` sob `prefixo` falham.
    pub fn armar(prefixo: &Path, onde: Onde, vezes: u32) {
        #[cfg(debug_assertions)]
        {
            let chave = super::absoluto(prefixo).to_string_lossy().into_owned();
            let mut a = super::trava(&ARMADAS);
            a.retain(|(p, o, _)| !(p == &chave && *o == onde));
            if vezes > 0 {
                a.push((chave, onde, vezes));
            }
            ALGUMA.store(a.len(), std::sync::atomic::Ordering::Release);
        }
        #[cfg(not(debug_assertions))]
        let _ = (prefixo, onde, vezes);
    }

    /// Desarma tudo sob `prefixo`: «o operador liberou espaco».
    pub fn desarmar(prefixo: &Path) {
        #[cfg(debug_assertions)]
        {
            let chave = super::absoluto(prefixo).to_string_lossy().into_owned();
            let mut a = super::trava(&ARMADAS);
            a.retain(|(p, _, _)| p != &chave);
            ALGUMA.store(a.len(), std::sync::atomic::Ordering::Release);
        }
        #[cfg(not(debug_assertions))]
        let _ = prefixo;
    }

    /// O ponto de passagem, no caminho de producao. `Some` = falhe com isto.
    #[cfg(debug_assertions)]
    pub(crate) fn disparar(caminho: &Path, onde: Onde) -> Option<std::io::Error> {
        if ALGUMA.load(std::sync::atomic::Ordering::Acquire) == 0 {
            return None;
        }
        let alvo = super::absoluto(caminho).to_string_lossy().into_owned();
        let mut a = super::trava(&ARMADAS);
        let i = a
            .iter()
            .position(|(p, o, _)| *o == onde && alvo.starts_with(p.as_str()))?;
        a[i].2 -= 1;
        if a[i].2 == 0 {
            a.remove(i);
        }
        ALGUMA.store(a.len(), std::sync::atomic::Ordering::Release);
        Some(std::io::Error::from_raw_os_error(match onde {
            Onde::Fsync => 5,
            Onde::PaginaDoIndice => 28,
        }))
    }
}

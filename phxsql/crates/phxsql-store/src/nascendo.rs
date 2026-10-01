//! O NOME QUE NASCE: a tabela (ou a pasta de database e de schema) criada sob
//! a trava global, cujo `fsync` ainda nao terminou -- pedido 605.
//!
//! # O defeito que ele existe para impedir
//!
//! Desde os pedidos 586/589 a criacao grava os arquivos sob a trava global e
//! leva ao disco FORA dela, antes de responder. Quem criou so ouve «ok» depois
//! do `fsync` da pasta, e para ELE a garantia vale. Para um TERCEIRO nao
//! valia: entre soltar a trava e terminar o `fsync`, a tabela ja era
//! visivel, B inseria nela, sincronizava os arquivos DELE e ouvia «ok» -- com
//! a entrada da tabela ainda fora do disco. Num ext4 sem diario (o `/` desta
//! bancada) a queda levava a tabela e a linha de B juntas. Medido pelo M2 do
//! desenho (`bancada/durabilidade/terceiro-605.py`): com o `fsync` da pasta
//! atrasado 1,5 s, o «ok» de B saia em 20-40 ms, 3 de 3 corridas.
//!
//! # A saida (c) do desenho: nasce RESERVADA
//!
//! Sob a trava, o nome entra aqui antes de o primeiro arquivo nascer; o
//! `fsync` acontece fora da trava; a [`Reserva`] sai quando o
//! `PorSincronizar` que a carrega termina de levar ao disco -- e e so ai que
//! quem criou responde. Quem abre um nome reservado ESPERA a publicacao.
//!
//! Os tres maduros convergem no comportamento (a criacao e duravel antes de
//! ser utilizavel por outra sessao) e divergem na janela: esperar (MariaDB 3 +
//! MySQL 2 = 5) contra nao enxergar (PostgreSQL 4). Ver
//! `docs/propostas/605-medicao-segura-01-10-2026.md` §3.
//!
//! # Onde se espera, e por que em dois lugares
//!
//! * **Fora da trava global**, no `executar` do servidor, para o nome que o
//!   pedido traz nos campos `database`/`tabela` -- o caso comum. Quem espera ali
//!   nao segura ninguem: as outras tabelas continuam atendendo.
//! * **Dentro dela**, em [`crate::table::Table::abrir_com`] -- o ponto por onde
//!   toda abertura de tabela passa, pelo mesmo motivo do congelamento (ver o
//!   cabecalho de `crate::congelamento`): a cascata, a conferencia da chave, a
//!   juncao e o SQL nao trazem a tabela no campo que o servidor le. E a
//!   GARANTIA; o de fora e o atalho que poupa a fila. A espera la dentro e
//!   limitada pelo `fsync` de quem criou e nao trava ninguem para sempre:
//!   publicar nao pede a trava global, so o mutex daqui.
//!
//! # Quem pode passar
//!
//! A propria thread que reservou: a replicacao cria a tabela e grava nela
//! antes de levar ao disco, e esperar por si mesma seria parar para sempre.
//!
//! # O portao vem ANTES do trabalho
//!
//! [`esperar`] le um `AtomicUsize` antes de montar chave, alocar ou tocar no
//! mutex. Sem nada nascendo -- que e sempre, menos na janela de uma criacao --
//! custa um `load` por ABERTURA de tabela (nao por linha). O `inserir` em
//! lote abre a tabela uma vez.

use phxsql_core::error::{PhxError, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::thread::ThreadId;

/// Caminho que nasce (a chave do [`crate::congelamento::chave`]) -> a thread
/// que o reservou.
static NASCENDO: Mutex<BTreeMap<PathBuf, ThreadId>> = Mutex::new(BTreeMap::new());

/// Quem espera dorme aqui; toda publicacao acorda todos, e cada um confere o
/// proprio nome. Publicacao e rara (uma por criacao), entao acordar a mais e
/// barato e dispensa uma variavel por nome.
static PUBLICADA: Condvar = Condvar::new();

/// O interruptor do portao barato -- ver o cabecalho.
static QUANTAS: AtomicUsize = AtomicUsize::new(0);

/// O mapa, mesmo envenenado: o que ele guarda e `PathBuf` e `ThreadId`, que um
/// panico nao entorta. Recusar aqui deixaria quem espera parado para sempre,
/// ou a reserva sem sair.
fn mapa() -> MutexGuard<'static, BTreeMap<PathBuf, ThreadId>> {
    NASCENDO.lock().unwrap_or_else(|e| e.into_inner())
}

/// A posse do nome que nasce. Publica no `Drop`.
///
/// `Drop`, e nao um par reservar/publicar escrito a mao, pelo motivo do
/// congelamento: um `?` entre criar e levar ao disco sairia sem publicar, e
/// todo terceiro esperaria para sempre. No erro a reserva sai do mesmo jeito
/// -- e quem criou responde erro, sem prometer nada.
#[derive(Debug)]
pub struct Reserva {
    chave: PathBuf,
}

impl Drop for Reserva {
    fn drop(&mut self) {
        let mut m = mapa();
        if m.remove(&self.chave).is_some() {
            QUANTAS.fetch_sub(1, Ordering::SeqCst);
        }
        drop(m);
        PUBLICADA.notify_all();
    }
}

/// Reserva `diretorio/nome` -- uma tabela ou uma pasta que vai nascer.
///
/// Recusa o nome que ja esta nascendo: dois criadores do mesmo nome sao o que
/// a trava global ja impede no servidor, e fora dele a segunda criacao tem de
/// ouvir isso em vez de reservar por cima.
pub fn reservar(diretorio: &Path, nome: &str) -> Result<Reserva> {
    let chave = crate::congelamento::chave(diretorio, nome);
    let mut m = mapa();
    if m.contains_key(&chave) {
        return Err(PhxError::Duplicado(format!(
            "{nome} esta nascendo neste instante por outra operacao"
        )));
    }
    m.insert(chave.clone(), std::thread::current().id());
    QUANTAS.fetch_add(1, Ordering::SeqCst);
    Ok(Reserva { chave })
}

/// Espera ate `diretorio/nome` -- e toda pasta acima dele -- deixar de
/// nascer por outra thread.
///
/// As pastas acima contam porque a tabela gravada dentro de um database ou de
/// um schema que ainda nasce perde a entrada da PASTA numa queda, e com ela a
/// tabela: e o mesmo 605 um nivel acima.
pub fn esperar(diretorio: &Path, nome: &str) {
    // O portao ANTES do trabalho: sem nada nascendo nao se monta chave, nao
    // se aloca e nao se toca no mutex.
    if QUANTAS.load(Ordering::SeqCst) == 0 {
        return;
    }
    let alvo = crate::congelamento::chave(diretorio, nome);
    let eu = std::thread::current().id();
    let mut m = mapa();
    while alvo
        .ancestors()
        .any(|a| m.get(a).is_some_and(|dono| *dono != eu))
    {
        m = PUBLICADA.wait(m).unwrap_or_else(|e| e.into_inner());
    }
}

/// `diretorio/nome` esta nascendo agora? So para teste e para medir.
pub fn reservada(diretorio: &Path, nome: &str) -> bool {
    mapa().contains_key(&crate::congelamento::chave(diretorio, nome))
}

/// Quantos nomes nascem agora -- o interruptor do portao.
pub fn quantas() -> usize {
    QUANTAS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    /// Roda `esperar` noutra thread e diz se ela ja voltou depois de `ms`.
    fn espera_noutra_thread(dir: PathBuf, nome: &'static str) -> Arc<AtomicBool> {
        let voltou = Arc::new(AtomicBool::new(false));
        let v = Arc::clone(&voltou);
        std::thread::spawn(move || {
            esperar(&dir, nome);
            v.store(true, Ordering::SeqCst);
        });
        voltou
    }

    fn ate(voltou: &AtomicBool) -> bool {
        for _ in 0..400 {
            if voltou.load(Ordering::SeqCst) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// O comportamento VELHO: sem nada nascendo, nada espera.
    #[test]
    fn sem_nada_nascendo_nada_espera() {
        let d = Path::new("/tmp/phxsql-nascendo-vazio");
        esperar(d, "clientes");
        assert!(!reservada(d, "clientes"));
    }

    #[test]
    fn o_nome_que_nasce_segura_o_terceiro_ate_publicar() {
        let d = PathBuf::from("/tmp/phxsql-nascendo-um");
        let posse = reservar(&d, "nova").unwrap();
        let voltou = espera_noutra_thread(d.clone(), "nova");
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            !voltou.load(Ordering::SeqCst),
            "o terceiro passou pelo nome que ainda nasce"
        );
        // A VIZINHA nao sente.
        esperar(&d, "vizinha");
        drop(posse);
        assert!(ate(&voltou), "a publicacao nao acordou quem esperava");
        assert!(!reservada(&d, "nova"));
    }

    /// Quem reservou passa: a replicacao cria e grava antes de levar ao disco.
    #[test]
    fn quem_reservou_passa() {
        let d = Path::new("/tmp/phxsql-nascendo-dono");
        let _posse = reservar(d, "nova").unwrap();
        esperar(d, "nova");
    }

    /// A tabela dentro de um database que ainda nasce espera o database.
    #[test]
    fn a_pasta_que_nasce_segura_o_que_mora_nela() {
        let base = PathBuf::from("/tmp/phxsql-nascendo-pasta");
        let posse = reservar(&base, "loja").unwrap();
        let voltou = espera_noutra_thread(base.join("loja").join("vendas"), "itens");
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            !voltou.load(Ordering::SeqCst),
            "a tabela do database nascendo passou"
        );
        drop(posse);
        assert!(ate(&voltou));
    }

    #[test]
    fn o_mesmo_nome_nao_nasce_duas_vezes() {
        let d = Path::new("/tmp/phxsql-nascendo-duas");
        let _posse = reservar(d, "nova").unwrap();
        let e = reservar(d, "NOVA").unwrap_err();
        assert!(matches!(e, PhxError::Duplicado(_)), "{e:?}");
    }
}

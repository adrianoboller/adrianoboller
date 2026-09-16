//! Semaforo de contagem, escrito aqui -- a `std` nao tem um.
//!
//! O `std::sync::Semaphore` saiu antes do Rust 1.0 e nunca voltou; o que a
//! `std` oferece e o par `Mutex` + `Condvar`, e e sobre ele que este arquivo
//! escreve o semaforo. Sem crate, pela mesma regra que fez o SHA-256 e o
//! PBKDF2 desta casa: o `tokio::sync::Semaphore` e uma dependencia externa e
//! e assincrono, e nenhuma das duas coisas cabe aqui.
//!
//! # O que ele responde
//!
//! «Quantas threads deste tipo podem existir AO MESMO TEMPO?» A resposta e o
//! `teto`, e a garantia e que nunca ha mais [`Permissao`] vivas do que ele.
//! Quem quer subir uma thread pede a permissao ANTES e a leva para dentro da
//! thread; quando a thread acaba -- pelo fim do corpo ou por um panico --, a
//! permissao morre e a vaga volta.
//!
//! # A permissao e RAII, e isso e o ponto
//!
//! O defeito que este desenho substitui e um contador de mao (`fetch_add` ao
//! aceitar, `fetch_sub` ao fim do fecho da thread). Um panico no meio do
//! corpo pula o `fetch_sub`, a vaga nunca volta, e depois de N panicos a
//! porta para de aceitar sem nada ter caido -- e o pior tipo de defeito: o
//! servico continua de pe, e recusa todo mundo. `Drop` roda no desenrolar
//! do panico, entao a permissao volta pelo mesmo caminho que qualquer outro
//! recurso do Rust. O teste `permissao_volta_mesmo_em_panico` prova isso
//! com `catch_unwind`, e nao por leitura.
//!
//! # Mutex envenenado nao derruba
//!
//! O `Drop` de uma permissao pode rodar DURANTE um panico -- e o `lock()` de
//! um mutex que outra thread envenenou devolveria `Err`. Aqui todo `lock`
//! recupera pelo `into_inner`: o estado guardado e um `usize`, e nao ha
//! invariante que um panico alheio possa ter deixado pela metade.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

struct Interno {
    /// Quantas permissoes estao vivas agora.
    em_uso: Mutex<usize>,
    /// Acordada a cada permissao que morre.
    vaga: Condvar,
    teto: usize,
    /// Quantas threads estao DORMINDO a espera de vaga agora. Atomico e fora
    /// do mutex de proposito: o monitor le isto a cada dois segundos e nao
    /// pode disputar a trava com quem esta pedindo vaga.
    esperando: AtomicUsize,
}

/// Conta uma thread na fila enquanto ela dorme; sai da conta no `Drop`, que
/// e o unico jeito de a conta bater tambem quando quem esperava e derrubado
/// por um panico alheio no meio do `wait`.
struct NaFila<'a>(&'a AtomicUsize);

impl<'a> NaFila<'a> {
    fn entrar(contador: &'a AtomicUsize) -> NaFila<'a> {
        contador.fetch_add(1, Ordering::Relaxed);
        NaFila(contador)
    }
}

impl Drop for NaFila<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Interno {
    /// O `lock` que nao falha por veneno. Ver o topo do modulo.
    fn estado(&self) -> MutexGuard<'_, usize> {
        self.em_uso.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Semaforo de contagem com teto fixo. Barato de clonar: e um `Arc`.
#[derive(Clone)]
pub struct Semaforo {
    interno: Arc<Interno>,
}

/// Uma vaga tomada. Morre, a vaga volta -- inclusive num panico.
///
/// `Send` de proposito: ela nasce na thread que aceita e vai para dentro da
/// thread que atende. E `'static`, porque segura o semaforo por `Arc` em vez
/// de emprestar.
pub struct Permissao {
    interno: Arc<Interno>,
}

impl Semaforo {
    /// Um semaforo com `teto` vagas. Teto zero nao existe: seria um semaforo
    /// em que ninguem entra nunca, e isso e um erro de quem configura, nao
    /// uma politica. Sobe para um.
    pub fn novo(teto: usize) -> Semaforo {
        Semaforo {
            interno: Arc::new(Interno {
                em_uso: Mutex::new(0),
                vaga: Condvar::new(),
                teto: teto.max(1),
                esperando: AtomicUsize::new(0),
            }),
        }
    }

    /// Um semaforo que nunca recusa. Existe para o «zero = sem teto imposto
    /// por aqui» dos `recursos`: quem escolhe isso ganha o comportamento de
    /// antes -- uma thread por pedido -- e o painel continua contando.
    pub fn sem_teto() -> Semaforo {
        Semaforo::novo(usize::MAX)
    }

    pub fn teto(&self) -> usize {
        self.interno.teto
    }

    /// `false` quando nasceu por [`Semaforo::sem_teto`].
    pub fn limitado(&self) -> bool {
        self.interno.teto != usize::MAX
    }

    /// Quantas permissoes estao vivas agora.
    pub fn em_uso(&self) -> usize {
        *self.interno.estado()
    }

    /// Quantas threads estao dormindo a espera de vaga agora. E o numero que
    /// diz se o teto esta apertado: `em_uso == teto` com `esperando == 0` e
    /// um teto justo; com `esperando > 0` e fila.
    pub fn esperando(&self) -> usize {
        self.interno.esperando.load(Ordering::Relaxed)
    }

    fn tomar(&self, estado: &mut usize) -> Permissao {
        *estado += 1;
        Permissao {
            interno: Arc::clone(&self.interno),
        }
    }

    /// Uma vaga agora, ou nada. Nunca espera.
    ///
    /// E o que a porta de dados usa: `max_connections` recusa na hora no
    /// PostgreSQL, no MySQL e no MariaDB, e tres motores maduros convergindo
    /// e aceite automatico.
    pub fn tentar(&self) -> Option<Permissao> {
        let mut estado = self.interno.estado();
        if *estado >= self.interno.teto {
            return None;
        }
        Some(self.tomar(&mut estado))
    }

    /// Espera ate haver vaga. Nao tem prazo: quem chama sabe que a vaga vai
    /// aparecer, porque toda permissao morre.
    pub fn adquirir(&self) -> Permissao {
        let mut estado = self.interno.estado();
        let mut na_fila = None;
        while *estado >= self.interno.teto {
            na_fila.get_or_insert_with(|| NaFila::entrar(&self.interno.esperando));
            estado = self
                .interno
                .vaga
                .wait(estado)
                .unwrap_or_else(|e| e.into_inner());
        }
        self.tomar(&mut estado)
    }

    /// Espera ate `prazo` por uma vaga; `None` se o prazo acabou antes.
    ///
    /// O laco reconta o que falta a cada acordar, porque a `Condvar` pode
    /// acordar sem motivo (o "spurious wakeup" da norma POSIX) e um so
    /// `wait_timeout` devolveria antes do prazo com a vaga ainda ocupada.
    pub fn adquirir_ate(&self, prazo: Duration) -> Option<Permissao> {
        let fim = Instant::now() + prazo;
        let mut estado = self.interno.estado();
        let mut na_fila = None;
        while *estado >= self.interno.teto {
            let agora = Instant::now();
            if agora >= fim {
                return None;
            }
            na_fila.get_or_insert_with(|| NaFila::entrar(&self.interno.esperando));
            let (e, _) = self
                .interno
                .vaga
                .wait_timeout(estado, fim - agora)
                .unwrap_or_else(|e| e.into_inner());
            estado = e;
        }
        Some(self.tomar(&mut estado))
    }
}

impl Drop for Permissao {
    fn drop(&mut self) {
        let mut estado = self.interno.estado();
        // `saturating_sub` e defesa contra um bug nosso, nunca contra o uso:
        // uma permissao so existe porque `tomar` somou um. Se um dia o
        // contador chegasse a zero com uma permissao viva, estourar para
        // `usize::MAX` fecharia o semaforo para sempre -- e o servidor
        // continuaria de pe recusando todo mundo, que e o defeito que este
        // arquivo existe para impedir.
        *estado = estado.saturating_sub(1);
        // Uma vaga, um acordado. `notify_all` faria K esperadores disputarem
        // uma vaga so, e K-1 voltariam a dormir -- trabalho a toa que cresce
        // com a fila.
        self.interno.vaga.notify_one();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn nunca_ha_mais_permissoes_que_o_teto() {
        let s = Semaforo::novo(2);
        let a = s.tentar().expect("primeira vaga");
        let b = s.tentar().expect("segunda vaga");
        assert!(s.tentar().is_none(), "a terceira nao podia existir");
        assert_eq!(s.em_uso(), 2);
        drop(a);
        assert_eq!(s.em_uso(), 1);
        let c = s.tentar().expect("a vaga voltou");
        assert!(s.tentar().is_none());
        drop(b);
        drop(c);
        assert_eq!(s.em_uso(), 0);
    }

    /// A prova real do desenho: a permissao volta quando a thread entra em
    /// panico. Sem ela, este semaforo seria o `fetch_add`/`fetch_sub` de
    /// antes com outro nome.
    #[test]
    fn permissao_volta_mesmo_em_panico() {
        let s = Semaforo::novo(1);
        let dentro = s.clone();
        let r = std::panic::catch_unwind(move || {
            let _p = dentro.adquirir();
            panic!("panico de proposito, com a permissao na mao");
        });
        assert!(r.is_err(), "o panico tinha de acontecer");
        assert_eq!(s.em_uso(), 0, "a permissao nao voltou depois do panico");
        assert!(s.tentar().is_some(), "a vaga tinha de estar livre");
    }

    /// O mesmo, atravessando uma thread de verdade: e assim que o servidor a
    /// usa -- a permissao nasce numa thread e morre noutra.
    #[test]
    fn permissao_viaja_para_outra_thread_e_volta_quando_ela_morre() {
        let s = Semaforo::novo(1);
        let p = s.tentar().unwrap();
        let fio = thread::spawn(move || {
            let _minha = p;
            panic!("morrendo com a permissao");
        });
        assert!(fio.join().is_err());
        assert_eq!(s.em_uso(), 0);
    }

    #[test]
    fn adquirir_ate_devolve_none_quando_o_prazo_acaba() {
        let s = Semaforo::novo(1);
        let _ocupada = s.adquirir();
        let t0 = Instant::now();
        assert!(s.adquirir_ate(Duration::from_millis(60)).is_none());
        let esperou = t0.elapsed();
        assert!(
            esperou >= Duration::from_millis(60),
            "voltou antes do prazo: {esperou:?}"
        );
        assert!(
            esperou < Duration::from_secs(2),
            "esperou muito alem do prazo: {esperou:?}"
        );
    }

    #[test]
    fn adquirir_ate_acorda_quando_a_vaga_volta() {
        let s = Semaforo::novo(1);
        let ocupada = s.adquirir();
        let dentro = s.clone();
        let fio = thread::spawn(move || dentro.adquirir_ate(Duration::from_secs(5)));
        thread::sleep(Duration::from_millis(40));
        drop(ocupada);
        let p = fio.join().unwrap();
        assert!(p.is_some(), "a espera nao viu a vaga voltar");
        assert_eq!(s.em_uso(), 1);
    }

    /// O monitor em runtime le `esperando()`: conta quem DORME na fila, e so
    /// enquanto dorme -- nem quem pegou vaga de primeira, nem quem ja saiu.
    #[test]
    fn esperando_conta_quem_dorme_na_fila_e_so_enquanto_dorme() {
        let s = Semaforo::novo(1);
        let ocupada = s.adquirir();
        assert_eq!(s.esperando(), 0, "pegar de primeira nao e esperar");
        let dentro = s.clone();
        let fio = thread::spawn(move || dentro.adquirir_ate(Duration::from_secs(5)));
        let fim = Instant::now() + Duration::from_secs(2);
        while s.esperando() == 0 && Instant::now() < fim {
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(s.esperando(), 1, "a thread dormindo nao entrou na conta");
        drop(ocupada);
        assert!(fio.join().unwrap().is_some());
        assert_eq!(s.esperando(), 0, "quem saiu da fila continuou contado");
        // E quem desiste pelo prazo tambem sai da conta.
        let _ocupada = s.adquirir();
        assert!(s.adquirir_ate(Duration::from_millis(30)).is_none());
        assert_eq!(s.esperando(), 0);
    }

    /// Prazo zero e o `tentar` com outro nome -- e o que `fila_web_ms = 0`
    /// escolhe.
    #[test]
    fn prazo_zero_nao_espera() {
        let s = Semaforo::novo(1);
        let _ocupada = s.adquirir();
        let t0 = Instant::now();
        assert!(s.adquirir_ate(Duration::ZERO).is_none());
        assert!(t0.elapsed() < Duration::from_millis(50));
    }

    /// Muitas threads disputando poucas vagas: em nenhum instante ha mais
    /// que o teto dentro, e todas acabam entrando.
    #[test]
    fn a_disputa_nunca_passa_do_teto() {
        let teto = 3;
        let s = Semaforo::novo(teto);
        let dentro = Arc::new(AtomicUsize::new(0));
        let pico = Arc::new(AtomicUsize::new(0));
        let entradas = Arc::new(AtomicUsize::new(0));
        let mut fios = Vec::new();
        for _ in 0..24 {
            let s = s.clone();
            let dentro = Arc::clone(&dentro);
            let pico = Arc::clone(&pico);
            let entradas = Arc::clone(&entradas);
            fios.push(thread::spawn(move || {
                // Com prazo, e nao `adquirir()`: com o defeito reposto (a
                // permissao que nao devolve a vaga) este teste tem de
                // FALHAR, nunca pendurar -- o executor de guardas roda o
                // binario inteiro, e um teste pendurado mata a rodada em vez
                // de acusar o defeito.
                let _p = s
                    .adquirir_ate(Duration::from_secs(5))
                    .expect("cinco segundos sem vaga: alguem nao devolveu a dele");
                let agora = dentro.fetch_add(1, Ordering::SeqCst) + 1;
                pico.fetch_max(agora, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(3));
                dentro.fetch_sub(1, Ordering::SeqCst);
                entradas.fetch_add(1, Ordering::SeqCst);
            }));
        }
        for f in fios {
            f.join().unwrap();
        }
        assert_eq!(entradas.load(Ordering::SeqCst), 24, "alguem nao entrou");
        assert!(
            pico.load(Ordering::SeqCst) <= teto,
            "pico {} acima do teto {teto}",
            pico.load(Ordering::SeqCst)
        );
        assert_eq!(s.em_uso(), 0);
    }

    #[test]
    fn teto_zero_vira_um_e_sem_teto_nao_limita() {
        assert_eq!(Semaforo::novo(0).teto(), 1);
        assert!(Semaforo::novo(0).limitado());
        let s = Semaforo::sem_teto();
        assert!(!s.limitado());
        let muitas: Vec<Permissao> = (0..1_000).map(|_| s.tentar().unwrap()).collect();
        assert_eq!(s.em_uso(), 1_000);
        drop(muitas);
        assert_eq!(s.em_uso(), 0);
    }

    /// O mutex de dentro envenenado por um panico alheio nao trava o
    /// semaforo: o `Drop` e o `tentar` continuam funcionando.
    #[test]
    fn mutex_envenenado_nao_derruba() {
        let s = Semaforo::novo(2);
        let dentro = s.clone();
        // Envenena: panico com o guard do mutex interno na mao.
        let r = std::panic::catch_unwind(move || {
            let _guard = dentro.interno.em_uso.lock().unwrap();
            panic!("envenenando");
        });
        assert!(r.is_err());
        assert!(s.interno.em_uso.is_poisoned(), "o teste nao envenenou nada");
        let p = s.tentar().expect("tentar com mutex envenenado");
        assert_eq!(s.em_uso(), 1);
        drop(p);
        assert_eq!(s.em_uso(), 0);
    }
}

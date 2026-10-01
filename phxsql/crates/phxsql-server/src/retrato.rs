//! O PORTAO DO RETRATO: enquanto o backup copia, a escrita espera FORA da
//! trava de dados -- pedido 513, passo 1 (H2 da decisao de 01/10/2026).
//!
//! # O que o passo 1 queria, e por que a trava de leitura sozinha nao entrega
//!
//! O backup segurava a ficha EXCLUSIVA da trava de dados a copia inteira:
//! 100 GB sao 50-64 min com o banco parado para tudo, leitura inclusive. A
//! H2 era trocar a ficha exclusiva pela COMPARTILHADA -- o leitor anda, o
//! escritor espera, e o retrato continua sem escrita no meio.
//!
//! So que o `std::sync::RwLock` do Linux prefere o ESCRITOR: com um escritor
//! na fila, o leitor NOVO tambem espera (`is_read_lockable` recusa quando ha
//! escritor esperando -- e e isso que o `o_escritor_nao_passa_fome_entre_
//! leitores` prova). Num servidor de verdade alguem grava no primeiro minuto
//! de um backup de uma hora, e dali em diante a leitura voltava a parar ate
//! o fim. A troca simples de trava compilava, passava num teste que lesse
//! ANTES de gravar, e nao comprava nada no dia de uso: medido pelo soquete,
//! 1.373 ms de leitura contra 1.358 ms com a ficha exclusiva.
//!
//! # O desenho
//!
//! O escritor nao entra na fila do `RwLock` enquanto ha retrato em curso:
//! ele espera AQUI, antes dela. Sem escritor na fila, o leitor novo nao tem
//! por que esperar. Duas pecas:
//!
//! * `escritores`: quantos ja passaram por este portao e ainda nao soltaram
//!   a ficha exclusiva (estao na fila do `RwLock` ou com ela na mao);
//! * `retrato`: ha um backup copiando.
//!
//! O escritor SOMA e depois OLHA o `retrato`; o backup LIGA e depois OLHA os
//! `escritores`. Com `SeqCst` dos dois lados, ao menos um ve o outro (o
//! argumento de Dekker): ou o escritor ve o retrato ligado e recua, ou o
//! backup ve o escritor e espera ele soltar. Nunca os dois passam.
//!
//! A consistencia do retrato NAO depende deste portao: o escritor que
//! escapasse dele ainda pararia na ficha de leitura que a copia segura. O
//! portao decide so se o LEITOR espera.
//!
//! # O que custa a quem nao esta fazendo backup
//!
//! O portao vem ANTES do trabalho, e o caminho comum nao toca mutex nenhum:
//! um `fetch_add` e um `load` na entrada, mais um `fetch_add` quando a ficha
//! chega na mao (as `entradas`, pedido 623), um `fetch_sub` e um `load` na
//! saida. O mutex e o `Condvar` so existem para DORMIR -- quem espera um
//! retrato, ou o retrato que espera o ultimo escritor.
//!
//! # O leitor que CEDE a vez -- pedido 623
//!
//! O mesmo `RwLock` tem um furo do outro lado, e quem o mostrou foi a
//! absorcao do bidirecional (pedido 330), o unico leitor que toma a ficha
//! compartilhada em LACO. Ao soltar a leitura com um escritor na fila, o
//! `read_unlock` limpa o bit de escritor esperando e acorda o escritor; o
//! leitor, que nao dormiu, pede a leitura de novo antes de o acordado ser
//! escalado -- e a leva. O escritor acorda, acha a leitura tomada e volta a
//! dormir. Medido pelo soquete (01/10/2026, build de teste, 300.000 eventos,
//! fatias de 10 ms): o escritor esperava 115-360 ms por `inserir` com a
//! maquina parada e ate **2,1 s** sozinho, e sob a suite inteira nenhum
//! `inserir` terminou durante a absorcao. «Espera no maximo uma fatia» era a
//! promessa, e o `RwLock` nao a cumpre para leitor em laco.
//!
//! Por isso a [`Fila`] e o [`PortaoDoRetrato::ceder`]: com a leitura AINDA na
//! mao o leitor anota quantos escritores estao na fila e quantas entradas ja
//! houve; depois de solta-la, espera ate uma entrada nova (o escritor PEGOU a
//! ficha exclusiva -- nao terminou, pegou: o disco dele nao entra na conta),
//! ou ate a fila esvaziar sem entrar (recuou para um retrato, ou achou o
//! veneno). Nenhum escritor na fila, nenhuma espera.
//!
//! # Um retrato por vez
//!
//! Dois backups ao mesmo tempo eram impossiveis com a ficha exclusiva, e
//! continuam: o `retrato` e um interruptor, e o segundo espera o primeiro
//! desligar. Sem isso, duas copias para o MESMO destino se misturariam --
//! a ficha compartilhada, sozinha, deixaria as duas entrarem juntas.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// O portao. Um por servidor, ao lado da trava de dados.
#[derive(Default)]
pub(crate) struct PortaoDoRetrato {
    escritores: AtomicUsize,
    /// Quantas vezes um escritor PEGOU a ficha exclusiva -- so sobe. E o que
    /// o leitor que cede espera ver andar (ver [`PortaoDoRetrato::ceder`]).
    entradas: AtomicU64,
    retrato: AtomicBool,
    /// So para dormir e acordar. O estado de verdade esta nos atomicos.
    sono: Mutex<()>,
    acorda: Condvar,
}

/// O escritor que passou pelo portao. Solta no `Drop`, e deve viver ATE a
/// ficha exclusiva cair -- por isso mora como o ULTIMO campo da
/// `TravaMedida`, e campo cai depois do guard declarado antes dele.
pub(crate) struct Passagem<'a> {
    portao: &'a PortaoDoRetrato,
}

/// A fila dos escritores vista por um LEITOR, com a ficha compartilhada na
/// mao -- e so com ela na mao a foto vale: nenhum escritor esta dentro, entao
/// todo `escritores` contado esta esperando, e `entradas` nao anda.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fila {
    na_fila: usize,
    entradas: u64,
}

/// O que um leitor que cedeu a vez viu na volta seguinte -- ver
/// [`PortaoDoRetrato::depois_de_ceder`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Vez {
    /// Escritores que pegaram a ficha exclusiva entre a foto e a volta.
    pub(crate) entraram: u64,
    /// Havia escritor na fila, nenhum entrou e ainda ha quem espere: o
    /// leitor passou na frente dele.
    pub(crate) furou: bool,
}

/// O retrato em curso. Religa a escrita no `Drop` -- inclusive no desenrolar
/// de um panico no meio da copia, senao o servidor ficaria sem escrita ate
/// reiniciar.
pub(crate) struct Retrato<'a> {
    portao: &'a PortaoDoRetrato,
}

impl PortaoDoRetrato {
    /// O mutex do sono. Envenenado so se alguem cair DENTRO dele, e aqui
    /// dentro nao ha codigo que caia: o `()` nao tem estado para entortar.
    fn dormitorio(&self) -> MutexGuard<'_, ()> {
        self.sono.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// O escritor pede passagem. Volta na hora quando nao ha retrato.
    pub(crate) fn passar(&self) -> Passagem<'_> {
        loop {
            self.escritores.fetch_add(1, Ordering::SeqCst);
            if !self.retrato.load(Ordering::SeqCst) {
                return Passagem { portao: self };
            }
            // Ha retrato: recua (o backup pode estar esperando justamente
            // este escritor sair da conta) e dorme ate ele terminar.
            self.sair();
            let mut sono = self.dormitorio();
            while self.retrato.load(Ordering::SeqCst) {
                sono = self.acorda.wait(sono).unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    fn sair(&self) {
        self.escritores.fetch_sub(1, Ordering::SeqCst);
        // Quem acorda o backup e o escritor que sai da conta, e so quando ha
        // retrato esperando. Sem retrato, o caminho comum nao toca mutex. O
        // `notify` vem com o mutex na mao: o backup confere a conta e dorme
        // segurando-o, entao o aviso nao se perde entre as duas coisas.
        if self.retrato.load(Ordering::SeqCst) {
            let _sono = self.dormitorio();
            self.acorda.notify_all();
        }
    }

    /// O backup liga o retrato: espera o retrato anterior acabar, fecha a
    /// porta aos escritores novos e espera os que ja passaram soltarem a
    /// ficha exclusiva. Na volta, nenhum escritor esta dentro nem na fila.
    pub(crate) fn tirar_retrato(&self) -> Retrato<'_> {
        let mut sono = self.dormitorio();
        while self
            .retrato
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            sono = self.acorda.wait(sono).unwrap_or_else(|e| e.into_inner());
        }
        while self.escritores.load(Ordering::SeqCst) > 0 {
            sono = self.acorda.wait(sono).unwrap_or_else(|e| e.into_inner());
        }
        Retrato { portao: self }
    }

    /// A foto da fila, tirada por um leitor COM a ficha compartilhada na mao
    /// (ver [`Fila`]). Dois `load`, nenhum mutex.
    pub(crate) fn fila(&self) -> Fila {
        Fila {
            na_fila: self.escritores.load(Ordering::SeqCst),
            entradas: self.entradas.load(Ordering::SeqCst),
        }
    }

    /// O leitor, ja SEM a ficha, cede a vez a quem estava na fila da foto:
    /// volta quando um escritor pegou a exclusiva, quando a fila esvaziou sem
    /// ninguem entrar, ou no `teto`. Ninguem na fila, volta na hora.
    ///
    /// A espera e por ENTRADA e nao por saida: o escritor que pegou a ficha
    /// ja exclui o leitor pela propria trava, e esperar a saida poria o disco
    /// dele (um `fsync` lento) dentro da espera de quem cedeu. O `teto` e so
    /// a vida do leitor -- sem ele, um escritor preso fora da fila do `RwLock`
    /// por um motivo que esta conta nao ve pararia a absorcao para sempre.
    pub(crate) fn ceder(&self, foto: Fila, teto: Duration) {
        if foto.na_fila == 0 {
            return;
        }
        let ate = Instant::now() + teto;
        while self.entradas.load(Ordering::SeqCst) == foto.entradas
            && self.escritores.load(Ordering::SeqCst) > 0
            && Instant::now() < ate
        {
            // Dormir e nao girar: o escritor acordado precisa de um nucleo, e
            // um leitor girando num conteiner de 4 nucleos e quem o tira dele.
            std::thread::sleep(Duration::from_micros(100));
        }
    }

    /// O que aconteceu com a fila da `foto` -- lido pelo leitor ao RETOMAR a
    /// ficha compartilhada, e por isso independente de ele ter cedido: e a
    /// medida que acusa o leitor que nao cede (`furou`).
    pub(crate) fn depois_de_ceder(&self, foto: Fila) -> Vez {
        let entraram = self
            .entradas
            .load(Ordering::SeqCst)
            .saturating_sub(foto.entradas);
        Vez {
            entraram,
            furou: foto.na_fila > 0 && entraram == 0 && self.escritores.load(Ordering::SeqCst) > 0,
        }
    }

    /// Ha retrato em curso? So para teste.
    #[cfg(test)]
    pub(crate) fn em_retrato(&self) -> bool {
        self.retrato.load(Ordering::SeqCst)
    }
}

impl Passagem<'_> {
    /// O escritor PEGOU a ficha exclusiva. Chamado por quem a tomou, logo
    /// depois do `write()` -- um `fetch_add` por tomada exclusiva, o preco de
    /// o leitor em laco saber que cedeu (pedido 623).
    pub(crate) fn entrou(&self) {
        self.portao.entradas.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for Passagem<'_> {
    fn drop(&mut self) {
        self.portao.sair();
    }
}

impl Drop for Retrato<'_> {
    fn drop(&mut self) {
        let _sono = self.portao.dormitorio();
        self.portao.retrato.store(false, Ordering::SeqCst);
        self.portao.acorda.notify_all();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Sem retrato, o escritor passa e a conta volta a zero -- o
    /// comportamento VELHO, que nao pode mudar para ninguem.
    #[test]
    fn sem_retrato_o_escritor_passa_na_hora() {
        let p = PortaoDoRetrato::default();
        {
            let _a = p.passar();
            let _b = p.passar();
            assert_eq!(p.escritores.load(Ordering::SeqCst), 2);
        }
        assert_eq!(p.escritores.load(Ordering::SeqCst), 0);
    }

    /// O retrato espera o escritor que ja estava dentro, e o escritor que
    /// chega depois espera o retrato. Prazo de 5 s: dormir para sempre e o
    /// defeito que este teste existe para acusar.
    #[test]
    fn o_retrato_espera_quem_esta_dentro_e_barra_quem_chega() {
        let p = Arc::new(PortaoDoRetrato::default());
        let dentro = p.passar();
        let p2 = Arc::clone(&p);
        let backup = std::thread::spawn(move || {
            let inicio = Instant::now();
            let r = p2.tirar_retrato();
            let esperou = inicio.elapsed();
            std::thread::sleep(Duration::from_millis(150));
            drop(r);
            esperou
        });
        std::thread::sleep(Duration::from_millis(100));
        drop(dentro);
        let ate = Instant::now() + Duration::from_secs(5);
        while !p.em_retrato() {
            assert!(Instant::now() < ate, "o retrato nunca ligou");
            std::thread::sleep(Duration::from_millis(1));
        }
        let inicio = Instant::now();
        let _novo = p.passar();
        let barrado = inicio.elapsed();
        let esperou = backup.join().unwrap();
        assert!(
            esperou >= Duration::from_millis(80),
            "o retrato nao esperou o escritor de dentro: {esperou:?}"
        );
        assert!(
            barrado >= Duration::from_millis(50),
            "o escritor novo passou com o retrato ligado: {barrado:?}"
        );
    }

    /// Pedido 623: o leitor que cede a vez so volta quando o escritor da
    /// fila PEGOU a ficha exclusiva. Sem prazo de parede na conta: a espera
    /// termina pela entrada, e o teto de 30 s so existe para o teste nao
    /// pendurar se o defeito voltar.
    #[test]
    fn o_leitor_que_cede_so_volta_com_o_escritor_dentro() {
        let p = Arc::new(PortaoDoRetrato::default());
        let trava = Arc::new(std::sync::RwLock::new(()));
        let leitura = trava.read().unwrap();
        let (p2, t2) = (Arc::clone(&p), Arc::clone(&trava));
        let escritor = std::thread::spawn(move || {
            let passagem = p2.passar();
            let _w = t2.write().unwrap();
            passagem.entrou();
        });
        while p.escritores.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
        let foto = p.fila();
        drop(leitura);
        p.ceder(foto, Duration::from_secs(30));
        assert!(
            p.fila().entradas > foto.entradas,
            "o leitor voltou sem o escritor da fila ter pegado a ficha"
        );
        let _de_novo = trava.read().unwrap();
        assert_eq!(
            p.depois_de_ceder(foto),
            Vez {
                entraram: 1,
                furou: false
            }
        );
        escritor.join().unwrap();
    }

    /// O escritor que nao consegue entrar (outro leitor segura a ficha) faz
    /// quem cede esperar ate o teto -- e o que acusa um `ceder` que volta na
    /// hora. Cota de BAIXO no relogio, que carga nenhuma faz falhar: carga
    /// so alonga a espera.
    #[test]
    fn o_leitor_que_cede_espera_o_escritor_ate_o_teto() {
        let p = PortaoDoRetrato::default();
        let _outro_leitor_entra_e_fica = p.passar();
        let foto = p.fila();
        let inicio = Instant::now();
        p.ceder(foto, Duration::from_millis(200));
        assert!(
            inicio.elapsed() >= Duration::from_millis(200),
            "o leitor voltou em {:?} com um escritor na fila e nenhuma entrada",
            inicio.elapsed()
        );
        assert!(p.depois_de_ceder(foto).furou);
    }

    /// Sem escritor na fila, ceder nao espera nada -- o comportamento VELHO
    /// de quem le em laco sem ninguem gravar.
    #[test]
    fn sem_escritor_na_fila_ceder_volta_na_hora() {
        let p = PortaoDoRetrato::default();
        let foto = p.fila();
        let inicio = Instant::now();
        p.ceder(foto, Duration::from_secs(30));
        assert!(inicio.elapsed() < Duration::from_secs(30));
        assert_eq!(p.depois_de_ceder(foto), Vez::default());
    }

    /// O panico no meio da copia religa a escrita -- sem o `Drop`, o
    /// servidor ficaria sem escrita ate reiniciar.
    #[test]
    fn o_panico_no_retrato_religa_a_escrita() {
        let p = Arc::new(PortaoDoRetrato::default());
        let p2 = Arc::clone(&p);
        let caiu = std::thread::spawn(move || {
            let _r = p2.tirar_retrato();
            panic!("panico de teste no meio da copia");
        })
        .join();
        assert!(caiu.is_err());
        assert!(!p.em_retrato());
        let _passou = p.passar();
    }
}

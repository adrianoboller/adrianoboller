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
//! um `fetch_add` e um `load` na entrada, um `fetch_sub` e um `load` na
//! saida. O mutex e o `Condvar` so existem para DORMIR -- quem espera um
//! retrato, ou o retrato que espera o ultimo escritor.
//!
//! # Um retrato por vez
//!
//! Dois backups ao mesmo tempo eram impossiveis com a ficha exclusiva, e
//! continuam: o `retrato` e um interruptor, e o segundo espera o primeiro
//! desligar. Sem isso, duas copias para o MESMO destino se misturariam --
//! a ficha compartilhada, sozinha, deixaria as duas entrarem juntas.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

/// O portao. Um por servidor, ao lado da trava de dados.
#[derive(Default)]
pub(crate) struct PortaoDoRetrato {
    escritores: AtomicUsize,
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

    /// Ha retrato em curso? So para teste.
    #[cfg(test)]
    pub(crate) fn em_retrato(&self) -> bool {
        self.retrato.load(Ordering::SeqCst)
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

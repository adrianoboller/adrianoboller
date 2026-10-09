//! A3 do pedido 707: o produtor UNICO dos alarmes, e as marcas na origem.
//!
//! Desenho em `docs/propostas/aquario-707.md` §2.4 e §11.3. O que este
//! arquivo decide, e por que cada coisa mora aqui:
//!
//! * [`sinal`] e o unico caminho que cria um alarme. Alarme de tarefa vira um
//!   bit no `AtomicU32` da [`Atividade`] amarrada a esta thread; alarme de
//!   servidor vai ao [`sedimento`]. A `Ocorrencia` do 495-F2 entra no MESMO
//!   corpo, e em nenhum outro: dois produtores decidiriam duas vezes o que
//!   e alarme.
//! * As marcas ficam onde o fato acontece -- `trava_reentrante()`,
//!   `trava_de_dados_sem_reparo()`, o prazo dentro do `siga`, o gancho de E/S
//!   do `anotar` --, uma linha em cada.
//! * O dado corrompido e o unico deduzido no desfecho ([`conferir_o_1001`]),
//!   e a deducao so e honesta POR CAUSA das marcas de cima: o codigo 1001 e o
//!   mesmo para a trava pedida duas vezes, para a trava envenenada e para o
//!   arquivo corrompido, e os 267 construtores de `Corrompido` vivem no
//!   `store` e no `core`, que nao enxergam a telemetria. Quem separa as tres
//!   causas e o bit que a trava deixou antes; sem ele, as tres empatam.
//!
//! # O preco
//!
//! So o caminho RARO paga: todo ponto de marca esta num erro. O caminho
//! normal nao chama nada daqui. Com a telemetria desligada nao ha atividade
//! amarrada (`Telemetria::entrar` devolve `None`), e o [`sinal`] de tarefa
//! para na leitura de uma `RefCell` de thread, antes de qualquer trabalho.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use super::{Alarme, Escopo, Grupo};
use crate::telemetria::Atividade;

/// O codigo de `PhxError::Corrompido`. Conferido contra o enum no teste
/// `os_codigos_daqui_sao_os_do_erro`, para nao divergir calado.
const CODIGO_CORROMPIDO: u16 = 1001;

/// O produtor unico: marca o alarme onde ele aconteceu.
///
/// `dados` e o detalhe do fato (a operacao, o arquivo). Hoje ninguem o le:
/// e a `Ocorrencia` do 495-F2 que o leva, e ela nasce AQUI, no mesmo corpo
/// -- a assinatura ja o recebe para que as marcas na origem nao precisem
/// mudar quando ela entrar.
pub fn sinal(alarme: Alarme, dados: &str) {
    match alarme.escopo() {
        // O portao: sem atividade amarrada (telemetria desligada, thread de
        // servico, teste sem conexao) nao ha onde marcar, e nada se faz.
        Escopo::Tarefa => {
            if let Some(a) = crate::telemetria::corrente() {
                produzir(Some(&a), alarme, dados);
            }
        }
        Escopo::Servidor => produzir(None, alarme, dados),
    }
}

/// O mesmo produtor, para quem ja tem a atividade na mao e pode nao ser a
/// desta thread -- o `siga` e um metodo da propria atividade.
pub fn sinal_em(atividade: &Atividade, alarme: Alarme, dados: &str) {
    produzir(Some(atividade), alarme, dados);
}

/// O corpo unico dos dois de cima.
fn produzir(atividade: Option<&Atividade>, alarme: Alarme, _dados: &str) {
    match alarme.escopo() {
        Escopo::Tarefa => {
            if let Some(a) = atividade {
                a.alarmes.fetch_or(alarme.bit(), Ordering::Relaxed);
            }
        }
        Escopo::Servidor => sedimentar(alarme),
    }
    // A `Ocorrencia` do 495-F2 entra aqui (silencio -> fila -> carteiro).
}

/// O desfecho com codigo 1001 que nenhuma trava marcou e dado corrompido.
///
/// Chamado pelo `anotar` do servidor, o unico sumidouro, porque e o primeiro
/// lugar do servidor que ve o erro TIPADO com a atividade ainda amarrada. A
/// comparacao de inteiro vem antes de tudo: todo outro desfecho para nela.
pub fn conferir_o_1001(codigo: u16) {
    if codigo != CODIGO_CORROMPIDO {
        return;
    }
    let Some(a) = crate::telemetria::corrente() else {
        return;
    };
    if a.alarmes() & mascara_da_trava() == 0 {
        produzir(Some(&a), Alarme::DadoCorrompido, "");
    }
}

/// Os bits do grupo LOCK -- os 1001 que NAO sao o dado.
///
/// Lido do proprio enum, e nao uma lista daqui: alarme de trava novo entra
/// no `grupo()` e ja deixa de ser confundido com corrupcao.
fn mascara_da_trava() -> u32 {
    Alarme::TODOS
        .into_iter()
        .filter(|a| a.grupo() == Grupo::Lock)
        .fold(0, |m, a| m | a.bit())
}

impl Atividade {
    /// Os alarmes marcados na operacao corrente, como mascara de bits. Volta
    /// a zero no `comecou_pedido`: o alarme e da TAREFA (chave#serial), e
    /// carregar o de ontem pintaria o pedido de hoje.
    pub fn alarmes(&self) -> u32 {
        self.alarmes.load(Ordering::Relaxed)
    }
}

// ------------------------------------------------------------- o sedimento

/// Uma vaga por alarme, na ordem de [`Alarme::TODOS`]; so as de servidor sao
/// usadas.
///
/// # Por que estatico, e nao dentro do `Aquario`
///
/// Porque as origens dos alarmes de servidor (o fecho recusado, o firewall,
/// a replica) sao funcoes livres e lacos de fundo que nao tem o `Telemetria`
/// na mao, e o [`sinal`] e uma funcao livre pelo mesmo motivo. O processo
/// tem um servidor; o sedimento e do processo.
const VAGAS: usize = Alarme::TODOS.len();
static VISTO_MS: [AtomicI64; VAGAS] = [const { AtomicI64::new(0) }; VAGAS];
static VEZES: [AtomicU64; VAGAS] = [const { AtomicU64::new(0) }; VAGAS];

fn vaga(alarme: Alarme) -> usize {
    Alarme::TODOS
        .into_iter()
        .position(|a| a == alarme)
        .unwrap_or_default()
}

fn sedimentar(alarme: Alarme) {
    let i = vaga(alarme);
    VISTO_MS[i].store(crate::agora_ms(), Ordering::Relaxed);
    VEZES[i].fetch_add(1, Ordering::Relaxed);
}

/// Uma pedra do sedimento: o alarme de servidor, quando foi visto pela
/// ultima vez e quantas vezes desde o arranque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pedra {
    pub alarme: Alarme,
    pub visto_ms: i64,
    pub vezes: u64,
}

/// Os alarmes de servidor ja vistos neste processo. Quanto tempo uma pedra
/// fica na tela e decisao da tela (A10), que tem o relogio de quem olha;
/// aqui se guarda o fato, e o fato nao vence.
pub fn sedimento() -> Vec<Pedra> {
    Alarme::TODOS
        .into_iter()
        .enumerate()
        .filter(|(_, a)| a.escopo() == Escopo::Servidor)
        .filter_map(|(i, alarme)| {
            let vezes = VEZES[i].load(Ordering::Relaxed);
            (vezes > 0).then(|| Pedra {
                alarme,
                visto_ms: VISTO_MS[i].load(Ordering::Relaxed),
                vezes,
            })
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::error::PhxError;

    #[test]
    fn os_codigos_daqui_sao_os_do_erro() {
        assert_eq!(
            PhxError::Corrompido(String::new()).codigo(),
            CODIGO_CORROMPIDO
        );
    }

    /// A mascara da trava pega os dois alarmes de trava e nenhum outro --
    /// senao o dado corrompido sumiria atras de um bit que nao e de trava.
    #[test]
    fn a_mascara_da_trava_e_so_da_trava() {
        let m = mascara_da_trava();
        assert_ne!(m & Alarme::TravaReentrante.bit(), 0);
        assert_ne!(m & Alarme::TravaEnvenenada.bit(), 0);
        assert_eq!(m & Alarme::DadoCorrompido.bit(), 0);
        assert_eq!(m & Alarme::ErroDeDisco.bit(), 0);
    }

    /// Alarme de servidor nao procura tarefa: vai ao sedimento, com o
    /// instante e a conta. `FechoRecusado` e usado so aqui, porque o
    /// sedimento e do processo e os testes correm juntos.
    #[test]
    fn alarme_de_servidor_vai_ao_sedimento() {
        let antes = crate::agora_ms();
        sinal(Alarme::FechoRecusado, "teste");
        sinal(Alarme::FechoRecusado, "teste");
        let pedra = sedimento()
            .into_iter()
            .find(|p| p.alarme == Alarme::FechoRecusado)
            .expect("o fecho recusado nao chegou ao sedimento");
        assert!(pedra.vezes >= 2, "{pedra:?}");
        assert!(pedra.visto_ms >= antes, "{pedra:?}");
    }
}

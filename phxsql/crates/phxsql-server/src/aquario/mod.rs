//! O aquario (pedido 707): o esqueleto onde as fatias seguintes entram.
//!
//! Desenho em `docs/propostas/aquario-707.md`, com a §11 (A0) mandando onde
//! as secoes antigas a contradizem. Esta e a fatia A2: o que as outras
//! PARTILHAM e que, escrito por qualquer uma delas primeiro, faria dela dona
//! por acidente --
//!
//! * o [`Alarme`], um enum so para o aquario e para a `Ocorrencia` do 495
//!   (§11.3, hipotese Hb3: um enum, um produtor, dois arquivos com papeis);
//! * o [`Aquario`] que mora dentro do `Telemetria`, e o UNICO ponto por onde o
//!   `anotar` do servidor o alimenta;
//! * as duas consultas, `aquario_log` e `aquario_contagens`, que o servidor
//!   ja despacha atras do direito `monitorar`.
//!
//! # O que AINDA NAO existe, e diz que nao existe
//!
//! As consultas devolvem erro nomeando a fatia que falta, em vez de uma lista
//! vazia: lista vazia diria «nada aconteceu» num servidor onde ninguem esta
//! escrevendo o log -- a mentira que a lei «configuracao que nao e lida
//! mente» ja pagou uma vez.
//!
//! # Onde cada fatia entra, cada uma no proprio arquivo
//!
//! | fatia | arquivo | o gancho aqui |
//! |---|---|---|
//! | A3 | `aquario/alarme.rs` | `telemetria::sinal(Alarme, dados)`: o bit [`Alarme::bit`] na tarefa corrente, e o sedimento para [`Escopo::Servidor`] |
//! | A4 | `aquario/base.rs` | em [`Aquario::anotar`]: a base Welford sobre `ln(µs)` |
//! | A6 | `aquario/log.rs` | o `aquario.log` pelo [`crate::acesso::LogAcessos::registrar_json`], e o corpo de [`Aquario::consultar_log`] |
//! | A8 | `aquario/contagem.rs` | em [`Aquario::anotar`]: o acumulador do minuto; o corpo de [`Aquario::contagens`] |

pub mod log;

use std::sync::atomic::{AtomicU64, Ordering};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::acesso::Acesso;

/// Quao grave e um alarme -- a cor que ele empresta a bolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravidade {
    Vermelho,
    Amarelo,
}

impl Gravidade {
    pub fn nome(self) -> &'static str {
        match self {
            Gravidade::Vermelho => "vermelho",
            Gravidade::Amarelo => "amarelo",
        }
    }
}

/// A familia do alarme -- a letra que a bolha vermelha carrega (§2.4).
///
/// As familias da IA (ataque, defeito, previsao; `ia-495-496-desenho.md`)
/// entram junto com o primeiro alarme delas, e nao antes: grupo sem alarme
/// nenhum seria chave morta na tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Lock,
    Disco,
    Dado,
    Replica,
    Seguranca,
    Prazo,
}

impl Grupo {
    pub fn nome(self) -> &'static str {
        match self {
            Grupo::Lock => "lock",
            Grupo::Disco => "disco",
            Grupo::Dado => "dado",
            Grupo::Replica => "replica",
            Grupo::Seguranca => "seguranca",
            Grupo::Prazo => "prazo",
        }
    }
}

/// De quem e o alarme: de UMA tarefa (vira bit na `Atividade` dela) ou do
/// servidor inteiro (vira sedimento, porque nenhuma tarefa o carrega).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escopo {
    Tarefa,
    Servidor,
}

/// Um alarme -- o mesmo tipo para a bolha do aquario e para a `Ocorrencia`
/// do 495 (`aquario-707.md` §11.3).
///
/// # Por que um enum so
///
/// Dois enums responderiam duas vezes «isto e grave?», e um dia pintariam a
/// bolha de uma cor e mandariam o e-mail por outra. O nome nao e `Sinal`
/// porque `sinais.rs` ja e o SIGTERM (pedido 687).
///
/// # O que NAO e alarme
///
/// O que se le do ESTADO da tarefa -- esperando a trava, segurando a trava
/// com fila, grande, acima do tempo fixo -- e classificacao, e mora na
/// `classificar` unica (A5). Alarme e o que alguem MARCA na origem, porque o
/// estado nao o distingue: o `trava_reentrante` e o dado corrompido devolvem
/// o mesmo codigo 1001, e o prazo e o encerrar manual o mesmo 6001.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alarme {
    // ---------------------------------------------- de tarefa: bit na origem
    /// A tarefa pediu a trava de dados que ja segurava.
    TravaReentrante,
    /// A tarefa achou a trava envenenada por um panico de outra.
    TravaEnvenenada,
    /// Erro de E/S, disco cheio ou so-leitura no caminho da tarefa.
    ErroDeDisco,
    /// A conferencia achou dado corrompido (o 1001 SEM o bit de reentrante).
    DadoCorrompido,
    /// A transacao passou do teto de volume que a replica aceita.
    TransacaoAcimaDoTeto,
    /// Tentativas seguidas de login recusado.
    ForcaBruta,
    /// Senha chegando em claro por um fio que deveria cifra-la (pedido 667).
    SenhaEmClaro,
    /// Cancelada pelo STATEMENT TIMEOUT -- nao pelo encerrar manual.
    PrazoEstourado,
    /// Tempo de servico fora do habitual da propria operacao e tabela: z ≥ 4
    /// e ≥ 250 ms (§11.1). Amarelo: o azul escuro da tarefa grande e CLASSE.
    ForaDoHabitual,
    /// Fora do habitual de novo, na mesma chave, em pouco tempo.
    ForaDoHabitualReincidente,
    /// Recusa de integridade, duplicado ou conflito. Amarelo de proposito
    /// (§2.4): o motor fez o trabalho dele e o dado esta PROTEGIDO.
    IntegridadeRecusada,
    // ---------------------------------------------- de servidor: sedimento
    /// O fecho da janela de escrita foi recusado.
    FechoRecusado,
    /// Um `fsync` recusado num arranque ANTERIOR (509: ao vivo o processo cai).
    FsyncRecusadoAntes,
    /// Marca impossivel, nao lida ou parada no arranque.
    MarcaNaoResolvida,
    /// Indice que ficou para tras no arranque.
    IndiceAtrasado,
    /// A continuidade da replica rompeu.
    ContinuidadeRompida,
    /// A origem da replica inalcancavel alem do prazo.
    OrigemInalcancavel,
    /// O firewall recebeu um bloqueio.
    FirewallBloqueou,
    /// A sonda do disco o achou lento.
    DiscoLento,
}

impl Alarme {
    /// Todos, na ordem da declaracao. Os testes conferem que nenhum fica de
    /// fora, pelo `match` exaustivo do [`Alarme::chave`].
    pub const TODOS: [Alarme; 19] = [
        Alarme::TravaReentrante,
        Alarme::TravaEnvenenada,
        Alarme::ErroDeDisco,
        Alarme::DadoCorrompido,
        Alarme::TransacaoAcimaDoTeto,
        Alarme::ForcaBruta,
        Alarme::SenhaEmClaro,
        Alarme::PrazoEstourado,
        Alarme::ForaDoHabitual,
        Alarme::ForaDoHabitualReincidente,
        Alarme::IntegridadeRecusada,
        Alarme::FechoRecusado,
        Alarme::FsyncRecusadoAntes,
        Alarme::MarcaNaoResolvida,
        Alarme::IndiceAtrasado,
        Alarme::ContinuidadeRompida,
        Alarme::OrigemInalcancavel,
        Alarme::FirewallBloqueou,
        Alarme::DiscoLento,
    ];

    /// A chave da fabrica de idiomas. E o MESMO texto da bolha e do e-mail,
    /// e o que vai ao log -- o log e neutro de idioma, a tela traduz.
    ///
    /// Escrita por extenso, e nao montada de prefixo + nome: o conferidor de
    /// idiomas acha chave usada procurando o literal no fonte.
    pub fn chave(self) -> &'static str {
        match self {
            Alarme::TravaReentrante => "aquario.motivo.trava_reentrante",
            Alarme::TravaEnvenenada => "aquario.motivo.trava_envenenada",
            Alarme::ErroDeDisco => "aquario.motivo.erro_de_disco",
            Alarme::DadoCorrompido => "aquario.motivo.dado_corrompido",
            Alarme::TransacaoAcimaDoTeto => "aquario.motivo.transacao_acima_do_teto",
            Alarme::ForcaBruta => "aquario.motivo.forca_bruta",
            Alarme::SenhaEmClaro => "aquario.motivo.senha_em_claro",
            Alarme::PrazoEstourado => "aquario.motivo.prazo_estourado",
            Alarme::ForaDoHabitual => "aquario.motivo.fora_do_habitual",
            Alarme::ForaDoHabitualReincidente => "aquario.motivo.fora_do_habitual_reincidente",
            Alarme::IntegridadeRecusada => "aquario.motivo.integridade_recusada",
            Alarme::FechoRecusado => "aquario.motivo.fecho_recusado",
            Alarme::FsyncRecusadoAntes => "aquario.motivo.fsync_recusado_antes",
            Alarme::MarcaNaoResolvida => "aquario.motivo.marca_nao_resolvida",
            Alarme::IndiceAtrasado => "aquario.motivo.indice_atrasado",
            Alarme::ContinuidadeRompida => "aquario.motivo.continuidade_rompida",
            Alarme::OrigemInalcancavel => "aquario.motivo.origem_inalcancavel",
            Alarme::FirewallBloqueou => "aquario.motivo.firewall_bloqueou",
            Alarme::DiscoLento => "aquario.motivo.disco_lento",
        }
    }

    /// O nome curto, que o log grava: a chave sem o prefixo.
    pub fn nome(self) -> &'static str {
        let chave = self.chave();
        chave.strip_prefix("aquario.motivo.").unwrap_or(chave)
    }

    /// O alarme pelo nome que o log gravou. `None` para nome desconhecido --
    /// um log escrito por versao mais nova nao vira outro alarme aqui.
    pub fn de_nome(nome: &str) -> Option<Alarme> {
        Alarme::TODOS.into_iter().find(|a| a.nome() == nome)
    }

    pub fn gravidade(self) -> Gravidade {
        match self {
            Alarme::ForaDoHabitual | Alarme::IntegridadeRecusada | Alarme::DiscoLento => {
                Gravidade::Amarelo
            }
            _ => Gravidade::Vermelho,
        }
    }

    pub fn grupo(self) -> Grupo {
        match self {
            Alarme::TravaReentrante | Alarme::TravaEnvenenada => Grupo::Lock,
            Alarme::ErroDeDisco
            | Alarme::FechoRecusado
            | Alarme::FsyncRecusadoAntes
            | Alarme::DiscoLento => Grupo::Disco,
            Alarme::DadoCorrompido
            | Alarme::IntegridadeRecusada
            | Alarme::MarcaNaoResolvida
            | Alarme::IndiceAtrasado => Grupo::Dado,
            Alarme::TransacaoAcimaDoTeto
            | Alarme::ContinuidadeRompida
            | Alarme::OrigemInalcancavel => Grupo::Replica,
            Alarme::ForcaBruta | Alarme::SenhaEmClaro | Alarme::FirewallBloqueou => {
                Grupo::Seguranca
            }
            Alarme::PrazoEstourado | Alarme::ForaDoHabitual | Alarme::ForaDoHabitualReincidente => {
                Grupo::Prazo
            }
        }
    }

    pub fn escopo(self) -> Escopo {
        if self.bit() == 0 {
            Escopo::Servidor
        } else {
            Escopo::Tarefa
        }
    }

    /// O bit deste alarme no `AtomicU32` da tarefa (A3), ou 0 quando o alarme
    /// e do servidor.
    ///
    /// Numeros fixos, e nao a posicao na lista: o bit pode ir ao log, e
    /// reordenar a declaracao nao pode mudar o que um log antigo quer dizer.
    pub fn bit(self) -> u32 {
        let posicao = match self {
            Alarme::TravaReentrante => 0,
            Alarme::TravaEnvenenada => 1,
            Alarme::ErroDeDisco => 2,
            Alarme::DadoCorrompido => 3,
            Alarme::TransacaoAcimaDoTeto => 4,
            Alarme::ForcaBruta => 5,
            Alarme::SenhaEmClaro => 6,
            Alarme::PrazoEstourado => 7,
            Alarme::ForaDoHabitual => 8,
            Alarme::ForaDoHabitualReincidente => 9,
            Alarme::IntegridadeRecusada => 10,
            Alarme::FechoRecusado
            | Alarme::FsyncRecusadoAntes
            | Alarme::MarcaNaoResolvida
            | Alarme::IndiceAtrasado
            | Alarme::ContinuidadeRompida
            | Alarme::OrigemInalcancavel
            | Alarme::FirewallBloqueou
            | Alarme::DiscoLento => return 0,
        };
        1 << posicao
    }

    /// Os alarmes de tarefa presentes numa mascara de bits.
    pub fn da_mascara(mascara: u32) -> impl Iterator<Item = Alarme> {
        Alarme::TODOS
            .into_iter()
            .filter(move |a| a.bit() != 0 && mascara & a.bit() != 0)
    }
}

/// O aquario que mora no `Telemetria`.
///
/// So se chega a ele para ANOTAR por [`crate::telemetria::Telemetria::aquario_se_ligada`],
/// que devolve `None` com a telemetria desligada: o portao vem antes de
/// qualquer trabalho, e nao ha outro caminho ate o [`Aquario::anotar`] que o
/// pule (lei «instrumentacao desligada custa zero»).
#[derive(Debug, Default)]
pub struct Aquario {
    /// Quantos pedidos passaram pelo [`Aquario::anotar`]. E a prova de que o
    /// portao vem antes: telemetria desligada, zero aqui -- e as fatias A4 e
    /// A8, que farao trabalho de verdade no mesmo ponto, herdam a prova.
    anotados: AtomicU64,
    /// O `aquario.log` (A6). Nasce fechado; o servidor o abre no arranque.
    log: log::LogDoAquario,
}

impl Aquario {
    /// O ponto unico onde o fim de todo pedido chega ao aquario. Chamado pelo
    /// `anotar` do servidor, o unico sumidouro por onde toda resposta passa.
    pub fn anotar(&self, _acesso: &Acesso) {
        self.anotados.fetch_add(1, Ordering::Relaxed);
        // A4 entra aqui: a base do `_acesso` (Welford sobre ln(µs), sem as
        // OPS_DE_REPLICACAO). A8 entra aqui: o acumulador do minuto, pela
        // categoria que a op devolveu.
    }

    pub fn anotados(&self) -> u64 {
        self.anotados.load(Ordering::Relaxed)
    }

    /// O `aquario.log`, para o servidor abrir no arranque e gravar pelo
    /// `anotar` dele (a falha de gravacao vai ao `evento_de_disco`, que e do
    /// servidor), e para as fatias que gravam `mudou`, `contagem`...
    pub fn log(&self) -> &log::LogDoAquario {
        &self.log
    }

    /// `aquario_log`: a linha do tempo, lida de tras para a frente (§4.3).
    /// Fechado, diz que ainda nao ha arquivo -- e nao «nada aconteceu».
    pub fn consultar_log(&self, pedido: &Json) -> Result<Json> {
        self.log.consultar(pedido)
    }

    /// `aquario_contagens`: as oito series por hora (§11.4).
    pub fn contagens(&self, _pedido: &Json) -> Result<Json> {
        Err(ainda_nao_existe(
            "aquario_contagens",
            "a contagem por minuto e por hora ainda nao existe neste servidor \
             (fatia A8 do pedido 707)",
        ))
    }
}

/// O erro de quem pergunta por uma parte do aquario que ainda nao entrou.
///
/// `NAO_ENCONTRADO`, e nao uma resposta vazia: quem recebe sabe que nao ha o
/// que ler, em vez de concluir que nada aconteceu.
fn ainda_nao_existe(op: &str, porque: &str) -> PhxError {
    PhxError::NaoEncontrado(format!("{op}: {porque}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cada_alarme_tem_chave_unica_e_nome_que_volta() {
        let mut vistos = std::collections::HashSet::new();
        for a in Alarme::TODOS {
            assert!(a.chave().starts_with("aquario.motivo."), "{a:?}");
            assert!(vistos.insert(a.chave()), "chave repetida: {}", a.chave());
            assert_eq!(Alarme::de_nome(a.nome()), Some(a), "{a:?}");
        }
        assert_eq!(Alarme::de_nome("alarme_que_nao_existe"), None);
    }

    /// O bit cabe num `AtomicU32`, nao se repete, e existe so para alarme de
    /// tarefa -- o de servidor nao tem tarefa onde marcar.
    #[test]
    fn os_bits_sao_unicos_e_cabem_em_32() {
        let mut usados = 0u32;
        let mut de_tarefa = 0;
        for a in Alarme::TODOS {
            let b = a.bit();
            match a.escopo() {
                Escopo::Servidor => assert_eq!(b, 0, "{a:?}"),
                Escopo::Tarefa => {
                    de_tarefa += 1;
                    assert_eq!(b.count_ones(), 1, "{a:?}");
                    assert_eq!(usados & b, 0, "bit repetido em {a:?}");
                    usados |= b;
                }
            }
        }
        assert!(de_tarefa <= 32);
        let mascara = Alarme::TravaReentrante.bit() | Alarme::PrazoEstourado.bit();
        let lidos: Vec<Alarme> = Alarme::da_mascara(mascara).collect();
        assert_eq!(lidos, vec![Alarme::TravaReentrante, Alarme::PrazoEstourado]);
    }

    /// Os dois que o codigo de erro NAO separa (§1.2, L2) sao alarmes
    /// diferentes -- e o motivo de o alarme existir.
    #[test]
    fn reentrante_e_corrompido_sao_alarmes_diferentes() {
        assert_ne!(Alarme::TravaReentrante.bit(), Alarme::DadoCorrompido.bit());
        assert_ne!(
            Alarme::TravaReentrante.grupo(),
            Alarme::DadoCorrompido.grupo()
        );
        assert_eq!(Alarme::IntegridadeRecusada.gravidade(), Gravidade::Amarelo);
        assert_eq!(Alarme::DadoCorrompido.gravidade(), Gravidade::Vermelho);
    }

    #[test]
    fn as_consultas_que_faltam_dizem_que_faltam() {
        let aq = Aquario::default();
        for r in [aq.consultar_log(&Json::Nulo), aq.contagens(&Json::Nulo)] {
            let e = r.unwrap_err();
            assert_eq!(e.nome(), "NAO_ENCONTRADO");
            assert!(e.to_string().contains("ainda nao"), "{e}");
        }
    }
}

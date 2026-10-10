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
//! # O que ainda nao existe diz que nao existe
//!
//! O `aquario_log` com o arquivo fechado (nao abriu no arranque) devolve
//! erro com o motivo, em vez de uma lista vazia: lista vazia diria «nada
//! aconteceu» num servidor onde ninguem esta escrevendo o log -- a mentira
//! que a lei «configuracao que nao e lida mente» ja pagou uma vez.
//!
//! # Como as fatias se ligam
//!
//! Todas pelo `anotar` do servidor e pelo amostrador, nunca uma chamando a
//! outra por dentro: a contagem (A8) fecha o minuto e o servidor grava a
//! linha `contagem` pelo escritor do log (A6), que a A8 le de volta no
//! arranque; a base (A4) devolve o desvio, e o servidor o entrega ao
//! produtor unico (A3) e grava a linha `mudou` (A6).
//!
//! # Onde cada fatia entra, cada uma no proprio arquivo
//!
//! | fatia | arquivo | o gancho aqui |
//! |---|---|---|
//! | A3 | `aquario/alarme.rs` | `telemetria::sinal(Alarme, dados)`: o bit [`Alarme::bit`] na tarefa corrente, e o sedimento para [`Escopo::Servidor`] |
//! | A4 | `aquario/base.rs` | em [`Aquario::anotar`]: a base Welford sobre `ln(µs)`; o desvio volta ao servidor |
//! | A5 | `aquario/classe.rs` | a [`classificar`] unica: o retrato (`Telemetria::retrato_do_aquario` e o `nivel` do painel) e as linhas `estourou`/`mudou` do log pintam por ela |
//! | A6 | `aquario/log.rs` | o `aquario.log` pelo [`crate::acesso::LogAcessos::registrar_json`], e o corpo de [`Aquario::consultar_log`] |
//! | 780 | `aquario/anel.rs` | o anel da bolha que passou do teto do raio: o retrato o manda em `anel`, e o amostrador grava a linha `anel` do log quando ele sobe |
//! | A8 | `aquario/contagem.rs` | em [`Aquario::anotar`]: o acumulador do minuto; o corpo de [`Aquario::contagens`] (feito) |

pub mod alarme;
pub mod anel;
pub mod base;
pub mod classe;
pub mod contagem;
pub mod log;
pub mod perfil;

use std::sync::atomic::{AtomicU64, Ordering};

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::acesso::Acesso;

pub use classe::{classificar, Classe, Cor, Fatos, Tamanho};

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
    /// A familia «previsao» do 495/496 (`aquario-707.md` §11.3): o recurso
    /// que ainda nao acabou, mas vai. Grupo proprio porque memoria e
    /// descritores nao sao DISCO, e pintar a previsao de memoria com a letra
    /// do disco seria mentir sobre o que esta acabando.
    Previsao,
    /// A familia «ataque» do 495 (`aquario-707.md` §11.3): o pedido que tem
    /// a forma de uma injecao. Grupo proprio, e nao SEGURANCA: forca bruta e
    /// senha em claro falam do LOGIN; esta fala do que o pedido de alguem ja
    /// autenticado tentou fazer com os dados.
    Ataque,
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
            Grupo::Previsao => "previsao",
            Grupo::Ataque => "ataque",
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
    /// Pedido 496, F5: a tendencia de um recurso que so diminui (disco livre,
    /// `MemAvailable`, descritores) cruza o piso em menos de 24 h. Amarelo:
    /// nada quebrou ainda -- e o degrau do aviso, no molde do 40M/3M do xid.
    EsgotamentoPrevisto,
    /// O mesmo, em menos de 2 h: o degrau critico.
    EsgotamentoIminente,
    /// Pedido 496, F7: uma tabela da replica ficando para tras da origem --
    /// o residuo crescendo em 3 amostras, ou o evento mais velho nao
    /// consumido esperando mais de 60 s. Amarelo: nada se perdeu ainda, mas
    /// e o RPO de uma promocao feita agora.
    ReplicaAtrasada,
    /// Pedido 495, F3: o pedido tem a forma de uma injecao de SQL -- uma das
    /// quatro classes do `phxsql_sql::sinais`. Observa e nunca recusa:
    /// vermelho porque a tautologia que da CERTO ja devolveu as linhas.
    InjecaoSuspeita,
    /// Pedido 496, F8 (C10/C11): o plano de um `UPDATE`/`DELETE` por faixa,
    /// ou de uma cascata, alcanca >= 50% das linhas vivas e >= 1.000 --
    /// medido ANTES da primeira escrita. De TAREFA, embora esteja no fim da
    /// lista de servidor: o plano e de um pedido. Amarelo, porque so observa:
    /// nada foi recusado e a resposta nao muda.
    PlanoLargo,
    /// Pedidos 765/767: a camada de protecao recusou um comando da lista de
    /// perigo porque a sessao nao estava liberada pela senha de execucao. De
    /// TAREFA e vermelho: a bolha mostra que o comando foi BLOQUEADO, e o
    /// motivo e esta chave.
    ComandoBloqueado,
    /// Pedido 767: a senha de execucao nao conferiu (errada, a de login no
    /// lugar dela, ou a conta bloqueada por tentativas). Vermelho e de
    /// TAREFA: e a tentativa de liberar comando perigoso que falhou.
    SenhaDeExecucaoRecusada,
    /// Pedido 765, P6: o primeiro login com sucesso de (usuario, database,
    /// IP) nunca visto em 90 dias. De TAREFA (e o pedido de login) e
    /// amarelo: nada foi recusado -- o aviso e para quem administra olhar se
    /// reconhece a origem.
    IpNovo,
}

impl Alarme {
    /// Todos, na ordem da declaracao. Os testes conferem que nenhum fica de
    /// fora, pelo `match` exaustivo do [`Alarme::chave`].
    pub const TODOS: [Alarme; 27] = [
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
        Alarme::EsgotamentoPrevisto,
        Alarme::EsgotamentoIminente,
        Alarme::ReplicaAtrasada,
        Alarme::InjecaoSuspeita,
        Alarme::PlanoLargo,
        Alarme::ComandoBloqueado,
        Alarme::SenhaDeExecucaoRecusada,
        Alarme::IpNovo,
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
            Alarme::EsgotamentoPrevisto => "aquario.motivo.esgotamento_previsto",
            Alarme::EsgotamentoIminente => "aquario.motivo.esgotamento_iminente",
            Alarme::ReplicaAtrasada => "aquario.motivo.replica_atrasada",
            Alarme::InjecaoSuspeita => "aquario.motivo.injecao_suspeita",
            Alarme::PlanoLargo => "aquario.motivo.plano_largo",
            Alarme::ComandoBloqueado => "aquario.motivo.comando_bloqueado",
            Alarme::SenhaDeExecucaoRecusada => "aquario.motivo.senha_de_execucao_recusada",
            Alarme::IpNovo => "aquario.motivo.ip_novo",
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
            Alarme::ForaDoHabitual
            | Alarme::IntegridadeRecusada
            | Alarme::DiscoLento
            | Alarme::EsgotamentoPrevisto
            | Alarme::ReplicaAtrasada
            | Alarme::PlanoLargo
            | Alarme::IpNovo => Gravidade::Amarelo,
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
            | Alarme::OrigemInalcancavel
            | Alarme::ReplicaAtrasada => Grupo::Replica,
            Alarme::ForcaBruta
            | Alarme::SenhaEmClaro
            | Alarme::FirewallBloqueou
            | Alarme::ComandoBloqueado
            | Alarme::SenhaDeExecucaoRecusada
            | Alarme::IpNovo => Grupo::Seguranca,
            Alarme::PrazoEstourado | Alarme::ForaDoHabitual | Alarme::ForaDoHabitualReincidente => {
                Grupo::Prazo
            }
            // O plano largo e previsao do DANO, nao do recurso: o numero
            // existe antes da primeira escrita (horizonte «total», §6).
            Alarme::EsgotamentoPrevisto | Alarme::EsgotamentoIminente | Alarme::PlanoLargo => {
                Grupo::Previsao
            }
            Alarme::InjecaoSuspeita => Grupo::Ataque,
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
            Alarme::InjecaoSuspeita => 11,
            Alarme::PlanoLargo => 12,
            Alarme::ComandoBloqueado => 13,
            Alarme::SenhaDeExecucaoRecusada => 14,
            Alarme::IpNovo => 15,
            Alarme::FechoRecusado
            | Alarme::FsyncRecusadoAntes
            | Alarme::MarcaNaoResolvida
            | Alarme::IndiceAtrasado
            | Alarme::ContinuidadeRompida
            | Alarme::OrigemInalcancavel
            | Alarme::FirewallBloqueou
            | Alarme::DiscoLento
            | Alarme::EsgotamentoPrevisto
            | Alarme::EsgotamentoIminente
            | Alarme::ReplicaAtrasada => return 0,
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
    /// A linha de base (A4).
    base: base::BaseDeConsultas,
    /// O acumulador da contagem (A8, `contagem.rs`).
    contagem: contagem::Contagem,
}

impl Aquario {
    /// O ponto unico onde o fim de todo pedido chega ao aquario. Chamado pelo
    /// `anotar` do servidor, o unico sumidouro por onde toda resposta passa.
    ///
    /// Devolve o julgamento da base: o desvio quando houve, e se havia
    /// habitual -- a [`classificar`] do pedido que termina precisa dos dois.
    pub fn anotar(&self, acesso: &Acesso) -> base::Habitual {
        self.anotados.fetch_add(1, Ordering::Relaxed);
        self.contagem.somar(acesso);
        // O desvio volta ao `anotar` do servidor, que o entrega ao produtor
        // unico da A3 e grava a linha `mudou` (A6): o bit e da tarefa e a
        // falha de disco e do servidor, e nenhum dos dois mora aqui.
        self.base.anotar(acesso)
    }

    /// A base, para ler (tela e testes).
    pub fn base(&self) -> &base::BaseDeConsultas {
        &self.base
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
    pub fn contagens(&self, pedido: &Json) -> Result<Json> {
        self.contagem.consultar(pedido)
    }

    /// O acumulador, para a virada do amostrador e o arranque.
    pub fn contagem(&self) -> &contagem::Contagem {
        &self.contagem
    }
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

    /// As chaves da tabela `MOTIVOS` do `ui/aquario.js`, lidas do PROPRIO
    /// arquivo que o `http.rs` embute: entre `const MOTIVOS = {` e o `};`
    /// que a fecha, o primeiro literal de cada linha.
    fn chaves_de_motivos(js: &str) -> Vec<&str> {
        let comeco = js
            .find("const MOTIVOS = {")
            .expect("a tabela MOTIVOS sumiu do aquario.js");
        let corpo = &js[comeco..];
        let fim = corpo.find("\n  };").expect("a tabela MOTIVOS nao fecha");
        corpo[..fim]
            .lines()
            .filter_map(|l| {
                let l = l.trim_start();
                let resto = l.strip_prefix('"')?;
                resto.split_once('"').map(|(chave, _)| chave)
            })
            .collect()
    }

    /// **A16 do 707:** todo alarme do enum tem motivo na tela. A tela que
    /// recebe uma chave que nao conhece mostra a chave crua -- honesto, mas e
    /// o alarme chegando ao operador sem frase. Tirar uma entrada da tabela
    /// `MOTIVOS` (ou acrescentar uma variante sem a entrada) derruba este
    /// teste nomeando a chave.
    #[test]
    fn todo_alarme_tem_motivo_na_tabela_da_tela() {
        let js = include_str!("../../ui/aquario.js");
        let motivos = chaves_de_motivos(js);
        assert!(
            motivos.len() >= Alarme::TODOS.len(),
            "a leitura da tabela achou so {} chaves: {motivos:?}",
            motivos.len()
        );
        let faltam: Vec<&str> = Alarme::TODOS
            .iter()
            .map(|a| a.chave())
            .filter(|c| !motivos.contains(c))
            .collect();
        assert!(
            faltam.is_empty(),
            "alarme sem motivo na tabela MOTIVOS do ui/aquario.js: {faltam:?}"
        );
    }

    /// O leitor de cima nao pode passar por engano: sem uma linha, a chave
    /// dela some da lista.
    #[test]
    fn a_leitura_de_motivos_ve_a_linha_tirada() {
        let js = include_str!("../../ui/aquario.js");
        let tirada = "    \"aquario.motivo.disco_lento\":";
        assert!(js.contains(tirada), "a linha do exemplo mudou de forma");
        let sem: String = js
            .lines()
            .filter(|l| !l.starts_with(tirada))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!chaves_de_motivos(&sem).contains(&"aquario.motivo.disco_lento"));
        assert!(chaves_de_motivos(js).contains(&"aquario.motivo.disco_lento"));
    }

    #[test]
    fn as_consultas_que_faltam_dizem_que_faltam() {
        let aq = Aquario::default();
        let e = aq.consultar_log(&Json::Nulo).unwrap_err();
        assert_eq!(e.nome(), "NAO_ENCONTRADO");
        assert!(e.to_string().contains("ainda nao"), "{e}");
    }
}

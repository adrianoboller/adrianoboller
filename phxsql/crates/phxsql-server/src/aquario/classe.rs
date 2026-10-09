//! A5 do pedido 707: a [`classificar`] UNICA -- a cor e o motivo de cada
//! tarefa, para o retrato (a tela, a TV, o painel da telemetria) e para o
//! `aquario.log`.
//!
//! Desenho em `docs/propostas/aquario-707.md` §2.3 e §2.4, com a §11 por
//! cima. A funcao recebe FATOS e devolve a [`Classe`]; quem monta os fatos e
//! quem os tem na mao -- a `Atividade` viva no retrato, o `Acesso` que acabou
//! no `anotar` do servidor --, e nenhum dos dois decide cor nenhuma.
//!
//! # Por que uma funcao so
//!
//! A cor ja saia do servidor (o `nivel` do painel), e o log ia ganhar a
//! dele: duas regras sao duas cores para a mesma tarefa, e a TV que volta
//! cinco minutos pelo log mostraria uma bolha de outra cor da que estava na
//! tela. O `nivel` de hoje passa a ser a PROJECAO da cor ([`Cor::nivel`]), e
//! nao uma terceira regra.
//!
//! # O que e estado e o que e alarme (§2.4)
//!
//! Esperar a trava, segurar a trava com fila, ser grande, passar do tempo
//! fixo: tudo isso se le do ESTADO e dos tempos, e mora aqui. Alarme e o que
//! alguem MARCOU na origem (o bit da A3), porque o estado nao o distingue.

use phxsql_core::json::Json;

use super::base::Habitual;
use super::{Alarme, Gravidade, Grupo};
use crate::config::Painel;
use crate::telemetria::Estado;

/// A partir daqui a tarefa e media: abaixo de um periodo do amostrador ela
/// nasce e morre entre duas olhadas (§2.1).
pub const TAREFA_MEDIA_MS: u64 = crate::telemetria::PERIODO_DA_AMOSTRA_MS;

/// A partir daqui a tarefa e grande: o `long_query_time` de fabrica do MySQL
/// e do MariaDB (§2.1, 5 × 0).
pub const TAREFA_GRANDE_MS: u64 = 10_000;

/// As seis cores do dono (§2.3), da mais urgente para a mais calma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cor {
    Rosa,
    Vermelho,
    Amarelo,
    AzulEscuro,
    AzulClaro,
    Verde,
}

impl Cor {
    pub fn nome(self) -> &'static str {
        match self {
            Cor::Rosa => "rosa",
            Cor::Vermelho => "vermelho",
            Cor::Amarelo => "amarelo",
            Cor::AzulEscuro => "azul_escuro",
            Cor::AzulClaro => "azul_claro",
            Cor::Verde => "verde",
        }
    }

    /// O `nivel` do painel da telemetria, que a tela de hoje pinta. E so a
    /// projecao da cor em quatro degraus: o painel e o aquario nunca
    /// discordam de quem esta vermelho.
    pub fn nivel(self) -> &'static str {
        match self {
            Cor::Rosa => "encerrando",
            Cor::Vermelho => "stress",
            Cor::Amarelo => "alto",
            Cor::AzulEscuro | Cor::AzulClaro | Cor::Verde => "normal",
        }
    }
}

/// O tamanho pelo tempo de SERVICO (§2.1, H3): a vitima da fila nao cresce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tamanho {
    Pequena,
    Media,
    Grande,
}

impl Tamanho {
    pub fn de_ms(servico_ms: u64) -> Tamanho {
        if servico_ms >= TAREFA_GRANDE_MS {
            Tamanho::Grande
        } else if servico_ms >= TAREFA_MEDIA_MS {
            Tamanho::Media
        } else {
            Tamanho::Pequena
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tamanho::Pequena => "pequena",
            Tamanho::Media => "media",
            Tamanho::Grande => "grande",
        }
    }
}

/// Os motivos que nao sao alarme -- os de ESTADO e de tempo. Chaves da
/// fabrica, por extenso (o conferidor de idiomas acha a chave pelo literal);
/// os de alarme sao o [`Alarme::chave`].
pub mod motivo {
    pub const ENCERRANDO: &str = "aquario.motivo.encerrando";
    pub const OCIOSA: &str = "aquario.motivo.ociosa";
    pub const ESPERANDO_A_TRAVA: &str = "aquario.motivo.esperando_a_trava";
    pub const SEGURANDO_A_TRAVA: &str = "aquario.motivo.segurando_a_trava";
    pub const ACIMA_DO_TEMPO_FIXO: &str = "aquario.motivo.acima_do_tempo_fixo";
    pub const GRANDE: &str = "aquario.motivo.grande";
    pub const NO_HABITUAL: &str = "aquario.motivo.no_habitual";
}

/// O que se sabe de uma tarefa, viva ou terminada. Nenhum campo e decisao.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fatos {
    /// `None`: a tarefa TERMINOU (o `aquario.log`). Viva, o estado dela.
    pub estado: Option<Estado>,
    /// Os bits da A3 ([`Alarme::bit`]).
    pub alarmes: u32,
    /// Tempo de servico (sem a espera na fila da trava).
    pub servico_ms: u64,
    /// Relogio de parede, espera incluida.
    pub parede_ms: u64,
    /// Esta com a trava de dados na mao.
    pub com_trava: bool,
    /// Ha alguem na fila da trava.
    pub ha_fila: bool,
    /// O servidor esta em stress (a barra do topo).
    pub servidor_em_stress: bool,
    /// O desvio da base (A4).
    pub habitual: Habitual,
}

impl Fatos {
    /// Os fatos de uma tarefa que terminou: sem estado, sem trava, sem fila.
    pub fn do_fim(alarmes: u32, servico_ms: u64, parede_ms: u64, habitual: Habitual) -> Fatos {
        Fatos {
            estado: None,
            alarmes,
            servico_ms,
            parede_ms,
            com_trava: false,
            ha_fila: false,
            servidor_em_stress: false,
            habitual,
        }
    }
}

/// O que a [`classificar`] decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classe {
    pub cor: Cor,
    pub tamanho: Tamanho,
    /// A chave da fabrica: o log e neutro de idioma, a tela traduz.
    pub motivo: &'static str,
    /// A letra da bolha, quando o motivo tem familia.
    pub grupo: Option<Grupo>,
}

impl Classe {
    /// Os campos que o retrato e o log escrevem -- um lugar so, para os dois
    /// nunca escreverem a mesma classe com nomes diferentes.
    pub fn campos(&self) -> Vec<(&'static str, Json)> {
        let mut v = vec![
            ("cor", Json::texto_de(self.cor.nome())),
            ("tamanho", Json::texto_de(self.tamanho.nome())),
            ("motivo", Json::texto_de(self.motivo)),
        ];
        if let Some(g) = self.grupo {
            v.push(("grupo", Json::texto_de(g.nome())));
        }
        v
    }
}

/// Os alarmes de uma mascara, pelos NOMES (nunca o numero do bit: o log
/// sobrevive a uma versao que renumere).
pub fn nomes_dos_alarmes(mascara: u32) -> Json {
    Json::Lista(
        Alarme::da_mascara(mascara)
            .map(|a| Json::texto_de(a.nome()))
            .collect(),
    )
}

/// **A classificacao.** Precedencia de cima para baixo, a primeira que casa
/// vence (§2.3):
///
/// 1. rosa -- `Encerrando`;
/// 2. verde -- `Ociosa`: a conexao sem pedido nao e tarefa, e o bit do
///    pedido que ja acabou nao pinta quem so esta conectado;
/// 3. vermelho -- um alarme vermelho marcado; esperando a trava ≥
///    `stress_ms`; segurando a trava ≥ `stress_ms`, ou com fila sob stress;
/// 4. amarelo -- um alarme amarelo marcado (fora o `fora_do_habitual`, que
///    e classe: §11.3); esperando a trava ≥ `alto_uso_ms`; anormal sem ser
///    grande; sem habitual formado e ≥ `alto_uso_ms` (o comportamento de
///    hoje, §2.4);
/// 5. azul escuro -- grande e anormal;
/// 6. azul claro -- grande;
/// 7. verde.
pub fn classificar(f: &Fatos, limiares: &Painel) -> Classe {
    let tamanho = Tamanho::de_ms(f.servico_ms);
    let classe = |cor, motivo, grupo| Classe {
        cor,
        tamanho,
        motivo,
        grupo,
    };
    match f.estado {
        Some(Estado::Encerrando) => return classe(Cor::Rosa, motivo::ENCERRANDO, None),
        Some(Estado::Ociosa) => return classe(Cor::Verde, motivo::OCIOSA, None),
        _ => {}
    }
    let esperando = f.estado == Some(Estado::Esperando);
    let executando = f.estado == Some(Estado::Executando);

    // Na ordem de `Alarme::TODOS`: dois vermelhos na mesma tarefa sempre
    // mostram o mesmo, no retrato e no log.
    let marcado = |g: Gravidade| {
        Alarme::da_mascara(f.alarmes).find(|a| a.gravidade() == g && *a != Alarme::ForaDoHabitual)
    };
    if let Some(a) = marcado(Gravidade::Vermelho) {
        return classe(Cor::Vermelho, a.chave(), Some(a.grupo()));
    }
    if esperando && f.parede_ms >= limiares.stress_ms {
        return classe(Cor::Vermelho, motivo::ESPERANDO_A_TRAVA, Some(Grupo::Lock));
    }
    // «Segurando todo mundo» exige a TRAVA na mao: quem nunca a pediu nao
    // segura ninguem (a bolha da propria tela que olha o painel).
    if executando
        && f.com_trava
        && (f.servico_ms >= limiares.stress_ms || (f.servidor_em_stress && f.ha_fila))
    {
        return classe(Cor::Vermelho, motivo::SEGURANDO_A_TRAVA, Some(Grupo::Lock));
    }
    if let Some(a) = marcado(Gravidade::Amarelo) {
        return classe(Cor::Amarelo, a.chave(), Some(a.grupo()));
    }
    if esperando && f.parede_ms >= limiares.alto_uso_ms {
        return classe(Cor::Amarelo, motivo::ESPERANDO_A_TRAVA, Some(Grupo::Lock));
    }
    let fora = Alarme::ForaDoHabitual;
    let anormal = f.habitual.desvio().is_some() || f.alarmes & fora.bit() != 0;
    let grande = tamanho == Tamanho::Grande;
    if anormal && !grande {
        return classe(Cor::Amarelo, fora.chave(), Some(fora.grupo()));
    }
    if !anormal && !f.habitual.formado() && f.parede_ms >= limiares.alto_uso_ms {
        return classe(Cor::Amarelo, motivo::ACIMA_DO_TEMPO_FIXO, None);
    }
    match (grande, anormal) {
        (true, true) => classe(Cor::AzulEscuro, fora.chave(), None),
        (true, false) => classe(Cor::AzulClaro, motivo::GRANDE, None),
        _ => classe(Cor::Verde, motivo::NO_HABITUAL, None),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::aquario::base::Desvio;

    const ALTO: u64 = 2_000;
    const STRESS: u64 = 5_000;

    fn limiares() -> Painel {
        let p = Painel::default();
        assert_eq!((p.alto_uso_ms, p.stress_ms), (ALTO, STRESS));
        p
    }

    fn fora() -> Habitual {
        Habitual::Fora(Desvio {
            alarme: Alarme::ForaDoHabitual,
            z: 6.0,
            n: 40,
            p95_habitual_us: 5_000,
            servico_us: 900_000,
        })
    }

    const DENTRO: Habitual = Habitual::Dentro { n: 40 };
    const POUCAS: Habitual = Habitual::Poucas { n: 3 };

    use Estado::{Encerrando as Enc, Esperando as Esp, Executando as Exe, Ociosa as Oci};

    /// Uma linha da tabela: `(bits, estado, trava/fila/stress, servico,
    /// parede, desvio) -> (cor, motivo)`.
    struct Caso {
        nome: &'static str,
        bits: u32,
        estado: Option<Estado>,
        trava_fila_stress: (bool, bool, bool),
        servico_ms: u64,
        parede_ms: u64,
        habitual: Habitual,
        cor: Cor,
        motivo: &'static str,
    }

    fn caso(
        nome: &'static str,
        bits: u32,
        estado: Option<Estado>,
        tfs: (bool, bool, bool),
        tempos: (u64, u64),
        habitual: Habitual,
        esperado: (Cor, &'static str),
    ) -> Caso {
        Caso {
            nome,
            bits,
            estado,
            trava_fila_stress: tfs,
            servico_ms: tempos.0,
            parede_ms: tempos.1,
            habitual,
            cor: esperado.0,
            motivo: esperado.1,
        }
    }

    /// **A tabela de casos da A5**: bits × estado × desvio -> cor + motivo.
    /// Cada linha trava uma regra da precedencia; a ordem das linhas segue a
    /// da §2.3.
    #[test]
    fn a_tabela_de_casos() {
        let reent = Alarme::TravaReentrante.bit();
        let corromp = Alarme::DadoCorrompido.bit();
        let integ = Alarme::IntegridadeRecusada.bit();
        let prazo = Alarme::PrazoEstourado.bit();
        let fdh = Alarme::ForaDoHabitual.bit();
        let nada = (false, false, false);
        let segura = (true, true, true);
        let casos = [
            // 1. rosa vence tudo, ate o alarme vermelho.
            caso(
                "encerrando",
                reent,
                Some(Enc),
                segura,
                (9_000, 9_000),
                fora(),
                (Cor::Rosa, motivo::ENCERRANDO),
            ),
            // 2. ociosa: o bit do pedido que acabou nao pinta a conexao.
            caso(
                "ociosa com bit velho",
                corromp,
                Some(Oci),
                nada,
                (0, 0),
                POUCAS,
                (Cor::Verde, motivo::OCIOSA),
            ),
            // 3. vermelhos.
            caso(
                "reentrante",
                reent,
                Some(Exe),
                nada,
                (5, 5),
                POUCAS,
                (Cor::Vermelho, Alarme::TravaReentrante.chave()),
            ),
            caso(
                "corrompido",
                corromp,
                Some(Exe),
                nada,
                (5, 5),
                POUCAS,
                (Cor::Vermelho, Alarme::DadoCorrompido.chave()),
            ),
            caso(
                "dois vermelhos: o primeiro da lista",
                corromp | reent,
                Some(Exe),
                nada,
                (5, 5),
                POUCAS,
                (Cor::Vermelho, Alarme::TravaReentrante.chave()),
            ),
            caso(
                "prazo, terminada",
                prazo,
                None,
                nada,
                (1_500, 1_500),
                DENTRO,
                (Cor::Vermelho, Alarme::PrazoEstourado.chave()),
            ),
            caso(
                "vermelho vence o amarelo marcado",
                integ | corromp,
                Some(Exe),
                nada,
                (5, 5),
                POUCAS,
                (Cor::Vermelho, Alarme::DadoCorrompido.chave()),
            ),
            caso(
                "esperando >= stress",
                0,
                Some(Esp),
                (false, true, false),
                (0, STRESS),
                POUCAS,
                (Cor::Vermelho, motivo::ESPERANDO_A_TRAVA),
            ),
            caso(
                "segurando com fila sob stress",
                0,
                Some(Exe),
                segura,
                (1, 1),
                DENTRO,
                (Cor::Vermelho, motivo::SEGURANDO_A_TRAVA),
            ),
            caso(
                "segurando sozinha >= stress",
                0,
                Some(Exe),
                (true, false, false),
                (STRESS, STRESS),
                DENTRO,
                (Cor::Vermelho, motivo::SEGURANDO_A_TRAVA),
            ),
            // 4. amarelos.
            caso(
                "com fila SEM stress nao e vermelho",
                0,
                Some(Exe),
                (true, true, false),
                (1, 1),
                DENTRO,
                (Cor::Verde, motivo::NO_HABITUAL),
            ),
            caso(
                "sem trava, longa: nao segura ninguem",
                0,
                Some(Exe),
                (false, true, true),
                (STRESS, STRESS),
                POUCAS,
                (Cor::Amarelo, motivo::ACIMA_DO_TEMPO_FIXO),
            ),
            caso(
                "integridade e amarelo",
                integ,
                Some(Exe),
                nada,
                (5, 5),
                POUCAS,
                (Cor::Amarelo, Alarme::IntegridadeRecusada.chave()),
            ),
            caso(
                "esperando >= alto",
                0,
                Some(Esp),
                nada,
                (0, ALTO),
                POUCAS,
                (Cor::Amarelo, motivo::ESPERANDO_A_TRAVA),
            ),
            caso(
                "esperando pouco e verde",
                0,
                Some(Esp),
                nada,
                (0, ALTO - 1),
                POUCAS,
                (Cor::Verde, motivo::NO_HABITUAL),
            ),
            caso(
                "anormal pequena",
                0,
                Some(Exe),
                nada,
                (900, 900),
                fora(),
                (Cor::Amarelo, Alarme::ForaDoHabitual.chave()),
            ),
            caso(
                "anormal media, pelo bit, terminada",
                fdh,
                None,
                nada,
                (3_000, 3_000),
                DENTRO,
                (Cor::Amarelo, Alarme::ForaDoHabitual.chave()),
            ),
            caso(
                "sem habitual e >= alto",
                0,
                Some(Exe),
                nada,
                (ALTO, ALTO),
                POUCAS,
                (Cor::Amarelo, motivo::ACIMA_DO_TEMPO_FIXO),
            ),
            caso(
                "sem base (replicacao) e >= alto",
                0,
                None,
                nada,
                (ALTO, ALTO),
                Habitual::SemBase,
                (Cor::Amarelo, motivo::ACIMA_DO_TEMPO_FIXO),
            ),
            caso(
                "habitual formado: o tempo fixo nao manda",
                0,
                Some(Exe),
                nada,
                (ALTO, ALTO),
                DENTRO,
                (Cor::Verde, motivo::NO_HABITUAL),
            ),
            // 5 e 6. azuis.
            caso(
                "grande e anormal",
                0,
                Some(Exe),
                nada,
                (12_000, 12_000),
                fora(),
                (Cor::AzulEscuro, Alarme::ForaDoHabitual.chave()),
            ),
            caso(
                "grande e anormal pelo bit, terminada",
                fdh,
                None,
                nada,
                (12_000, 12_000),
                DENTRO,
                (Cor::AzulEscuro, Alarme::ForaDoHabitual.chave()),
            ),
            caso(
                "grande e normal",
                0,
                Some(Exe),
                nada,
                (12_000, 12_000),
                DENTRO,
                (Cor::AzulClaro, motivo::GRANDE),
            ),
            caso(
                "grande sem habitual: o aviso vem antes do azul",
                0,
                None,
                nada,
                (12_000, 12_000),
                POUCAS,
                (Cor::Amarelo, motivo::ACIMA_DO_TEMPO_FIXO),
            ),
            // 7. verde.
            caso(
                "pequena no habitual",
                0,
                Some(Exe),
                nada,
                (3, 3),
                DENTRO,
                (Cor::Verde, motivo::NO_HABITUAL),
            ),
            caso(
                "vitima da fila: espera nao e servico",
                0,
                None,
                nada,
                (3, 1_900),
                DENTRO,
                (Cor::Verde, motivo::NO_HABITUAL),
            ),
        ];
        let l = limiares();
        for c in &casos {
            let (com_trava, ha_fila, servidor_em_stress) = c.trava_fila_stress;
            let f = Fatos {
                estado: c.estado,
                alarmes: c.bits,
                servico_ms: c.servico_ms,
                parede_ms: c.parede_ms,
                com_trava,
                ha_fila,
                servidor_em_stress,
                habitual: c.habitual,
            };
            let k = classificar(&f, &l);
            assert_eq!((k.cor, k.motivo), (c.cor, c.motivo), "caso «{}»", c.nome);
        }
    }

    #[test]
    fn o_tamanho_e_pelo_servico() {
        assert_eq!(Tamanho::de_ms(TAREFA_MEDIA_MS - 1), Tamanho::Pequena);
        assert_eq!(Tamanho::de_ms(TAREFA_MEDIA_MS), Tamanho::Media);
        assert_eq!(Tamanho::de_ms(TAREFA_GRANDE_MS - 1), Tamanho::Media);
        assert_eq!(Tamanho::de_ms(TAREFA_GRANDE_MS), Tamanho::Grande);
    }

    /// O `nivel` do painel e a projecao da cor, e o vermelho do painel e o
    /// vermelho do aquario sao o mesmo.
    #[test]
    fn o_nivel_e_a_projecao_da_cor() {
        assert_eq!(Cor::Vermelho.nivel(), "stress");
        assert_eq!(Cor::Amarelo.nivel(), "alto");
        assert_eq!(Cor::Rosa.nivel(), "encerrando");
        for c in [Cor::AzulEscuro, Cor::AzulClaro, Cor::Verde] {
            assert_eq!(c.nivel(), "normal");
        }
    }
}

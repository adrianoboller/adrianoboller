//! Pedido 496, fatia F5: quando um recurso que so diminui chega ao piso.
//!
//! Desenho em `docs/propostas/ia-495-496-desenho.md` (F5, C4/C12/C14) e a
//! conta medida pelo DBA em `parecer-dba-496-catastrofes-2026-09-24.md` §6.
//!
//! # Uma funcao so para os tres recursos
//!
//! Disco livre, `MemAvailable` e descritores livres (limite - abertos) sao a
//! MESMA pergunta -- «quanto falta, a esta taxa, para cruzar o piso?» -- e
//! por isso passam pela mesma [`esgota_em`]. Tres funcoes decidiriam tres
//! vezes o que e tendencia, e o dia em que uma ganhasse o portao de R² e as
//! outras nao seria o dia em que a memoria alarmasse por ruido.
//!
//! # Duas janelas, vale a MENOR
//!
//! Medido pelo DBA (tmpfs enchendo em rajada de 2 s ligado / 2 s parado): a
//! janela curta sozinha saiu OTIMISTA em 5 de 5 corridas, ate +1.281% -- na
//! pausa ela ve o disco parado e promete dias. A longa, cobrindo dois ciclos,
//! nunca saiu otimista. Por isso a previsao e o menor prazo entre as duas: a
//! curta reage ao arranque de uma rajada, a longa nao deixa a pausa mentir.
//! Divergencia do `predict_linear` do Prometheus (uma janela), causada por
//! essa medida.
//!
//! # O que NAO e previsao
//!
//! A rajada que enche 64 MiB em 4 s acaba antes de qualquer amostra de 15
//! min (hipotese E1, morta): previsao compra antecedencia para o LENTO; o
//! rapido e conserto. E nada aqui recusa escrita -- observa e avisa.

use std::collections::{HashMap, VecDeque};

/// Menos que isto nao e tendencia, e um ponto.
pub const AMOSTRAS_MINIMAS: usize = 4;
/// A janela curta, em amostras: 1 h a 15 min por amostra.
pub const JANELA_CURTA: usize = 4;
/// A janela longa e o tamanho do anel: 24 h a 15 min por amostra. Em
/// amostras, e nao em horas, porque o que a medida do DBA decidiu foi
/// «cobrir dois ciclos», e quem encurta o `checar_minutos` encurta os dois.
pub const JANELA_LONGA: usize = 96;
/// Abaixo disto a reta nao explica a serie: e ruido, e ruido nao preve.
pub const R2_MINIMO: f64 = 0.6;
/// O degrau do aviso (amarelo).
pub const HORAS_DO_AVISO: f64 = 24.0;
/// O degrau critico (vermelho).
pub const HORAS_DO_CRITICO: f64 = 2.0;

const MS_POR_HORA: f64 = 3_600_000.0;

/// Quando a serie cruza o piso.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Previsao {
    /// Horas, contadas da ULTIMA amostra, ate a reta cruzar o piso. Zero
    /// quando ja cruzou.
    pub horas: f64,
    /// Quanto o recurso perde por hora, na unidade da serie (positivo).
    pub por_hora: f64,
    /// O R² da janela que venceu.
    pub r2: f64,
    /// Quantas amostras a janela que venceu tinha.
    pub amostras: usize,
}

/// O que resta de um recurso que so diminui cruza `piso` quando?
///
/// `amostras` sao `(instante em ms, quanto resta)`, em ordem de tempo; so as
/// ultimas [`JANELA_LONGA`] contam. `None` = sem tendencia: menos de
/// [`AMOSTRAS_MINIMAS`], serie constante ou crescente, ou R² abaixo de
/// [`R2_MINIMO`] nas duas janelas.
pub fn esgota_em(amostras: &[(i64, u64)], piso: u64) -> Option<Previsao> {
    let n = amostras.len();
    let longa = &amostras[n.saturating_sub(JANELA_LONGA)..];
    let curta = &amostras[n.saturating_sub(JANELA_CURTA)..];
    let fim = amostras.last()?.0;
    // A menor das duas: o lado seguro e o pessimista (cabecalho do modulo).
    [regressao(longa, piso, fim), regressao(curta, piso, fim)]
        .into_iter()
        .flatten()
        .min_by(|a, b| a.horas.total_cmp(&b.horas))
}

/// Minimos quadrados numa janela, com a origem do tempo em `fim`.
///
/// A origem na ultima amostra, e nao na epoca, porque `ms` desde 1970 ao
/// quadrado passa de 10^24 e o `f64` perde a inclinacao no arredondamento.
fn regressao(am: &[(i64, u64)], piso: u64, fim: i64) -> Option<Previsao> {
    let n = am.len();
    if n < AMOSTRAS_MINIMAS {
        return None;
    }
    let x = |t: i64| (t - fim) as f64 / MS_POR_HORA;
    let nf = n as f64;
    let mx = am.iter().map(|&(t, _)| x(t)).sum::<f64>() / nf;
    let my = am.iter().map(|&(_, v)| v as f64).sum::<f64>() / nf;
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for &(t, v) in am {
        let dx = x(t) - mx;
        let dy = v as f64 - my;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    // Inclinacao >= 0: constante ou crescente nunca esgota. `sxx` zero e a
    // janela com todos os instantes iguais (relogio parado): sem divisao.
    if sxx <= 0.0 || syy <= 0.0 || sxy >= 0.0 {
        return None;
    }
    let r2 = sxy * sxy / (sxx * syy);
    if r2 < R2_MINIMO {
        return None;
    }
    let b = sxy / sxx;
    // O valor da reta no instante da ultima amostra (x = 0).
    let agora = my - b * mx;
    let horas = ((piso as f64 - agora) / b).max(0.0);
    Some(Previsao {
        horas,
        por_hora: -b,
        r2,
        amostras: n,
    })
}

/// Os dois degraus, no molde do aviso a 40 M e da recusa a 3 M do xid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Degrau {
    Aviso,
    Critico,
}

impl Degrau {
    /// O degrau de uma previsao; `None` quando ela passa de 24 h.
    pub fn de(p: &Previsao) -> Option<Degrau> {
        if p.horas < HORAS_DO_CRITICO {
            Some(Degrau::Critico)
        } else if p.horas < HORAS_DO_AVISO {
            Some(Degrau::Aviso)
        } else {
            None
        }
    }

    pub fn alarme(self) -> crate::aquario::Alarme {
        match self {
            Degrau::Aviso => crate::aquario::Alarme::EsgotamentoPrevisto,
            Degrau::Critico => crate::aquario::Alarme::EsgotamentoIminente,
        }
    }
}

// ------------------------------------------------------------------ F6
//
// A contagem regressiva ate um TETO DURO: o que so cresce e tem um fim
// declarado (slots de uma tabela paginada, volumes do diario, a idade do
// backup, a duracao dele). Mesma pergunta da F5 -- «quanto falta?» --, mesmo
// degrau, mesmo `Alarme`: o recurso vai no `dados`. Variante nova partiria o
// sedimento em dois lugares para ler a mesma noticia («isto acaba»).

/// Acima disto do teto, aviso. Estritamente acima: 80% exatos ainda e dia
/// comum, e o teste de fronteira trava o `>` contra o `>=`.
pub const PERCENTUAL_DO_AVISO: u128 = 80;
/// Acima disto do teto, critico.
pub const PERCENTUAL_DO_CRITICO: u128 = 95;
/// A tendencia cruza o teto em menos de 30 dias: aviso.
pub const DIAS_DO_AVISO: f64 = 30.0;
/// ...em menos de 3 dias: critico.
pub const DIAS_DO_CRITICO: f64 = 3.0;

/// O degrau pela RAZAO usado/teto -- a conta que nao precisa de historico.
///
/// Em inteiros (`u128`) e nao em `f64`: `slots` e `capacidade` chegam a 10^12,
/// e a fronteira de 80% tem de ser exata, nao «quase».
pub fn degrau_da_razao(usado: u64, teto: u64) -> Option<Degrau> {
    if teto == 0 {
        return None;
    }
    let u = usado as u128 * 100;
    let t = teto as u128;
    if u > PERCENTUAL_DO_CRITICO * t {
        Some(Degrau::Critico)
    } else if u > PERCENTUAL_DO_AVISO * t {
        Some(Degrau::Aviso)
    } else {
        None
    }
}

/// O degrau pela TENDENCIA: quantos dias faltam, na taxa de agora.
pub fn degrau_dos_dias(p: &Previsao) -> Option<Degrau> {
    if p.horas < DIAS_DO_CRITICO * 24.0 {
        Some(Degrau::Critico)
    } else if p.horas < DIAS_DO_AVISO * 24.0 {
        Some(Degrau::Aviso)
    } else {
        None
    }
}

/// O mais grave dos dois: a razao e a tendencia respondem coisas diferentes
/// (quanto ja encheu, quando enche) e vale a pior noticia.
fn pior(a: Option<Degrau>, b: Option<Degrau>) -> Option<Degrau> {
    match (a, b) {
        (Some(Degrau::Critico), _) | (_, Some(Degrau::Critico)) => Some(Degrau::Critico),
        (Some(Degrau::Aviso), _) | (_, Some(Degrau::Aviso)) => Some(Degrau::Aviso),
        _ => None,
    }
}

/// Uma leitura contra um teto duro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contagem {
    /// `tabela:<db>.<t>`, `diario:<db>.<t>`, `backup:idade` ou `backup:janela`.
    pub recurso: String,
    pub usado: u64,
    pub teto: u64,
    /// Guarda a serie e preve por tendencia? Falso para o que ja nasce como
    /// razao (idade e duracao do backup): a serie de uma idade que zera a cada
    /// backup e um dente de serra, e o R² a descartaria de qualquer jeito.
    pub tendencia: bool,
}

/// Uma contagem dentro dos degraus.
#[derive(Debug, Clone, PartialEq)]
pub struct AvisoDaContagem {
    pub recurso: String,
    pub degrau: Degrau,
    pub usado: u64,
    pub teto: u64,
    /// So quando a tendencia existe e e a que pesou.
    pub previsao: Option<Previsao>,
    pub mudou: bool,
}

impl AvisoDaContagem {
    pub fn percentual(&self) -> f64 {
        if self.teto == 0 {
            0.0
        } else {
            self.usado as f64 * 100.0 / self.teto as f64
        }
    }
}

/// Quantos periodos sem backup bom ate o teto duro: `1,5 x periodo` (C6).
/// Em fracao inteira (3/2) pela mesma razao da fronteira de 80%.
const TOLERANCIA_NUM: u64 = 3;
const TOLERANCIA_DEN: u64 = 2;

/// C6: a IDADE do ultimo backup bom contra `1,5 x periodo`.
///
/// `ultimo_ok_ms` zero = nunca houve um; ai a conta parte de `desde_ms` (a
/// primeira vez que o vigia olhou): um servidor que acabou de subir nao deve
/// um backup ao mundo, mas tambem nao ganha prazo infinito -- e o backup que
/// NUNCA rodou e o caso que a falha-avisa-desde-o-510 nao alcanca.
///
/// O teto e `1,5 x periodo`, e o aviso comeca a 80% dele (1,2 periodo): o
/// mesmo degrau de tudo na F6. Divergencia da letra do desenho (alarme so
/// acima de 1,5), causada por uma restricao nossa: o degrau e UM so para os
/// quatro sinais, e dois degraus diferentes seriam a decisao escrita duas
/// vezes.
pub fn contagem_da_idade(
    recurso: String,
    agora_ms: i64,
    ultimo_ok_ms: i64,
    desde_ms: i64,
    periodo_ms: u64,
) -> Contagem {
    let referencia = if ultimo_ok_ms > 0 {
        ultimo_ok_ms
    } else {
        desde_ms
    };
    Contagem {
        recurso,
        usado: (agora_ms - referencia).max(0) as u64,
        teto: periodo_ms.saturating_mul(TOLERANCIA_NUM) / TOLERANCIA_DEN,
        tendencia: false,
    }
}

/// C5: quanto o backup LEVOU contra o periodo dele. Um backup que dura mais
/// que o periodo atropela o seguinte -- e o teto duro que a configuracao ja
/// implica, sem inventar um campo de «janela aceita» que ninguem preencheu.
pub fn contagem_da_janela(recurso: String, duracao_ms: u64, periodo_ms: u64) -> Contagem {
    Contagem {
        recurso,
        usado: duracao_ms,
        teto: periodo_ms,
        tendencia: false,
    }
}

/// O que o backup agendado deixou, para o vigia ler de outra thread.
#[derive(Debug, Default)]
pub struct MarcasDoBackup {
    ultimo_ok_ms: std::sync::atomic::AtomicI64,
    ultima_duracao_ms: std::sync::atomic::AtomicI64,
    /// A primeira vez que o vigia olhou. Zero = nunca olhou.
    desde_ms: std::sync::atomic::AtomicI64,
}

impl MarcasDoBackup {
    /// O backup agendado terminou BEM. So o sucesso entra: a falha ja avisa
    /// (pedido 510) e nao renova a validade de ninguem.
    pub fn deu_certo(&self, fim_ms: i64, duracao_ms: i64) {
        use std::sync::atomic::Ordering::Relaxed;
        self.ultimo_ok_ms.store(fim_ms, Relaxed);
        self.ultima_duracao_ms.store(duracao_ms.max(0), Relaxed);
    }

    /// Fixa e devolve o instante da primeira olhada.
    pub fn primeira_olhada(&self, agora_ms: i64) -> i64 {
        use std::sync::atomic::Ordering::Relaxed;
        match self
            .desde_ms
            .compare_exchange(0, agora_ms, Relaxed, Relaxed)
        {
            Ok(_) => agora_ms,
            Err(ja) => ja,
        }
    }

    /// O ultimo sucesso. Se a memoria nao tem (processo novo), `do_disco` diz
    /// quando o destino foi tocado pela ultima vez -- o disco sobrevive ao
    /// reinicio, a memoria nao. Chamado a cada rodada; so consulta o disco
    /// enquanto a memoria esta vazia.
    pub fn ultimo_ok(&self, do_disco: impl FnOnce() -> i64) -> i64 {
        use std::sync::atomic::Ordering::Relaxed;
        match self.ultimo_ok_ms.load(Relaxed) {
            0 => do_disco(),
            ms => ms,
        }
    }

    pub fn ultima_duracao_ms(&self) -> u64 {
        self.ultima_duracao_ms
            .load(std::sync::atomic::Ordering::Relaxed)
            .max(0) as u64
    }
}

/// Uma leitura de um recurso: quanto resta e onde fica o piso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leitura {
    /// `disco:<montagem>`, `memoria` ou `descritores`.
    pub recurso: String,
    pub resta: u64,
    pub piso: u64,
}

/// Uma previsao dentro dos degraus.
#[derive(Debug, Clone, PartialEq)]
pub struct Aviso {
    pub recurso: String,
    pub degrau: Degrau,
    pub previsao: Previsao,
    /// O degrau deste recurso mudou desde a rodada anterior -- e so entao que
    /// o vigia escreve no `stderr`, para 96 rodadas por dia nao virarem 96
    /// linhas iguais.
    pub mudou: bool,
}

/// As series do vigia, uma por recurso, cada uma num anel de
/// [`JANELA_LONGA`] amostras (~1,5 KiB por recurso).
#[derive(Debug, Default)]
pub struct Previsor {
    series: HashMap<String, VecDeque<(i64, u64)>>,
    degraus: HashMap<String, Degrau>,
}

impl Previsor {
    /// Uma rodada: guarda as leituras e devolve as previsoes nos degraus.
    ///
    /// Recurso que sumiu das leituras (o `df` falhou num caminho) mantem a
    /// serie: amostra que falta nao e amostra que zerou.
    pub fn rodada(&mut self, agora_ms: i64, leituras: &[Leitura]) -> Vec<Aviso> {
        let mut avisos = Vec::new();
        for l in leituras {
            let previsao = self.empilhar(&l.recurso, agora_ms, l.resta, l.piso);
            match previsao.and_then(|p| Degrau::de(&p).map(|d| (p, d))) {
                Some((previsao, degrau)) => {
                    let mudou = self.anotar_degrau(&l.recurso, Some(degrau));
                    avisos.push(Aviso {
                        recurso: l.recurso.clone(),
                        degrau,
                        previsao,
                        mudou,
                    });
                }
                None => {
                    self.anotar_degrau(&l.recurso, None);
                }
            }
        }
        avisos
    }

    /// Guarda a amostra no anel do recurso e preve a partir dele. UM lugar
    /// para a F5 e a F6, senao o tamanho do anel divergiria entre as duas.
    fn empilhar(
        &mut self,
        recurso: &str,
        agora_ms: i64,
        resta: u64,
        piso: u64,
    ) -> Option<Previsao> {
        let serie = self.series.entry(recurso.to_string()).or_default();
        if serie.len() == JANELA_LONGA {
            serie.pop_front();
        }
        serie.push_back((agora_ms, resta));
        esgota_em(serie.make_contiguous(), piso)
    }

    /// Anota o degrau de um recurso e diz se ele MUDOU desde a rodada
    /// anterior (`None` = saiu dos degraus).
    fn anotar_degrau(&mut self, recurso: &str, degrau: Option<Degrau>) -> bool {
        match degrau {
            Some(d) => self.degraus.insert(recurso.to_string(), d) != Some(d),
            None => {
                self.degraus.remove(recurso);
                false
            }
        }
    }

    /// Uma rodada da F6: cada leitura contra o seu teto duro. Vale a pior
    /// entre a razao (`> 80%` / `> 95%`) e a tendencia (`< 30 d` / `< 3 d`).
    pub fn contar(&mut self, agora_ms: i64, leituras: &[Contagem]) -> Vec<AvisoDaContagem> {
        let mut avisos = Vec::new();
        for c in leituras {
            let previsao = if c.tendencia {
                self.empilhar(&c.recurso, agora_ms, c.teto.saturating_sub(c.usado), 0)
            } else {
                None
            };
            let dias = previsao.as_ref().and_then(degrau_dos_dias);
            match pior(degrau_da_razao(c.usado, c.teto), dias) {
                Some(degrau) => {
                    let mudou = self.anotar_degrau(&c.recurso, Some(degrau));
                    avisos.push(AvisoDaContagem {
                        recurso: c.recurso.clone(),
                        degrau,
                        usado: c.usado,
                        teto: c.teto,
                        previsao: previsao.filter(|_| dias.is_some()),
                        mudou,
                    });
                }
                None => {
                    self.anotar_degrau(&c.recurso, None);
                }
            }
        }
        avisos
    }

    /// Quantas amostras o recurso tem guardadas.
    pub fn amostras(&self, recurso: &str) -> usize {
        self.series.get(recurso).map_or(0, VecDeque::len)
    }
}

/// As leituras de memoria e descritores desta maquina e deste processo. O
/// disco vem do servidor, que sabe quais caminhos vigiar.
///
/// Piso da memoria: 5% do `MemTotal` (raciocinado, nao medido -- o OOM do
/// nucleo age antes do zero). Piso dos descritores: o limite inteiro, ou
/// seja, zero livres, porque ali o `open` recebe `EMFILE` e nao ha aviso
/// mais tarde que isso.
pub fn leituras_da_maquina() -> Vec<Leitura> {
    let mut v = Vec::new();
    if let Some((total, disponivel)) = crate::sistema::memoria_kb() {
        v.push(Leitura {
            recurso: "memoria".into(),
            resta: disponivel,
            piso: total / 20,
        });
    }
    if let Some((abertos, limite)) = crate::sistema::descritores() {
        v.push(Leitura {
            recurso: "descritores".into(),
            resta: limite.saturating_sub(abertos),
            piso: 0,
        });
    }
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    const MIN: i64 = 60_000;

    /// Gerador congruente para ruido reprodutivel, sem crate.
    struct Lcg(u64);
    impl Lcg {
        fn prox(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Serie linear decrescente, a 15 min por amostra, com o ruido de
    /// `ruido` vezes a queda de uma amostra (o `df` arredonda; a carga
    /// oscila). Devolve o pior erro do cruzamento previsto, sobre a vida
    /// inteira (a medida do DBA), em todas as rodadas depois da quarta.
    fn pior_erro_linear(ruido: f64, semente: u64) -> f64 {
        let inicio: f64 = 100_000_000.0; // kB
        let taxa_por_hora: f64 = 1_000_000.0;
        let queda = taxa_por_hora / 4.0;
        let piso: u64 = 10_000_000;
        let cruza_h = (inicio - piso as f64) / taxa_por_hora; // 90 h
        let mut r = Lcg(semente);
        let mut am = Vec::new();
        let mut pior: f64 = 0.0;
        for i in 0..400i64 {
            let h = i as f64 * 0.25;
            if h >= cruza_h {
                break;
            }
            let desvio = (r.prox() - 0.5) * 2.0 * ruido * queda;
            am.push((i * 15 * MIN, (inicio - taxa_por_hora * h + desvio) as u64));
            if am.len() < AMOSTRAS_MINIMAS {
                assert_eq!(esgota_em(&am, piso), None);
                continue;
            }
            let p = esgota_em(&am, piso).unwrap_or_else(|| panic!("sem previsao na amostra {i}"));
            let erro = (h + p.horas - cruza_h).abs() / cruza_h;
            pior = pior.max(erro);
        }
        pior
    }

    /// A serie linear acerta dentro de 2%, com ruido de ate 2% da queda de
    /// uma amostra, cinco sementes cada. Medido ao escrever: 0,000% sem
    /// ruido, 0,705% a 1%, 1,399% a 2%; a 5% o pior vai a 3,419%, e o lado
    /// otimista dele fica em 1,113% -- a janela curta de 4 amostras e a que
    /// sente o ruido, e a menor das duas a puxa para o lado seguro. RED:
    /// sinal da inclinacao invertido -> `None` -> cai.
    #[test]
    fn serie_linear_acerta_dentro_de_2_por_cento() {
        for ruido in [0.0, 0.01, 0.02] {
            for semente in 1..=5 {
                let pior = pior_erro_linear(ruido, semente);
                assert!(
                    pior <= 0.02,
                    "ruido {ruido}, semente {semente}: pior erro {:.3}%",
                    pior * 100.0
                );
            }
        }
    }

    #[test]
    fn constante_ou_crescente_nunca_esgota() {
        let constante: Vec<(i64, u64)> = (0..50).map(|i| (i * 15 * MIN, 5_000)).collect();
        assert_eq!(esgota_em(&constante, 0), None);
        let crescente: Vec<(i64, u64)> = (0..50)
            .map(|i| (i * 15 * MIN, 5_000 + i as u64 * 10))
            .collect();
        assert_eq!(esgota_em(&crescente, 0), None);
        // Instantes repetidos (relogio parado): nao divide por zero.
        let parado: Vec<(i64, u64)> = (0..10).map(|i| (1_000, 5_000 - i)).collect();
        assert_eq!(esgota_em(&parado, 0), None);
    }

    /// Ruido sobre uma descida leve: a reta nao explica a serie (R² < 0,6), e
    /// ruido nao preve. RED: sem o portao de R² este ruido «esgota».
    #[test]
    fn r2_abaixo_de_0_6_nao_preve() {
        let mut r = Lcg(42);
        let am: Vec<(i64, u64)> = (0..96)
            .map(|i| {
                let tendencia = 1_000_000.0 - 50.0 * i as f64;
                let ruido = (r.prox() - 0.5) * 20_000.0;
                (i * 15 * MIN, (tendencia + ruido) as u64)
            })
            .collect();
        // A serie tem de fato inclinacao negativa e R² baixo nas duas
        // janelas -- senao o teste nao estaria provando o portao.
        let curta = &am[am.len() - JANELA_CURTA..];
        for janela in [&am[..], curta] {
            let n = janela.len() as f64;
            let mx = janela.iter().map(|a| a.0 as f64).sum::<f64>() / n;
            let my = janela.iter().map(|a| a.1 as f64).sum::<f64>() / n;
            let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
            for &(t, v) in janela {
                sxx += (t as f64 - mx).powi(2);
                sxy += (t as f64 - mx) * (v as f64 - my);
                syy += (v as f64 - my).powi(2);
            }
            let r2 = sxy * sxy / (sxx * syy);
            if sxy < 0.0 {
                assert!(r2 < R2_MINIMO, "r2 {r2}");
            }
        }
        assert_eq!(esgota_em(&am, 0), None);
    }

    /// Rajada: 4 amostras escrevendo, 4 paradas (2 s / 2 s a 0,5 s por
    /// amostra, a bancada do DBA), a serie inteira ate o piso. Em toda
    /// rodada, o cruzamento previsto nao passa do real mais MEIO CICLO.
    ///
    /// Por que meio ciclo, e nao zero: numa escada o cruzamento real cai no
    /// fim de um trecho ligado, e regressao nenhuma sabe em que fase a
    /// proxima rajada comeca. Medido aqui: a melhor folga possivel fica em
    /// +1.722 ms num ciclo de 4.000 ms. O que a janela longa compra e o
    /// TETO dessa folga; sem ela a pausa promete o infinito. RED: so a
    /// janela curta -> na pausa ela ve o disco quase parado e passa longe.
    #[test]
    fn rajada_nunca_da_previsao_otimista() {
        let passo = 500i64; // ms por amostra
        let por_amostra = 4_096u64; // kB perdidos por amostra ligada (8 MiB/s)
        let ciclo = 8; // 4 ligadas + 4 paradas: a janela curta cabe na pausa
        let folga_ms = (ciclo * passo / 2) as f64;
        let inicio = 65_536u64;
        let piso = 0u64;
        // A serie real, amostra a amostra, ate cruzar o piso.
        let mut serie = vec![(0i64, inicio)];
        let mut resta = inicio;
        let mut i = 0i64;
        while resta > piso {
            i += 1;
            if (i - 1) % ciclo < ciclo / 2 {
                resta = resta.saturating_sub(por_amostra);
            }
            serie.push((i * passo, resta));
        }
        let cruza_ms = serie.last().unwrap().0 as f64;
        let mut previsoes = 0;
        for k in 1..serie.len() {
            let am = &serie[..k];
            if let Some(p) = esgota_em(am, piso) {
                previsoes += 1;
                let previsto = am.last().unwrap().0 as f64 + p.horas * MS_POR_HORA;
                assert!(
                    previsto <= cruza_ms + folga_ms,
                    "otimista na amostra {k}: previu {previsto:.0} ms, cruza em {cruza_ms:.0} ms"
                );
            }
        }
        assert!(
            previsoes > serie.len() / 2,
            "{previsoes} de {}",
            serie.len()
        );
    }

    #[test]
    fn os_degraus_e_o_previsor() {
        let p = |horas| Previsao {
            horas,
            por_hora: 1.0,
            r2: 1.0,
            amostras: 4,
        };
        assert_eq!(Degrau::de(&p(1.9)), Some(Degrau::Critico));
        assert_eq!(Degrau::de(&p(2.0)), Some(Degrau::Aviso));
        assert_eq!(Degrau::de(&p(23.9)), Some(Degrau::Aviso));
        assert_eq!(Degrau::de(&p(24.0)), None);

        let mut pv = Previsor::default();
        let l = |resta| {
            vec![Leitura {
                recurso: "disco:/x".into(),
                resta,
                piso: 0,
            }]
        };
        // 1.000 por amostra de 15 min, 10.000 no inicio: cruza em 2,5 h.
        let mut ultimo = Vec::new();
        for i in 0..5 {
            ultimo = pv.rodada(i * 15 * MIN, &l(10_000 - 1_000 * i as u64));
        }
        assert_eq!(pv.amostras("disco:/x"), 5);
        assert_eq!(ultimo.len(), 1);
        assert_eq!(ultimo[0].degrau, Degrau::Critico, "{ultimo:?}");
        // O anel nao passa da janela longa.
        for i in 5..300 {
            pv.rodada(i * 15 * MIN, &l(5_000));
        }
        assert_eq!(pv.amostras("disco:/x"), JANELA_LONGA);
    }

    #[test]
    fn a_mudanca_de_degrau_e_dita_uma_vez() {
        let mut pv = Previsor::default();
        let l = |resta| {
            vec![Leitura {
                recurso: "memoria".into(),
                resta,
                piso: 0,
            }]
        };
        let mut mudancas = 0;
        for i in 0..8i64 {
            for a in pv.rodada(i * MIN, &l(1_000_000 - 1_000 * i as u64)) {
                mudancas += a.mudou as usize;
            }
        }
        assert_eq!(mudancas, 1);
    }

    #[test]
    fn a_razao_tem_fronteira_estrita_em_80_e_95() {
        // RED: `>` trocado por `>=` -> o 80% exato vira aviso e cai aqui.
        assert_eq!(degrau_da_razao(790, 1_000), None);
        assert_eq!(degrau_da_razao(800, 1_000), None);
        assert_eq!(degrau_da_razao(801, 1_000), Some(Degrau::Aviso));
        assert_eq!(degrau_da_razao(810, 1_000), Some(Degrau::Aviso));
        assert_eq!(degrau_da_razao(950, 1_000), Some(Degrau::Aviso));
        assert_eq!(degrau_da_razao(951, 1_000), Some(Degrau::Critico));
        assert_eq!(degrau_da_razao(5, 0), None, "teto zero nao e cheio");
        // Sem estouro de aritmetica no limite do tipo.
        assert_eq!(degrau_da_razao(u64::MAX, u64::MAX), Some(Degrau::Critico));
        assert_eq!(degrau_da_razao(u64::MAX / 2, u64::MAX), None);
    }

    #[test]
    fn os_dias_tem_fronteira_em_30_e_3() {
        let p = |dias: f64| Previsao {
            horas: dias * 24.0,
            por_hora: 1.0,
            r2: 1.0,
            amostras: 4,
        };
        assert_eq!(degrau_dos_dias(&p(31.0)), None);
        assert_eq!(degrau_dos_dias(&p(30.0)), None);
        assert_eq!(degrau_dos_dias(&p(29.0)), Some(Degrau::Aviso));
        assert_eq!(degrau_dos_dias(&p(3.0)), Some(Degrau::Aviso));
        assert_eq!(degrau_dos_dias(&p(2.9)), Some(Degrau::Critico));
    }

    fn contagem(usado: u64, tendencia: bool) -> Vec<Contagem> {
        vec![Contagem {
            recurso: "tabela:b.t".into(),
            usado,
            teto: 1_000,
            tendencia,
        }]
    }

    /// A tendencia sozinha avisa: a 50% cheia, mas enchendo a 1 slot por
    /// quarto de hora, o teto chega em ~5 dias. Controle: a mesma ocupacao
    /// parada nao avisa.
    #[test]
    fn a_tendencia_avisa_antes_da_razao() {
        let mut crescendo = Previsor::default();
        let mut parado = Previsor::default();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for i in 0..6i64 {
            a = crescendo.contar(i * 15 * MIN, &contagem(500 + i as u64, true));
            b = parado.contar(i * 15 * MIN, &contagem(500, true));
        }
        assert_eq!(a.len(), 1, "{a:?}");
        assert_eq!(a[0].degrau, Degrau::Aviso);
        assert!(a[0].previsao.is_some());
        assert!(b.is_empty(), "{b:?}");
    }

    #[test]
    fn a_mudanca_de_degrau_da_contagem_e_dita_uma_vez() {
        let mut pv = Previsor::default();
        let mut mudancas = 0;
        for (i, usado) in [700u64, 810, 820, 830, 700].into_iter().enumerate() {
            for a in pv.contar(i as i64 * MIN, &contagem(usado, false)) {
                mudancas += a.mudou as usize;
            }
        }
        assert_eq!(mudancas, 1);
    }

    #[test]
    fn a_idade_do_backup_tem_teto_de_um_periodo_e_meio() {
        let hora = 3_600_000u64;
        let c = |agora: i64, ok: i64| contagem_da_idade("b".into(), agora, ok, 1_000, hora);
        assert_eq!(c(0, 0).teto, hora * 3 / 2);
        // Nunca rodou: parte do `desde`; 1,6 periodo depois, critico.
        let nunca = c(1_000 + (hora as i64) * 16 / 10, 0);
        assert_eq!(
            degrau_da_razao(nunca.usado, nunca.teto),
            Some(Degrau::Critico)
        );
        // Rodou ha pouco: nada, mesmo com o `desde` muito antigo.
        let bom = c(10 * hora as i64, 10 * hora as i64 - 60_000);
        assert_eq!(degrau_da_razao(bom.usado, bom.teto), None);
        // Relogio que andou para tras nao vira idade negativa.
        assert_eq!(c(0, 5_000_000).usado, 0);
    }

    #[test]
    fn as_marcas_do_backup_semeiam_do_disco_so_com_a_memoria_vazia() {
        let m = MarcasDoBackup::default();
        assert_eq!(m.ultimo_ok(|| 42), 42, "memoria vazia: vale o disco");
        m.deu_certo(100, 7);
        assert_eq!(m.ultimo_ok(|| panic!("nao consulta o disco")), 100);
        assert_eq!(m.ultima_duracao_ms(), 7);
        assert_eq!(m.primeira_olhada(5), 5);
        assert_eq!(m.primeira_olhada(9), 5, "a primeira vale");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_maquina_tem_memoria_e_descritores() {
        let l = leituras_da_maquina();
        let mem = l.iter().find(|l| l.recurso == "memoria").expect("memoria");
        assert!(mem.resta > 0 && mem.piso > 0, "{mem:?}");
        // `descritores` pode faltar quando o limite e `unlimited`.
        if let Some(d) = l.iter().find(|l| l.recurso == "descritores") {
            assert!(d.resta > 0, "{d:?}");
        }
    }
}

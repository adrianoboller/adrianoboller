//! A linha de base do aquario (fatia A4 do pedido 707, a mesma F4 do 495).
//!
//! Desenho em `docs/propostas/aquario-707.md` §11.1 (hipotese Ha4, a que
//! sobreviveu a medicao): Welford sobre `ln(µs)` do tempo de SERVICO, em duas
//! metades de 30 min, por chave op + `database.tabela` (ou a digital, para o
//! `sql`). Alarma com n ≥ 20, z ≥ 4 e servico ≥ 250 ms.
//!
//! # Por que o `sql` entra pela DIGITAL
//!
//! Todo `sql` tem a mesma op e, no mais das vezes, a mesma base: numa chave
//! so, o `SELECT` de 300 ms legitimo alarmaria contra o habitual dos `SELECT`
//! de 1 ms -- alarme falso na TV e pior que alarme nenhum. A digital
//! (`phxsql_sql::digital`, F1 do 495) e a forma da consulta com todo literal
//! virando marcador: o mesmo `SELECT` com outro nome no `WHERE` cai na mesma
//! linha, e outro `SELECT` cai noutra.
//!
//! # Por que `ln`, e nao o tempo cru
//!
//! Tempo de banco tem cauda longa: z sobre µs crus acusa a cauda normal como
//! anormal (o caso de 1,3 ms contra um habitual de 1 ms alarmaria). Sobre o
//! logaritmo a cauda vira quase simetrica, e o z volta a querer dizer «raro».
//!
//! # Por que o desvio tem CHAO
//!
//! A serie constante -- vinte pedidos de exatamente 1 ms -- tem desvio zero, e
//! o `z` do Apendice A da IA, que devolvia 0 quando o desvio era 0, deixava
//! passar o pedido de 10 s logo depois: o caso mais anormal possivel, calado
//! justamente por ser o habitual mais estavel. O chao de 0,1 em `ln` (≈ 10%)
//! diz que variacao menor que a resolucao do relogio nao e informacao.
//!
//! # Por que SERVICO, e nao duracao
//!
//! A vitima da fila nao fez nada de errado: esperou a trava que outro
//! segurava. Medir a duracao inteira acusaria as oito conexoes paradas atras
//! da culpada, e nenhuma pista de qual e a culpada -- o mesmo defeito que o
//! peso da bolha ja pagou (`Atividade::definir_estado`).
//!
//! # Por que o teto tem CORINGA, e nao despejo
//!
//! Quem inunda chaves novas (tabela por cliente, `sql` com literal na
//! digital) apagaria pelo despejo o habitual de quem ja estava aqui. A
//! 5.001a chave vai a uma linha unica, que soma mas nunca alarma: misturar
//! chaves diferentes num habitual so faria o z mentir.

use std::cell::Cell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Mutex;

use super::Alarme;
use crate::acesso::Acesso;
use crate::servidor::OPS_DE_REPLICACAO;

/// Abaixo disto o habitual ainda nao existe (§11.1: 20 e 30 aquecem igual).
pub const N_MINIMO: u64 = 20;
/// O corte do «fora do habitual», em desvios sobre `ln(µs)`.
pub const Z_MINIMO: f64 = 4.0;
/// O piso em µs de servico: sem ele, 40 de 9.317 alarmes nos logs reais eram
/// ruido de 1 ms de resolucao; com ele, 1.
pub const PISO_US: u64 = 250_000;
/// O chao do desvio em `ln`, ≈ 10%.
pub const CHAO_DO_DESVIO: f64 = 0.1;
/// Uma metade da janela. O habitual e o das duas ultimas metades: o de ontem
/// nao manda no de agora, e uma metade sozinha esqueceria tudo de uma vez.
pub const METADE_MS: i64 = 30 * 60 * 1_000;
/// Quantas chaves proprias a base guarda antes da coringa.
pub const TETO_DE_CHAVES: usize = 5_000;
/// O quantil normal de 95%, para o p95 habitual estimado que a tela mostra.
const Z_DO_P95: f64 = 1.645;

thread_local! {
    /// A espera pela trava de dados do pedido em curso nesta thread, em µs.
    ///
    /// Mora na thread porque quem mede a espera (`travar_dados`) e quem monta
    /// o `Acesso` sao o mesmo laco de pedido, e o `Acesso` e montado em
    /// muitos lugares que nao veem a trava. Quem monta o `Acesso` a TOMA
    /// (zera), e por isso a espera de um pedido nao vaza para o seguinte.
    static ESPERA_DO_PEDIDO_US: Cell<u64> = const { Cell::new(0) };

    /// A digital do `sql` do pedido em curso nesta thread.
    ///
    /// Thread local pelo mesmo motivo da espera: quem le o texto do `sql` (o
    /// `op_sql`) e quem monta o `Acesso` sao o mesmo laco de pedido, e entre
    /// os dois ha o `despachar` inteiro. Quem monta o `Acesso` a TOMA.
    static DIGITAL_DO_PEDIDO: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Guarda a digital do `sql` desta thread. Chamado pelo `op_sql`, so com a
/// telemetria ligada: e la que o portao vem antes do lexico.
pub fn anotar_digital(digital: Option<u64>) {
    DIGITAL_DO_PEDIDO.with(|c| c.set(digital));
}

/// A digital do pedido desta thread, zerando-a.
pub fn tomar_digital() -> Option<u64> {
    DIGITAL_DO_PEDIDO.with(|c| c.replace(None))
}

/// Soma espera de trava ao pedido desta thread. Chamado por
/// `Telemetria::contar_espera`, que ja vem atras do portao da telemetria.
pub fn somar_espera(micros: u64) {
    ESPERA_DO_PEDIDO_US.with(|c| c.set(c.get().saturating_add(micros)));
}

/// A espera acumulada do pedido desta thread, zerando-a.
pub fn tomar_espera() -> u64 {
    ESPERA_DO_PEDIDO_US.with(|c| c.replace(0))
}

/// Media e soma dos quadrados dos desvios, sem guardar amostra nenhuma.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Welford {
    n: u64,
    media: f64,
    m2: f64,
}

impl Welford {
    fn somar(&mut self, x: f64) {
        self.n += 1;
        let d = x - self.media;
        self.media += d / self.n as f64;
        self.m2 += d * (x - self.media);
    }

    /// As duas metades unidas pela formula de Chan: O(1), e o resultado e o
    /// mesmo de ter somado as amostras das duas numa so.
    fn unir(a: Welford, b: Welford) -> Welford {
        if a.n == 0 {
            return b;
        }
        if b.n == 0 {
            return a;
        }
        let n = a.n + b.n;
        let d = b.media - a.media;
        Welford {
            n,
            media: a.media + d * b.n as f64 / n as f64,
            m2: a.m2 + b.m2 + d * d * (a.n as f64 * b.n as f64) / n as f64,
        }
    }

    /// Desvio amostral (n − 1), sob o chao.
    fn desvio(&self) -> f64 {
        let cru = if self.n > 1 {
            (self.m2 / (self.n - 1) as f64).sqrt()
        } else {
            0.0
        };
        cru.max(CHAO_DO_DESVIO)
    }
}

/// A chave de uma linha da base, emprestada do `Acesso` -- nada se aloca
/// para procurar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chave<'a> {
    Op {
        op: &'a str,
        database: &'a str,
        tabela: &'a str,
    },
    /// A digital do `sql` (F1 do 495: FNV-1a 64 sobre os simbolos).
    Digital(u64),
}

impl Chave<'_> {
    /// FNV-1a 64, com separador entre as partes: `ab`+`c` e `a`+`bc` nao
    /// podem cair na mesma chave.
    fn hash(&self) -> u64 {
        const BASE: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIMO: u64 = 0x0000_0100_0000_01b3;
        let mut h = BASE;
        let mut passar = |b: u8| {
            h ^= b as u64;
            h = h.wrapping_mul(PRIMO);
        };
        match self {
            Chave::Op {
                op,
                database,
                tabela,
            } => {
                passar(b'o');
                for parte in [op, database, tabela] {
                    parte.bytes().for_each(&mut passar);
                    passar(0);
                }
            }
            Chave::Digital(d) => {
                passar(b'd');
                d.to_le_bytes().into_iter().for_each(&mut passar);
            }
        }
        h
    }
}

/// A identidade guardada, para conferir o hash: colisao nao funde duas chaves
/// caladas num habitual so -- a segunda vai a coringa.
#[derive(Debug)]
enum Identidade {
    Op {
        op: String,
        database: String,
        tabela: String,
    },
    Digital(u64),
}

impl Identidade {
    fn de(chave: Chave<'_>) -> Identidade {
        match chave {
            Chave::Op {
                op,
                database,
                tabela,
            } => Identidade::Op {
                op: op.to_string(),
                database: database.to_string(),
                tabela: tabela.to_string(),
            },
            Chave::Digital(d) => Identidade::Digital(d),
        }
    }

    fn e(&self, chave: Chave<'_>) -> bool {
        match (self, chave) {
            (
                Identidade::Op {
                    op,
                    database,
                    tabela,
                },
                Chave::Op {
                    op: o,
                    database: d,
                    tabela: t,
                },
            ) => op == o && database == d && tabela == t,
            (Identidade::Digital(a), Chave::Digital(b)) => *a == b,
            _ => false,
        }
    }
}

/// O hash da chave ja e FNV: passar por SipHash de novo seria pagar duas
/// vezes no laco quente. A `HashMap` so precisa espalhar, e o FNV espalha.
#[derive(Debug, Default)]
struct Passante(u64);

impl Hasher for Passante {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        // So `write_u64` e usado; isto existe porque o trait exige.
        for b in bytes {
            self.0 = (self.0 << 8) | *b as u64;
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }
}

/// Uma linha: as duas metades e o que a tela mostra ao lado.
#[derive(Debug, Default)]
struct Linha {
    /// `None` so na coringa.
    identidade: Option<Identidade>,
    anterior: Welford,
    atual: Welford,
    /// A metade (`agora_ms / METADE_MS`) a que o `atual` pertence.
    periodo: i64,
    maximo_us: u64,
    alarmes: u64,
}

impl Linha {
    /// Vira a janela ate `periodo`. Relogio que recuou nao vira nada: o
    /// pedido entra na metade corrente, e o habitual nao se perde por causa
    /// de um ajuste de hora.
    fn girar(&mut self, periodo: i64) {
        if periodo <= self.periodo {
            return;
        }
        self.anterior = if periodo == self.periodo + 1 {
            self.atual
        } else {
            Welford::default()
        };
        self.atual = Welford::default();
        self.periodo = periodo;
    }

    fn habitual(&self) -> Welford {
        Welford::unir(self.anterior, self.atual)
    }

    /// O habitual como o [`Linha::girar`] o deixaria em `periodo`, SEM girar:
    /// quem so le (o retrato da tarefa viva) nao pode mexer na janela de quem
    /// soma.
    fn habitual_em(&self, periodo: i64) -> Welford {
        if periodo <= self.periodo {
            self.habitual()
        } else if periodo == self.periodo + 1 {
            self.atual
        } else {
            Welford::default()
        }
    }
}

/// O julgamento UNICO de um tempo de servico contra um habitual -- o mesmo
/// para o pedido que termina (que depois soma) e para a tarefa viva (que so
/// le). Dois cortes aqui seriam duas respostas para «isto e anormal?», e a
/// bolha viva mudaria de cor no instante em que estoura.
fn julgar(habitual: Welford, servico_us: u64) -> Habitual {
    if habitual.n < N_MINIMO {
        return Habitual::Poucas { n: habitual.n };
    }
    if servico_us >= PISO_US {
        let x = (servico_us.max(1) as f64).ln();
        let z = (x - habitual.media) / habitual.desvio();
        if z >= Z_MINIMO {
            return Habitual::Fora(Desvio {
                alarme: Alarme::ForaDoHabitual,
                z,
                n: habitual.n,
                p95_habitual_us: p95(habitual),
                servico_us,
            });
        }
    }
    Habitual::Dentro { n: habitual.n }
}

/// A chave de um pedido, ou `None` quando ele fica FORA da base por desenho.
///
/// Um lugar so para o pedido que termina e para a tarefa viva: a replicacao
/// excluida de um e nao do outro pintaria o `replicar_aguardar` de anormal
/// so enquanto ele esta vivo.
fn chave_do_pedido<'a>(
    op: &'a str,
    database: &'a str,
    tabela: &'a str,
    digital: Option<u64>,
) -> Option<Chave<'a>> {
    // A replica espera POR DESENHO: `replicar_aguardar` de 1 s e o habitual
    // dele, e sem esta exclusao eram 14 dos 51 alarmes da A0.
    if op.is_empty() || OPS_DE_REPLICACAO.contains(&op) {
        return None;
    }
    // O `sql` entra pela digital (ver o topo do arquivo). Sem ela -- o
    // lexico recusou o texto, ou a telemetria ligou no meio do pedido --
    // fica FORA, e nunca na chave op + tabela: ali todo `sql` dividiria um
    // habitual so.
    if op == "sql" {
        return digital.map(Chave::Digital);
    }
    Some(Chave::Op {
        op,
        database,
        tabela,
    })
}

/// O que a base achou de um pedido fora do habitual: o alarme e os numeros
/// que vao junto dele ao sinal (A3) e a tela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Desvio {
    pub alarme: Alarme,
    pub z: f64,
    /// Quantos pedidos formaram o habitual.
    pub n: u64,
    /// `exp(media + 1,645 · desvio)`: o «p95 da propria operacao e tabela»
    /// que o dono leu, ESTIMADO -- o corte e o z, nao este numero.
    pub p95_habitual_us: u64,
    pub servico_us: u64,
}

/// O que a base diz de um tempo de servico -- o «desvio» que a
/// [`super::classificar`] (A5) recebe.
///
/// Mais que `Option<Desvio>` porque a classificacao precisa saber POR QUE
/// nao houve desvio: sem habitual ainda (vale o limiar fixo de hoje, §2.4) e
/// dentro do habitual (o limiar fixo ja nao manda) pintam diferente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Habitual {
    /// Fora da base por desenho: replicacao, `sql` sem digital, pedido sem
    /// medida em µs, a coringa (cujo habitual e de ninguem).
    SemBase,
    /// Ainda sem habitual: menos de [`N_MINIMO`] amostras.
    Poucas {
        n: u64,
    },
    Dentro {
        n: u64,
    },
    Fora(Desvio),
}

impl Habitual {
    pub fn desvio(&self) -> Option<Desvio> {
        match self {
            Habitual::Fora(d) => Some(*d),
            _ => None,
        }
    }

    /// Ha habitual formado (com ou sem desvio)?
    pub fn formado(&self) -> bool {
        matches!(self, Habitual::Dentro { .. } | Habitual::Fora(_))
    }
}

/// O que a tela le de uma chave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leitura {
    pub n: u64,
    pub p95_habitual_us: u64,
    pub maximo_us: u64,
    pub alarmes: u64,
}

#[derive(Debug, Default)]
struct Base {
    linhas: HashMap<u64, Linha, BuildHasherDefault<Passante>>,
    coringa: Linha,
    /// Quantas amostras entraram. E a prova de que o portao vem antes: com a
    /// telemetria desligada, zero.
    atualizacoes: u64,
}

impl Base {
    fn observar(&mut self, chave: Chave<'_>, servico_us: u64, agora_ms: i64) -> Habitual {
        self.atualizacoes += 1;
        let periodo = agora_ms.div_euclid(METADE_MS);
        let h = chave.hash();
        let cabe = self.linhas.len() < TETO_DE_CHAVES;
        let linha = match self.linhas.get(&h) {
            Some(l) if l.identidade.as_ref().is_some_and(|i| i.e(chave)) => self.linhas.get_mut(&h),
            // Colisao de hash: a segunda chave nao funde com a primeira.
            Some(_) => None,
            None if cabe => Some(self.linhas.entry(h).or_insert_with(|| Linha {
                identidade: Some(Identidade::de(chave)),
                periodo,
                ..Linha::default()
            })),
            None => None,
        };
        let (linha, pode_alarmar) = match linha {
            Some(l) => (l, true),
            None => (&mut self.coringa, false),
        };
        linha.girar(periodo);
        let x = (servico_us.max(1) as f64).ln();
        // Avalia ANTES de somar: o pedido nao entra no habitual contra o qual
        // ele mesmo e julgado. Depois entra, inclusive o anormal -- o regra.py
        // da A0 mediu assim, e um habitual que mudou de verdade tem de poder
        // virar habitual.
        let julgado = if pode_alarmar {
            julgar(linha.habitual(), servico_us)
        } else {
            Habitual::SemBase
        };
        if julgado.desvio().is_some() {
            linha.alarmes += 1;
        }
        linha.atual.somar(x);
        linha.maximo_us = linha.maximo_us.max(servico_us);
        julgado
    }

    /// A tarefa VIVA contra o habitual da chave dela, sem somar nada.
    fn avaliar(&self, chave: Chave<'_>, servico_us: u64, agora_ms: i64) -> Habitual {
        let periodo = agora_ms.div_euclid(METADE_MS);
        match self
            .linhas
            .get(&chave.hash())
            .filter(|l| l.identidade.as_ref().is_some_and(|i| i.e(chave)))
        {
            Some(l) => julgar(l.habitual_em(periodo), servico_us),
            // Chave que nunca terminou um pedido: nenhum habitual ainda. (A
            // que caiu na coringa tambem cai aqui -- e a coringa nunca alarma.)
            None => Habitual::Poucas { n: 0 },
        }
    }

    fn ler(&self, chave: Chave<'_>) -> Option<Leitura> {
        let l = self
            .linhas
            .get(&chave.hash())
            .filter(|l| l.identidade.as_ref().is_some_and(|i| i.e(chave)))?;
        let h = l.habitual();
        Some(Leitura {
            n: h.n,
            p95_habitual_us: p95(h),
            maximo_us: l.maximo_us,
            alarmes: l.alarmes,
        })
    }
}

fn p95(w: Welford) -> u64 {
    (w.media + Z_DO_P95 * w.desvio()).exp() as u64
}

/// A base unica do aquario e do 495 (§11.1, Ha3 morreu pela lei: duas bases
/// pintariam a bolha de uma cor e mandariam o e-mail por outra).
#[derive(Debug, Default)]
pub struct BaseDeConsultas {
    dentro: Mutex<Base>,
}

impl BaseDeConsultas {
    /// O fim de um pedido. Chega aqui so pelo `Aquario::anotar`, que so se
    /// alcanca com a telemetria ligada.
    ///
    /// Devolve o julgamento inteiro (A5): a classificacao do pedido que
    /// termina precisa saber se havia habitual, e nao so se houve desvio.
    pub fn anotar(&self, acesso: &Acesso) -> Habitual {
        // Sem medida em µs nao ha o que somar: recusa de porta, job (medido
        // em ms noutra thread) e linha antiga do log.
        if acesso.us == 0 {
            return Habitual::SemBase;
        }
        let Some(chave) =
            chave_do_pedido(&acesso.op, &acesso.database, &acesso.tabela, acesso.digital)
        else {
            return Habitual::SemBase;
        };
        let servico_us = acesso.us.saturating_sub(acesso.espera_us);
        self.julgar_e_somar(chave, servico_us, acesso.quando_ms)
    }

    /// A tarefa VIVA contra o habitual da propria operacao e tabela, sem
    /// somar (o retrato do aquario, A5). Mesmo julgamento e mesma exclusao do
    /// [`BaseDeConsultas::anotar`].
    pub fn avaliar_viva(
        &self,
        op: &str,
        database: &str,
        tabela: &str,
        digital: Option<u64>,
        servico_us: u64,
        agora_ms: i64,
    ) -> Habitual {
        let Some(chave) = chave_do_pedido(op, database, tabela, digital) else {
            return Habitual::SemBase;
        };
        match self.dentro.lock() {
            Ok(b) => b.avaliar(chave, servico_us, agora_ms),
            Err(veneno) => veneno.into_inner().avaliar(chave, servico_us, agora_ms),
        }
    }

    pub fn observar(&self, chave: Chave<'_>, servico_us: u64, agora_ms: i64) -> Option<Desvio> {
        self.julgar_e_somar(chave, servico_us, agora_ms).desvio()
    }

    fn julgar_e_somar(&self, chave: Chave<'_>, servico_us: u64, agora_ms: i64) -> Habitual {
        let mut base = match self.dentro.lock() {
            Ok(b) => b,
            Err(veneno) => veneno.into_inner(),
        };
        base.observar(chave, servico_us, agora_ms)
    }

    pub fn ler(&self, chave: Chave<'_>) -> Option<Leitura> {
        self.dentro.lock().ok()?.ler(chave)
    }

    pub fn atualizacoes(&self) -> u64 {
        self.dentro.lock().map(|b| b.atualizacoes).unwrap_or(0)
    }

    /// Chaves proprias (sem a coringa).
    pub fn chaves(&self) -> usize {
        self.dentro.lock().map(|b| b.linhas.len()).unwrap_or(0)
    }

    /// Amostras que foram parar na coringa.
    pub fn na_coringa(&self) -> u64 {
        self.dentro
            .lock()
            .map(|b| b.coringa.anterior.n + b.coringa.atual.n)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const T0: i64 = 1_760_000_000_000;

    fn chave(tabela: &str) -> Chave<'_> {
        Chave::Op {
            op: "inserir",
            database: "loja",
            tabela,
        }
    }

    fn acesso(op: &str, us: u64, espera_us: u64) -> Acesso {
        Acesso {
            quando_ms: T0,
            op: op.into(),
            ok: true,
            database: "loja".into(),
            tabela: "pedidos".into(),
            duracao_ms: us / 1_000,
            us,
            espera_us,
            ..Acesso::default()
        }
    }

    /// O defeito do Apendice A: serie constante, desvio zero, z devolvido 0,
    /// e o pedido de 10 s depois de vinte de 1 ms EXATOS passava calado.
    /// Tirar o `.max(CHAO_DO_DESVIO)` do `desvio` (e devolver z 0 quando o
    /// desvio e 0, como la) faz este teste cair.
    #[test]
    fn vinte_de_1ms_exatos_e_um_de_10s_alarma() {
        let b = BaseDeConsultas::default();
        for _ in 0..20 {
            assert_eq!(b.observar(chave("p"), 1_000, T0), None);
        }
        let d = b
            .observar(chave("p"), 10_000_000, T0)
            .expect("10 s contra um habitual de 1 ms exato tem de alarmar");
        assert_eq!(d.alarme, Alarme::ForaDoHabitual);
        assert_eq!(d.n, 20);
        assert!(d.z >= Z_MINIMO, "z {}", d.z);
    }

    /// Os outros casos da F4: variacao normal nao alarma; o piso segura o
    /// rapido que e raro mas nao doi; abaixo de n = 20 nao ha habitual.
    #[test]
    fn os_casos_da_f4() {
        let b = BaseDeConsultas::default();
        // 19 de ~1 ms com variacao: ainda sem habitual.
        for i in 0..19u64 {
            b.observar(chave("v"), 900 + (i % 5) * 50, T0);
        }
        assert_eq!(b.observar(chave("v"), 400_000, T0), None, "n = 19");
        let b = BaseDeConsultas::default();
        for i in 0..20u64 {
            b.observar(chave("v"), 900 + (i % 5) * 50, T0);
        }
        assert_eq!(b.observar(chave("v"), 1_300, T0), None, "1,3 ms e normal");
        assert_eq!(b.observar(chave("v"), 200_000, T0), None, "abaixo do piso");
        assert!(
            b.observar(chave("v"), 400_000, T0).is_some(),
            "400 ms contra ~1 ms alarma"
        );
        let l = b.ler(chave("v")).unwrap();
        assert_eq!(l.alarmes, 1);
        assert_eq!(l.n, 23);
    }

    /// A vitima da fila: o pedido de 1 ms que esperou 5 s pela trava de
    /// outro NAO e anormal -- o servico dele foi o de sempre. Trocar o
    /// `acesso.us.saturating_sub(acesso.espera_us)` por `acesso.us` faz este
    /// teste cair.
    #[test]
    fn a_vitima_da_fila_nao_vira_anormal() {
        let b = BaseDeConsultas::default();
        for _ in 0..30 {
            assert_eq!(b.anotar(&acesso("inserir", 1_000, 0)).desvio(), None);
        }
        assert_eq!(
            b.anotar(&acesso("inserir", 5_001_000, 5_000_000)).desvio(),
            None,
            "a vitima da fila virou anormal"
        );
        // E a culpada, que gastou os 5 s de SERVICO, alarma.
        assert!(b
            .anotar(&acesso("inserir", 5_000_000, 0))
            .desvio()
            .is_some());
    }

    /// O `sql` entra pela digital: duas formas, duas linhas. Sem a digital
    /// (a chave do `sql` voltando a op + tabela), os dois `SELECT` dividem um
    /// habitual, e o de 300 ms legitimo alarma contra o de 1 ms.
    #[test]
    fn cada_forma_de_sql_tem_o_seu_habitual() {
        let b = BaseDeConsultas::default();
        let sql = |digital: u64, us: u64| Acesso {
            digital: Some(digital),
            ..acesso("sql", us, 0)
        };
        for _ in 0..25 {
            b.anotar(&sql(0xA, 1_000));
        }
        for _ in 0..25 {
            assert_eq!(b.anotar(&sql(0xB, 300_000)).desvio(), None);
        }
        assert_eq!(b.chaves(), 2);
        assert_eq!(b.ler(Chave::Digital(0xA)).unwrap().n, 25);
        // E a forma rapida continua alarmando contra o habitual DELA.
        assert!(b.anotar(&sql(0xA, 300_000)).desvio().is_some());
    }

    #[test]
    fn replicacao_e_sql_sem_digital_ficam_fora() {
        let b = BaseDeConsultas::default();
        for op in OPS_DE_REPLICACAO.iter().chain(&["sql"]) {
            for _ in 0..25 {
                b.anotar(&acesso(op, 1_000, 0));
            }
            assert_eq!(
                b.anotar(&acesso(op, 1_000_000, 0)),
                Habitual::SemBase,
                "{op}"
            );
        }
        assert_eq!(b.atualizacoes(), 0);
        // Sem medida em µs, tambem nao.
        assert_eq!(b.anotar(&acesso("inserir", 0, 0)), Habitual::SemBase);
        assert_eq!(b.atualizacoes(), 0);
    }

    /// A 5.001a chave vai a coringa, e a base das outras nao muda. Trocar a
    /// coringa por despejo derruba o «habitual intacto».
    #[test]
    fn o_teto_manda_para_a_coringa_sem_apagar_ninguem() {
        let b = BaseDeConsultas::default();
        for _ in 0..20 {
            b.observar(chave("primeira"), 1_000, T0);
        }
        let antes = b.ler(chave("primeira")).unwrap();
        let nomes: Vec<String> = (0..TETO_DE_CHAVES).map(|i| format!("t{i}")).collect();
        for n in &nomes {
            b.observar(chave(n), 1_000, T0);
        }
        assert_eq!(b.chaves(), TETO_DE_CHAVES);
        assert!(b.na_coringa() > 0);
        assert_eq!(b.ler(chave("primeira")), Some(antes));
        // A coringa soma mas nunca alarma: o habitual dela e de ninguem.
        for _ in 0..30 {
            b.observar(chave("inundacao"), 1_000, T0);
        }
        assert_eq!(b.observar(chave("inundacao"), 10_000_000, T0), None);
        assert!(b.observar(chave("primeira"), 10_000_000, T0).is_some());
    }

    /// Duas metades: o habitual de uma hora atras some, o da metade anterior
    /// fica. E a uniao de Chan da o mesmo que somar tudo junto.
    #[test]
    fn a_janela_esquece_o_que_passou_de_uma_hora() {
        let b = BaseDeConsultas::default();
        for _ in 0..20 {
            b.observar(chave("j"), 1_000, T0);
        }
        // Na metade seguinte, o habitual ainda esta la.
        let seguinte = T0 + METADE_MS;
        assert_eq!(b.ler(chave("j")).unwrap().n, 20);
        assert!(b.observar(chave("j"), 10_000_000, seguinte).is_some());
        // Duas metades depois, ja nao: 10 s sem habitual nao e anormal.
        let depois = T0 + 3 * METADE_MS;
        assert_eq!(b.observar(chave("j"), 10_000_000, depois), None);

        let mut a = Welford::default();
        let mut c = Welford::default();
        let mut tudo = Welford::default();
        for i in 0..50 {
            let x = (i as f64).sin() * 3.0 + 7.0;
            if i < 17 {
                a.somar(x)
            } else {
                c.somar(x)
            }
            tudo.somar(x);
        }
        let u = Welford::unir(a, c);
        assert_eq!(u.n, tudo.n);
        assert!((u.media - tudo.media).abs() < 1e-9);
        assert!((u.m2 - tudo.m2).abs() < 1e-6);
    }

    #[test]
    fn a_espera_da_thread_se_toma_uma_vez() {
        tomar_espera();
        somar_espera(30);
        somar_espera(12);
        assert_eq!(tomar_espera(), 42);
        assert_eq!(tomar_espera(), 0);
    }
}

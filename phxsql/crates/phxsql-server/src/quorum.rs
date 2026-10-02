//! O cubo do quorum de escrita -- pedido 207.
//!
//! # O que ele e
//!
//! O ponto de encontro entre o commit que espera e as replicas que confirmam.
//! O commit (no `Drop` da `TravaMedida`, com a trava de dados do master na
//! mao) entrega os eventos que acabou de gravar e dorme numa `Condvar`; cada
//! replica, pela conexao que ELA abriu e mantem (`replicar_aguardar`), leva os
//! eventos, aplica, grava em disco e confirma no pedido seguinte.
//!
//! # Por que ele NAO conhece a trava de dados
//!
//! E a decisao D-trava do contrato (`docs/propostas/207-e-513p2-contrato-01-10-2026.md`):
//! o commit espera com a trava exclusiva na mao, e o `replicar` de sempre
//! pede essa mesma trava. Se a confirmacao dependesse dela, a replica
//! esperaria o commit que a espera, e TODO commit estouraria o prazo. Entao
//! os eventos viajam materializados aqui dentro, e o caminho do ack so toca
//! este `Mutex` -- que nunca e tomado por quem segura a trava de dados por
//! mais que um empurrar de lista. A prova e o teste `o_ack_nao_pede_a_trava`.
//!
//! # Os tres estados
//!
//! - **desligado**: `minimo == 0`. Nada se anota, nada se espera.
//! - **sincrono**: o commit espera `minimo` replicas confirmarem.
//! - **degradado**: a ultima espera venceu o prazo. O commit responde na hora,
//!   DIZENDO `degradado:true`, e o servidor so volta ao sincrono quando
//!   `minimo` replicas alcancaram o que o master tem -- e nunca antes do
//!   recuo (decisao F2: aqui a espera prende o servidor inteiro, e voltar no
//!   primeiro ack deixaria uma replica que pisca parar o servidor a cada
//!   pulso).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;

/// A janela em que degradacoes seguidas fazem o recuo dobrar. Uma
/// degradacao depois de dez minutos inteiros sincrono recomeca da base.
pub const JANELA_DO_RECUO: Duration = Duration::from_secs(600);

/// Quanto um `replicar_aguardar` entrega de uma vez, em bytes de fio
/// estimados: abaixo do teto do registro do `Canal` (128 MiB) com folga, e o
/// resto fica pendente para a volta seguinte. Sempre sai ao menos um lote.
pub const ORCAMENTO_DA_ENTREGA: usize = 48 * 1024 * 1024;

/// Uma tabela e a posicao do diario dela -- o que o commit exige, o que a
/// replica confirma e o que o degradado anuncia como alcance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Exigencia {
    pub database: String,
    pub tabela: String,
    pub posicao: u64,
}

impl Exigencia {
    pub fn chave(&self) -> String {
        chave(&self.database, &self.tabela)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("database", Json::texto_de(&self.database)),
            ("tabela", Json::texto_de(&self.tabela)),
            ("posicao", Json::de_u64(self.posicao)),
        ])
    }

    /// A lista do fio; item sem `database` ou `tabela` e ignorado.
    pub fn da_lista(j: Option<&Json>) -> Vec<Exigencia> {
        j.and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .filter_map(|e| {
                let database = e.texto_ou("database", "").trim().to_string();
                let tabela = e.texto_ou("tabela", "").trim().to_string();
                (!database.is_empty() && !tabela.is_empty()).then(|| Exigencia {
                    database,
                    tabela,
                    posicao: e.inteiro_ou("posicao", 0).max(0) as u64,
                })
            })
            .collect()
    }
}

/// Os eventos de UMA tabela que um commit gravou, prontos para o fio.
///
/// Materializados uma vez, com a trava de dados do master na mao, e
/// entregues a todas as replicas: o `Arc` e o que deixa N replicas levarem o
/// mesmo lote sem copia.
pub struct LoteDoQuorum {
    pub database: String,
    pub tabela: String,
    /// A tabela tem coluna marcada (LGPD): so viaja por fio cifrado -- o
    /// mesmo portao do `replicar` (pedido 342), conferido na ENTREGA, porque
    /// e la que se sabe por qual fio a replica veio.
    pub dado_pessoal: bool,
    /// Quantos eventos o diario do master tem depois deste commit.
    pub eventos_do_master: u64,
    /// A posicao que a replica tem de ter para aplicar este lote. Com
    /// `posicao > 0` o primeiro evento da lista e o `posicao - 1`, o que a
    /// replica JA tem -- a conferencia de continuidade do pull, sem ida e
    /// volta a mais (`aplicar_lote_da_replica`).
    pub posicao: u64,
    pub eventos: Vec<Json>,
    pub linhagem: Json,
    /// O esquema cru da tabela, em hexadecimal -- o mesmo bloco do `posicao`
    /// com `com_esquema`. A replica que ainda nao tem a tabela a cria por
    /// ele (`abrir_para_replicar`), em vez de recusar o lote e esperar o pull
    /// que a trava do commit nao deixaria passar.
    pub esquema: String,
    /// O tamanho estimado no fio, para o [`ORCAMENTO_DA_ENTREGA`].
    pub bytes: usize,
}

impl LoteDoQuorum {
    pub fn chave(&self) -> String {
        chave(&self.database, &self.tabela)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("database", Json::texto_de(&self.database)),
            ("tabela", Json::texto_de(&self.tabela)),
            ("posicao", Json::de_u64(self.posicao)),
            ("eventos_do_master", Json::de_u64(self.eventos_do_master)),
            ("linhagem", self.linhagem.clone()),
            ("esquema", Json::texto_de(&self.esquema)),
            ("eventos", Json::Lista(self.eventos.clone())),
        ])
    }
}

/// A chave de uma tabela no cubo: a mesma forma (`db/tabela`, minusculas) da
/// chave do diario do servidor, para as duas nunca divergirem.
pub fn chave(database: &str, tabela: &str) -> String {
    format!("{}/{}", database.to_lowercase(), tabela.to_lowercase())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    Sincrono,
    Degradado,
}

impl Modo {
    pub fn nome(self) -> &'static str {
        match self {
            Modo::Sincrono => "sincrono",
            Modo::Degradado => "degradado",
        }
    }
}

/// O que UMA espera deu -- e o que a resposta ao cliente carrega.
#[derive(Clone, Debug, PartialEq)]
pub struct Resultado {
    pub pedido: u64,
    pub confirmado: u64,
    pub alcancado: bool,
    pub degradado: bool,
    pub ms: f64,
}

impl Resultado {
    /// Dois commits no mesmo pedido (um `op` que toma a trava duas vezes):
    /// vale o PIOR, e o tempo soma -- o cliente esperou os dois.
    pub fn juntar(self, outro: Resultado) -> Resultado {
        Resultado {
            pedido: self.pedido.max(outro.pedido),
            confirmado: self.confirmado.min(outro.confirmado),
            alcancado: self.alcancado && outro.alcancado,
            degradado: self.degradado || outro.degradado,
            ms: self.ms + outro.ms,
        }
    }

    pub fn para_json(&self) -> Json {
        let mut campos = vec![
            ("pedido", Json::de_u64(self.pedido)),
            ("confirmado", Json::de_u64(self.confirmado)),
            ("alcancado", Json::Bool(self.alcancado)),
            ("ms", Json::Numero((self.ms * 1000.0).round() / 1000.0)),
        ];
        if self.degradado {
            campos.push(("degradado", Json::Bool(true)));
        }
        Json::objeto(campos)
    }
}

/// O que uma replica recebe de um `replicar_aguardar`.
pub struct Entrega {
    pub lotes: Vec<Arc<LoteDoQuorum>>,
    pub modo: Modo,
    /// Degradado: as posicoes que o master conhece, para a replica saber se
    /// alcancou -- e dizer, no pedido seguinte, que alcancou.
    pub alcance: Vec<Exigencia>,
}

struct Replica {
    pendentes: Vec<Arc<LoteDoQuorum>>,
    /// A ultima posicao CONFIRMADA (aplicada e em disco) por tabela.
    posicoes: HashMap<String, u64>,
    visto: Instant,
}

struct Estado {
    modo: Modo,
    /// Degradado nao volta antes disto -- o recuo.
    volta_depois_de: Option<Instant>,
    ultima_degradacao: Option<Instant>,
    ritmo: crate::replica::Ritmo,
    replicas: HashMap<String, Replica>,
    /// A ultima posicao que o master conhece por tabela, anotada por todo
    /// commit com o quorum ligado -- inclusive no degradado, que e quando a
    /// replica precisa dela para saber se alcancou.
    mestre: HashMap<String, Exigencia>,
    /// A ficha com que as replicas entram (o usuario do cluster), copiada da
    /// sessao do `replicar_aguardar`. O commit a consulta para saber que
    /// tabela a replica alcanca SEM pedir o cadastro com a trava de dados na
    /// mao.
    ficha: Option<crate::usuarios::Usuario>,
    degradacoes: u64,
    ultima_degradacao_ms: i64,
    espera_maxima_ms: f64,
    esperas: u64,
    alcancadas: u64,
}

/// O cubo. Um por servidor em cluster.
pub struct Cubo {
    minimo: AtomicU64,
    prazo_ms: AtomicU64,
    /// Arquivos que o master mandou ao disco ANTES de esperar (D-local). E
    /// a medida do efeito, contada pelo armazem, e nao a da chamada: sem o
    /// `sincronizar` ela fica parada (teste 6 do contrato).
    fsync_local: AtomicU64,
    /// Do lado da replica: confirmacoes mandadas, e quantas delas vieram
    /// depois de um `fsync` que levou arquivo ao disco (teste 4: o «ok» e
    /// aplicou E gravou).
    acks: AtomicU64,
    acks_com_fsync: AtomicU64,
    estado: Mutex<Estado>,
    sinal: Condvar,
}

impl Cubo {
    /// `base_do_recuo` e o `pulso_s` do cluster: o recuo dobra a partir dele.
    pub fn novo(minimo: u64, prazo_ms: u64, base_do_recuo: Duration) -> Cubo {
        Cubo {
            minimo: AtomicU64::new(minimo),
            prazo_ms: AtomicU64::new(prazo_ms),
            fsync_local: AtomicU64::new(0),
            acks: AtomicU64::new(0),
            acks_com_fsync: AtomicU64::new(0),
            estado: Mutex::new(Estado {
                modo: Modo::Sincrono,
                volta_depois_de: None,
                ultima_degradacao: None,
                ritmo: crate::replica::Ritmo::novo(base_do_recuo),
                replicas: HashMap::new(),
                mestre: HashMap::new(),
                ficha: None,
                degradacoes: 0,
                ultima_degradacao_ms: 0,
                espera_maxima_ms: 0.0,
                esperas: 0,
                alcancadas: 0,
            }),
            sinal: Condvar::new(),
        }
    }

    /// A quente (D-quente): o commit seguinte ja le os valores novos.
    pub fn definir(&self, minimo: u64, prazo_ms: u64) {
        self.minimo.store(minimo, Ordering::SeqCst);
        self.prazo_ms.store(prazo_ms, Ordering::SeqCst);
        // Quem esta dormindo numa espera reconta com o minimo novo.
        self.sinal.notify_all();
    }

    pub fn contar_fsync_local(&self, arquivos: u64) {
        self.fsync_local.fetch_add(arquivos, Ordering::Relaxed);
    }

    /// A replica vai confirmar uma tabela; `arquivos` e o que o `fsync` dela
    /// levou ao disco antes.
    pub fn contar_ack(&self, arquivos: u64) {
        self.acks.fetch_add(1, Ordering::Relaxed);
        if arquivos > 0 {
            self.acks_com_fsync.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn minimo(&self) -> u64 {
        self.minimo.load(Ordering::Relaxed)
    }

    pub fn prazo(&self) -> Duration {
        Duration::from_millis(self.prazo_ms.load(Ordering::Relaxed))
    }

    /// O `Mutex` do cubo envenenado so por panico de fora: aqui dentro so ha
    /// `HashMap` e `Vec`, que o desenrolar nao entorta.
    fn travar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|v| v.into_inner())
    }

    pub fn modo(&self) -> Modo {
        self.travar().modo
    }

    /// A ficha com que as replicas entram, se alguma ja entrou.
    pub fn ficha(&self) -> Option<crate::usuarios::Usuario> {
        self.travar().ficha.clone()
    }

    /// Anota as posicoes do master sem esperar -- o commit no degradado.
    pub fn anotar_mestre(&self, exigencias: &[Exigencia]) {
        let mut e = self.travar();
        anotar(&mut e, exigencias);
    }

    /// O commit: entrega os lotes, espera `minimo` replicas confirmarem
    /// `exigencias` (`chave -> posicao`), ou o prazo. Vencido o prazo, o
    /// servidor entra em degradado -- e a gravacao FICA (D-falha).
    pub fn esperar(&self, lotes: Vec<LoteDoQuorum>, exigencias: &[Exigencia]) -> Resultado {
        let comeco = Instant::now();
        let prazo = self.prazo();
        let lotes: Vec<Arc<LoteDoQuorum>> = lotes.into_iter().map(Arc::new).collect();
        let mut e = self.travar();
        anotar(&mut e, exigencias);
        for r in e.replicas.values_mut() {
            r.pendentes.extend(lotes.iter().cloned());
        }
        e.esperas += 1;
        self.sinal.notify_all();
        let (confirmado, alcancado) = loop {
            let minimo = self.minimo();
            let k = confirmadas(&e, exigencias);
            if minimo == 0 || k >= minimo {
                break (k, true);
            }
            let passou = comeco.elapsed();
            // Menos replicas CONHECIDAS que o minimo: ninguem que chegue
            // agora leva este lote (ele foi entregue so a quem ja estava na
            // lista), e o pull de quem chega pede a trava que este commit
            // segura. Esperar o prazo seria parar o servidor por nada -- o
            // caso do arranque, antes de a primeira replica abrir o canal.
            let conhecidas = e.replicas.len() as u64;
            if passou >= prazo || conhecidas < minimo {
                degradar(&mut e, Instant::now());
                break (k, false);
            }
            e = self
                .sinal
                .wait_timeout(e, prazo - passou)
                .map(|(g, _)| g)
                .unwrap_or_else(|v| v.into_inner().0);
        };
        let ms = comeco.elapsed().as_secs_f64() * 1000.0;
        if alcancado {
            e.alcancadas += 1;
        }
        if ms > e.espera_maxima_ms {
            e.espera_maxima_ms = ms;
        }
        Resultado {
            pedido: self.minimo(),
            confirmado,
            alcancado,
            degradado: !alcancado,
            ms,
        }
    }

    /// A replica `id` confirma o que aplicou e gravou, e espera ate
    /// `esperar` por lotes novos. NUNCA toca a trava de dados.
    pub fn aguardar(
        &self,
        id: &str,
        ficha: Option<&crate::usuarios::Usuario>,
        confirmados: &[Exigencia],
        esperar: Duration,
    ) -> Entrega {
        let ate = Instant::now() + esperar;
        let mut e = self.travar();
        if ficha.is_some() {
            e.ficha = ficha.cloned();
        }
        let agora = Instant::now();
        let r = e.replicas.entry(id.to_string()).or_insert_with(|| Replica {
            pendentes: Vec::new(),
            posicoes: HashMap::new(),
            visto: agora,
        });
        r.visto = agora;
        for c in confirmados {
            let atual = r.posicoes.entry(c.chave()).or_insert(0);
            *atual = (*atual).max(c.posicao);
        }
        voltar_se_alcancou(&mut e, self.minimo(), agora);
        // Quem espera num commit reconta com a confirmacao nova.
        self.sinal.notify_all();
        loop {
            let r = e.replicas.get_mut(id).expect("inserida acima");
            if !r.pendentes.is_empty() {
                // Ao menos um, e depois so o que cabe no orcamento: o resto
                // fica para a volta seguinte, que vem logo -- a replica
                // confirma o que aplicou e pede de novo.
                let mut usados = 0usize;
                let mut quantos = 0usize;
                for l in &r.pendentes {
                    if quantos > 0 && usados + l.bytes > ORCAMENTO_DA_ENTREGA {
                        break;
                    }
                    usados += l.bytes;
                    quantos += 1;
                }
                let lotes: Vec<Arc<LoteDoQuorum>> = r.pendentes.drain(..quantos).collect();
                return Entrega {
                    lotes,
                    modo: e.modo,
                    alcance: Vec::new(),
                };
            }
            let agora = Instant::now();
            if agora >= ate {
                let alcance = if e.modo == Modo::Degradado {
                    let mut v: Vec<Exigencia> = e.mestre.values().cloned().collect();
                    v.sort();
                    v
                } else {
                    Vec::new()
                };
                return Entrega {
                    lotes: Vec::new(),
                    modo: e.modo,
                    alcance,
                };
            }
            e = self
                .sinal
                .wait_timeout(e, ate - agora)
                .map(|(g, _)| g)
                .unwrap_or_else(|v| v.into_inner().0);
        }
    }

    /// O bloco `quorum` do `replicacao_estado`.
    pub fn para_json(&self) -> Json {
        let e = self.travar();
        let minimo = self.minimo();
        let agora = Instant::now();
        let mut nos: Vec<(String, Json)> = e
            .replicas
            .iter()
            .map(|(id, r)| {
                let mut pos: Vec<(String, Json)> = r
                    .posicoes
                    .iter()
                    .map(|(k, p)| (k.clone(), Json::de_u64(*p)))
                    .collect();
                pos.sort_by(|a, b| a.0.cmp(&b.0));
                (
                    id.clone(),
                    Json::objeto(vec![
                        (
                            "visto_ha_ms",
                            Json::de_u64(agora.duration_since(r.visto).as_millis() as u64),
                        ),
                        ("posicoes", Json::Objeto(pos)),
                    ]),
                )
            })
            .collect();
        nos.sort_by(|a, b| a.0.cmp(&b.0));
        Json::objeto(vec![
            (
                "estado",
                Json::texto_de(if minimo == 0 {
                    "desligado"
                } else {
                    e.modo.nome()
                }),
            ),
            ("minimo", Json::de_u64(minimo)),
            (
                "prazo_ms",
                Json::de_u64(self.prazo_ms.load(Ordering::Relaxed)),
            ),
            ("esperas", Json::de_u64(e.esperas)),
            ("alcancadas", Json::de_u64(e.alcancadas)),
            ("degradacoes", Json::de_u64(e.degradacoes)),
            ("ultima_degradacao_ms", Json::de_i64(e.ultima_degradacao_ms)),
            (
                "espera_maxima_ms",
                Json::Numero((e.espera_maxima_ms * 1000.0).round() / 1000.0),
            ),
            (
                "recuo_restante_ms",
                Json::de_u64(
                    e.volta_depois_de
                        .map(|v| v.saturating_duration_since(agora).as_millis() as u64)
                        .unwrap_or(0),
                ),
            ),
            (
                "arquivos_sincronizados_antes_de_esperar",
                Json::de_u64(self.fsync_local.load(Ordering::Relaxed)),
            ),
            (
                "acks_mandados",
                Json::de_u64(self.acks.load(Ordering::Relaxed)),
            ),
            (
                "acks_depois_do_fsync",
                Json::de_u64(self.acks_com_fsync.load(Ordering::Relaxed)),
            ),
            ("replicas", Json::Objeto(nos)),
        ])
    }

    /// So para a prova do recuo: quanto falta para o degradado poder voltar.
    #[cfg(test)]
    fn recuo_restante(&self, agora: Instant) -> Duration {
        self.travar()
            .volta_depois_de
            .map(|v| v.saturating_duration_since(agora))
            .unwrap_or_default()
    }
}

/// Quantas replicas confirmaram TODAS as exigencias. Conta so replicas: o
/// master nunca entra na conta (C2, os tres motores contam replicas).
fn confirmadas(e: &Estado, exigencias: &[Exigencia]) -> u64 {
    e.replicas
        .values()
        .filter(|r| {
            exigencias
                .iter()
                .all(|x| r.posicoes.get(&x.chave()).is_some_and(|q| *q >= x.posicao))
        })
        .count() as u64
}

fn anotar(e: &mut Estado, exigencias: &[Exigencia]) {
    for x in exigencias {
        e.mestre.insert(x.chave(), x.clone());
    }
}

/// A espera venceu: degradado, com o recuo do `replica::Ritmo` -- o MESMO
/// motor do laco da replica (pedido 585): base × 2^n, teto 60 s. Degradacao
/// depois de dez minutos inteiros sem nenhuma recomeca da base.
fn degradar(e: &mut Estado, agora: Instant) {
    if e.ultima_degradacao
        .is_some_and(|u| agora.duration_since(u) >= JANELA_DO_RECUO)
    {
        e.ritmo.sucesso();
    }
    let recuo = match e.ritmo.apos(crate::replica::Falha::Rede) {
        crate::replica::Decisao::Dormir(d) => d,
        crate::replica::Decisao::Estacionar => e.ritmo.base(),
    };
    e.modo = Modo::Degradado;
    e.volta_depois_de = Some(agora + recuo);
    e.ultima_degradacao = Some(agora);
    e.degradacoes += 1;
    e.ultima_degradacao_ms = crate::agora_ms();
    // O que estava pendurado para entrega e de um commit que ja respondeu:
    // a replica alcanca pelo `replicar` de sempre, e nao por lote velho.
    for r in e.replicas.values_mut() {
        r.pendentes.clear();
    }
}

/// Degradado volta ao sincrono quando `minimo` replicas confirmaram tudo o
/// que o master conhece -- e nunca antes do recuo (F2).
fn voltar_se_alcancou(e: &mut Estado, minimo: u64, agora: Instant) {
    if e.modo != Modo::Degradado || minimo == 0 {
        return;
    }
    if e.volta_depois_de.is_some_and(|v| agora < v) {
        return;
    }
    let exigencias: Vec<Exigencia> = e.mestre.values().cloned().collect();
    if confirmadas(e, &exigencias) >= minimo {
        e.modo = Modo::Sincrono;
        e.volta_depois_de = None;
    }
}

/// A replica aceita o lote deste master? So se a epoca dele nao for MENOR
/// que a que ela conhece (D-epoca): master rebaixado que ainda nao soube
/// disso nao obtem confirmacao de ninguem -- e o que faz o quorum fechar o
/// paragrafo do `cluster.rs` sobre o master isolado.
pub fn aceita_a_epoca(minha: u64, do_master: u64) -> bool {
    do_master >= minha
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lote(db: &str, t: &str, depois: u64) -> LoteDoQuorum {
        LoteDoQuorum {
            database: db.into(),
            tabela: t.into(),
            dado_pessoal: false,
            eventos_do_master: depois,
            posicao: depois.saturating_sub(1),
            eventos: vec![],
            linhagem: Json::Nulo,
            esquema: String::new(),
            bytes: 10,
        }
    }

    fn x(db: &str, t: &str, posicao: u64) -> Exigencia {
        Exigencia {
            database: db.into(),
            tabela: t.into(),
            posicao,
        }
    }

    /// Teste 8 do contrato: `quorum_minimo:2` com tres nos exige as DUAS
    /// replicas. O vermelho e contar o master (C1): com ele, uma replica so
    /// fecharia o quorum de 2.
    #[test]
    fn o_quorum_conta_so_replicas() {
        let c = Arc::new(Cubo::novo(2, 400, Duration::from_secs(1)));
        // As duas replicas ja conversaram uma vez.
        c.aguardar("no2", None, &[], Duration::ZERO);
        c.aguardar("no3", None, &[], Duration::ZERO);
        let c2 = Arc::clone(&c);
        let so_uma = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            c2.aguardar("no2", None, &[x("loja", "t", 5)], Duration::ZERO);
        });
        let r = c.esperar(vec![lote("loja", "t", 5)], &[x("loja", "t", 5)]);
        so_uma.join().unwrap();
        assert!(!r.alcancado, "uma replica so nao fecha quorum de 2: {r:?}");
        assert_eq!(r.confirmado, 1);
        assert!(r.degradado);
    }

    #[test]
    fn duas_replicas_fecham_o_quorum_de_dois() {
        let c = Arc::new(Cubo::novo(2, 5_000, Duration::from_secs(1)));
        c.aguardar("no2", None, &[], Duration::ZERO);
        c.aguardar("no3", None, &[], Duration::ZERO);
        let mut filhos = Vec::new();
        for id in ["no2", "no3"] {
            let c2 = Arc::clone(&c);
            filhos.push(std::thread::spawn(move || {
                let e = c2.aguardar(id, None, &[], Duration::from_secs(2));
                assert_eq!(e.lotes.len(), 1, "o lote chega a cada replica");
                c2.aguardar(id, None, &[x("loja", "t", 5)], Duration::ZERO);
            }));
        }
        std::thread::sleep(Duration::from_millis(30));
        let r = c.esperar(vec![lote("loja", "t", 5)], &[x("loja", "t", 5)]);
        for f in filhos {
            f.join().unwrap();
        }
        assert!(r.alcancado, "{r:?}");
        assert_eq!(r.confirmado, 2);
        assert!(r.ms < 2_000.0);
    }

    /// Sem replica nenhuma no canal (o arranque), o commit degrada NA HORA:
    /// ninguem que chegue depois leva o lote, e o pull de quem chega pede a
    /// trava que o commit segura. O vermelho e esperar o prazo inteiro.
    #[test]
    fn sem_replica_conhecida_degrada_sem_esperar_o_prazo() {
        let c = Cubo::novo(1, 5_000, Duration::from_secs(1));
        let r = c.esperar(vec![lote("loja", "t", 1)], &[x("loja", "t", 1)]);
        assert!(r.degradado && !r.alcancado, "{r:?}");
        assert!(r.ms < 1_000.0, "esperou {} ms por ninguem", r.ms);
    }

    /// Teste 7 do contrato: o recuo DOBRA a cada degradacao dentro da janela
    /// de dez minutos, e recomeca da base depois dela. O vermelho e F1 --
    /// voltar no primeiro ack, sem recuo nenhum.
    #[test]
    fn o_recuo_dobra_a_cada_degradacao() {
        let c = Cubo::novo(1, 100, Duration::from_secs(2));
        let mut vistos = Vec::new();
        for _ in 0..3 {
            let agora = Instant::now();
            degradar(&mut c.travar(), agora);
            let r = c.recuo_restante(agora);
            vistos.push(r.as_secs());
            // Volta ao sincrono para a degradacao seguinte.
            c.travar().modo = Modo::Sincrono;
        }
        assert_eq!(vistos, vec![2, 4, 8], "o recuo tem de dobrar");
        // Depois da janela, a base de novo.
        let depois = Instant::now() + JANELA_DO_RECUO + Duration::from_secs(1);
        degradar(&mut c.travar(), depois);
        assert_eq!(c.recuo_restante(depois).as_secs(), 2);
    }

    /// O degradado NAO volta antes do recuo, mesmo com a replica em dia -- e
    /// volta assim que o recuo passa e ela confirma o que o master conhece.
    #[test]
    fn o_degradado_volta_so_depois_do_recuo_e_do_alcance() {
        let c = Cubo::novo(1, 100, Duration::from_millis(200));
        let r = c.esperar(vec![lote("loja", "t", 3)], &[x("loja", "t", 3)]);
        assert!(r.degradado);
        assert_eq!(c.modo(), Modo::Degradado);
        c.aguardar("no2", None, &[x("loja", "t", 3)], Duration::ZERO);
        assert_eq!(c.modo(), Modo::Degradado, "dentro do recuo nao volta");
        std::thread::sleep(Duration::from_millis(250));
        // Atras do que o master conhece: nao volta.
        c.anotar_mestre(&[x("loja", "t", 4)]);
        let e = c.aguardar("no2", None, &[x("loja", "t", 3)], Duration::ZERO);
        assert_eq!(
            e.alcance,
            vec![x("loja", "t", 4)],
            "o degradado diz o alcance"
        );
        assert_eq!(c.modo(), Modo::Degradado);
        c.aguardar("no2", None, &[x("loja", "t", 4)], Duration::ZERO);
        assert_eq!(c.modo(), Modo::Sincrono);
    }

    /// Teste 5 do contrato, a regra pura: lote de master com epoca menor que
    /// a conhecida nao se aplica nem se confirma.
    #[test]
    fn master_de_epoca_velha_nao_obtem_confirmacao() {
        assert!(!aceita_a_epoca(7, 6));
        assert!(aceita_a_epoca(7, 7));
        assert!(aceita_a_epoca(7, 8));
    }

    #[test]
    fn o_resultado_junta_pelo_pior() {
        let a = Resultado {
            pedido: 1,
            confirmado: 1,
            alcancado: true,
            degradado: false,
            ms: 1.0,
        };
        let b = Resultado {
            pedido: 1,
            confirmado: 0,
            alcancado: false,
            degradado: true,
            ms: 2.0,
        };
        let j = a.juntar(b);
        assert!(!j.alcancado && j.degradado);
        assert_eq!(j.confirmado, 0);
        assert_eq!(j.ms, 3.0);
    }
}

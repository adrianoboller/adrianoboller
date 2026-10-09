//! A contagem do aquario (pedido 707, fatia A8): as oito series dos tres
//! graficos -- select, insert, update, excluir suave, excluir fisico, backup,
//! erro e aviso --, por minuto e por hora.
//!
//! Desenho em `docs/propostas/aquario-707.md` §11.4. O que cada decisao comprou:
//!
//! * **Um acumulador so**, alimentado pelo [`super::Aquario::anotar`] -- o
//!   unico sumidouro por onde toda resposta passa. A hora e o FECHO do minuto,
//!   nao um segundo contador (ordem do dono, item 8 do 707).
//! * **A unidade e o pedido**, nao a linha: lote de 5.000 conta 1, `UPDATE`
//!   por faixa conta 1, e o derivado do `executar_derivado` nao conta de novo,
//!   porque nao passa pelo `anotar`. E o `Com_xxx` do MySQL e do MariaDB;
//!   linha por periodo fica para depois da versao.
//! * **Quem decide a categoria e o desfecho**, lido da RESPOSTA: o excluir que
//!   a tabela sem marca fez fisico conta como fisico, mesmo sem `"fisico"` no
//!   pedido. Ler a bandeira do pedido seria adivinhar o que o motor fez.
//! * **As barras contam so o que terminou `ok`**: o backup que falhou nao sobe
//!   a barra de backup -- seria mentira sobre a copia --, sobe so `erro`.
//! * **Zero nao e `null`.** O minuto em que a telemetria estava ligada e nada
//!   aconteceu sai com zeros; o minuto em que o servidor estava fora, ou a
//!   telemetria desligada, NAO sai -- e e a ausencia que a tela desenha como
//!   `null`. A hora carrega `minutos_medidos` pelo mesmo motivo: parcial e
//!   desenhado parcial, nunca completado com zero inventado.
//! * **Hora UTC, fora do rodizio**, em `aquario-horas.jsonl`. O motor nao tem
//!   fuso (`datahora.rs`); fechar por DIA UTC poria 3 das 24 h do dia de
//!   Brasilia no dia errado. A tela soma as horas no fuso do navegador.
//!
//! # Quem escreve o que
//!
//! A linha por minuto vai ao `aquario.log`, e o escritor dele e da fatia A6.
//! Daqui ela sai como [`Json`] pronto, na [`Virada`], e o `virar_a_contagem`
//! do servidor a grava pelo escritor da A6 -- nao nasce um
//! segundo escritor do `aquario.log` aqui dentro (lei «vem do mesmo motor»).
//! O `aquario-horas.jsonl` e desta fatia, e e escrito pelo MESMO escritor do
//! `acessos.log` ([`LogAcessos::registrar_json`]) com o rodizio desligado.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::acesso::{Acesso, LogAcessos};

pub const MINUTO_MS: i64 = 60_000;
pub const HORA_MS: i64 = 3_600_000;

/// O arquivo das horas fechadas, ao lado do `acessos.log`.
pub const ARQUIVO_DE_HORAS: &str = "aquario-horas.jsonl";

/// Quantas horas fechadas esperam em memoria quando o disco recusa a
/// gravacao: uma semana. Passou disso, a mais velha sai e entra na conta de
/// `horas_perdidas` -- memoria sem teto para um disco que nao volta seria o
/// estrago seguinte, e a perda contada e dita na consulta.
const TETO_DE_HORAS_POR_GRAVAR: usize = 168;

/// As oito series, na ordem em que a tela as desenha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serie {
    Select,
    Insert,
    Update,
    ExcluirSuave,
    ExcluirFisico,
    Backup,
    Erro,
    Aviso,
}

impl Serie {
    pub const TODAS: [Serie; 8] = [
        Serie::Select,
        Serie::Insert,
        Serie::Update,
        Serie::ExcluirSuave,
        Serie::ExcluirFisico,
        Serie::Backup,
        Serie::Erro,
        Serie::Aviso,
    ];

    /// O nome que vai ao arquivo e a resposta. Fixo: reordenar a declaracao
    /// nao pode mudar o que uma linha antiga quer dizer.
    pub fn nome(self) -> &'static str {
        match self {
            Serie::Select => "select",
            Serie::Insert => "insert",
            Serie::Update => "update",
            Serie::ExcluirSuave => "excluir_suave",
            Serie::ExcluirFisico => "excluir_fisico",
            Serie::Backup => "backup",
            Serie::Erro => "erro",
            Serie::Aviso => "aviso",
        }
    }

    fn indice(self) -> usize {
        self as usize
    }
}

/// As operacoes que devolvem LINHAS, e por isso contam como `select`.
///
/// Nao e a `OPS_QUE_DEVOLVEM_LINHAS` do `servico_consulta_01.rs`, de
/// proposito: aquela responde «o que a composicao aceita como sub-pedido», e
/// acrescentar `ler` ou `juntar` la para servir a contagem mudaria o que o
/// `consultar` aceita. Mesmo nome, pergunta diferente. Os apelidos entram
/// porque a contagem ve o nome que o cliente mandou; o teste
/// `as_listas_so_nomeiam_operacoes_do_catalogo` reprova nome que nao existe.
pub const OPS_DE_LEITURA: &[&str] = &[
    "ler",
    "varrer",
    "buscar",
    "procurar_texto",
    "consultar",
    "agrupar",
    "group_by",
    "pivotar",
    "pivot",
    "juntar",
    "join",
    "unir",
    "union",
    "diferencas",
    "diff",
    "SelectMemory",
    "selectmemory",
    "selecionar_memoria",
];

/// Inserir uma linha ou um lote: o lote conta 1, porque a unidade e o pedido.
pub const OPS_DE_INSERCAO: &[&str] = &["inserir", "inserir_lote", "importar", "carga"];

/// O backup pedido e o agendado (o job de backup chega com a op `backup`).
pub const OPS_DE_BACKUP: &[&str] = &["backup", "backup_agendado"];

/// O que um pedido terminado entrega a contagem. Calculado por quem tem a
/// resposta na mao, e levado pelo [`Acesso`] ate o `anotar` -- so em memoria,
/// nunca no `acessos.log`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Desfecho {
    /// A barra da categoria, quando o pedido tem uma.
    pub serie: Option<Serie>,
    /// A resposta trouxe `aviso` ou `avisos` nao vazio no nivel de cima.
    pub aviso: bool,
}

impl Desfecho {
    /// O desfecho de `op`, pela RESPOSTA. `None` e o pedido que falhou: o erro
    /// conta pelo `ok:false` do `Acesso`, e a barra nao sobe.
    ///
    /// A op `sql` diz na propria resposta qual operacao atendeu (`"op"`), e e
    /// essa que decide -- o `SELECT` vira `varrer`/`consultar`, o `DELETE`
    /// vira `excluir` e traz o `modo` quando ha um so. O `DELETE` por faixa nao
    /// traz `modo` e conta como suave, que e o que o derivado dele pede.
    pub fn da_resposta(op: &str, resposta: Option<&Json>) -> Desfecho {
        let Some(r) = resposta else {
            return Desfecho::default();
        };
        let efetiva = if op == "sql" {
            r.texto_ou("op", "")
        } else {
            op
        };
        let serie = serie_pela_op(efetiva, r.texto_ou("modo", "suave") == "fisico");
        let aviso = ["aviso", "avisos"]
            .iter()
            .any(|c| r.campo(c).is_some_and(nao_vazio));
        Desfecho { serie, aviso }
    }
}

/// A serie de uma operacao, pelo NOME -- a decisao que a contagem e a faixa
/// do aquario dividem. O modo do `excluir` vem de quem o sabe: a resposta,
/// na contagem; ninguem, na tarefa viva (a faixa e `delete` dos dois jeitos).
fn serie_pela_op(op: &str, excluir_fisico: bool) -> Option<Serie> {
    if OPS_DE_LEITURA.contains(&op) {
        Some(Serie::Select)
    } else if OPS_DE_INSERCAO.contains(&op) {
        Some(Serie::Insert)
    } else if op == "atualizar" {
        Some(Serie::Update)
    } else if op == "excluir" {
        match excluir_fisico {
            true => Some(Serie::ExcluirFisico),
            false => Some(Serie::ExcluirSuave),
        }
    } else if OPS_DE_BACKUP.contains(&op) {
        Some(Serie::Backup)
    } else {
        None
    }
}

/// A FAIXA do aquario (A10) em que a bolha de uma tarefa viva nada:
/// `select`, `insert`, `update`, `delete` ou `outras`.
///
/// Sai da mesma [`serie_pela_op`] da contagem, e nao de uma lista na tela:
/// a bolha de `varrer` na faixa «consulta» e a barra de `select` no grafico
/// tem de concordar, e duas listas divergiriam no dia em que uma operacao
/// nova entrasse numa so (lei «funcao e comando vem do mesmo motor»).
pub fn faixa_da_op(op: &str) -> &'static str {
    match serie_pela_op(op, false) {
        Some(Serie::Select) => "select",
        Some(Serie::Insert) => "insert",
        Some(Serie::Update) => "update",
        Some(Serie::ExcluirSuave) | Some(Serie::ExcluirFisico) => "delete",
        _ => "outras",
    }
}

fn nao_vazio(j: &Json) -> bool {
    match j {
        Json::Nulo => false,
        Json::Bool(b) => *b,
        Json::Numero(_) => true,
        Json::Texto(t) => !t.trim().is_empty(),
        Json::Lista(l) => !l.is_empty(),
        Json::Objeto(o) => !o.is_empty(),
    }
}

/// Oito numeros -- um minuto ou uma hora.
pub type Contas = [u64; 8];

fn contas_para_json(c: &Contas) -> Json {
    Json::objeto(
        Serie::TODAS
            .iter()
            .map(|s| (s.nome(), Json::de_u64(c[s.indice()])))
            .collect(),
    )
}

/// Le as oito do objeto `c` de uma linha. Serie ausente vira zero: a linha e
/// de um minuto MEDIDO, e serie que uma versao mais velha nao conhecia nao
/// aconteceu nele.
fn contas_de_json(j: &Json) -> Contas {
    let mut c = [0; 8];
    for s in Serie::TODAS {
        c[s.indice()] = j.inteiro_ou(s.nome(), 0).max(0) as u64;
    }
    c
}

fn piso(ms: i64, passo: i64) -> i64 {
    ms - ms.rem_euclid(passo)
}

/// A linha por minuto, como o `aquario.log` a recebe (§11.4).
pub fn linha_do_minuto(minuto_ms: i64, c: &Contas) -> Json {
    Json::objeto(vec![
        ("evento", Json::texto_de("contagem")),
        ("minuto_ms", Json::de_i64(minuto_ms)),
        ("c", contas_para_json(c)),
    ])
}

/// Uma hora fechada.
#[derive(Debug, Clone, PartialEq)]
pub struct HoraFechada {
    pub hora_ms: i64,
    pub minutos_medidos: u32,
    pub c: Contas,
}

impl HoraFechada {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("hora", Json::texto_de(instante_iso(self.hora_ms))),
            ("hora_ms", Json::de_i64(self.hora_ms)),
            ("minutos_medidos", Json::de_u64(self.minutos_medidos as u64)),
            ("c", contas_para_json(&self.c)),
        ])
    }

    fn de_json(j: &Json) -> Option<HoraFechada> {
        Some(HoraFechada {
            hora_ms: j.campo("hora_ms")?.inteiro()?,
            minutos_medidos: j.inteiro_ou("minutos_medidos", 0).clamp(0, 60) as u32,
            c: contas_de_json(j.campo("c")?),
        })
    }
}

/// A hora corrente: os minutos ja fechados dela, cada um com as suas contas.
#[derive(Debug, Default)]
struct HoraAberta {
    hora_ms: i64,
    /// Em ordem; minuto que falta e minuto que nao foi medido.
    minutos: Vec<(i64, Contas)>,
}

impl HoraAberta {
    fn acrescentar(&mut self, minuto_ms: i64, c: &Contas) {
        // Dois processos no mesmo minuto (o que caiu e o que subiu) somam:
        // cada um contou pedidos diferentes.
        if let Some((m, ja)) = self.minutos.last_mut() {
            if *m == minuto_ms {
                for (a, b) in ja.iter_mut().zip(c) {
                    *a += b;
                }
                return;
            }
        }
        self.minutos.push((minuto_ms, *c));
    }

    fn fechar(self) -> HoraFechada {
        let mut c = [0; 8];
        for (_, m) in &self.minutos {
            for (a, b) in c.iter_mut().zip(m) {
                *a += b;
            }
        }
        HoraFechada {
            hora_ms: self.hora_ms,
            minutos_medidos: self.minutos.len().min(60) as u32,
            c,
        }
    }
}

#[derive(Debug, Default)]
struct Horas {
    aberta: Option<HoraAberta>,
    /// Fechadas que o disco ainda nao aceitou -- tentadas de novo a cada virada.
    por_gravar: Vec<HoraFechada>,
    perdidas: u64,
    /// Onde gravar. `None` fora do servidor (testes de unidade, ferramentas):
    /// a hora fecha e fica em `por_gravar`, e a consulta diz que nao ha arquivo.
    arquivo: Option<PathBuf>,
}

/// O que a virada de minuto devolve a quem a chamou.
#[derive(Debug, Default)]
pub struct Virada {
    /// A linha do minuto fechado, para o escritor do `aquario.log` (A6).
    pub minuto: Option<Json>,
    /// A gravacao do `aquario-horas.jsonl` recusada -- quem chama a entrega
    /// ao `evento_de_disco`, como o `anotar` faz com o `acessos.log`.
    pub falha: Option<PhxError>,
}

/// O acumulador.
#[derive(Debug, Default)]
pub struct Contagem {
    /// O inicio do minuto corrente, em ms UTC; 0 antes da primeira virada.
    minuto_ms: AtomicI64,
    contas: [AtomicU64; 8],
    horas: Mutex<Horas>,
}

impl Contagem {
    /// Soma um pedido. Chamado so pelo `Aquario::anotar`, atras do portao.
    pub fn somar(&self, acesso: &Acesso) {
        if !acesso.ok {
            self.contas[Serie::Erro.indice()].fetch_add(1, Ordering::Relaxed);
            return;
        }
        if let Some(s) = acesso.desfecho.serie {
            self.contas[s.indice()].fetch_add(1, Ordering::Relaxed);
        }
        if acesso.desfecho.aviso {
            self.contas[Serie::Aviso.indice()].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// As contas do minuto em curso, sem zerar.
    pub fn parcial(&self) -> Contas {
        let mut c = [0; 8];
        for (d, a) in c.iter_mut().zip(&self.contas) {
            *d = a.load(Ordering::Relaxed);
        }
        c
    }

    pub fn definir_arquivo(&self, caminho: PathBuf) {
        self.tomar().arquivo = Some(caminho);
    }

    fn tomar(&self) -> std::sync::MutexGuard<'_, Horas> {
        // Envenenada so por panico no meio de um `push`; o estado continua
        // legivel, e perder a contagem por isso seria pior que o panico.
        self.horas.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A virada, chamada pelo amostrador de segundo em segundo -- so com a
    /// telemetria ligada, e e isso que faz o minuto desligado sair `null`.
    ///
    /// Fecha o minuto quando `agora_ms` ja esta noutro, e a hora quando o
    /// minuto novo ja esta noutra hora. Minuto em que ninguem virou (servidor
    /// fora, amostrador parado) nao gera linha nenhuma: fica ausente.
    ///
    /// O pedido que termina entre a virada do relogio e esta chamada conta no
    /// minuto que fecha -- ate um periodo do amostrador (1 s) de deslize, o
    /// preco de o `anotar` nao olhar o relogio.
    pub fn virar(&self, agora_ms: i64) -> Virada {
        let novo = piso(agora_ms, MINUTO_MS);
        let corrente = self.minuto_ms.load(Ordering::Relaxed);
        if corrente == 0 {
            self.minuto_ms.store(novo, Ordering::Relaxed);
            return Virada::default();
        }
        // Relogio que recuou nao fecha nada: o minuto so anda para a frente.
        if novo <= corrente {
            return Virada::default();
        }
        let mut c = [0; 8];
        for (d, a) in c.iter_mut().zip(&self.contas) {
            *d = a.swap(0, Ordering::Relaxed);
        }
        self.minuto_ms.store(novo, Ordering::Relaxed);

        let mut h = self.tomar();
        let hora = piso(corrente, HORA_MS);
        if h.aberta.as_ref().is_some_and(|a| a.hora_ms != hora) {
            let velha = h.aberta.take().map(HoraAberta::fechar);
            h.por_gravar.extend(velha);
        }
        h.aberta
            .get_or_insert_with(|| HoraAberta {
                hora_ms: hora,
                minutos: Vec::new(),
            })
            .acrescentar(corrente, &c);
        if piso(novo, HORA_MS) > hora {
            let fechada = h.aberta.take().map(HoraAberta::fechar);
            h.por_gravar.extend(fechada);
        }
        let falha = gravar_pendentes(&mut h);
        Virada {
            minuto: Some(linha_do_minuto(corrente, &c)),
            falha,
        }
    }

    /// O arranque no meio da hora: refaz a hora das linhas de minuto que o
    /// processo anterior deixou no `aquario.log`, lidas pelo leitor da A6
    /// ([`super::log::LogDoAquario::contagens_desde`]).
    ///
    /// A hora corrente volta para a memoria; hora ANTERIOR que o processo
    /// velho nao chegou a fechar -- caiu nos ultimos segundos dela -- e
    /// fechada agora, se o `aquario-horas.jsonl` ainda nao a tem. Minuto que o
    /// rodizio comeu fica fora de `minutos_medidos`: parcial, nunca inventado.
    pub fn retomar<'a>(&self, linhas: impl IntoIterator<Item = &'a Json>, agora_ms: i64) {
        let hora_agora = piso(agora_ms, HORA_MS);
        let mut minutos: Vec<(i64, Contas)> = linhas
            .into_iter()
            .filter(|j| j.texto_ou("evento", "") == "contagem")
            .filter_map(|j| {
                Some((
                    j.campo("minuto_ms")?.inteiro()?,
                    contas_de_json(j.campo("c")?),
                ))
            })
            .filter(|(m, _)| *m < hora_agora + HORA_MS)
            .collect();
        minutos.sort_by_key(|(m, _)| *m);

        let mut h = self.tomar();
        let ultima_gravada = h
            .arquivo
            .as_deref()
            .and_then(ultima_hora_do_arquivo)
            .into_iter()
            .chain(h.por_gravar.iter().map(|f| f.hora_ms))
            .max();
        let mut aberta: Option<HoraAberta> = None;
        for (m, c) in minutos {
            let hora = piso(m, HORA_MS);
            if ultima_gravada.is_some_and(|u| hora <= u) {
                continue;
            }
            if aberta.as_ref().is_some_and(|a| a.hora_ms != hora) {
                h.por_gravar.extend(aberta.take().map(HoraAberta::fechar));
            }
            aberta
                .get_or_insert_with(|| HoraAberta {
                    hora_ms: hora,
                    minutos: Vec::new(),
                })
                .acrescentar(m, &c);
        }
        match aberta {
            Some(a) if a.hora_ms < hora_agora => h.por_gravar.push(a.fechar()),
            outra => h.aberta = outra,
        }
        // A gravacao das recuperadas espera a primeira virada, que ja sabe
        // entregar a falha ao `evento_de_disco`.
    }

    /// A consulta `aquario_contagens`: as horas fechadas no intervalo, os
    /// minutos da hora corrente com `null` onde nao houve medida, e o minuto
    /// parcial.
    pub fn consultar(&self, pedido: &Json) -> Result<Json> {
        let desde = pedido.inteiro_ou("desde", i64::MIN);
        let ate = pedido.inteiro_ou("ate", i64::MAX);
        let corrente = self.minuto_ms.load(Ordering::Relaxed);
        let parcial = self.parcial();
        let h = self.tomar();

        let mut horas: Vec<HoraFechada> = match h.arquivo.as_deref() {
            Some(caminho) => ler_horas(caminho)?,
            None => Vec::new(),
        };
        horas.extend(h.por_gravar.iter().cloned());
        horas.retain(|f| f.hora_ms >= desde && f.hora_ms < ate);

        // A hora corrente e a do minuto em curso. Lista de cada minuto dela
        // ate o em curso, exclusive: o que nao foi medido sai `null`, e e a
        // diferenca entre «nada aconteceu» (zeros) e «ninguem estava olhando».
        let hora_corrente = if corrente == 0 {
            Json::Nulo
        } else {
            let hora = piso(corrente, HORA_MS);
            let medidos: &[(i64, Contas)] = match &h.aberta {
                Some(a) if a.hora_ms == hora => &a.minutos,
                _ => &[],
            };
            let minutos = (0..(corrente - hora) / MINUTO_MS)
                .map(|i| {
                    let m = hora + i * MINUTO_MS;
                    medidos
                        .iter()
                        .find(|(x, _)| *x == m)
                        .map_or(Json::Nulo, |(_, c)| contas_para_json(c))
                })
                .collect();
            Json::objeto(vec![
                ("hora_ms", Json::de_i64(hora)),
                ("minutos", Json::Lista(minutos)),
            ])
        };
        let minuto_parcial = if corrente == 0 {
            Json::Nulo
        } else {
            Json::objeto(vec![
                ("minuto_ms", Json::de_i64(corrente)),
                ("c", contas_para_json(&parcial)),
            ])
        };
        Ok(Json::objeto(vec![
            (
                "series",
                Json::Lista(
                    Serie::TODAS
                        .iter()
                        .map(|s| Json::texto_de(s.nome()))
                        .collect(),
                ),
            ),
            (
                "horas",
                Json::Lista(horas.iter().map(HoraFechada::para_json).collect()),
            ),
            ("hora_corrente", hora_corrente),
            ("minuto_parcial", minuto_parcial),
            (
                "arquivo_de_horas",
                h.arquivo
                    .as_deref()
                    .map_or(Json::Nulo, |c| Json::texto_de(c.display().to_string())),
            ),
            ("horas_por_gravar", Json::de_u64(h.por_gravar.len() as u64)),
            ("horas_perdidas", Json::de_u64(h.perdidas)),
        ]))
    }
}

/// Grava as horas fechadas, na ordem, e para na primeira recusa -- a que
/// falhou e as seguintes esperam a proxima virada.
fn gravar_pendentes(h: &mut Horas) -> Option<PhxError> {
    if h.por_gravar.len() > TETO_DE_HORAS_POR_GRAVAR {
        let sobra = h.por_gravar.len() - TETO_DE_HORAS_POR_GRAVAR;
        h.por_gravar.drain(..sobra);
        h.perdidas += sobra as u64;
    }
    let caminho = h.arquivo.clone()?;
    if h.por_gravar.is_empty() {
        return None;
    }
    // Aberto a cada hora, e nao segurado: uma linha por hora nao paga um
    // descritor aberto o dia inteiro. Rodizio desligado (o padrao do
    // `abrir`): o arquivo so acrescenta.
    let mut log = match LogAcessos::abrir(&caminho) {
        Ok(l) => l,
        Err(e) => return Some(e),
    };
    while let Some(f) = h.por_gravar.first() {
        if let Err(e) = log.registrar_json(&f.para_json()) {
            return Some(e);
        }
        h.por_gravar.remove(0);
    }
    None
}

/// As horas do arquivo. Linha ilegivel e pulada: um arquivo com a ultima
/// linha cortada por uma queda ainda deve ser lido.
fn ler_horas(caminho: &Path) -> Result<Vec<HoraFechada>> {
    let texto = match std::fs::read_to_string(caminho) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(texto
        .lines()
        .filter_map(|l| Json::analisar(l).ok())
        .filter_map(|j| HoraFechada::de_json(&j))
        .collect())
}

fn ultima_hora_do_arquivo(caminho: &Path) -> Option<i64> {
    ler_horas(caminho).ok()?.iter().map(|f| f.hora_ms).max()
}

/// O caminho do `aquario-horas.jsonl`, ao lado do `acessos.log` -- a mesma
/// regra do `Diario::ao_lado_de`.
pub fn arquivo_ao_lado_de(log_acessos: &Path) -> PathBuf {
    match log_acessos.parent().filter(|d| !d.as_os_str().is_empty()) {
        Some(dir) => dir.join(ARQUIVO_DE_HORAS),
        None => PathBuf::from(ARQUIVO_DE_HORAS),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    /// A faixa da bolha e a barra do grafico pela MESMA decisao: tirar o
    /// `serie_pela_op` de uma das duas e este teste acusa a divergencia.
    #[test]
    fn a_faixa_da_bolha_concorda_com_a_serie_da_contagem() {
        for (op, faixa) in [
            ("varrer", "select"),
            ("inserir", "insert"),
            ("inserir_lote", "insert"),
            ("atualizar", "update"),
            ("excluir", "delete"),
            ("backup", "outras"),
            ("sql", "outras"),
            ("criar_tabela", "outras"),
        ] {
            assert_eq!(faixa_da_op(op), faixa, "{op}");
        }
        let vazia = Json::objeto(vec![]);
        for op in ["varrer", "inserir", "atualizar", "excluir"] {
            let serie = Desfecho::da_resposta(op, Some(&vazia)).serie.unwrap();
            let nome = serie.nome();
            let esperado = if nome.starts_with("excluir") {
                "delete"
            } else {
                nome
            };
            assert_eq!(faixa_da_op(op), esperado, "{op}");
        }
    }

    /// 2026-10-09 13:00:00 UTC.
    const T0: i64 = 1_791_550_800_000;

    fn pedido(op: &str, ok: bool, resposta: &str) -> Acesso {
        let r = Json::analisar(resposta).unwrap();
        Acesso {
            op: op.into(),
            ok,
            desfecho: Desfecho::da_resposta(op, ok.then_some(&r)),
            ..Acesso::default()
        }
    }

    fn conta(c: &Contas, s: Serie) -> u64 {
        c[s.indice()]
    }

    #[test]
    fn t0_e_uma_hora_cheia() {
        assert_eq!(T0 % HORA_MS, 0);
        assert_eq!(instante_iso(T0), "2026-10-09 13:00:00,000");
    }

    /// RED: o excluir conta pelo `modo` da RESPOSTA. Uma tabela sem marca
    /// exclui fisico sem `"fisico"` no pedido; quem lesse a bandeira (ou
    /// caisse no padrao suave) contaria suave, e este teste cai.
    #[test]
    fn excluir_conta_pelo_modo_que_a_resposta_devolve() {
        let fisico = Desfecho::da_resposta(
            "excluir",
            Some(&Json::analisar(r#"{"rowid":1,"excluido":true,"modo":"fisico"}"#).unwrap()),
        );
        assert_eq!(fisico.serie, Some(Serie::ExcluirFisico));
        let suave = Desfecho::da_resposta(
            "excluir",
            Some(&Json::analisar(r#"{"rowid":1,"modo":"suave"}"#).unwrap()),
        );
        assert_eq!(suave.serie, Some(Serie::ExcluirSuave));
        // O DELETE do SQL: a op que atendeu vem na resposta, e o modo junto.
        let sql = Desfecho::da_resposta(
            "sql",
            Some(&Json::analisar(r#"{"op":"excluir","afetadas":1,"modo":"fisico"}"#).unwrap()),
        );
        assert_eq!(sql.serie, Some(Serie::ExcluirFisico));
    }

    /// RED: o backup que falhou sobe so `erro`; a barra de backup fica parada.
    #[test]
    fn backup_que_falhou_sobe_so_erro() {
        let c = Contagem::default();
        c.somar(&pedido("backup", false, "null"));
        // Mesmo um desfecho com serie (quem o calculasse sem olhar o `ok`)
        // nao sobe a barra de um pedido que falhou.
        c.somar(&Acesso {
            op: "backup".into(),
            ok: false,
            desfecho: Desfecho {
                serie: Some(Serie::Backup),
                aviso: true,
            },
            ..Acesso::default()
        });
        let p = c.parcial();
        assert_eq!(conta(&p, Serie::Erro), 2);
        assert_eq!(conta(&p, Serie::Backup), 0);
        assert_eq!(conta(&p, Serie::Aviso), 0);
        c.somar(&pedido("backup", true, r#"{"destino":"/x"}"#));
        assert_eq!(conta(&c.parcial(), Serie::Backup), 1);
    }

    #[test]
    fn aviso_conta_tambem_na_barra_da_categoria() {
        let c = Contagem::default();
        c.somar(&pedido("inserir", true, r#"{"rowid":3,"aviso":"truncou"}"#));
        c.somar(&pedido("atualizar", true, r#"{"avisos":["a"]}"#));
        c.somar(&pedido("atualizar", true, r#"{"avisos":[],"aviso":""}"#));
        // Aninhado nao e o nivel de cima: nao conta.
        c.somar(&pedido("ler", true, r#"{"linha":{"aviso":"x"}}"#));
        let p = c.parcial();
        assert_eq!(conta(&p, Serie::Insert), 1);
        assert_eq!(conta(&p, Serie::Update), 2);
        assert_eq!(conta(&p, Serie::Select), 1);
        assert_eq!(conta(&p, Serie::Aviso), 2);
        assert_eq!(conta(&p, Serie::Erro), 0);
    }

    #[test]
    fn sql_conta_pela_op_que_atendeu_e_o_resto_nao_tem_barra() {
        let d = |op: &str, r: &str| Desfecho::da_resposta(op, Some(&Json::analisar(r).unwrap()));
        assert_eq!(d("sql", r#"{"op":"varrer"}"#).serie, Some(Serie::Select));
        assert_eq!(d("sql", r#"{"op":"consultar"}"#).serie, Some(Serie::Select));
        assert_eq!(d("sql", r#"{"op":"inserir"}"#).serie, Some(Serie::Insert));
        assert_eq!(d("sql", r#"{"op":"atualizar"}"#).serie, Some(Serie::Update));
        // DELETE por faixa nao traz modo: o derivado e suave.
        assert_eq!(
            d("sql", r#"{"op":"excluir"}"#).serie,
            Some(Serie::ExcluirSuave)
        );
        assert_eq!(d("sql", r#"{"op":"begin"}"#).serie, None);
        assert_eq!(d("ping", "{}").serie, None);
        assert_eq!(
            d("inserir_lote", r#"{"inseridas":5000}"#).serie,
            Some(Serie::Insert)
        );
        // Um `sql` com a palavra `varrer` em outro campo nao vira select.
        assert_eq!(d("sql", r#"{"sql":"varrer"}"#).serie, None);
    }

    /// As listas nao inventam operacao: todo nome delas esta no catalogo.
    #[test]
    fn as_listas_so_nomeiam_operacoes_do_catalogo() {
        let conhecidas: Vec<&str> = crate::catalogo::OPERACOES
            .iter()
            .flat_map(|o| o.nomes())
            .chain(["backup_agendado"])
            .collect();
        for n in OPS_DE_LEITURA
            .iter()
            .chain(OPS_DE_INSERCAO)
            .chain(OPS_DE_BACKUP)
            .chain(&["atualizar", "excluir"])
        {
            assert!(conhecidas.contains(n), "{n} nao e operacao do catalogo");
        }
    }

    #[test]
    fn zero_e_uma_linha_de_zeros_e_nao_ausencia() {
        let c = Contagem::default();
        assert!(
            c.virar(T0 + 10).minuto.is_none(),
            "a primeira virada so arma"
        );
        let v = c.virar(T0 + MINUTO_MS + 5);
        let linha = v.minuto.expect("minuto sem nada tem de sair com zeros");
        assert_eq!(linha.inteiro_ou("minuto_ms", 0), T0);
        let cs = linha.campo("c").unwrap();
        for s in Serie::TODAS {
            assert_eq!(cs.campo(s.nome()), Some(&Json::de_u64(0)), "{}", s.nome());
        }
        // A mesma virada de novo, no mesmo minuto, nao fecha outro.
        assert!(c.virar(T0 + MINUTO_MS + 900).minuto.is_none());
        // Relogio que recua nao fecha nada.
        assert!(c.virar(T0).minuto.is_none());
    }

    /// RED: servidor fora 2 minutos = 2 `null`, e a hora sai com 58 minutos
    /// medidos -- nunca zero inventado. Pelo arranque de verdade: o processo
    /// novo nasce sem memoria e refaz a hora das linhas de minuto do velho.
    #[test]
    fn dois_minutos_fora_do_ar_sao_dois_null_e_58_medidos() {
        let dir = DirTemp::novo("aq-contagem-fora");
        let arquivo = dir.join(ARQUIVO_DE_HORAS);

        // O processo velho: minutos 0..10, um select em cada.
        let velho = Contagem::default();
        velho.definir_arquivo(arquivo.clone());
        let mut log = Vec::new();
        velho.virar(T0);
        for m in 1..=10 {
            velho.somar(&pedido("varrer", true, "{}"));
            log.extend(velho.virar(T0 + m * MINUTO_MS + 1).minuto);
        }
        assert_eq!(log.len(), 10);
        // Cai no minuto 10 e volta no 12: os minutos 10 e 11 ninguem viu.
        let novo = Contagem::default();
        novo.definir_arquivo(arquivo.clone());
        novo.retomar(&log, T0 + 12 * MINUTO_MS);
        novo.virar(T0 + 12 * MINUTO_MS + 1);
        let r = novo.consultar(&Json::Nulo).unwrap();
        let mins = r.campo("hora_corrente").unwrap().campo("minutos").unwrap();
        let mins = mins.lista().unwrap();
        assert_eq!(mins.len(), 12);
        assert!(mins[..10].iter().all(|m| m.inteiro_ou("select", -1) == 1));
        assert!(mins[10].e_nulo() && mins[11].e_nulo(), "{}", r.escrever());

        // Vai ate o fim da hora, e a hora fecha com 58 medidos.
        for m in 13..=60 {
            novo.virar(T0 + m * MINUTO_MS + 1);
        }
        let horas = ler_horas(&arquivo).unwrap();
        assert_eq!(horas.len(), 1, "{horas:?}");
        assert_eq!(horas[0].hora_ms, T0);
        assert_eq!(horas[0].minutos_medidos, 58);
        assert_eq!(conta(&horas[0].c, Serie::Select), 10);
    }

    /// O mesmo, dentro de um processo so: o amostrador que nao virou por dois
    /// minutos (telemetria desligada) deixa dois buracos, nao dois zeros.
    #[test]
    fn minuto_sem_virada_fica_ausente() {
        let c = Contagem::default();
        c.virar(T0);
        c.virar(T0 + MINUTO_MS);
        c.virar(T0 + 4 * MINUTO_MS);
        let r = c.consultar(&Json::Nulo).unwrap();
        let mins = r.campo("hora_corrente").unwrap().campo("minutos").unwrap();
        let nulos: Vec<bool> = mins.lista().unwrap().iter().map(Json::e_nulo).collect();
        assert_eq!(nulos, vec![false, false, true, true]);
    }

    /// A hora fecha na virada para a hora seguinte e vai ao arquivo; a
    /// consulta a devolve pelo intervalo.
    #[test]
    fn a_hora_fecha_no_arquivo_e_volta_na_consulta() {
        let dir = DirTemp::novo("aq-contagem-hora");
        let c = Contagem::default();
        c.definir_arquivo(dir.join(ARQUIVO_DE_HORAS));
        c.virar(T0 + 59 * MINUTO_MS);
        c.somar(&pedido("excluir", true, r#"{"modo":"fisico"}"#));
        c.somar(&pedido("excluir", false, "null"));
        let v = c.virar(T0 + HORA_MS + 2);
        assert!(v.falha.is_none());
        let r = c.consultar(&Json::Nulo).unwrap();
        let horas = r.campo("horas").unwrap().lista().unwrap();
        assert_eq!(horas.len(), 1);
        assert_eq!(horas[0].inteiro_ou("minutos_medidos", 0), 1);
        let cs = horas[0].campo("c").unwrap();
        assert_eq!(cs.inteiro_ou("excluir_fisico", 0), 1);
        assert_eq!(cs.inteiro_ou("excluir_suave", -1), 0);
        assert_eq!(cs.inteiro_ou("erro", 0), 1);
        // Fora do intervalo pedido, nao vem.
        let r = c
            .consultar(&Json::analisar(&format!(r#"{{"desde":{}}}"#, T0 + 1)).unwrap())
            .unwrap();
        assert!(r.campo("horas").unwrap().lista().unwrap().is_empty());
        // A hora corrente nova ainda nao tem minuto fechado.
        let r = c.consultar(&Json::Nulo).unwrap();
        let hc = r.campo("hora_corrente").unwrap();
        assert_eq!(hc.inteiro_ou("hora_ms", 0), T0 + HORA_MS);
        assert!(hc.campo("minutos").unwrap().lista().unwrap().is_empty());
    }

    /// O disco que recusa nao perde a hora: ela espera e entra na proxima
    /// virada que conseguir gravar.
    #[test]
    fn hora_recusada_pelo_disco_espera_a_proxima_virada() {
        let dir = DirTemp::novo("aq-contagem-recusa");
        // Um ARQUIVO no lugar do diretorio: o `abrir` recusa.
        let bloqueio = dir.join("bloqueio");
        std::fs::write(&bloqueio, b"x").unwrap();
        let c = Contagem::default();
        c.definir_arquivo(bloqueio.join(ARQUIVO_DE_HORAS));
        c.virar(T0 + 59 * MINUTO_MS);
        let v = c.virar(T0 + HORA_MS);
        assert!(v.falha.is_some(), "a recusa tem de chegar a quem virou");
        let r = c.consultar(&Json::Nulo);
        // A consulta le o arquivo; o caminho torto tambem recusa ali.
        assert!(r.is_err() || r.unwrap().inteiro_ou("horas_por_gravar", 0) == 1);

        let bom = dir.join(ARQUIVO_DE_HORAS);
        c.definir_arquivo(bom.clone());
        let v = c.virar(T0 + HORA_MS + MINUTO_MS);
        assert!(v.falha.is_none());
        assert_eq!(ler_horas(&bom).unwrap().len(), 1);
    }

    /// O processo que caiu nos ultimos segundos da hora nao a fechou; o
    /// arranque seguinte a fecha das linhas de minuto, e so uma vez.
    #[test]
    fn retomar_fecha_a_hora_que_o_processo_velho_nao_fechou() {
        let dir = DirTemp::novo("aq-contagem-retomar");
        let arquivo = dir.join(ARQUIVO_DE_HORAS);
        let linhas: Vec<Json> = (0..3)
            .map(|m| {
                let mut c = [0; 8];
                c[Serie::Insert.indice()] = 2;
                linha_do_minuto(T0 + (57 + m) * MINUTO_MS, &c)
            })
            .collect();
        for _ in 0..2 {
            let c = Contagem::default();
            c.definir_arquivo(arquivo.clone());
            c.retomar(&linhas, T0 + HORA_MS + 5 * MINUTO_MS);
            c.virar(T0 + HORA_MS + 5 * MINUTO_MS);
            c.virar(T0 + HORA_MS + 6 * MINUTO_MS);
        }
        let horas = ler_horas(&arquivo).unwrap();
        assert_eq!(horas.len(), 1, "a hora foi gravada duas vezes: {horas:?}");
        assert_eq!(horas[0].minutos_medidos, 3);
        assert_eq!(conta(&horas[0].c, Serie::Insert), 6);
    }
}

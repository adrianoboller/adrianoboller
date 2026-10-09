//! A camada de ocorrencias (pedido 495, fatia F2): o FATO de cada alarme,
//! redigido, num arquivo que so quem administra le.
//!
//! Desenho em `docs/propostas/ia-495-496-desenho.md` (linha F2, §11) e
//! `docs/propostas/aquario-707.md` §11.3. O que este arquivo decide:
//!
//! * **O tipo da `Ocorrencia` e o [`Alarme`]** do 707, e nao um enum
//!   proprio: dois enums responderiam duas vezes «isto e grave?».
//! * **Um produtor:** so o `telemetria::sinal` (A3) cria ocorrencia, pelo
//!   [`Ocorrencias::receber`]. Nenhum outro caminho chama este arquivo para
//!   PRODUZIR.
//! * **Um carteiro:** a fila e o `Condvar` que moravam na
//!   `saude_do_disco.rs` sairam de la e moram aqui, no [`Correio`]. A saude
//!   do disco virou o PRIMEIRO cliente dele (as cartas [`Carta::Saude`]); a
//!   ocorrencia e o segundo. A thread `sonda-disco` continua sendo o unico
//!   carteiro -- uma fila so, uma thread so, e quem entrega (com a trava de
//!   dados na mao, as vezes) nunca fala com disco nem com rede.
//! * **Silencio antes de tudo:** `jobs::pode_avisar` por (alarme, usuario,
//!   IP), com teto no mapa e uma chave-coringa, para quem inunda nao crescer
//!   a memoria do servidor. O silencio vem ANTES da redacao: a enxurrada
//!   calada nao paga analise de JSON.
//! * **Redige ANALISANDO, nunca recortando** (lei da casa): o `dados` que
//!   chega e analisado. Pedido JSON vira a FORMA dele
//!   ([`crate::profiler::forma_do_pedido`] -- o mesmo motor do Profiler, com
//!   o SQL normalizado e todo valor trocado por `?`); texto que nao e JSON
//!   passa pelo [`phxsql_sql::usuario::normalizado`], e o que nao se analisa
//!   vira o tamanho. O corte de tamanho so acontece DEPOIS, sobre o texto ja
//!   redigido.
//!
//! # Dois arquivos, dois papeis (§11.3, Hb3)
//!
//! O `aquario.log` e lido por quem tem `monitorar` e pela TV, e nunca leva
//! login nem IP. O `ocorrencias.log` leva os dois, porque e o fato de
//! seguranca, e por isso so quem ADMINISTRA o le: nasce 0600 pelo motor de
//! permissao do banco, e so a op `ocorrencias` (F9) o devolve, atras de
//! `administrar` na regra do SERVIDOR (`OPS_DO_SERVIDOR`, o furo do 756).
//! Um arquivo so para os dois obrigaria um filtro na
//! leitura, e o filtro esquecido vazaria o IP.
//!
//! O ESCRITOR e o mesmo dos dois (e do `acessos.log`): o
//! [`crate::aquario::log::LogDoAquario`], sobre o `LogAcessos::registrar_json`
//! -- abrir 0600, girar por tamanho, contar a falha.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::Duration;

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::aquario::log::LogDoAquario;
use crate::aquario::Alarme;
use crate::telemetria::Atividade;

/// O nome do arquivo, ao lado do `acessos.log`.
pub const NOME_DO_ARQUIVO: &str = "ocorrencias.log";

/// Quantas cartas a fila do carteiro segura quando ninguem a esvazia.
///
/// Era o `TETO_DA_FILA` da saude do disco, e continua o mesmo numero: sem
/// carteiro (o servidor dos testes, que nao sobe a thread) ninguem tira da
/// fila, e ela cresceria para sempre.
pub const TETO_DA_FILA: usize = 32;

/// A janela do silencio por (alarme, usuario, IP). *Raciocinado:* o arquivo
/// e registro, e nao canal de aviso -- um minuto basta para a forca bruta de
/// mil tentativas virar uma linha por minuto, e nao mil.
const SILENCIO_MS: i64 = 60_000;

/// Quantas chaves o silencio guarda. Acima disto, vencidas saem; e se ainda
/// nao couber, a chave nova cai na coringa do alarme -- quem varia IP e
/// usuario para inundar nao apaga o silencio dos outros nem cresce a memoria.
const TETO_DO_SILENCIO: usize = 1_024;

/// Teto do `dados` redigido, em caracteres, DEPOIS da redacao. Linha maior
/// que 64 KiB o leitor do escritor comum pula como lixo.
const TETO_DOS_DADOS: usize = 4_096;

/// Quantas tabelas uma ocorrencia nomeia.
const TETO_DAS_TABELAS: usize = 64;

// ------------------------------------------------------------- o correio

/// O que o carteiro leva. Uma fila so para as duas familias: duas filas
/// pediriam dois `Condvar` -- e dois carteiros, porque uma thread nao dorme
/// em dois.
#[derive(Debug)]
pub enum Carta {
    /// Um evento da saude do disco que passou pelo silencio dela.
    Saude(crate::saude_do_disco::Evento),
    /// Uma ocorrencia que passou pelo silencio daqui.
    Ocorrencia(Ocorrencia),
}

impl Carta {
    fn mesma_familia(&self, outra: &Carta) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(outra)
    }
}

/// A fila e o carteiro -- extraidos da `saude_do_disco.rs` (F2).
///
/// Quem entrega pode estar com a trava de dados na mao (o fecho da janela
/// esta, a trava reentrante esta); quem tira nunca esta. Entregar e so
/// empurrar e acordar.
pub struct Correio {
    fila: Mutex<VecDeque<Carta>>,
    /// Acorda o carteiro na hora em que uma carta entra -- «imediato» nao
    /// espera o relogio da sonda.
    carteiro: Condvar,
}

impl Default for Correio {
    fn default() -> Self {
        Correio::novo()
    }
}

impl Correio {
    pub fn novo() -> Correio {
        Correio {
            fila: Mutex::new(VecDeque::new()),
            carteiro: Condvar::new(),
        }
    }

    /// Poe a carta na fila e acorda o carteiro. Barato e sem rede.
    ///
    /// Cheia, sai a mais velha da MESMA familia: uma enxurrada de ocorrencias
    /// nao empurra para fora o aviso de disco que espera o carteiro (e vice-
    /// -versa). Sem nenhuma da familia, sai a mais velha de todas.
    pub fn entregar(&self, carta: Carta) {
        if let Ok(mut fila) = self.fila.lock() {
            if fila.len() >= TETO_DA_FILA {
                let i = fila
                    .iter()
                    .position(|c| c.mesma_familia(&carta))
                    .unwrap_or(0);
                fila.remove(i);
            }
            fila.push_back(carta);
        }
        self.carteiro.notify_one();
    }

    /// Espera ate `ate` por uma carta que `quer` aceite, e devolve todas as
    /// que ele aceitar -- vazio quando o prazo venceu sem nada. As outras
    /// ficam na fila, na ordem em que estavam.
    ///
    /// O carteiro de verdade aceita tudo; o filtro existe para quem so
    /// enxerga uma familia (o `SaudeDoDisco::esperar`) nao engolir a carta
    /// da outra.
    pub fn retirar(&self, ate: Duration, quer: impl Fn(&Carta) -> bool) -> Vec<Carta> {
        let Ok(mut fila) = self.fila.lock() else {
            return Vec::new();
        };
        if !fila.iter().any(&quer) {
            // Um despertar espurio (ou uma carta da outra familia) devolve
            // vazio, e o laco de fora volta a esperar: nada se perde.
            match self.carteiro.wait_timeout(fila, ate) {
                Ok((guarda, _)) => fila = guarda,
                Err(_) => return Vec::new(),
            }
        }
        let mut fica = VecDeque::with_capacity(fila.len());
        let mut sai = Vec::new();
        for c in fila.drain(..) {
            if quer(&c) {
                sai.push(c);
            } else {
                fica.push_back(c);
            }
        }
        *fila = fica;
        sai
    }

    /// Quantas cartas que `quer` aceita esperam o carteiro.
    pub fn contar(&self, quer: impl Fn(&Carta) -> bool) -> usize {
        self.fila
            .lock()
            .map(|f| f.iter().filter(|c| quer(c)).count())
            .unwrap_or(0)
    }
}

// ------------------------------------------------------------ a ocorrencia

/// Um alarme registrado: o que, quando, quem, de onde e sobre o que.
///
/// `dados` ja chega redigido -- nao ha campo que guarde o texto cru, entao
/// nao ha como grava-lo por engano.
#[derive(Debug, Clone)]
pub struct Ocorrencia {
    pub id: u64,
    pub quando_ms: i64,
    pub alarme: Alarme,
    /// A atividade (`dados:17`), quando havia uma.
    pub tarefa: String,
    pub usuario: String,
    pub ip: String,
    pub op: String,
    pub database: String,
    pub tabela: String,
    /// Toda tabela que o pedido nomeia, pela arvore inteira -- inclusive o
    /// lado B de um `juntar`, que nao esta no campo `tabela`.
    pub tabelas: Vec<(String, String)>,
    /// A digital do SQL (F1), quando havia SQL.
    pub digital: Option<u64>,
    /// As classes do observador de injecao (F3) que acusaram o pedido.
    /// Vazio para todo outro alarme. Sao nomes fixos do motor
    /// (`Sinais::TODAS`), nunca texto do cliente: nao passam pela redacao.
    pub sinais: phxsql_sql::Sinais,
    /// O `dados` do produtor, REDIGIDO.
    pub dados: Json,
}

impl Ocorrencia {
    /// A linha do arquivo. Campo vazio nao entra; campo que veio do cliente
    /// e reduzido a uma linha (o furo da linha forjada do Profiler).
    pub fn para_json(&self) -> Json {
        let livre = |s: &str| {
            Json::texto_de(crate::profiler::de_uma_linha(
                s,
                crate::profiler::TETO_DO_CAMPO,
            ))
        };
        let mut pares = vec![
            ("quando", Json::texto_de(instante_iso(self.quando_ms))),
            ("quando_ms", Json::de_i64(self.quando_ms)),
            ("id", Json::de_u64(self.id)),
            ("alarme", Json::texto_de(self.alarme.nome())),
            ("gravidade", Json::texto_de(nome_da_gravidade(self.alarme))),
            ("grupo", Json::texto_de(self.alarme.grupo().nome())),
        ];
        for (nome, valor) in [
            ("tarefa", &self.tarefa),
            ("usuario", &self.usuario),
            ("ip", &self.ip),
            ("op", &self.op),
            ("database", &self.database),
            ("tabela", &self.tabela),
        ] {
            if !valor.is_empty() {
                pares.push((nome, livre(valor)));
            }
        }
        if !self.tabelas.is_empty() {
            pares.push((
                "tabelas",
                Json::Lista(
                    self.tabelas
                        .iter()
                        .map(|(d, t)| {
                            Json::objeto(vec![("database", livre(d)), ("tabela", livre(t))])
                        })
                        .collect(),
                ),
            ));
        }
        if let Some(d) = self.digital {
            // Em hexadecimal: u64 nao cabe num numero JSON sem perder bits.
            pares.push(("digital", Json::texto_de(format!("{d:016x}"))));
        }
        if !self.sinais.vazio() {
            pares.push((
                "sinais",
                Json::Lista(self.sinais.nomes().map(Json::texto_de).collect()),
            ));
        }
        if !matches!(&self.dados, Json::Texto(t) if t.is_empty()) {
            pares.push(("dados", self.dados.clone()));
        }
        Json::objeto(pares)
    }
}

/// A redacao do `dados`: a forma, as tabelas e a digital.
///
/// Pedido JSON vira a FORMA pelo motor do Profiler; texto vira o
/// [`phxsql_sql::usuario::normalizado`] (todo literal vira `?`, comentario
/// some, o que nao se analisa vira o tamanho). E so DEPOIS da redacao o
/// tamanho se corta -- cortar antes seria o recorte que a lei proibe, e o
/// recorte e exatamente o que deixa passar a senha que esta depois do corte
/// ou antes dele.
fn redigir(dados: &str, database: &str) -> (Json, Vec<(String, String)>, Option<u64>) {
    let (forma, tabelas, digital) = match crate::profiler::forma_do_pedido(dados, database) {
        Some((forma, tabelas)) => (forma, tabelas, None),
        None => {
            let digital = phxsql_sql::lexico::analisar_com_comentarios(dados)
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| phxsql_sql::digital(&s));
            (
                Json::texto_de(phxsql_sql::usuario::normalizado(dados)),
                Vec::new(),
                digital,
            )
        }
    };
    let escrito = forma.escrever();
    let forma = if escrito.chars().nth(TETO_DOS_DADOS).is_some() {
        Json::texto_de(crate::profiler::de_uma_linha(&escrito, TETO_DOS_DADOS))
    } else {
        forma
    };
    let mut tabelas = tabelas;
    tabelas.truncate(TETO_DAS_TABELAS);
    (forma, tabelas, digital)
}

// ---------------------------------------------------------------- a camada

/// O mutex envenenado nao derruba a camada: o que ela guarda sao horas e
/// cartas, e nenhuma fica pela metade num panico.
fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A camada: silencio, numeracao, a entrega ao [`Correio`] e o escritor do
/// `ocorrencias.log`. Uma por servidor.
pub struct Ocorrencias {
    correio: Arc<Correio>,
    silencio: Mutex<HashMap<String, i64>>,
    /// Semeado com o relogio (ms × 1000): o id nao se repete entre dois
    /// arranques, e o `aquario.log` que o cita aponta uma linha so.
    proximo_id: AtomicU64,
    log: LogDoAquario,
    /// Ocorrencias que o silencio calou -- todas contam, mesmo as caladas.
    caladas: AtomicU64,
}

impl Ocorrencias {
    /// Sobre um correio que ja existe: o da saude do disco, para o carteiro
    /// ser um so.
    pub fn nova(correio: Arc<Correio>) -> Ocorrencias {
        Ocorrencias {
            correio,
            silencio: Mutex::new(HashMap::new()),
            proximo_id: AtomicU64::new((crate::agora_ms().max(0) as u64).saturating_mul(1_000)),
            log: LogDoAquario::default(),
            caladas: AtomicU64::new(0),
        }
    }

    /// Abre o `ocorrencias.log`, com o rodizio do aquario (8 MiB × 8).
    pub fn abrir(&self, caminho: impl AsRef<Path>) -> Result<()> {
        self.log.abrir(caminho)
    }

    pub fn correio(&self) -> &Arc<Correio> {
        &self.correio
    }

    pub fn caminho(&self) -> Option<std::path::PathBuf> {
        self.log.caminho()
    }

    pub fn gravadas(&self) -> u64 {
        self.log.gravadas()
    }

    pub fn falhas(&self) -> u64 {
        self.log.falhas()
    }

    pub fn caladas(&self) -> u64 {
        self.caladas.load(Ordering::Relaxed)
    }

    /// O corpo do produtor: silencio, redacao, numero e entrega. Devolve o
    /// id quando a ocorrencia foi entregue, `None` quando o silencio a calou.
    ///
    /// SO o `aquario::alarme::produzir` chama isto -- e o «um produtor» do
    /// §11.3. Pode estar debaixo da trava de dados: aqui so ha mutex de
    /// folha e um `notify`, nada de disco nem de rede.
    pub fn receber(
        &self,
        alarme: Alarme,
        dados: &str,
        atividade: Option<&Atividade>,
        agora_ms: i64,
    ) -> Option<u64> {
        self.receber_com_sinais(
            alarme,
            dados,
            phxsql_sql::Sinais::NENHUM,
            atividade,
            agora_ms,
        )
    }

    /// O mesmo corpo, com as classes do observador de injecao (F3). Um
    /// corpo so: o [`receber`](Self::receber) e este com as classes vazias.
    pub fn receber_com_sinais(
        &self,
        alarme: Alarme,
        dados: &str,
        sinais: phxsql_sql::Sinais,
        atividade: Option<&Atividade>,
        agora_ms: i64,
    ) -> Option<u64> {
        let ctx = atividade.map(Atividade::contexto).unwrap_or_default();
        let ip = atividade.map(|a| a.ip.clone()).unwrap_or_default();
        if !self.pode(alarme, &ctx.usuario, &ip, agora_ms) {
            self.caladas.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let (dados, mut tabelas, digital) = redigir(dados, &ctx.database);
        if tabelas.is_empty() && !ctx.tabela.is_empty() {
            tabelas.push((ctx.database.clone(), ctx.tabela.clone()));
        }
        let id = self.proximo_id.fetch_add(1, Ordering::Relaxed);
        self.correio.entregar(Carta::Ocorrencia(Ocorrencia {
            id,
            quando_ms: agora_ms,
            alarme,
            tarefa: atividade.map(|a| a.chave.clone()).unwrap_or_default(),
            usuario: ctx.usuario,
            ip,
            op: ctx.op,
            database: ctx.database,
            tabela: ctx.tabela,
            tabelas,
            digital: ctx.digital.or(digital),
            sinais,
            dados,
        }));
        Some(id)
    }

    /// O silencio por (alarme, usuario, IP), com teto e coringa.
    fn pode(&self, alarme: Alarme, usuario: &str, ip: &str, agora_ms: i64) -> bool {
        let mut s = travar(&self.silencio);
        // Uma vaga por alarme fica para a coringa dele: assim o mapa nunca
        // passa do teto, nem com todas as coringas de pe.
        let das_comuns = TETO_DO_SILENCIO - Alarme::TODOS.len();
        let mut chave = format!("{}\u{0}{usuario}\u{0}{ip}", alarme.nome());
        if s.len() >= das_comuns && !s.contains_key(&chave) {
            s.retain(|_, quando| agora_ms - *quando < SILENCIO_MS);
            if s.len() >= das_comuns {
                chave = format!("{}\u{0}*", alarme.nome());
            }
        }
        crate::jobs::pode_avisar(&mut s, &chave, agora_ms, SILENCIO_MS)
    }

    /// Grava uma ocorrencia. So o carteiro chama, fora de toda trava. A
    /// falha volta para quem chamou a levar a saude do disco.
    pub fn gravar(&self, o: &Ocorrencia) -> Result<()> {
        self.log.gravar_json(&o.para_json())
    }

    /// A op `ocorrencias` (pedido 495, F9): o arquivo lido pela MESMA
    /// consulta do `aquario_log` -- periodo (`desde`/`ate`), `max` --, e o
    /// filtro `alarme` (um nome, ou uma lista).
    ///
    /// # O que volta ja e redigido
    ///
    /// Nada se redige aqui: a linha ja nasceu redigida na F2 (o `dados` passou
    /// pela forma do Profiler antes de existir no disco). Redigir de novo na
    /// leitura seria a mesma decisao escrita duas vezes, e a segunda copia e
    /// a que alguem «conserta» sem mexer na primeira.
    ///
    /// # Nome de alarme desconhecido e ERRO
    ///
    /// E nao «nenhuma linha»: `"alarme":"forca_brutta"` devolvendo vazio diria
    /// ao administrador que ninguem tentou senha nenhuma.
    ///
    /// # A lista dos alarmes vai junto
    ///
    /// `alarmes` traz nome, gravidade e grupo de todos, na ordem do
    /// [`Alarme::TODOS`]: o filtro da tela sai do codigo, e nao de uma lista
    /// digitada no JavaScript que envelheceria no primeiro alarme novo.
    pub fn consultar(&self, pedido: &Json) -> Result<Json> {
        let forma =
            || PhxError::Tipo("ocorrencias: \"alarme\" e um nome ou uma lista de nomes".into());
        let nomes: Vec<String> = match pedido.campo("alarme") {
            None | Some(Json::Nulo) => Vec::new(),
            Some(Json::Texto(t)) if t.is_empty() => Vec::new(),
            Some(Json::Texto(t)) => vec![t.clone()],
            Some(Json::Lista(l)) => l
                .iter()
                .map(|v| v.texto().map(String::from).ok_or_else(forma))
                .collect::<Result<_>>()?,
            Some(_) => return Err(forma()),
        };
        if let Some(errado) = nomes.iter().find(|n| Alarme::de_nome(n).is_none()) {
            return Err(PhxError::Tipo(format!(
                "ocorrencias: alarme desconhecido \"{}\"; os que existem: {}",
                crate::profiler::de_uma_linha(errado, crate::profiler::TETO_DO_CAMPO),
                Alarme::TODOS.map(Alarme::nome).join(", ")
            )));
        }
        let mut r = self
            .log
            .consultar_como("ocorrencias", NOME_DO_ARQUIVO, pedido, |j| {
                nomes.is_empty() || nomes.iter().any(|n| j.texto_ou("alarme", "") == n)
            })?;
        r.definir(
            "alarmes",
            Json::Lista(
                Alarme::TODOS
                    .iter()
                    .map(|a| {
                        Json::objeto(vec![
                            ("nome", Json::texto_de(a.nome())),
                            ("gravidade", Json::texto_de(nome_da_gravidade(*a))),
                            ("grupo", Json::texto_de(a.grupo().nome())),
                        ])
                    })
                    .collect(),
            ),
        );
        Ok(r)
    }
}

/// O nome da gravidade como a linha do arquivo o grava -- um lugar so, para
/// a linha e a lista do filtro nunca discordarem.
pub fn nome_da_gravidade(a: Alarme) -> &'static str {
    match a.gravidade() {
        crate::aquario::Gravidade::Vermelho => "vermelho",
        crate::aquario::Gravidade::Amarelo => "amarelo",
    }
}

// --------------------------------------------- o destino de quem nao tem

/// A camada do processo, para o alarme que nenhuma atividade carrega.
///
/// Pelo mesmo motivo do sedimento (`aquario::alarme`): as origens dos
/// alarmes de servidor sao funcoes livres e lacos de fundo que nao tem o
/// servidor na mao. O alarme de uma TAREFA vai a camada do servidor da
/// tarefa (a atividade a carrega, [`Atividade::ocorrencias`]); so o que nao
/// tem atividade cai aqui. `Weak`: servidor que caiu nao fica vivo por isto.
static DO_PROCESSO: Mutex<Weak<Ocorrencias>> = Mutex::new(Weak::new());

/// O servidor diz que e o dono das ocorrencias sem tarefa deste processo.
pub fn instalar(o: &Arc<Ocorrencias>) {
    *travar(&DO_PROCESSO) = Arc::downgrade(o);
}

/// A camada do processo, se houver servidor de pe.
pub fn do_processo() -> Option<Arc<Ocorrencias>> {
    travar(&DO_PROCESSO).upgrade()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use crate::telemetria::Telemetria;

    const SENTINELA: &str = "SENTINELA-495-f2-x9q";

    fn camada(dir: &Path) -> Arc<Ocorrencias> {
        let o = Arc::new(Ocorrencias::nova(Arc::new(Correio::novo())));
        o.abrir(dir.join(NOME_DO_ARQUIVO)).unwrap();
        o
    }

    /// O que o carteiro faz, sem a thread: tira tudo e grava.
    fn carteiro_uma_volta(o: &Ocorrencias) -> usize {
        let mut n = 0;
        for c in o.correio().retirar(Duration::ZERO, |_| true) {
            if let Carta::Ocorrencia(oc) = c {
                o.gravar(&oc).unwrap();
                n += 1;
            }
        }
        n
    }

    /// Conta o simbolo do `Condvar` nas linhas de CODIGO (comentario fora).
    /// A agulha e montada aqui para este proprio teste nao se contar.
    fn condvars(fonte: &str) -> usize {
        let agulha = ["Cond", "var::new"].concat();
        fonte
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .map(|l| l.matches(agulha.as_str()).count())
            .sum()
    }

    /// **Um carteiro so (F2).** A fila e o `Condvar` sairam da saude do disco
    /// e moram no `Correio`; a saude, o produtor de alarmes e a camada nao
    /// tem `Condvar` proprio.
    ///
    /// # Prova real
    ///
    /// Com o `carteiro: Condvar` reposto na `SaudeDoDisco`, a contagem da
    /// saude passa de 0 a 1 e o teste cai -- o vermelho medido.
    #[test]
    fn um_carteiro_so() {
        assert_eq!(condvars(include_str!("saude_do_disco.rs")), 0, "saude");
        assert_eq!(condvars(include_str!("aquario/alarme.rs")), 0, "alarme");
        assert_eq!(
            condvars(include_str!("servidor/servico_telemetria_01.rs")),
            0,
            "a thread do carteiro"
        );
        assert_eq!(condvars(include_str!("ocorrencias.rs")), 1, "o correio");
    }

    /// **A prova da F2 que mais importa.** `CREATE USER ... PASSWORD
    /// '<sentinela>'` marcado -- pelo `sinal`, com a atividade amarrada como a
    /// conexao faz -- nao deixa a sentinela no arquivo: nem no pedido JSON,
    /// nem no SQL cru, nem nos `parametros` irmaos.
    ///
    /// # Prova real
    ///
    /// Com o `redigir` trocado por um recorte (`de_uma_linha(dados, 120)`),
    /// a sentinela aparece no arquivo e o teste cai -- o vermelho medido.
    #[test]
    fn create_user_marcado_nao_deixa_a_sentinela_no_arquivo() {
        let dir = DirTemp::novo("ocorrencias-sentinela");
        let o = camada(&dir);
        let t = Telemetria::nova(true);
        t.definir_ocorrencias(&o);
        let agora = crate::agora_ms();
        let casos = [
            format!(r#"{{"op":"sql","texto":"CREATE USER c PASSWORD '{SENTINELA}'"}}"#),
            format!("CREATE USER c PASSWORD '{SENTINELA}'"),
            format!(
                r#"{{"op":"sql","texto":"ALTER USER c PASSWORD ?","parametros":["{SENTINELA}"]}}"#
            ),
            format!(r#"{{"op":"inserir","tabela":"t","valores":{{"x":"{SENTINELA}"}}}}"#),
        ];
        for (i, dados) in casos.iter().enumerate() {
            // Uma atividade por caso: o silencio e por (alarme, usuario, IP).
            let a = t
                .entrar(
                    &format!("dados:{i}"),
                    "dados",
                    &format!("10.0.0.{i}"),
                    1,
                    agora,
                )
                .unwrap();
            let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
            a.comecou_pedido("sql", "ana", "loja", "", agora);
            let id = crate::telemetria::sinal(Alarme::SenhaEmClaro, dados);
            assert!(id.is_some(), "o caso {i} nao virou ocorrencia");
        }
        assert_eq!(carteiro_uma_volta(&o), casos.len());
        let texto = std::fs::read_to_string(dir.join(NOME_DO_ARQUIVO)).unwrap();
        assert_eq!(texto.lines().count(), casos.len(), "{texto}");
        assert!(!texto.contains(SENTINELA), "a sentinela vazou:\n{texto}");
        // E o arquivo continua servindo: o alarme, quem e de onde.
        assert!(texto.contains(r#""alarme":"senha_em_claro""#), "{texto}");
        assert!(texto.contains(r#""usuario":"ana""#), "{texto}");
        assert!(texto.contains(r#""ip":"10.0.0.1""#), "{texto}");
        assert!(texto.contains("CREATE USER c PASSWORD ?"), "{texto}");
        assert!(texto.contains("\"digital\":"), "{texto}");
    }

    /// **A tabela no lado B do `juntar` aparece em `tabelas`.** O campo
    /// `tabela` do primeiro nivel nao a tem; so a arvore inteira a acha (a
    /// porta dos fundos que o portao de permissao ja teve).
    ///
    /// Vermelho: colhendo so o `tabela` do primeiro nivel, `segredos` some.
    #[test]
    fn a_tabela_do_lado_b_do_juntar_aparece_em_tabelas() {
        let dir = DirTemp::novo("ocorrencias-juntar");
        let o = camada(&dir);
        let id = o.receber(
            Alarme::IntegridadeRecusada,
            r#"{"op":"juntar","database":"loja","a":{"tabela":"clientes"},"b":{"tabela":"segredos","onde":"cpf = '123'"}}"#,
            None,
            1_000,
        );
        assert!(id.is_some());
        assert_eq!(carteiro_uma_volta(&o), 1);
        let texto = std::fs::read_to_string(dir.join(NOME_DO_ARQUIVO)).unwrap();
        let linha = Json::analisar(texto.trim()).unwrap();
        let tabelas: Vec<String> = linha
            .campo("tabelas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .filter_map(|t| t.campo("tabela").and_then(Json::texto).map(String::from))
            .collect();
        assert!(tabelas.contains(&"segredos".to_string()), "{texto}");
        assert!(tabelas.contains(&"clientes".to_string()), "{texto}");
        assert!(!texto.contains("123"), "o valor do filtro vazou: {texto}");
    }

    /// O silencio: o segundo do mesmo (alarme, usuario, IP) na janela cala e
    /// conta; outro IP passa; o mapa nao passa do teto, e a coringa segura o
    /// resto.
    #[test]
    fn o_silencio_cala_o_repetido_e_tem_teto() {
        let dir = DirTemp::novo("ocorrencias-silencio");
        let o = camada(&dir);
        assert!(o.receber(Alarme::ForcaBruta, "", None, 1_000).is_some());
        assert!(o.receber(Alarme::ForcaBruta, "", None, 1_001).is_none());
        assert_eq!(o.caladas(), 1);
        assert!(o
            .receber(Alarme::ForcaBruta, "", None, 1_000 + SILENCIO_MS)
            .is_some());
        for i in 0..(TETO_DO_SILENCIO + 50) {
            o.pode(Alarme::ForcaBruta, "u", &format!("ip{i}"), 5_000);
        }
        assert!(travar(&o.silencio).len() <= TETO_DO_SILENCIO);
        assert!(!o.pode(Alarme::ForcaBruta, "u", "outro", 5_001), "coringa");
    }

    /// Cheia, a fila tira a mais velha da MESMA familia: a enxurrada de
    /// ocorrencias nao empurra para fora o aviso de disco.
    #[test]
    fn a_fila_cheia_preserva_a_outra_familia() {
        let c = Correio::novo();
        c.entregar(Carta::Saude(crate::saude_do_disco::Evento {
            quando_ms: 1,
            tipo: crate::saude_do_disco::Tipo::SoLeitura,
            origem: "sonda".into(),
            database: String::new(),
            tabela: String::new(),
            texto: String::new(),
        }));
        let o = Ocorrencias::nova(Arc::new(Correio::novo()));
        for i in 0..(TETO_DA_FILA * 2) {
            o.receber(Alarme::ForcaBruta, "", None, i as i64 * SILENCIO_MS);
        }
        for carta in o.correio().retirar(Duration::ZERO, |_| true) {
            c.entregar(carta);
        }
        assert_eq!(c.contar(|_| true), TETO_DA_FILA);
        assert_eq!(c.contar(|k| matches!(k, Carta::Saude(_))), 1);
    }

    /// So quem administra le: o arquivo nasce 0600 (motor de permissao do
    /// banco), como o `acessos.log`.
    #[cfg(unix)]
    #[test]
    fn o_arquivo_nasce_so_do_dono() {
        use std::os::unix::fs::PermissionsExt;
        let dir = DirTemp::novo("ocorrencias-0600");
        let _o = camada(&dir);
        let modo = std::fs::metadata(dir.join(NOME_DO_ARQUIVO))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o077, 0, "{modo:o}");
    }
}

//! A saude do disco onde o banco grava (pedido 249).
//!
//! # O que existia, e o que faltava
//!
//! Havia vigia de ESPACO (`df -k` a cada quinze minutos, e-mail quando aperta)
//! e nenhum vigia de SAUDE: uma montagem remontada so-leitura passava calada
//! ate a proxima gravacao, um `fsync` recusado no fecho da janela virava so
//! uma chave de volta na lista de sujas, e um `PhxError::Io` respondido ao
//! cliente ia para o `acessos.log` e para mais ninguem. O pedido do dono e
//! literal: *«em caso de log de erro, aviso imediato por e-mail e SMS»*.
//!
//! # As tres pecas
//!
//! 1. **A sonda canario.** A cada `alertas.disco.checar_segundos` uma thread
//!    propria escreve `<base>/.saude-do-disco` com um conteudo unico,
//!    sincroniza (`fsync`), rele, confere byte a byte e apaga -- medindo a
//!    duracao. Qualquer passo que falhe e um evento de saude, classificado
//!    pelo erro do sistema operacional: `EROFS` (so-leitura), `ENOSPC` (sem
//!    espaco) ou o resto (`EIO` e companhia). Conteudo que volta diferente
//!    tambem e evento, de tipo proprio: e o disco mentindo.
//! 2. **O gancho no erro de E/S.** Quem responde um `PhxError::Io` passa por
//!    `Servidor::anotar` -- e o unico sumidouro de resposta, o mesmo do
//!    `acessos.log`. Ali, uma comparacao de inteiro (`codigo == 5001`) decide
//!    se este modulo e chamado; o portao vem ANTES de qualquer trabalho, e
//!    o caminho de sucesso nao paga nada. O irmao que NAO passa por `anotar`
//!    e o fecho da janela de durabilidade, e ele ganhou o gancho proprio.
//! 3. **O aviso imediato, com silencio por tipo.** O primeiro evento de cada
//!    tipo dispara na hora; o seguinte do mesmo tipo dentro de
//!    `repetir_minutos` cala. E «dispara» quer dizer ENTREGA A UMA FILA: quem
//!    registra o evento pode estar com a trava global de dados na mao (o
//!    fecho da janela esta), e mandar e-mail ali ataria o servidor inteiro ao
//!    tempo de resposta do rele -- a lei da trava presa atras da rede. A fila
//!    acorda a thread da saude por `Condvar` na hora, e e ela, fora de toda
//!    trava, quem fala com o rele. E o que impede um erro por linha numa carga de
//!    cem mil linhas de virar cem mil e-mails: um aviso, e depois silencio
//!    por chave. A sonda voltando a passar zera o silencio dos tipos que ela
//!    propria consegue provar (so-leitura, sem espaco, conferencia, lento);
//!    o de E/S generico so zera pelo relogio, porque o canario passar num
//!    arquivo nao prova que o `.reg` de outra tabela voltou a aceitar
//!    escrita.
//!
//! # O que este modulo NAO faz
//!
//! Nao le SMART, nao le `/proc/mounts`, nao mede latencia do dispositivo por
//! fora da propria escrita, e nao repara nada. Ver `docs/SAUDE-DO-DISCO.md`.
//!
//! # Por que a classificacao olha o `kind` E o numero do errno
//!
//! Medido em 16/09/2026 com a `std` 1.94.1: `from_raw_os_error(30)` da
//! `ErrorKind::ReadOnlyFilesystem` e `28` da `StorageFull`, mas `5` (EIO) da
//! `Uncategorized`. Um erro construido no codigo (`io::Error::new(kind, ..)`)
//! chega sem `raw_os_error`; um erro do nucleo chega com os dois. Olhar so um
//! dos lados deixaria metade dos casos passar como «generico».

use std::collections::{HashMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;

use crate::config::Disco;

/// O nome fixo do canario, dentro do `base`. Comeca com ponto para nao ser
/// confundido com banco -- `Raiz::databases` so lista subdiretorios, e um
/// arquivo oculto nao aparece em lugar nenhum da tela.
pub const CANARIO: &str = ".saude-do-disco";

/// Quantos bytes o canario carrega. Pequeno de proposito: a sonda mede se o
/// disco ACEITA escrita, e nao quanto ele aguenta.
const TAMANHO_DO_CANARIO: usize = 64;

/// Quantos eventos a fila do carteiro segura quando ninguem a esvazia.
const TETO_DA_FILA: usize = 32;

/// A familia de um evento. E a CHAVE do silencio: dois eventos da mesma
/// familia dentro da janela sao um aviso so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tipo {
    /// `EROFS`: a montagem esta (ou virou) so-leitura.
    SoLeitura,
    /// `ENOSPC`/`EDQUOT`: sem espaco, sem inode ou sem cota.
    SemEspaco,
    /// `EIO` e todo o resto: o sistema de arquivos recusou por outro motivo.
    EntradaSaida,
    /// A sonda releu o que escreveu e o conteudo voltou diferente.
    Conferencia,
    /// A sonda passou, mas acima de `lento_ms`. Nao e erro: e aviso.
    Lento,
    /// O backup agendado FALHOU -- pedido 510. Nao e do disco do banco (o
    /// destino e outro, e a falha pode nem ser de disco), e por isso nao
    /// entra no `ultimo_evento` nem pinta o painel: so pega carona no
    /// carteiro, com silencio proprio. Ver [`SaudeDoDisco::falha_do_backup`].
    Backup,
}

impl Tipo {
    pub fn nome(self) -> &'static str {
        match self {
            Tipo::SoLeitura => "so_leitura",
            Tipo::SemEspaco => "sem_espaco",
            Tipo::EntradaSaida => "entrada_saida",
            Tipo::Conferencia => "conferencia",
            Tipo::Lento => "lento",
            Tipo::Backup => "backup",
        }
    }

    /// Lentidao e aviso de painel; o resto e erro e dispara o canal.
    pub fn e_erro(self) -> bool {
        !matches!(self, Tipo::Lento)
    }

    /// Os tipos que uma sonda bem-sucedida PROVA que passaram: o canario
    /// escreveu, entao a montagem nao esta so-leitura, ha espaco e o dado
    /// volta igual. O silencio deles zera; o de E/S generico, nao.
    fn provados_pela_sonda() -> [Tipo; 4] {
        [
            Tipo::SoLeitura,
            Tipo::SemEspaco,
            Tipo::Conferencia,
            Tipo::Lento,
        ]
    }
}

/// Classifica um erro do sistema operacional pelo `kind` E pelo errno.
pub fn classificar(e: &std::io::Error) -> Tipo {
    tipo_do_codigo(e.raw_os_error()).unwrap_or(match e.kind() {
        ErrorKind::ReadOnlyFilesystem => Tipo::SoLeitura,
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Tipo::SemEspaco,
        _ => Tipo::EntradaSaida,
    })
}

/// O tipo pelo numero do errno do Linux, quando ele decide sozinho.
fn tipo_do_codigo(codigo: Option<i32>) -> Option<Tipo> {
    match codigo {
        Some(30) => Some(Tipo::SoLeitura),
        Some(28) | Some(122) => Some(Tipo::SemEspaco),
        _ => None,
    }
}

/// O tipo a partir do TEXTO de um `PhxError::Io` ja formatado.
///
/// O `acessos.log` guarda o erro como texto, e e desse texto que o gancho do
/// `anotar` parte. A `std` formata todo erro do nucleo como
/// `<descricao> (os error N)`, e o sufixo e estavel -- nao e traduzido nem
/// muda com a redacao da descricao. Analisa-se o sufixo, nunca a frase.
pub fn tipo_do_texto(texto: &str) -> Tipo {
    let codigo = texto
        .rsplit_once("(os error ")
        .and_then(|(_, resto)| resto.split(')').next())
        .and_then(|n| n.trim().parse::<i32>().ok());
    tipo_do_codigo(codigo).unwrap_or(Tipo::EntradaSaida)
}

/// O nome do erro do sistema, para a mensagem: `EROFS (30)`, `EIO (5)`...
///
/// O numero vai junto porque o nome e a nossa leitura dele, e quem investiga
/// procura o numero no `dmesg`.
pub fn nome_do_erro(e: &std::io::Error) -> String {
    let nome = match e.raw_os_error() {
        Some(2) => "ENOENT",
        Some(5) => "EIO",
        Some(13) => "EACCES",
        Some(20) => "ENOTDIR",
        Some(21) => "EISDIR",
        Some(28) => "ENOSPC",
        Some(30) => "EROFS",
        Some(122) => "EDQUOT",
        _ => "",
    };
    match e.raw_os_error() {
        Some(c) if !nome.is_empty() => format!("{nome} ({c}) {e}"),
        Some(c) => format!("errno {c} {e}"),
        None => format!("{:?} {e}", e.kind()),
    }
}

/// Um evento de saude: o que aconteceu, onde, e quando.
#[derive(Clone, Debug)]
pub struct Evento {
    pub quando_ms: i64,
    pub tipo: Tipo,
    /// De onde veio: `sonda`, `fecho`, `acessos.log`, ou a operacao do
    /// pedido que recebeu o erro (`inserir`, `atualizar`...).
    pub origem: String,
    pub database: String,
    pub tabela: String,
    /// O texto do erro. Pode carregar caminho de disco -- e por isso so sai
    /// inteiro no painel para quem administra.
    pub texto: String,
}

/// O resultado de uma passada da sonda.
#[derive(Clone, Debug)]
pub struct Sonda {
    pub medido_em_ms: i64,
    pub duracao_us: u64,
    /// `None` quando passou.
    pub falha: Option<(Tipo, String)>,
}

/// O que ja saiu pelos canais, para o painel.
#[derive(Clone, Debug, Default)]
pub struct Avisos {
    pub email: u64,
    pub sms: u64,
    pub ultimo_email_ms: i64,
    pub ultimo_sms_ms: i64,
    /// A ultima falha em AVISAR -- rele fora do ar, gateway recusando. E
    /// noticia tambem, e nao pode sumir junto com o aviso que nao saiu.
    pub ultima_falha: Option<String>,
}

/// O estado vivo da saude do disco. Um por servidor.
pub struct SaudeDoDisco {
    cfg: Disco,
    caminho_do_canario: PathBuf,
    ultima_sonda: Mutex<Option<Sonda>>,
    /// Quando a sonda passou pela ultima vez. E o que decide se um erro de
    /// E/S antigo ainda pinta o painel de vermelho: so pinta se veio DEPOIS
    /// da ultima prova de que o disco estava bem.
    ultima_boa_ms: AtomicU64,
    sondas: AtomicU64,
    erros_es: AtomicU64,
    ultimo_evento: Mutex<Option<Evento>>,
    /// Ultimo aviso por tipo -- o silencio, no mesmo desenho do vigia de
    /// espaco e dos jobs (`jobs::pode_avisar`).
    silencio: Mutex<HashMap<String, i64>>,
    avisos: Mutex<Avisos>,
    /// Contador do canario: cada passada escreve um conteudo diferente, para
    /// que reler um arquivo VELHO (de outra passada, ou de outro processo no
    /// mesmo `base`) nao passe por conferencia.
    passada: AtomicU64,
    /// Os eventos que passaram pelo silencio e esperam o carteiro. Quem
    /// entrega pode estar com a trava de dados na mao; quem tira nunca esta.
    fila: Mutex<VecDeque<Evento>>,
    /// Acorda o carteiro na hora em que um evento entra -- «imediato» nao
    /// espera o relogio da sonda.
    carteiro: Condvar,
}

impl SaudeDoDisco {
    pub fn nova(cfg: Disco, base: &Path) -> SaudeDoDisco {
        SaudeDoDisco {
            cfg,
            caminho_do_canario: base.join(CANARIO),
            ultima_sonda: Mutex::new(None),
            ultima_boa_ms: AtomicU64::new(0),
            sondas: AtomicU64::new(0),
            erros_es: AtomicU64::new(0),
            ultimo_evento: Mutex::new(None),
            silencio: Mutex::new(HashMap::new()),
            avisos: Mutex::new(Avisos::default()),
            passada: AtomicU64::new(0),
            fila: Mutex::new(VecDeque::new()),
            carteiro: Condvar::new(),
        }
    }

    /// Poe o evento na fila do carteiro e o acorda. Barato e sem rede: e o
    /// unico trabalho permitido a quem esta dentro de uma secao critica.
    pub fn entregar(&self, evento: Evento) {
        if let Ok(mut fila) = self.fila.lock() {
            // Teto: sem carteiro (sonda E e-mail desligados) ninguem tira da
            // fila, e o silencio por tipo deixa passar um evento por tipo a
            // cada janela -- cinco tipos, 30 min, e a fila cresceria 240 por
            // dia para sempre. O mais velho sai; o painel guarda o ultimo.
            if fila.len() >= TETO_DA_FILA {
                fila.pop_front();
            }
            fila.push_back(evento);
        }
        self.carteiro.notify_one();
    }

    /// Espera ate `ate` por eventos na fila e devolve todos os que houver --
    /// vazio quando o prazo venceu sem nada. So o carteiro chama isto, fora
    /// de qualquer trava do servidor.
    pub fn esperar(&self, ate: Duration) -> Vec<Evento> {
        let Ok(mut fila) = self.fila.lock() else {
            return Vec::new();
        };
        if fila.is_empty() {
            // Um despertar espurio devolve vazio, e o laco de fora so confere
            // o relogio da sonda e volta a esperar: nada se perde.
            match self.carteiro.wait_timeout(fila, ate) {
                Ok((guarda, _)) => fila = guarda,
                Err(_) => return Vec::new(),
            }
        }
        fila.drain(..).collect()
    }

    /// Quantos eventos esperam o carteiro. Para o teste e o painel.
    pub fn na_fila(&self) -> usize {
        self.fila.lock().map(|f| f.len()).unwrap_or(0)
    }

    pub fn ligada(&self) -> bool {
        self.cfg.ligado
    }

    pub fn checar_segundos(&self) -> u64 {
        self.cfg.checar_segundos
    }

    fn silencio_ms(&self) -> i64 {
        self.cfg.repetir_minutos as i64 * 60_000
    }

    /// Uma passada da sonda. Devolve o evento que merece AVISO -- ja passado
    /// pelo silencio --, ou nada.
    pub fn sondar(&self, agora_ms: i64) -> Option<Evento> {
        let n = self.passada.fetch_add(1, Ordering::Relaxed) + 1;
        let inicio = Instant::now();
        let resultado = canario(&self.caminho_do_canario, agora_ms, n);
        let duracao_us = inicio.elapsed().as_micros() as u64;
        self.sondas.fetch_add(1, Ordering::Relaxed);

        let falha = resultado.err();
        let lenta =
            falha.is_none() && self.cfg.lento_ms > 0 && duracao_us / 1_000 >= self.cfg.lento_ms;
        if let Ok(mut u) = self.ultima_sonda.lock() {
            *u = Some(Sonda {
                medido_em_ms: agora_ms,
                duracao_us,
                falha: falha.clone(),
            });
        }
        match falha {
            Some((tipo, texto)) => self.registrar(Evento {
                quando_ms: agora_ms,
                tipo,
                origem: "sonda".into(),
                database: String::new(),
                tabela: String::new(),
                texto,
            }),
            None => {
                self.ultima_boa_ms.store(agora_ms as u64, Ordering::Relaxed);
                // Passou: o que a sonda prova volta a poder avisar na hora.
                // Lento fica de fora da limpeza quando ESTA passada foi lenta,
                // senao a proxima lenta avisaria de novo a cada sondagem.
                if let Ok(mut s) = self.silencio.lock() {
                    for t in Tipo::provados_pela_sonda() {
                        if !(lenta && t == Tipo::Lento) {
                            s.remove(t.nome());
                        }
                    }
                }
                if lenta {
                    self.registrar(Evento {
                        quando_ms: agora_ms,
                        tipo: Tipo::Lento,
                        origem: "sonda".into(),
                        database: String::new(),
                        tabela: String::new(),
                        texto: format!("canario levou {} ms", duracao_us / 1_000),
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Um erro de E/S visto por outro caminho: a resposta de um pedido, o
    /// fecho da janela, o proprio `acessos.log`. Conta, guarda, e devolve o
    /// evento se ele merece aviso agora.
    pub fn erro_de_es(
        &self,
        agora_ms: i64,
        tipo: Tipo,
        origem: &str,
        database: &str,
        tabela: &str,
        texto: &str,
    ) -> Option<Evento> {
        self.erros_es.fetch_add(1, Ordering::Relaxed);
        self.registrar(Evento {
            quando_ms: agora_ms,
            tipo,
            origem: origem.to_string(),
            database: database.to_string(),
            tabela: tabela.to_string(),
            texto: texto.to_string(),
        })
    }

    /// Guarda o evento como o ultimo e decide, pelo silencio, se avisa.
    fn registrar(&self, evento: Evento) -> Option<Evento> {
        if let Ok(mut u) = self.ultimo_evento.lock() {
            *u = Some(evento.clone());
        }
        let Ok(mut silencio) = self.silencio.lock() else {
            return None;
        };
        if crate::jobs::pode_avisar(
            &mut silencio,
            evento.tipo.nome(),
            evento.quando_ms,
            self.silencio_ms(),
        ) {
            Some(evento)
        } else {
            None
        }
    }

    /// O backup agendado falhou -- pedido 510. Devolve o evento para o
    /// carteiro quando o silencio deixa.
    ///
    /// # Por que fora do `registrar`
    ///
    /// Porque o `registrar` guarda o evento como o ULTIMO do disco, e o
    /// `estado` pinta o painel por ele: um backup que falhou por destino sem
    /// espaco pintaria de vermelho o disco do banco, que esta bem -- e
    /// esconderia o erro de E/S de verdade que viesse antes dele. Do
    /// mecanismo, o backup so usa o que e dele: o silencio (chave propria,
    /// `backup`) e a fila do carteiro.
    pub fn falha_do_backup(&self, agora_ms: i64, texto: &str) -> Option<Evento> {
        let Ok(mut silencio) = self.silencio.lock() else {
            return None;
        };
        crate::jobs::pode_avisar(
            &mut silencio,
            Tipo::Backup.nome(),
            agora_ms,
            self.silencio_ms(),
        )
        .then(|| Evento {
            quando_ms: agora_ms,
            tipo: Tipo::Backup,
            origem: "backup_agendado".into(),
            database: String::new(),
            tabela: String::new(),
            texto: texto.to_string(),
        })
    }

    /// O backup voltou a dar certo: a proxima falha e noticia nova, e avisa na
    /// hora -- o mesmo desenho dos jobs.
    pub fn backup_voltou(&self) {
        if let Ok(mut s) = self.silencio.lock() {
            s.remove(Tipo::Backup.nome());
        }
    }

    /// Anota o resultado de um envio, para o painel.
    pub fn anotar_aviso(&self, canal: &str, resultado: Result<(), String>, agora_ms: i64) {
        let Ok(mut a) = self.avisos.lock() else {
            return;
        };
        match resultado {
            Ok(()) => {
                if canal == "sms" {
                    a.sms += 1;
                    a.ultimo_sms_ms = agora_ms;
                } else {
                    a.email += 1;
                    a.ultimo_email_ms = agora_ms;
                }
            }
            Err(e) => a.ultima_falha = Some(format!("{canal}: {e}")),
        }
    }

    pub fn erros_es(&self) -> u64 {
        self.erros_es.load(Ordering::Relaxed)
    }

    pub fn ultima_sonda(&self) -> Option<Sonda> {
        self.ultima_sonda.lock().ok().and_then(|u| u.clone())
    }

    pub fn ultimo_evento(&self) -> Option<Evento> {
        self.ultimo_evento.lock().ok().and_then(|u| u.clone())
    }

    /// `ok`, `aviso`, `erro` ou `nao_medido`.
    ///
    /// `erro` quando a ultima sonda falhou, ou quando o ultimo evento de erro
    /// veio DEPOIS da ultima sonda boa -- um erro de E/S de tres dias atras,
    /// com mil sondas boas desde entao, nao pinta o painel de vermelho.
    pub fn estado(&self) -> &'static str {
        let sonda = self.ultima_sonda();
        let evento = self.ultimo_evento();
        let ultima_boa = self.ultima_boa_ms.load(Ordering::Relaxed) as i64;
        if let Some(e) = &evento {
            if e.tipo.e_erro() && e.quando_ms >= ultima_boa {
                return "erro";
            }
        }
        match sonda {
            Some(s) if s.falha.is_some() => "erro",
            Some(s) if self.cfg.lento_ms > 0 && s.duracao_us / 1_000 >= self.cfg.lento_ms => {
                "aviso"
            }
            Some(_) => "ok",
            None => "nao_medido",
        }
    }

    /// O bloco do painel. `inteiro` decide se o texto do ultimo evento (e a
    /// base/tabela dele) sai -- o texto pode carregar caminho de disco, e
    /// caminho e nome de base sao para quem administra.
    pub fn para_json(&self, inteiro: bool) -> Json {
        let sonda = self.ultima_sonda();
        let evento = self.ultimo_evento();
        let avisos = self.avisos.lock().map(|a| a.clone()).unwrap_or_default();
        let iso = |ms: i64| {
            if ms > 0 {
                Json::texto_de(phxsql_core::datahora::instante_iso(ms))
            } else {
                Json::Nulo
            }
        };
        Json::objeto(vec![
            ("ligado", Json::Bool(self.cfg.ligado)),
            ("estado", Json::texto_de(self.estado())),
            ("checar_segundos", Json::de_u64(self.cfg.checar_segundos)),
            ("repetir_minutos", Json::de_u64(self.cfg.repetir_minutos)),
            ("lento_ms", Json::de_u64(self.cfg.lento_ms)),
            ("sondas", Json::de_u64(self.sondas.load(Ordering::Relaxed))),
            (
                "medido_em",
                sonda
                    .as_ref()
                    .map(|s| iso(s.medido_em_ms))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "canario_ms",
                sonda
                    .as_ref()
                    .map(|s| Json::Numero(s.duracao_us as f64 / 1_000.0))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "canario_falha",
                sonda
                    .as_ref()
                    .and_then(|s| s.falha.as_ref())
                    .map(|(_, t)| Json::texto_de(t))
                    .unwrap_or(Json::Nulo),
            ),
            ("erros_es", Json::de_u64(self.erros_es())),
            (
                "ultimo_evento",
                match evento {
                    Some(e) => {
                        let mut campos = vec![
                            ("quando", iso(e.quando_ms)),
                            ("tipo", Json::texto_de(e.tipo.nome())),
                            ("origem", Json::texto_de(&e.origem)),
                        ];
                        // Base e tabela tambem ficam com quem administra: o
                        // nome de uma base que a sessao nao pode abrir nao e
                        // dela, e o painel ja segue essa regra nos totais.
                        if inteiro {
                            campos.push(("database", Json::texto_de(&e.database)));
                            campos.push(("tabela", Json::texto_de(&e.tabela)));
                            campos.push(("texto", Json::texto_de(&e.texto)));
                        }
                        Json::objeto(campos)
                    }
                    None => Json::Nulo,
                },
            ),
            (
                "avisos",
                Json::objeto(vec![
                    ("email", Json::de_u64(avisos.email)),
                    ("sms", Json::de_u64(avisos.sms)),
                    ("ultimo_email", iso(avisos.ultimo_email_ms)),
                    ("ultimo_sms", iso(avisos.ultimo_sms_ms)),
                    (
                        "ultima_falha",
                        avisos
                            .ultima_falha
                            .as_deref()
                            .map(Json::texto_de)
                            .unwrap_or(Json::Nulo),
                    ),
                ]),
            ),
        ])
    }
}

/// Escreve, sincroniza, rele, confere e apaga o canario.
///
/// Cada passo nomeado na falha, porque «o disco recusou» sem dizer em que
/// passo manda o operador olhar o lugar errado: `abrir` falhando com `EROFS`
/// e montagem; `fsync` falhando com `EIO` e o dispositivo.
fn canario(caminho: &Path, agora_ms: i64, passada: u64) -> Result<(), (Tipo, String)> {
    let falha = |passo: &str, e: std::io::Error| {
        (classificar(&e), format!("{passo}: {}", nome_do_erro(&e)))
    };
    let mut conteudo = format!(
        "phxsql canario {} {agora_ms} {passada}\n",
        std::process::id()
    );
    conteudo.truncate(TAMANHO_DO_CANARIO);
    let esperado = conteudo.as_bytes();

    let mut arquivo = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(caminho)
        .map_err(|e| falha("abrir", e))?;
    arquivo
        .write_all(esperado)
        .map_err(|e| falha("escrever", e))?;
    // `sync_all` e o `fsync`: sem ele a sonda mediria o cache do nucleo, que
    // aceita escrita num disco que ja morreu.
    arquivo.sync_all().map_err(|e| falha("fsync", e))?;
    drop(arquivo);

    let mut lido = Vec::with_capacity(esperado.len());
    std::fs::File::open(caminho)
        .and_then(|mut f| f.read_to_end(&mut lido))
        .map_err(|e| falha("reler", e))?;
    if lido != esperado {
        // Nao se apaga antes de sair: o arquivo com o conteudo errado e a
        // evidencia, e a proxima passada o sobrescreve de qualquer jeito.
        return Err((
            Tipo::Conferencia,
            format!(
                "o conteudo voltou diferente: {} bytes escritos, {} lidos",
                esperado.len(),
                lido.len()
            ),
        ));
    }
    std::fs::remove_file(caminho).map_err(|e| falha("apagar", e))?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    fn dir(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("saude-{rotulo}"))
    }

    fn cfg() -> Disco {
        Disco {
            ligado: true,
            checar_segundos: 1,
            repetir_minutos: 30,
            lento_ms: 0,
        }
    }

    /// A prova mais basica: num diretorio gravavel a sonda passa, mede, e
    /// nao deixa o canario para tras.
    #[test]
    fn a_sonda_passa_num_diretorio_gravavel_e_apaga_o_canario() {
        let d = dir("ok");
        let s = SaudeDoDisco::nova(cfg(), &d);
        assert_eq!(s.estado(), "nao_medido");
        assert!(s.sondar(1_000).is_none());
        let u = s.ultima_sonda().unwrap();
        assert!(u.falha.is_none(), "{:?}", u.falha);
        assert!(u.duracao_us > 0, "a duracao tem de ser medida");
        assert!(!d.join(CANARIO).exists(), "o canario ficou para tras");
        assert_eq!(s.estado(), "ok");
    }

    /// PROVA REAL CONTRA O SISTEMA OPERACIONAL, e nao contra um dublê.
    ///
    /// Tres recusas reais do nucleo, cada uma nomeada no resultado:
    ///
    /// 1. um ARQUIVO no lugar do diretorio (`ENOTDIR`, errno 20);
    /// 2. um caminho que NAO EXISTE (`ENOENT`, errno 2);
    /// 3. um diretorio `0o555` -- **so vale sem root**: como root o `chmod`
    ///    nao segura nada, e o teste DIZ isso em vez de fingir que provou.
    ///    Medido em 16/09/2026: esta bateria roda como root (`id -u` = 0), e
    ///    e por isso que as duas primeiras existem.
    ///
    /// Reponha o defeito fazendo `canario` engolir o erro do `abrir` e este
    /// teste cai nas tres.
    #[test]
    fn a_sonda_falha_contra_o_sistema_operacional() {
        // 1. Arquivo onde deveria haver diretorio.
        let d = dir("enotdir");
        let falso = d.join("arquivo-no-lugar-do-diretorio");
        std::fs::write(&falso, b"x").unwrap();
        let s = SaudeDoDisco::nova(cfg(), &falso);
        let e = s.sondar(1_000).expect("ENOTDIR tem de virar evento");
        assert_eq!(e.tipo, Tipo::EntradaSaida);
        assert!(e.texto.starts_with("abrir: ENOTDIR (20)"), "{}", e.texto);
        assert_eq!(s.estado(), "erro");

        // 2. Caminho inexistente.
        let s = SaudeDoDisco::nova(cfg(), &d.join("nao").join("existe"));
        let e = s.sondar(1_000).expect("ENOENT tem de virar evento");
        assert!(e.texto.starts_with("abrir: ENOENT (2)"), "{}", e.texto);

        // 3. Sem permissao -- so prova sem root.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let trancado = d.join("trancado");
            std::fs::create_dir(&trancado).unwrap();
            std::fs::set_permissions(&trancado, std::fs::Permissions::from_mode(0o555)).unwrap();
            let sou_root = std::fs::write(trancado.join("prova-de-root"), b"").is_ok();
            let s = SaudeDoDisco::nova(cfg(), &trancado);
            let r = s.sondar(1_000);
            if sou_root {
                eprintln!(
                    "a prova por permissao NAO VALE como root (o chmod 0555 nao segura): \
                     provado pelos casos 1 e 2 acima"
                );
                assert!(r.is_none(), "root nao deveria ser barrado por 0555");
            } else {
                let e = r.expect("EACCES tem de virar evento");
                assert!(e.texto.starts_with("abrir: EACCES (13)"), "{}", e.texto);
            }
            let _ = std::fs::set_permissions(&trancado, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// EROFS e ENOSPC pelos DOIS caminhos: o `kind` (erro construido no
    /// codigo, sem errno) e o numero (erro vindo do nucleo).
    #[test]
    fn erofs_e_enospc_sao_reconhecidos_pelo_kind_e_pelo_numero() {
        assert_eq!(
            classificar(&std::io::Error::from_raw_os_error(30)),
            Tipo::SoLeitura
        );
        assert_eq!(
            classificar(&std::io::Error::new(ErrorKind::ReadOnlyFilesystem, "x")),
            Tipo::SoLeitura
        );
        assert_eq!(
            classificar(&std::io::Error::from_raw_os_error(28)),
            Tipo::SemEspaco
        );
        assert_eq!(
            classificar(&std::io::Error::new(ErrorKind::StorageFull, "x")),
            Tipo::SemEspaco
        );
        // EIO e `Uncategorized` nesta std: so o numero o nomeia.
        let eio = std::io::Error::from_raw_os_error(5);
        assert_eq!(classificar(&eio), Tipo::EntradaSaida);
        assert!(
            nome_do_erro(&eio).starts_with("EIO (5)"),
            "{}",
            nome_do_erro(&eio)
        );
        assert!(nome_do_erro(&std::io::Error::from_raw_os_error(30)).starts_with("EROFS (30)"));
    }

    /// O gancho do `anotar` parte do TEXTO do erro, e o sufixo `(os error N)`
    /// e o que se analisa -- nunca a frase.
    #[test]
    fn o_tipo_sai_do_sufixo_os_error_e_nao_da_frase() {
        let erofs = phxsql_core::error::PhxError::Io(std::io::Error::from_raw_os_error(30));
        assert_eq!(tipo_do_texto(&erofs.to_string()), Tipo::SoLeitura);
        let enospc = phxsql_core::error::PhxError::Io(std::io::Error::from_raw_os_error(28));
        assert_eq!(tipo_do_texto(&enospc.to_string()), Tipo::SemEspaco);
        assert_eq!(
            tipo_do_texto("[SP000010] erro de E/S: Read-only file system"),
            Tipo::EntradaSaida,
            "sem o sufixo nao se adivinha pela frase"
        );
        assert_eq!(
            tipo_do_texto("qualquer coisa (os error 5)"),
            Tipo::EntradaSaida
        );
    }

    /// O silencio por tipo: o primeiro avisa na hora, o segundo do MESMO tipo
    /// dentro da janela cala, um de OUTRO tipo avisa, e passada a janela
    /// avisa de novo. Reponha o defeito fazendo `registrar` devolver o evento
    /// sem consultar `pode_avisar` e este teste cai no segundo passo.
    #[test]
    fn o_primeiro_erro_avisa_e_o_segundo_na_janela_cala() {
        let d = dir("silencio");
        let s = SaudeDoDisco::nova(cfg(), &d);
        let janela = 30 * 60_000;
        assert!(s
            .erro_de_es(1_000, Tipo::EntradaSaida, "inserir", "b", "t", "x")
            .is_some());
        assert!(s
            .erro_de_es(1_001, Tipo::EntradaSaida, "inserir", "b", "t", "x")
            .is_none());
        assert!(s
            .erro_de_es(1_002, Tipo::SoLeitura, "inserir", "b", "t", "x")
            .is_some());
        assert!(s
            .erro_de_es(
                1_000 + janela,
                Tipo::EntradaSaida,
                "atualizar",
                "b",
                "t",
                "x"
            )
            .is_some());
        assert_eq!(s.erros_es(), 4, "TODOS contam, inclusive os calados");
    }

    /// A sonda voltando a passar zera o silencio do que ela prova -- e NAO o
    /// do erro de E/S generico, que so zera pelo relogio.
    #[test]
    fn a_sonda_boa_zera_o_silencio_do_que_ela_prova() {
        let d = dir("alivio");
        let s = SaudeDoDisco::nova(cfg(), &d);
        assert!(s
            .erro_de_es(1_000, Tipo::SoLeitura, "inserir", "b", "t", "x")
            .is_some());
        assert!(s
            .erro_de_es(1_000, Tipo::EntradaSaida, "inserir", "b", "t", "x")
            .is_some());
        assert!(s.sondar(2_000).is_none());
        assert_eq!(
            s.estado(),
            "ok",
            "sonda boa depois do erro: o painel volta ao normal"
        );
        assert!(
            s.erro_de_es(2_001, Tipo::SoLeitura, "inserir", "b", "t", "x")
                .is_some(),
            "so-leitura de novo depois de uma sonda boa e noticia nova"
        );
        assert!(
            s.erro_de_es(2_001, Tipo::EntradaSaida, "inserir", "b", "t", "x")
                .is_none(),
            "o E/S generico continua calado: o canario passar nao prova o .reg"
        );
    }

    /// O estado: erro que veio DEPOIS da ultima sonda boa pinta; erro antigo,
    /// com sonda boa depois, nao.
    #[test]
    fn o_estado_segue_a_ordem_entre_erro_e_sonda_boa() {
        let d = dir("estado");
        let s = SaudeDoDisco::nova(cfg(), &d);
        assert!(s.sondar(1_000).is_none());
        assert_eq!(s.estado(), "ok");
        s.erro_de_es(2_000, Tipo::EntradaSaida, "inserir", "b", "t", "x");
        assert_eq!(s.estado(), "erro");
        assert!(s.sondar(3_000).is_none());
        assert_eq!(s.estado(), "ok");
    }

    /// Lentidao e AVISO de painel, nunca erro -- e com `lento_ms` zero nao
    /// existe. A duracao e PLANTADA na ficha da ultima sonda, e nao dormida
    /// dentro dela: um `sleep` no teste mediria o relogio, nao a regra.
    #[test]
    fn a_sonda_lenta_vira_aviso_e_nao_erro() {
        let d = dir("lento");
        let mut c = cfg();
        c.lento_ms = 0;
        let s = SaudeDoDisco::nova(c, &d);
        assert!(s.sondar(1_000).is_none());
        assert_eq!(s.estado(), "ok", "lento_ms zero desliga o aviso");
        let j = s.para_json(true);
        assert!(j.campo("canario_ms").and_then(Json::numero).unwrap() > 0.0);

        let mut c = cfg();
        c.lento_ms = 1;
        let s = SaudeDoDisco::nova(c, &d);
        *s.ultima_sonda.lock().unwrap() = Some(Sonda {
            medido_em_ms: 1_000,
            duracao_us: 5_000,
            falha: None,
        });
        assert_eq!(s.estado(), "aviso", "5 ms acima de 1 ms e aviso");
        *s.ultima_sonda.lock().unwrap() = Some(Sonda {
            medido_em_ms: 1_000,
            duracao_us: 5_000,
            falha: Some((Tipo::SoLeitura, "abrir: EROFS (30)".into())),
        });
        assert_eq!(s.estado(), "erro", "falha vence lentidao");
    }

    /// O texto do ultimo evento pode carregar caminho: sai so para quem
    /// administra (`inteiro`), e o resto do bloco sai para todos.
    #[test]
    fn o_texto_do_evento_so_sai_no_bloco_inteiro() {
        let d = dir("json");
        let s = SaudeDoDisco::nova(cfg(), &d);
        s.erro_de_es(
            1_000,
            Tipo::EntradaSaida,
            "inserir",
            "b",
            "t",
            "/segredo/t.reg (os error 5)",
        );
        let reduzido = s.para_json(false);
        let ev = reduzido.campo("ultimo_evento").unwrap();
        assert!(ev.campo("texto").is_none(), "{}", reduzido.escrever());
        assert!(ev.campo("database").is_none(), "{}", reduzido.escrever());
        assert_eq!(ev.campo("origem").and_then(Json::texto), Some("inserir"));
        let inteiro = s.para_json(true);
        assert!(inteiro.escrever().contains("/segredo/t.reg"));
        assert_eq!(inteiro.campo("erros_es").and_then(Json::numero), Some(1.0));
    }

    /// Conteudo que volta diferente e evento proprio. Prova-se plantando um
    /// canario alheio com o mesmo nome entre a escrita e a releitura -- o que
    /// nao da para fazer de fora --, entao a prova e pela funcao de baixo
    /// nivel: um caminho que e um link para outro arquivo com conteudo fixo
    /// nao existe sem root em toda maquina, e o que se prova aqui e o
    /// contador de passada, que e o que impede um arquivo VELHO de conferir.
    #[test]
    fn cada_passada_escreve_um_conteudo_diferente() {
        let d = dir("passada");
        let s = SaudeDoDisco::nova(cfg(), &d);
        assert!(s.sondar(1_000).is_none());
        assert!(s.sondar(1_000).is_none());
        assert_eq!(s.passada.load(Ordering::Relaxed), 2);
        let j = s.para_json(false);
        assert_eq!(j.campo("sondas").and_then(Json::numero), Some(2.0));
    }

    /// A fila do carteiro: entregar acorda quem espera NA HORA, e esperar
    /// sem nada vence pelo prazo com a mao vazia.
    #[test]
    fn entregar_acorda_o_carteiro_na_hora_e_o_prazo_vence_vazio() {
        let d = dir("fila");
        let s = std::sync::Arc::new(SaudeDoDisco::nova(cfg(), &d));
        let inicio = Instant::now();
        assert!(s.esperar(Duration::from_millis(80)).is_empty());
        assert!(
            inicio.elapsed() >= Duration::from_millis(80),
            "venceu antes do prazo"
        );

        let carteiro = std::sync::Arc::clone(&s);
        let fio = std::thread::spawn(move || {
            let inicio = Instant::now();
            let eventos = carteiro.esperar(Duration::from_secs(10));
            (eventos, inicio.elapsed())
        });
        std::thread::sleep(Duration::from_millis(30));
        let e = s
            .erro_de_es(1_000, Tipo::EntradaSaida, "inserir", "b", "t", "x")
            .unwrap();
        s.entregar(e);
        let (eventos, levou) = fio.join().unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].origem, "inserir");
        assert!(
            levou < Duration::from_secs(2),
            "o carteiro nao acordou na hora: {levou:?}"
        );
        assert_eq!(s.na_fila(), 0);

        // Sem carteiro a fila tem teto: o mais velho sai.
        for i in 0..(TETO_DA_FILA + 5) {
            s.entregar(Evento {
                quando_ms: i as i64,
                tipo: Tipo::EntradaSaida,
                origem: format!("op{i}"),
                database: String::new(),
                tabela: String::new(),
                texto: String::new(),
            });
        }
        assert_eq!(s.na_fila(), TETO_DA_FILA);
        let sobrou = s.esperar(Duration::from_millis(1));
        assert_eq!(
            sobrou.first().map(|e| e.origem.as_str()),
            Some("op5"),
            "o mais velho sai"
        );
    }

    /// Os avisos contam por canal, e a falha em avisar fica registrada.
    #[test]
    fn os_avisos_contam_por_canal_e_a_falha_fica() {
        let d = dir("avisos");
        let s = SaudeDoDisco::nova(cfg(), &d);
        s.anotar_aviso("email", Ok(()), 5_000);
        s.anotar_aviso("sms", Ok(()), 6_000);
        s.anotar_aviso("sms", Err("smtp recusou: 550".into()), 7_000);
        let j = s.para_json(false);
        let a = j.campo("avisos").unwrap();
        assert_eq!(a.campo("email").and_then(Json::numero), Some(1.0));
        assert_eq!(a.campo("sms").and_then(Json::numero), Some(1.0));
        assert_eq!(
            a.campo("ultima_falha").and_then(Json::texto),
            Some("sms: smtp recusou: 550")
        );
        assert!(a.campo("ultimo_email").and_then(Json::texto).is_some());
    }
}

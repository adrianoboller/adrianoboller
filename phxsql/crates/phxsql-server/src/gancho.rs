//! O gancho externo do operador (pedido 249): o meio do SMS.
//!
//! # Por que um programa do operador, e nao um cliente de SMS aqui dentro
//!
//! Todo gateway de SMS que se contrata fala HTTPS, e HTTPS pede TLS de
//! cliente, que pede crate -- e zero dependencias e petrea. O que os maduros
//! fazem (PostgreSQL com `archive_command`, e o resto por convencao) e nao
//! embutir o canal: chamam um programa do operador. O PhxSql entrega o aviso
//! a ele; o canal (SMS, WhatsApp, pager, o que for) e do operador, e o
//! segredo do gateway mora NO SCRIPT dele, nunca no `config.json`.
//!
//! # Lente de seguranca: isto executa um programa
//!
//! O que cada decisao fecha:
//!
//! * `comando` e um vetor `argv`, passado direto ao `execve`. Nao existe
//!   `sh -c` e nao existe substituicao de `%`, de `{}` nem de `$`: um
//!   argumento com `;` ou `$(...)` chega LITERAL ao programa.
//! * O campo NAO esta em `CAMPOS_EDITAVEIS`: se estivesse, quem tem
//!   `administrar` pela API executaria codigo no servidor. Edita-se so pelo
//!   arquivo, que e de quem tem a maquina.
//! * O ambiente do filho e LIMPO (`env_clear`) e leva so o `PATH` fixo e as
//!   tres variaveis do evento. A senha do rele, o token e a chave do fio
//!   moram no ambiente do servidor e nao atravessam.
//! * O que entra no filho e o tipo, a origem, a hora e a mesma linha de ate
//!   160 caracteres do SMS por e-mail (stdin) -- NUNCA o pedido, e nunca o
//!   caminho do disco.
//! * O que o filho imprime e DESCARTADO (`/dev/null`), nao capturado: texto
//!   de programa externo que entrasse no log ou no painel seria o canal por
//!   onde o segredo do gateway (impresso por um `curl -v`) chegaria ao
//!   `acessos.log`. O que nao se captura nao vaza, e e a unica forma de a
//!   garantia nao depender de um crivo.
//! * Prazo duro: o filho e vigiado por `try_wait` ate `timeout_s`, e ao
//!   estourar leva `kill` e `wait` (colhido, sem zumbi). A vigia roda no
//!   [`lancador`] (pedido 759), ou na thread do carteiro quando nao ha
//!   lancador.
//! * No maximo UMA execucao em voo; a que chega com outra rodando e
//!   descartada, nao enfileirada.
//!
//! # O limite que fica dito
//!
//! O `kill` alcanca o filho DIRETO. Um script que dispara um neto
//! (`curl ... &`) e sai deixa o neto vivo: a `std` nao tem `killpg` e
//! `unsafe`/FFI nao entra aqui. O operador que quer o prazo inteiro termina o
//! script com `exec` no ultimo programa. Esta dito no MANUAL e em
//! `docs/SEGURANCA.md`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::config::Gancho;

/// O `PATH` do filho. Fixo: o do servidor e do ambiente de quem o subiu, e
/// o gancho nao tem de depender dele.
pub const PATH_DO_FILHO: &str = "/usr/local/bin:/usr/bin:/bin";

/// O maximo de `stdout` que volta de uma execucao que pede saida (o `listar`
/// do firewall). 1 MiB cabe ~40.000 enderecos v4 na forma do `nft list set`;
/// o lancador recusa pedido acima disto, para um pedido torto nao fazer o
/// processo de fora guardar memoria sem fim.
pub const TETO_DA_SAIDA_LIDA: usize = 1 << 20;

/// De quanto em quanto tempo a vigia olha se o filho acabou. Curto o bastante
/// para um script rapido nao esperar, longo o bastante para nao queimar CPU.
const PASSO_DA_VIGIA: Duration = Duration::from_millis(20);

/// O que o filho recebe.
pub struct Chamada<'a> {
    /// `PHXSQL_TIPO`: o nome do tipo do evento (`entrada_saida`...).
    pub tipo: &'a str,
    /// `PHXSQL_ORIGEM`: de onde veio (`inserir`, `sonda`...), ja limpo.
    pub origem: &'a str,
    /// `PHXSQL_QUANDO`: o instante em ISO.
    pub quando: &'a str,
    /// A linha de ate 160 caracteres, que vai no stdin.
    pub linha: &'a str,
}

/// A origem e o nome de uma operacao ou de um componente, mas chega de
/// fora do modulo: o que nao e `[A-Za-z0-9._-]` vira `_`, e o tamanho e
/// limitado. Variavel de ambiente com quebra de linha ou NUL e o que um
/// script desatento interpola numa linha de comando.
pub fn origem_limpa(origem: &str) -> String {
    origem
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Soltura da reserva por `Drop` (pedido 640): um `panic` entre pegar a
/// reserva e devolve-la deixava `em_voo=true` para sempre, e o gancho morria
/// calado. Com o `Drop` a reserva volta tambem durante o desenrolamento.
struct Reserva<'a>(&'a AtomicBool);

impl Drop for Reserva<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Pega a reserva da execucao unica e roda `f`; quem a perde e descartado. E o
/// UNICO ponto que toca em `em_voo`, e por isso o teste do panic o chama
/// direto: provar a guarda aqui e provar a do `executar`.
fn com_reserva<T>(em_voo: &AtomicBool, f: impl FnOnce() -> T) -> Result<T, String> {
    if em_voo
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("gancho descartado: a execucao anterior ainda esta em andamento".into());
    }
    let _reserva = Reserva(em_voo);
    Ok(f())
}

/// Executa o gancho UMA vez, com prazo duro. `Ok(())` quando o programa saiu
/// com codigo zero; o erro e uma frase curta que NUNCA carrega o que o
/// programa imprimiu.
///
/// O interruptor `ligado` NAO e conferido aqui: quem decide e o carteiro
/// (`avisar_pelo_gancho`), ANTES de montar qualquer texto. A decisao escrita
/// duas vezes e o que esta casa proibe -- uma das duas envelheceria.
///
/// `em_voo` e a reserva da execucao unica: quem a perde e descartado.
pub fn executar(g: &Gancho, c: &Chamada, em_voo: &AtomicBool) -> Result<(), String> {
    if g.comando.is_empty() {
        return Err("gancho ligado sem comando".into());
    }
    let origem = origem_limpa(c.origem);
    let ambiente = [
        ("PHXSQL_TIPO", c.tipo),
        ("PHXSQL_ORIGEM", origem.as_str()),
        ("PHXSQL_QUANDO", c.quando),
    ];
    com_reserva(em_voo, || {
        rodar(&Execucao {
            rotulo: "gancho",
            argv: &g.comando,
            path: PATH_DO_FILHO,
            ambiente: &ambiente,
            entrada: Some(c.linha),
            prazo_s: g.timeout_s,
            teto_da_saida: 0,
        })
    })?
}

/// O que o motor de execucao precisa saber. O gancho do operador e o comando
/// de firewall da lista negra (pedido 638) sao DUAS maneiras de pedir a mesma
/// coisa -- rodar um programa de fora, sem shell, sem ambiente herdado, com
/// prazo e sem capturar o que ele imprime --, e por isso passam por aqui: um
/// segundo motor envelheceria sem o outro, e o firewall ja tinha envelhecido
/// (`output()` sem prazo, ambiente herdado, `stderr` dentro do erro).
pub struct Execucao<'a> {
    /// Como o erro chama o programa («gancho», «comando de firewall»).
    pub rotulo: &'a str,
    /// O argv inteiro; o `[0]` e o programa.
    pub argv: &'a [String],
    /// O `PATH` do filho. Fixo: o do servidor nao atravessa.
    pub path: &'a str,
    /// As variaveis do evento, alem do `PATH`.
    pub ambiente: &'a [(&'a str, &'a str)],
    /// O que vai no stdin; `None` fecha o stdin (`/dev/null`).
    pub entrada: Option<&'a str>,
    /// O prazo duro, em segundos (no minimo 1).
    pub prazo_s: u64,
    /// Quantos bytes do `stdout` voltam a quem pediu. Zero -- o caso de todo
    /// gancho e de todo `bloquear`/`desbloquear` -- descarta a saida, como
    /// sempre foi. So o `listar` do firewall (pedido 766, P11) pede saida, e
    /// pede porque a reconciliacao precisa ANALISAR o que o SO diz; o erro
    /// continua nunca carregando o que o programa imprimiu.
    pub teto_da_saida: usize,
}

/// Roda o programa UMA vez, com prazo duro, e devolve uma frase que nunca
/// carrega o que ele imprimiu nem o caminho dele.
///
/// Pelo [`lancador`] quando o executavel o habilitou (o `phxsqld`): o filho
/// nasce de um processo que nunca segurou a trava de instancia (pedido 759).
/// Sem lancador -- binario de teste, ou um lancador que nao nasceu --, roda
/// aqui mesmo, pelo MESMO [`rodar_aqui`] que o lancador usa do lado dele.
pub fn rodar(e: &Execucao) -> Result<(), String> {
    rodar_e_ler(e).map(|_| ())
}

/// O [`rodar`] que devolve o `stdout` -- ate `e.teto_da_saida` bytes, em
/// texto com a troca do que nao e UTF-8. O MESMO motor, pelo lancador quando
/// ha um: a saida e o unico acrescimo, e so no sucesso; a frase do erro
/// continua sem ela.
pub fn rodar_e_ler(e: &Execucao) -> Result<String, String> {
    #[cfg(unix)]
    if let Some(r) = lancador::pedir(e) {
        return r;
    }
    rodar_aqui(e)
}

/// Le o `stdout` do filho numa thread propria: o filho que imprime mais que
/// o buffer do pipe pararia no `write` esperando quem leia, e a vigia o
/// mataria no prazo por um motivo que nao e dele. Guarda ate `teto` bytes e
/// DRENA o resto, pelo mesmo motivo.
fn ler_a_saida(
    saida: std::process::ChildStdout,
    teto: usize,
) -> std::sync::mpsc::Receiver<Vec<u8>> {
    use std::io::Read;
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("saida-do-filho".into())
        .spawn(move || {
            let mut guardado = Vec::new();
            let mut saida = saida;
            let mut bloco = [0u8; 8192];
            loop {
                match saida.read(&mut bloco) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let cabe = teto.saturating_sub(guardado.len()).min(n);
                        guardado.extend_from_slice(&bloco[..cabe]);
                    }
                }
            }
            let _ = tx.send(guardado);
        });
    rx
}

/// O motor de execucao de fato. E o que o [`lancador`] chama do lado dele, e
/// o que o servidor chama quando nao ha lancador: uma copia so das garantias
/// (`env_clear`, sem shell, saida descartada, prazo com `kill`+`wait`).
fn rodar_aqui(e: &Execucao) -> Result<String, String> {
    let Some(programa) = e.argv.first() else {
        return Err(format!("{} sem comando", e.rotulo));
    };
    let rotulo = e.rotulo;
    let mut cmd = Command::new(programa);
    cmd.args(&e.argv[1..]).env_clear().env("PATH", e.path);
    for (k, v) in e.ambiente {
        cmd.env(k, v);
    }
    cmd.current_dir(diretorio_neutro())
        .stdin(if e.entrada.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if e.teto_da_saida > 0 {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::null());
    let mut filho = iniciar(&mut cmd, rotulo)?;
    let leitura = filho.stdout.take().map(|s| ler_a_saida(s, e.teto_da_saida));
    if let (Some(linha), Some(mut entrada)) = (e.entrada, filho.stdin.take()) {
        // 160 caracteres cabem no pipe sem bloquear. Um programa que nao
        // le o stdin faz o `write` falhar com EPIPE, e isso nao e erro.
        let _ = entrada.write_all(linha.as_bytes());
        let _ = entrada.write_all(b"\n");
        // `entrada` cai aqui: o filho ve o fim do stdin.
    }
    let prazo = e.prazo_s.max(1);
    let limite = Instant::now() + Duration::from_secs(prazo);
    loop {
        match filho.try_wait() {
            Ok(Some(estado)) => {
                return if estado.success() {
                    // O neto que herdou o pipe e ficou vivo seguraria a
                    // leitura para sempre: espera-se no maximo um passo de
                    // vigia por segundo de prazo, e o que nao chegou fica
                    // vazio -- a reconciliacao le «nada no SO», que e o lado
                    // seguro (ela so ACRESCENTA o que falta).
                    let texto = leitura
                        .and_then(|rx| rx.recv_timeout(Duration::from_secs(prazo)).ok())
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .unwrap_or_default();
                    Ok(texto)
                } else {
                    Err(match estado.code() {
                        Some(n) => format!("o {rotulo} saiu com codigo {n}"),
                        None => format!("o {rotulo} foi encerrado por um sinal"),
                    })
                };
            }
            Ok(None) => {}
            Err(er) => {
                matar_e_colher(&mut filho);
                return Err(format!(
                    "nao foi possivel vigiar o {rotulo} ({:?})",
                    er.kind()
                ));
            }
        }
        if Instant::now() >= limite {
            matar_e_colher(&mut filho);
            return Err(format!("o {rotulo} passou de {prazo} s e foi morto"));
        }
        std::thread::sleep(PASSO_DA_VIGIA);
    }
}

/// **Pedido 759: o filho do gancho nasce de um processo que nunca segurou a
/// trava.** O irmao do `sistema::Lancador` do `df` (pedido 758).
///
/// # O defeito
///
/// A `.phxsql.trava` e `flock`, da DESCRICAO aberta: todo `spawn` copia o
/// descritor para o filho, que o segura ate o `exec`. O gancho do operador e o
/// comando de firewall nasciam com a trava na mao do servidor; se ele morre por
/// `SIGKILL` nesse instante, o orfao segura a instancia e a abertura gravavel
/// seguinte ouve `InstanciaOcupada`. Medido pelo SO com o `phxsqld` real e o
/// gancho chamado no arranque (`tests/queda-nao-prende-a-trava.rs`): 31 de
/// 800 quedas recusadas, 8 corridas vermelhas em 8; com o gancho desligado,
/// 0 em 300.
///
/// # O conserto
///
/// O `phxsqld` reexecuta a si mesmo com [`ARGUMENTO`] ANTES da primeira trava
/// (`Servidor::novo`, so quando o gancho ou o firewall estao ligados), e esse
/// processo -- que nunca teve a trava -- roda cada [`Execucao`] pelo MESMO
/// [`rodar_aqui`]. O `Lancador` do `df` e um `sh` de uma linha; o gancho
/// precisa de `env_clear`, entrada, prazo e `kill`, e reescreve-los em `sh`
/// seria a segunda copia do motor que esta casa proibe.
///
/// Cada pedido leva um numero e roda numa thread propria do lancador: o
/// firewall (thread de conexao) nao espera atras de um gancho de 120 s. A
/// resposta e so o `Result` -- a mesma frase que nunca carrega a saida do
/// programa.
///
/// O lancador morre com o servidor: o fim da entrada o faz esperar as
/// execucoes em voo (cada uma ja tem prazo) e sair. Se ele cair sozinho, o
/// proximo pedido cria outro -- e ai a janela do `spawn` volta a existir uma
/// vez, a do proprio lancador, em vez de a cada evento.
#[cfg(unix)]
pub mod lancador {
    use super::{rodar_aqui, Execucao, TETO_DA_SAIDA_LIDA};
    use phxsql_core::json::Json;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{mpsc, Arc, Mutex, OnceLock};
    use std::time::Duration;

    /// O argumento UNICO com que o `phxsqld` vira lancador. Interceptado no
    /// `main` antes de tudo, inclusive da conferencia das flags.
    pub const ARGUMENTO: &str = "--lancador-de-ganchos";

    /// Folga sobre o prazo do programa para a resposta do lancador chegar. O
    /// prazo e do lancador; isto so impede um lancador pendurado de prender
    /// para sempre quem pediu.
    const FOLGA: Duration = Duration::from_secs(10);

    type Resposta = Result<String, String>;
    type Pendentes = HashMap<u64, mpsc::Sender<Resposta>>;

    static EXECUTAVEL: OnceLock<PathBuf> = OnceLock::new();
    static ATUAL: Mutex<Option<Arc<Lancador>>> = Mutex::new(None);

    struct Lancador {
        entrada: Mutex<ChildStdin>,
        /// `None` depois que a saida do lancador acabou: ninguem mais responde.
        pendentes: Mutex<Option<Pendentes>>,
        proximo: AtomicU64,
    }

    impl Lancador {
        fn pendentes(&self) -> std::sync::MutexGuard<'_, Option<Pendentes>> {
            self.pendentes.lock().unwrap_or_else(|e| e.into_inner())
        }
    }

    /// O executavel que sabe ser lancador (o `phxsqld` passa o proprio
    /// caminho). Sem isto, [`pedir`] devolve `None` e o servidor roda direto.
    pub fn habilitar(executavel: PathBuf) {
        let _ = EXECUTAVEL.set(executavel);
    }

    /// Cria o lancador se o executavel o habilitou e ele ainda nao existe.
    /// Chamado ANTES da primeira trava de instancia.
    pub fn preparar() {
        let _ = obter();
    }

    fn obter() -> Option<Arc<Lancador>> {
        let executavel = EXECUTAVEL.get()?;
        let mut g = ATUAL.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = nascer(executavel);
        }
        g.clone()
    }

    fn nascer(executavel: &Path) -> Option<Arc<Lancador>> {
        let mut filho = Command::new(executavel)
            .arg(ARGUMENTO)
            // Fora da pasta de quem o subiu: o zelador prova que ninguem usa
            // uma pasta pelo `cwd` dos processos.
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let (Some(entrada), Some(saida)) = (filho.stdin.take(), filho.stdout.take()) else {
            let _ = filho.kill();
            let _ = filho.wait();
            return None;
        };
        let l = Arc::new(Lancador {
            entrada: Mutex::new(entrada),
            pendentes: Mutex::new(Some(HashMap::new())),
            proximo: AtomicU64::new(1),
        });
        let leitor = Arc::clone(&l);
        std::thread::Builder::new()
            .name("lancador-gancho".into())
            .spawn(move || ler_respostas(leitor, saida, filho))
            .ok()
            .map(|_| l)
    }

    /// A thread que entrega cada resposta a quem a pediu. No fim da saida (o
    /// lancador saiu), solta todos os que esperam, tira o lancador de
    /// circulacao e o colhe -- sem zumbi.
    fn ler_respostas(l: Arc<Lancador>, saida: ChildStdout, mut filho: Child) {
        for linha in BufReader::new(saida).lines() {
            let Ok(linha) = linha else { break };
            let Ok(j) = Json::analisar(&linha) else {
                continue;
            };
            let Some(id) = j.campo("id").and_then(Json::inteiro) else {
                continue;
            };
            let r = match j.campo("erro").and_then(Json::texto) {
                Some(e) => Err(e.to_string()),
                None => Ok(j.texto_ou("saida", "").to_string()),
            };
            let tx = l.pendentes().as_mut().and_then(|m| m.remove(&(id as u64)));
            if let Some(tx) = tx {
                let _ = tx.send(r);
            }
        }
        l.pendentes().take();
        descartar(&l);
        let _ = filho.wait();
    }

    fn descartar(l: &Arc<Lancador>) {
        let mut g = ATUAL.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().is_some_and(|a| Arc::ptr_eq(a, l)) {
            *g = None;
        }
    }

    /// Roda `e` pelo lancador. `None` quando nao ha lancador (e entao quem
    /// chama roda direto); `Some` com o resultado, ou com a frase de que o
    /// lancador caiu DEPOIS de receber o pedido -- e ai nao se roda de novo,
    /// porque o programa pode ter rodado (um SMS em dobro).
    pub fn pedir(e: &Execucao) -> Option<Resposta> {
        // Duas tentativas: o pedido que nem chegou a ser escrito num lancador
        // morto vai ao proximo, criado na hora.
        for _ in 0..2 {
            let l = obter()?;
            let id = l.proximo.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = mpsc::channel();
            let registrado = l.pendentes().as_mut().map(|m| m.insert(id, tx)).is_some();
            if !registrado {
                descartar(&l);
                continue;
            }
            let mut linha = pedido(id, e).escrever();
            linha.push('\n');
            let escrito = {
                let mut w = l.entrada.lock().unwrap_or_else(|e| e.into_inner());
                w.write_all(linha.as_bytes()).and_then(|_| w.flush())
            };
            if escrito.is_err() {
                if let Some(m) = l.pendentes().as_mut() {
                    m.remove(&id);
                }
                descartar(&l);
                continue;
            }
            let prazo = Duration::from_secs(e.prazo_s.max(1)) + FOLGA;
            return Some(match rx.recv_timeout(prazo) {
                Ok(r) => r,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(m) = l.pendentes().as_mut() {
                        m.remove(&id);
                    }
                    Err(format!("o lancador do {} nao respondeu no prazo", e.rotulo))
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
                    "o lancador do {} caiu antes de responder",
                    e.rotulo
                )),
            });
        }
        None
    }

    fn pedido(id: u64, e: &Execucao) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(id)),
            ("rotulo", Json::texto_de(e.rotulo)),
            (
                "argv",
                Json::Lista(e.argv.iter().map(|a| Json::texto_de(a.as_str())).collect()),
            ),
            ("path", Json::texto_de(e.path)),
            (
                "ambiente",
                Json::Lista(
                    e.ambiente
                        .iter()
                        .map(|(k, v)| Json::Lista(vec![Json::texto_de(*k), Json::texto_de(*v)]))
                        .collect(),
                ),
            ),
            (
                "entrada",
                e.entrada.map(Json::texto_de).unwrap_or(Json::Nulo),
            ),
            ("prazo_s", Json::de_u64(e.prazo_s)),
            ("teto_da_saida", Json::de_u64(e.teto_da_saida as u64)),
        ])
    }

    /// Um pedido lido do fio, de volta a `Execucao`, rodado pelo motor.
    fn atender(j: &Json) -> Resposta {
        let texto = |c: &str| j.campo(c).and_then(Json::texto).unwrap_or("");
        let lista = |c: &str| j.campo(c).and_then(Json::lista).unwrap_or(&[]);
        let argv: Vec<String> = lista("argv")
            .iter()
            .filter_map(|a| a.texto().map(str::to_string))
            .collect();
        let pares: Vec<(&str, &str)> = lista("ambiente")
            .iter()
            .filter_map(|p| match p.lista()? {
                [k, v] => Some((k.texto()?, v.texto()?)),
                _ => None,
            })
            .collect();
        rodar_aqui(&Execucao {
            rotulo: texto("rotulo"),
            argv: &argv,
            path: texto("path"),
            ambiente: &pares,
            entrada: j.campo("entrada").and_then(Json::texto),
            prazo_s: j
                .campo("prazo_s")
                .and_then(Json::inteiro)
                .unwrap_or(1)
                .max(1) as u64,
            teto_da_saida: j
                .campo("teto_da_saida")
                .and_then(Json::inteiro)
                .unwrap_or(0)
                .clamp(0, TETO_DA_SAIDA_LIDA as i64) as usize,
        })
    }

    /// O lado do lancador: le um pedido por linha, roda cada um numa thread,
    /// e responde `{"id":n}` ou `{"id":n,"erro":"..."}`. No fim da entrada (o
    /// servidor saiu ou morreu), espera as execucoes em voo -- cada uma com
    /// prazo, `kill` e `wait` -- e sai.
    pub fn servir() {
        let saida = Arc::new(Mutex::new(std::io::stdout()));
        let mut em_voo: Vec<std::thread::JoinHandle<()>> = Vec::new();
        for linha in std::io::stdin().lock().lines() {
            let Ok(linha) = linha else { break };
            let Ok(j) = Json::analisar(&linha) else {
                continue;
            };
            let Some(id) = j.campo("id").and_then(Json::inteiro) else {
                continue;
            };
            em_voo.retain(|t| !t.is_finished());
            let saida = Arc::clone(&saida);
            em_voo.push(std::thread::spawn(move || {
                let mut pares = vec![("id", Json::de_i64(id))];
                match atender(&j) {
                    Err(e) => pares.push(("erro", Json::texto_de(e))),
                    Ok(saida) if !saida.is_empty() => pares.push(("saida", Json::texto_de(saida))),
                    Ok(_) => {}
                }
                let mut w = saida.lock().unwrap_or_else(|e| e.into_inner());
                let _ = writeln!(w, "{}", Json::objeto(pares).escrever());
                let _ = w.flush();
            }));
        }
        for t in em_voo {
            let _ = t.join();
        }
    }
}

/// `kill` e depois `wait`: sem o segundo o filho morto fica zumbi ate o
/// servidor cair.
fn matar_e_colher(filho: &mut std::process::Child) {
    let _ = filho.kill();
    let _ = filho.wait();
}

/// Inicia o filho. Um script recem-escrito pode dar `ETXTBSY` (26) se outra
/// thread estava com o arquivo aberto para escrita no instante do `fork`; e
/// transiente, e repetir poucas vezes e a resposta documentada.
fn iniciar(cmd: &mut Command, rotulo: &str) -> Result<std::process::Child, String> {
    let mut tentativas = 0;
    loop {
        match cmd.spawn() {
            Ok(f) => return Ok(f),
            Err(e) if e.raw_os_error() == Some(26) && tentativas < 5 => {
                tentativas += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            // So o tipo do erro: o texto da `std` carrega o caminho do
            // programa, e este texto vai ao painel.
            Err(e) => {
                return Err(format!(
                    "nao foi possivel iniciar o {rotulo} ({:?})",
                    e.kind()
                ))
            }
        }
    }
}

/// O diretorio de trabalho do filho: a raiz, e nao o `base` do banco -- um
/// script que escreve relativo nao deve cair dentro dos dados.
fn diretorio_neutro() -> &'static Path {
    if cfg!(unix) {
        Path::new("/")
    } else {
        Path::new(".")
    }
}

/// Troca por espaco tudo o que e controle (CR, LF, ESC, NUL, DEL, os C1) e
/// corta em `limite` caracteres. E o ajudante UNICO das linhas de aviso que
/// saem do servidor para fora -- o SMS por e-mail e o stdin do gancho --, e
/// existe porque a linha leva `database`/`tabela` vindos do pedido (pedido
/// 643): so CR/LF eram trocados, e um nome com `ESC[` chegava ao terminal do
/// operador, ou a um `eval` descuidado do script dele. Duas copias desta
/// decisao seriam a que alguem esquece de atualizar.
pub fn linha_limpa(texto: &str, limite: usize) -> String {
    texto
        .chars()
        .map(|c| {
            if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                ' '
            } else {
                c
            }
        })
        .take(limite)
        .collect()
}

/// O uid com que o servidor roda. `/proc/self` pertence a quem executa o
/// processo; fora do Linux nao ha como saber sem FFI, e `None` faz so o root
/// valer como dono.
#[cfg(unix)]
fn uid_do_servidor() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// Este dono pode ter escrito o programa (ou o diretorio dele)? Root ou o
/// proprio usuario do servidor: qualquer outro que possa reescrever o arquivo
/// ganha execucao de codigo com os privilegios do servidor.
#[cfg(unix)]
fn dono_confiavel(uid: u32, servidor: Option<u32>) -> bool {
    uid == 0 || Some(uid) == servidor
}

/// O programa e um caminho absoluto, existe, e um arquivo comum, executavel e
/// que ninguem alem de um dono confiavel pode trocar? Devolve a frase da
/// recusa.
///
/// Roda no arranque (`Gancho::validar`), pela mesma razao do `Sms::validar`:
/// o erro certo e o lido ao subir o servidor, e nao o que o carteiro descobre
/// as tres da manha com o disco morrendo. Tambem roda em `gravar_a_arvore`:
/// com o script removido depois do boot, qualquer edicao de config pela tela
/// falha -- de proposito (fail-closed), porque config que aponta para um
/// programa que nao existe mais nao deve ser regravada em silencio.
///
/// # O que a conferencia cobre (pedido 639)
///
/// E o equivalente ao `StrictModes` do ssh e a checagem de permissao da chave
/// do PostgreSQL: o arquivo (ja resolvido, com todos os links seguidos) nao
/// pode ser gravavel por grupo nem por outros, tem de ser de root ou do
/// usuario do servidor, e cada diretorio da cadeia ate a raiz idem -- ou, se
/// gravavel por outros, ter o bit sticky (`/tmp`), que impede quem nao e dono
/// de renomear a entrada.
///
/// Link simbolico NAO e recusado por ser link (`/bin/sh` e `/usr/bin/python3`
/// sao links e sao o caso comum), mas cada elo da cadeia entra na conta: o
/// dono do link e os diretorios onde ele mora. Quem escreve no diretorio do
/// link o repontar para outro programa sem tocar no arquivo conferido.
///
/// # O limite, dito
///
/// Conferencia no arranque nao e conferencia no `exec`: quem ganhar escrita
/// DEPOIS do boot vence a corrida (TOCTOU), e a `std` nao tem `fexecve`.
/// No Windows nao ha checagem de dono nem de ACL (so existencia): o bloco e
/// `cfg(unix)`, e o operador la protege o programa pelas ACLs do sistema.
pub fn conferir_programa(programa: &str) -> Result<(), String> {
    conferir_programa_de("alertas.gancho.comando[0]", programa)
}

/// A [`conferir_programa`] com o CAMPO que a frase da recusa nomeia: o
/// gancho do operador e os tres comandos do firewall (pedido 766, P11) sao
/// a mesma pergunta -- «este programa pode rodar com os poderes do
/// servidor?» --, e uma segunda conferencia para o firewall seria a copia que
/// envelhece sem a outra.
pub fn conferir_programa_de(campo: &str, programa: &str) -> Result<(), String> {
    let caminho = Path::new(programa);
    if !caminho.is_absolute() {
        return Err(format!(
            "{campo}: {programa:?} nao e um caminho absoluto \
             (o PATH do servidor nao vale aqui)"
        ));
    }
    let meta = std::fs::metadata(caminho).map_err(|e| {
        format!(
            "{campo}: {programa:?} nao existe ou nao se le ({:?})",
            e.kind()
        )
    })?;
    if !meta.is_file() {
        return Err(format!("{campo}: {programa:?} nao e um arquivo comum"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!("{campo}: {programa:?} nao e executavel"));
        }
        let servidor = uid_do_servidor();
        // Cada ELO da cadeia (o caminho dado, os links que ele atravessa e o
        // arquivo final) e uma entrada que alguem pode ter trocado: o dono dela
        // e os diretorios onde ela mora entram na conta. Um link num diretorio
        // sadio apontando para um arquivo sadio passa (`/bin/sh`,
        // `/usr/bin/python3`); o que se recusa e o elo que um terceiro
        // consegue repontar.
        let mut atual = caminho.to_path_buf();
        for _ in 0..16 {
            let m = std::fs::symlink_metadata(&atual)
                .map_err(|e| format!("{campo}: {programa:?} nao se resolve ({:?})", e.kind()))?;
            if !dono_confiavel(m.uid(), servidor) {
                return Err(format!(
                    "{campo}: {programa:?} pertence a outro usuario \
                     (uid {}); so root ou o usuario do servidor podem ser donos",
                    m.uid()
                ));
            }
            // Os diretorios onde a entrada mora, ja com os links deles
            // resolvidos: e neles que mora quem pode trocar o programa.
            let pai = atual.parent().unwrap_or(Path::new("/"));
            let pai_real = std::fs::canonicalize(pai).map_err(|e| {
                format!(
                    "{campo}: o diretorio de {programa:?} nao se resolve ({:?})",
                    e.kind()
                )
            })?;
            for dir in pai_real.ancestors() {
                let d = std::fs::metadata(dir).map_err(|e| {
                    format!(
                        "{campo}: o diretorio de {programa:?} nao se le ({:?})",
                        e.kind()
                    )
                })?;
                let modo = d.permissions().mode();
                let sticky = modo & 0o1000 != 0;
                if modo & 0o022 != 0 && !sticky {
                    return Err(format!(
                        "{campo}: um diretorio de {programa:?} e gravavel \
                         por grupo ou outros e sem o bit sticky; quem escreve nele troca o \
                         programa por `rename`"
                    ));
                }
                if !dono_confiavel(d.uid(), servidor) {
                    return Err(format!(
                        "{campo}: um diretorio de {programa:?} pertence a \
                         outro usuario (uid {})",
                        d.uid()
                    ));
                }
            }
            if m.file_type().is_symlink() {
                let alvo = std::fs::read_link(&atual).map_err(|e| {
                    format!("{campo}: o link de {programa:?} nao se le ({:?})", e.kind())
                })?;
                atual = if alvo.is_absolute() {
                    alvo
                } else {
                    pai_real.join(alvo)
                };
                continue;
            }
            // O arquivo final. Programa que qualquer usuario -- ou o grupo dele
            // -- pode reescrever e execucao de codigo oferecida a quem tem uma
            // conta na maquina.
            let modo = m.permissions().mode();
            if modo & 0o002 != 0 {
                return Err(format!(
                    "{campo}: {programa:?} e gravavel por qualquer usuario \
                     (tire a permissao de escrita de \"outros\")"
                ));
            }
            if modo & 0o020 != 0 {
                return Err(format!(
                    "{campo}: {programa:?} e gravavel pelo grupo \
                     (tire a permissao de escrita do \"grupo\")"
                ));
            }
            return Ok(());
        }
        Err(format!("{campo}: {programa:?} atravessa links demais"))
    }
    #[cfg(not(unix))]
    Ok(())
}

/// O que os testes deste modulo e os do servidor dividem: um script de shell
/// executavel. Uma funcao so, para que a segunda copia nao envelheca.
#[cfg(all(test, unix))]
pub(crate) mod apoio_de_teste {
    use crate::apoio_teste::DirTemp;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// Pelo guarda de sempre (`DirTemp`): apaga no `Drop`, e a catraca dos
    /// temporarios nao acusa.
    pub fn dir(nome: &str) -> DirTemp {
        DirTemp::novo(&format!("gancho-{nome}"))
    }

    /// Escreve um script executavel. `ETXTBSY` e tratado pelo `iniciar`.
    pub fn script(d: &Path, nome: &str, corpo: &str) -> String {
        let p = d.join(nome);
        std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.display().to_string()
    }
}

#[cfg(all(test, unix))]
mod testes {
    use super::apoio_de_teste::{dir, script};
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn gancho(comando: Vec<String>, timeout_s: u64) -> Gancho {
        Gancho {
            ligado: true,
            comando,
            timeout_s,
        }
    }

    fn chamada() -> Chamada<'static> {
        Chamada {
            tipo: "entrada_saida",
            origem: "inserir",
            quando: "2026-10-02T10:00:00.000",
            linha: "PhxSql x: disco erro",
        }
    }

    #[test]
    fn o_filho_recebe_as_variaveis_e_a_linha_e_nada_mais() {
        let d = dir("env");
        let saida = d.join("saida.txt");
        let s = script(
            &d,
            "g.sh",
            &format!(
                "{{ echo \"$PHXSQL_TIPO|$PHXSQL_ORIGEM|$PHXSQL_QUANDO\"; read l; echo \"$l\"; \
                 env | sort; pwd; }} > {}",
                saida.display()
            ),
        );
        // O ambiente do servidor NAO pode atravessar: o `cargo test` exporta
        // `CARGO_*` para este processo, e nenhuma delas pode chegar ao filho.
        assert!(std::env::vars().any(|(k, _)| k.starts_with("CARGO")));
        let em_voo = AtomicBool::new(false);
        executar(&gancho(vec![s], 5), &chamada(), &em_voo).unwrap();
        let t = std::fs::read_to_string(&saida).unwrap();
        assert!(
            t.starts_with("entrada_saida|inserir|2026-10-02T10:00:00.000\nPhxSql x: disco erro\n"),
            "{t}"
        );
        assert!(!t.contains("CARGO"), "o ambiente vazou: {t}");
        assert!(t.contains("PATH=/usr/local/bin:/usr/bin:/bin"), "{t}");
        assert!(t.trim_end().ends_with("\n/"), "cwd nao e neutro: {t}");
        assert!(!em_voo.load(Ordering::SeqCst), "a reserva nao foi solta");
    }

    #[test]
    fn argumento_com_metacaractere_chega_literal() {
        let d = dir("literal");
        let saida = d.join("args.txt");
        let s = script(
            &d,
            "g.sh",
            &format!("printf '%s\\n' \"$@\" > {}", saida.display()),
        );
        let perigosos = [
            "a;touch /nao/existe/NAO",
            "$(id)",
            "`id`",
            "x && y",
            "%s%n",
            "{ip}",
        ];
        let mut argv = vec![s];
        argv.extend(perigosos.iter().map(|p| p.to_string()));
        executar(&gancho(argv, 5), &chamada(), &AtomicBool::new(false)).unwrap();
        let t = std::fs::read_to_string(&saida).unwrap();
        assert_eq!(t.lines().collect::<Vec<_>>(), perigosos, "{t}");
    }

    #[test]
    fn saida_do_filho_nao_volta_no_erro() {
        let d = dir("saida");
        let s = script(
            &d,
            "g.sh",
            "echo SEGREDO-NO-STDOUT; echo SEGREDO-NO-STDERR >&2; exit 7",
        );
        let e = executar(&gancho(vec![s], 5), &chamada(), &AtomicBool::new(false)).unwrap_err();
        assert_eq!(e, "o gancho saiu com codigo 7");
        assert!(!e.contains("SEGREDO"), "{e}");
    }

    /// Sobe um programa que passa do prazo de 1 s e devolve o pid dele. O
    /// `DirTemp` volta junto para o pidfile viver ate o fim do teste.
    fn estourar_o_prazo(nome: &str) -> (crate::apoio_teste::DirTemp, u32) {
        let d = dir(nome);
        let pidfile = d.join("pid");
        let s = script(
            &d,
            "g.sh",
            &format!("echo $$ > {}; exec sleep 30", pidfile.display()),
        );
        let inicio = Instant::now();
        let e = executar(&gancho(vec![s], 1), &chamada(), &AtomicBool::new(false)).unwrap_err();
        assert!(
            inicio.elapsed() < Duration::from_secs(5),
            "{:?}",
            inicio.elapsed()
        );
        assert!(e.contains("foi morto"), "{e}");
        let pid: u32 = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        (d, pid)
    }

    /// **Morto**, separado de **colhido** (pedido 655): o teste de baixo
    /// servia as duas guardas e caia igual para as duas -- o filho vivo e o
    /// zumbi deixam `/proc/<pid>` de pe. Aqui so se exige que ele tenha
    /// deixado de RODAR: zumbi (`State: Z`) passa, porque o `kill` aconteceu;
    /// o filho que segue em `sleep` cai. O `kill` e assincrono, por isso a
    /// espera curta antes do veredito.
    #[test]
    fn programa_que_estoura_o_prazo_e_morto() {
        let (_d, pid) = estourar_o_prazo("prazo-morto");
        if !cfg!(target_os = "linux") {
            return;
        }
        let estado = |pid: u32| {
            std::fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|t| {
                    t.lines()
                        .find_map(|l| l.strip_prefix("State:").map(|v| v.trim().to_string()))
                })
        };
        let ate = Instant::now() + Duration::from_secs(2);
        loop {
            match estado(pid) {
                None => return,
                Some(e) if e.starts_with('Z') => return,
                Some(e) if Instant::now() >= ate => {
                    panic!("o filho {pid} passou do prazo e continua rodando ({e}): nao foi morto")
                }
                Some(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    #[test]
    fn programa_que_estoura_o_prazo_e_morto_e_colhido() {
        let (_d, pid) = estourar_o_prazo("prazo");
        // Contra o sistema operacional: morto E colhido. Zumbi deixaria
        // `/proc/<pid>` de pe, com `State: Z`.
        if cfg!(target_os = "linux") {
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "o filho {pid} continua na tabela de processos"
            );
        }
    }

    #[test]
    fn com_uma_execucao_em_voo_a_segunda_e_descartada() {
        let d = dir("voo");
        let marca = d.join("rodou");
        let s = script(&d, "g.sh", &format!("echo x >> {}", marca.display()));
        let em_voo = AtomicBool::new(true);
        let e = executar(&gancho(vec![s], 5), &chamada(), &em_voo).unwrap_err();
        assert!(e.contains("descartado"), "{e}");
        assert!(!marca.exists(), "executou com outra em voo");
        assert!(em_voo.load(Ordering::SeqCst), "soltou a reserva alheia");
    }

    #[test]
    fn a_origem_perde_o_que_nao_e_nome() {
        assert_eq!(origem_limpa("ins\nert; rm -rf /"), "ins_ert__rm_-rf__");
        assert_eq!(origem_limpa("acessos.log"), "acessos.log");
        assert_eq!(origem_limpa(&"a".repeat(500)).len(), 64);
    }

    #[test]
    fn a_conferencia_do_programa_recusa_o_que_nao_executaria() {
        let d = dir("conf");
        let bom = script(&d, "bom.sh", "exit 0");
        conferir_programa(&bom).unwrap();
        assert!(conferir_programa("g.sh").unwrap_err().contains("absoluto"));
        assert!(
            conferir_programa(&d.join("nao-existe").display().to_string())
                .unwrap_err()
                .contains("nao existe")
        );
        assert!(conferir_programa(&d.display().to_string())
            .unwrap_err()
            .contains("arquivo comum"));
        let sem_x = d.join("sem-x");
        std::fs::write(&sem_x, "x").unwrap();
        std::fs::set_permissions(&sem_x, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(conferir_programa(&sem_x.display().to_string())
            .unwrap_err()
            .contains("executavel"));
        let aberto = d.join("aberto");
        std::fs::write(&aberto, "x").unwrap();
        std::fs::set_permissions(&aberto, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(conferir_programa(&aberto.display().to_string())
            .unwrap_err()
            .contains("qualquer usuario"));
    }

    // ------------------------------------------------ pedido 639

    /// Script de grupo 0775: antes passava (so `0o002` era conferido).
    #[test]
    fn programa_gravavel_pelo_grupo_e_recusado() {
        let d = dir("grupo");
        let s = script(&d, "g.sh", "exit 0");
        std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o775)).unwrap();
        let e = conferir_programa(&s).unwrap_err();
        assert!(e.contains("grupo"), "{e}");
        // Pedido 657: «grupo» sai tambem da recusa do diretorio («por grupo
        // ou outros»); a do ARQUIVO e a unica que diz «pelo grupo».
        assert!(e.contains("e gravavel pelo grupo"), "{e}");
        // O irmao que impede um portao que recusaria tudo: 0755 passa.
        std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o755)).unwrap();
        conferir_programa(&s).unwrap();
    }

    /// Script 0755 em diretorio 0777 sem sticky: trocavel por `rename`.
    /// Com o sticky (o caso do `/tmp`) a troca por quem nao e dono nao passa,
    /// e o diretorio e aceito.
    #[test]
    fn programa_em_diretorio_gravavel_sem_sticky_e_recusado() {
        let d = dir("dirabto");
        let sub = d.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let s = script(&sub, "g.sh", "exit 0");
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o777)).unwrap();
        let e = conferir_programa(&s).unwrap_err();
        assert!(e.contains("sticky"), "{e}");
        // Pedido 657: «sticky» aparece em comentario e codigo do arquivo; a
        // recusa do diretorio aberto e a unica com esta frase.
        assert!(e.contains("por grupo ou outros e sem o bit sticky"), "{e}");
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o1777)).unwrap();
        conferir_programa(&s).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// O link entra na conta, nao sai dela. Tres casos:
    /// 1. link para um script 0777: o ALVO e que reprova (o `metadata` antigo
    ///    ja seguia o link, mas so uma vez e so o modo);
    /// 2. link num diretorio 0777 sem sticky, apontando para um script
    ///    perfeito: quem escreve no diretorio repontar o link troca o
    ///    programa sem tocar no arquivo conferido -- recusa;
    /// 3. link num diretorio sadio para um script sadio (o caso de
    ///    `/bin/sh` e `/usr/bin/python3`): passa. Sem este, o portao
    ///    recusaria tudo que o mundo real tem por link.
    #[test]
    fn link_simbolico_entra_na_conta() {
        let d = dir("link");
        // 1
        let aberto = script(&d, "aberto.sh", "exit 0");
        std::fs::set_permissions(&aberto, std::fs::Permissions::from_mode(0o777)).unwrap();
        let elo1 = d.join("elo1.sh");
        std::os::unix::fs::symlink(&aberto, &elo1).unwrap();
        let e = conferir_programa(&elo1.display().to_string()).unwrap_err();
        assert!(e.contains("qualquer usuario"), "{e}");
        assert!(e.contains("e gravavel por qualquer usuario"), "{e}");
        // 2
        let real = script(&d, "real.sh", "exit 0");
        let sub = d.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let elo2 = sub.join("elo2.sh");
        std::os::unix::fs::symlink(&real, &elo2).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o777)).unwrap();
        let e = conferir_programa(&elo2.display().to_string()).unwrap_err();
        assert!(e.contains("sticky"), "{e}");
        assert!(e.contains("por grupo ou outros e sem o bit sticky"), "{e}");
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
        // 3
        conferir_programa(&elo2.display().to_string()).unwrap();
        conferir_programa(&real).unwrap();
    }

    /// Dono que nao e root nem o usuario do servidor: reescreve o programa
    /// quando quiser, mesmo com 0755. So da para plantar o dono sendo root
    /// (o contêiner e); fora disso o teste diz que pulou.
    #[test]
    fn programa_de_outro_usuario_e_recusado() {
        if uid_do_servidor() != Some(0) {
            eprintln!("pulado: plantar o dono exige root");
            return;
        }
        let d = dir("dono");
        let s = script(&d, "g.sh", "exit 0");
        std::os::unix::fs::chown(&s, Some(54321), None).unwrap();
        let e = conferir_programa(&s).unwrap_err();
        assert!(e.contains("outro usuario"), "{e}");
        // Pedido 657: «pertence a outro usuario» a recusa do DIRETORIO tambem
        // escreve; so a do arquivo diz quem pode ser dono, e nunca «um
        // diretorio de». Antes este teste servia a duas guardas e a do
        // diretorio passava por ele com a do arquivo reposta.
        assert!(
            e.contains("so root ou o usuario do servidor podem ser donos"),
            "{e}"
        );
        assert!(!e.contains("um diretorio de"), "{e}");
    }

    /// O irmao do de cima, separado dele (pedido 657): o diretorio de outro
    /// usuario recusa pela recusa PROPRIA -- o dono dele troca o arquivo por
    /// `rename` --, e nao pela do arquivo, que aqui e do usuario certo.
    #[test]
    fn diretorio_de_outro_usuario_e_recusado() {
        if uid_do_servidor() != Some(0) {
            eprintln!("pulado: plantar o dono exige root");
            return;
        }
        let d = dir("dono-dir");
        let sub = d.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let s2 = script(&sub, "h.sh", "exit 0");
        conferir_programa(&s2).unwrap();
        std::os::unix::fs::chown(&sub, Some(54321), None).unwrap();
        let e = conferir_programa(&s2).unwrap_err();
        assert!(
            e.contains("diretorio") && e.contains("outro usuario"),
            "{e}"
        );
        assert!(e.contains("(uid 54321)"), "{e}");
        assert!(
            !e.contains("so root ou o usuario do servidor podem ser donos"),
            "{e}"
        );
    }

    // ------------------------------------------------ pedido 640

    /// Um `panic` com a reserva na mao nao pode deixar `em_voo` preso. A
    /// proxima execucao tem de rodar.
    #[test]
    fn panico_com_a_reserva_na_mao_nao_a_prende() {
        let em_voo = AtomicBool::new(false);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = com_reserva(&em_voo, || -> () { panic!("panico de teste") });
        }));
        assert!(r.is_err(), "o panico nao aconteceu");
        assert!(!em_voo.load(Ordering::SeqCst), "a reserva ficou presa");
        let d = dir("panico");
        let marca = d.join("rodou");
        let s = script(&d, "g.sh", &format!("echo x >> {}", marca.display()));
        executar(&gancho(vec![s], 5), &chamada(), &em_voo).unwrap();
        assert!(marca.exists(), "a execucao seguinte nao rodou");
    }

    // ------------------------------------------------ pedido 642

    /// Lista os descritores do script (`/proc/$$/fd`) com `ls -l`: `$$` e o
    /// `sh` do script, e o `ls` e filho dele, entao a lista e a DO SCRIPT
    /// (uma substituicao de comando abriria um `pipe` proprio no meio dela).
    fn lista_fds(saida: &Path) -> String {
        format!("ls -l /proc/$$/fd > {}", saida.display())
    }

    /// `(fd, alvo)` de cada linha do `ls -l`.
    fn fds_listados(t: &str) -> Vec<(String, String)> {
        t.lines()
            .filter_map(|l| l.split_once(" -> "))
            .map(|(esq, alvo)| {
                (
                    esq.rsplit(' ').next().unwrap_or("").to_string(),
                    alvo.to_string(),
                )
            })
            .collect()
    }

    /// So 0, 1 e 2 (mais o do proprio script, que o `sh` segura). O servidor
    /// do teste tem arquivo aberto, soquete escutando e uma trava: nada
    /// disso pode atravessar o `exec`.
    /// Os descritores que ESTE processo ja recebeu do pai sem `O_CLOEXEC`
    /// (bit `02000000` do `flags:` em `/proc/self/fdinfo`). Nao sao do
    /// servidor: o executor do GitHub entrega dois pipes assim ao `cargo test`,
    /// e eles atravessam o `exec` porque a `std` nao fecha o que o processo
    /// herdou -- fechar exigiria `pre_exec`, que e `unsafe`, e o servidor nao
    /// usa `unsafe` (SEGURANCA.md). Lidos ANTES de o teste abrir qualquer
    /// coisa, entao um descritor do servidor sem `O_CLOEXEC` continua caindo.
    #[cfg(target_os = "linux")]
    fn herdados_do_pai() -> Vec<String> {
        let mut v = Vec::new();
        for e in std::fs::read_dir("/proc/self/fdinfo")
            .into_iter()
            .flatten()
            .flatten()
        {
            let n = e.file_name().to_string_lossy().to_string();
            if matches!(n.as_str(), "0" | "1" | "2") {
                continue;
            }
            let info = std::fs::read_to_string(e.path()).unwrap_or_default();
            let flags = info
                .lines()
                .find_map(|l| l.strip_prefix("flags:"))
                .and_then(|f| u32::from_str_radix(f.trim(), 8).ok());
            if flags.is_some_and(|f| f & 0o2000000 == 0) {
                v.push(n);
            }
        }
        v
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn o_filho_nao_herda_descritores_do_servidor() {
        let herdados = herdados_do_pai();
        let d = dir("fds");
        let sentinela = d.join("sentinela-aberta.dat");
        std::fs::write(&sentinela, "x").unwrap();
        let _aberto = std::fs::File::open(&sentinela).unwrap();
        let _ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let saida = d.join("fds.txt");
        let s = script(&d, "g.sh", &lista_fds(&saida));
        executar(
            &gancho(vec![s.clone()], 5),
            &chamada(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let t = std::fs::read_to_string(&saida).unwrap();
        let estranhos: Vec<(String, String)> = fds_listados(&t)
            .into_iter()
            // O `sh` guarda copias do stdout/stderr originais (`/dev/null`,
            // descartados de proposito) acima do fd 10, ao redirecionar.
            .filter(|(n, alvo)| {
                !matches!(n.as_str(), "0" | "1" | "2")
                    && *alvo != s
                    && alvo != "/dev/null"
                    && !herdados.contains(n)
            })
            .collect();
        assert!(!fds_listados(&t).is_empty(), "a listagem veio vazia: {t:?}");
        assert!(
            estranhos.is_empty(),
            "descritores herdados: {estranhos:?}\n{t}"
        );
        assert!(!t.contains("sentinela-aberta"), "{t}");
        assert!(!t.contains("socket:"), "o soquete atravessou: {t}");
    }

    /// Controle do detector: um descritor que NAO e CLOEXEC (aberto por um
    /// shell pai com `exec 7<`) aparece na mesma listagem. Sem isto o teste
    /// de cima passaria por engano com um detector cego.
    #[cfg(target_os = "linux")]
    #[test]
    fn o_detector_de_descritores_ve_o_que_vaza() {
        let d = dir("fds-controle");
        let alvo = d.join("vazado.dat");
        std::fs::write(&alvo, "x").unwrap();
        let saida = d.join("fds.txt");
        let s = script(&d, "g.sh", &lista_fds(&saida));
        let st = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("exec 7< {}; {s}", alvo.display()))
            .status()
            .unwrap();
        assert!(st.success());
        let t = std::fs::read_to_string(&saida).unwrap();
        assert!(
            fds_listados(&t)
                .iter()
                .any(|(n, alvo)| n == "7" && alvo.ends_with("vazado.dat")),
            "{t}"
        );
    }

    // ------------------------------------------------ pedido 643

    #[test]
    fn a_linha_perde_controles_e_respeita_o_corte() {
        let l = linha_limpa("a\x1b[31mb\r\nc\u{0}d\x7fe\u{85}f\u{2028}g\th", 160);
        assert!(!l.chars().any(char::is_control), "{l:?}");
        assert!(!l.contains('\u{2028}'), "{l:?}");
        assert!(l.starts_with("a [31mb") && l.contains('g'), "{l:?}");
        assert_eq!(linha_limpa(&"a".repeat(500), 160).chars().count(), 160);
        // Acento e texto comum passam intactos.
        assert_eq!(linha_limpa("coração ok", 160), "coração ok");
    }
}

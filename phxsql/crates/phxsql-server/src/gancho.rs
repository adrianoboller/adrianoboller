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
//!   estourar leva `kill` e `wait` (colhido, sem zumbi). Sem thread nova:
//!   a vigia e a propria thread do carteiro.
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
    let Some(programa) = g.comando.first() else {
        return Err("gancho ligado sem comando".into());
    };
    if em_voo
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("gancho descartado: a execucao anterior ainda esta em andamento".into());
    }
    let r = executar_reservado(g, programa, c);
    em_voo.store(false, Ordering::Release);
    r
}

fn executar_reservado(g: &Gancho, programa: &str, c: &Chamada) -> Result<(), String> {
    let mut cmd = Command::new(programa);
    cmd.args(&g.comando[1..])
        .env_clear()
        .env("PATH", PATH_DO_FILHO)
        .env("PHXSQL_TIPO", c.tipo)
        .env("PHXSQL_ORIGEM", origem_limpa(c.origem))
        .env("PHXSQL_QUANDO", c.quando)
        .current_dir(diretorio_neutro())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut filho = iniciar(&mut cmd)?;
    if let Some(mut entrada) = filho.stdin.take() {
        // 160 caracteres cabem no pipe sem bloquear. Um programa que nao
        // le o stdin faz o `write` falhar com EPIPE, e isso nao e erro.
        let _ = entrada.write_all(c.linha.as_bytes());
        let _ = entrada.write_all(b"\n");
        // `entrada` cai aqui: o filho ve o fim do stdin.
    }
    let limite = Instant::now() + Duration::from_secs(g.timeout_s.max(1));
    loop {
        match filho.try_wait() {
            Ok(Some(estado)) => {
                return if estado.success() {
                    Ok(())
                } else {
                    Err(match estado.code() {
                        Some(n) => format!("o gancho saiu com codigo {n}"),
                        None => "o gancho foi encerrado por um sinal".into(),
                    })
                };
            }
            Ok(None) => {}
            Err(e) => {
                matar_e_colher(&mut filho);
                return Err(format!("nao foi possivel vigiar o gancho ({:?})", e.kind()));
            }
        }
        if Instant::now() >= limite {
            matar_e_colher(&mut filho);
            return Err(format!(
                "o gancho passou de {} s e foi morto",
                g.timeout_s.max(1)
            ));
        }
        std::thread::sleep(PASSO_DA_VIGIA);
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
fn iniciar(cmd: &mut Command) -> Result<std::process::Child, String> {
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
                    "nao foi possivel iniciar o gancho ({:?})",
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

/// O programa e um caminho absoluto, existe, e um arquivo comum, executavel e
/// que ninguem alem do dono pode trocar? Devolve a frase da recusa.
///
/// Roda no arranque (`Gancho::validar`), pela mesma razao do `Sms::validar`:
/// o erro certo e o lido ao subir o servidor, e nao o que o carteiro descobre
/// as tres da manha com o disco morrendo.
pub fn conferir_programa(programa: &str) -> Result<(), String> {
    let caminho = Path::new(programa);
    if !caminho.is_absolute() {
        return Err(format!(
            "alertas.gancho.comando[0]: {programa:?} nao e um caminho absoluto \
             (o PATH do servidor nao vale aqui)"
        ));
    }
    let meta = std::fs::metadata(caminho).map_err(|e| {
        format!(
            "alertas.gancho.comando[0]: {programa:?} nao existe ou nao se le ({:?})",
            e.kind()
        )
    })?;
    if !meta.is_file() {
        return Err(format!(
            "alertas.gancho.comando[0]: {programa:?} nao e um arquivo comum"
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = meta.permissions().mode();
        if modo & 0o111 == 0 {
            return Err(format!(
                "alertas.gancho.comando[0]: {programa:?} nao e executavel"
            ));
        }
        // Programa que qualquer usuario pode reescrever e execucao de codigo
        // oferecida a quem tem uma conta na maquina.
        if modo & 0o002 != 0 {
            return Err(format!(
                "alertas.gancho.comando[0]: {programa:?} e gravavel por qualquer usuario \
                 (tire a permissao de escrita de \"outros\")"
            ));
        }
    }
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

    #[test]
    fn programa_que_estoura_o_prazo_e_morto_e_colhido() {
        let d = dir("prazo");
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
}

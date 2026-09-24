//! Pedido 504 -- o `abort` do servidor nao pode deixar no disco a chave do
//! cofre.
//!
//! # Por que um processo de verdade
//!
//! O que se prova aqui e o que o NUCLEO escreve quando o processo cai por
//! `SIGABRT` -- o sinal do `std::process::abort()` da H5 do pedido 451. Teste
//! unitario nao alcanca isso: e o `phxsqld` compilado, subindo com o cofre
//! ligado, recebendo o sinal, e o arquivo `core` que sobra no diretorio dele.
//!
//! # O que se mede, e o que nao se pode medir aqui
//!
//! Duas coisas, e a segunda e a que vale em qualquer maquina:
//!
//! 1. o `core` que o nucleo escreve, lido byte a byte atras da SENHA do cofre.
//!    A chave de cada volume e `PBKDF2(senha, sal)`, e o sal vai em claro no
//!    cabecalho do arquivo: quem tem a senha tem todas as chaves. So se mede
//!    quando o `core_pattern` do nucleo e um arquivo relativo (`core`); com
//!    um `|programa` (systemd-coredump, apport) o `core` vai para outro
//!    lugar, e o teste DIZ que nao mediu em vez de passar calado;
//! 2. o `/proc/<pid>/coredump_filter` do processo vivo -- o filtro que decide
//!    quais regioes de memoria vao ao `core`, qualquer que seja o destino.
//!
//! O `ulimit -c` do teste e aberto de proposito (`unlimited`) para o nucleo
//! PODER escrever: e o caso da maquina de quem ligou o `core` para depurar, e
//! o conserto tem de valer justamente ali.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A senha do cofre desta prova: longa e unica, para a contagem no `core`
/// nao achar a sequencia por acaso em outro lugar da memoria.
const SENHA: &str = "SENHA-DO-COFRE-504-core-que-nao-pode-guardar-a-chave";

/// Guarda que mata o processo no `Drop` -- inclusive quando uma assercao
/// falha no meio (a mesma licao do pedido 150 para diretorio de teste).
struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn porta_aberta(erro_padrao: &Path) -> u16 {
    const LINHA: &str = "porta de dados escutando em ";
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        let texto = std::fs::read_to_string(erro_padrao).unwrap_or_default();
        if let Some(resto) = texto.lines().find_map(|l| l.strip_prefix(LINHA)) {
            let alvo: SocketAddr = resto.trim().parse().unwrap();
            return alvo.port();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "o phxsqld nao abriu a porta de dados em 20 s: {}",
        std::fs::read_to_string(erro_padrao).unwrap_or_default()
    );
}

fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{linha}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

/// Quantas vezes `agulha` aparece em `palheiro`.
fn ocorrencias(palheiro: &[u8], agulha: &[u8]) -> usize {
    palheiro
        .windows(agulha.len())
        .filter(|j| *j == agulha)
        .count()
}

/// O `core` que o nucleo deixou no diretorio, se deixou.
fn achar_core(dir: &Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "core" || n.starts_with("core."))
        })
}

/// **A prova do pedido 504.** O `phxsqld` sobe com o cofre ligado, deriva a
/// chave de um volume cifrado de verdade, e cai por `SIGABRT`.
///
/// Com o defeito (medido em 24/09/2026 neste conteiner, antes do conserto):
/// `coredump_filter` = `00000033` e a senha do cofre aparece no `core`. Com o
/// conserto: o filtro e `00000000`, e o `core` nao carrega regiao de memoria
/// nenhuma -- nem a senha, nem a chave derivada, nem linha em claro.
#[cfg(target_os = "linux")]
#[test]
fn o_core_do_abort_nao_carrega_a_senha_do_cofre() {
    let dir = DirTemp::novo("core-sem-segredo");
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "t",
                "base": {base:?},
                "cifra": {{ "ligada": true, "senha": "{SENHA}", "iteracoes": 10000 }},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": false }}
            }}"#,
            base = dir.join("dados").display().to_string()
        ),
    )
    .unwrap();
    let erro_padrao = dir.join("stderr.txt");
    // O `sh` abre o limite do `core` e da `exec` no binario: o pid do filho
    // passa a ser o do `phxsqld`, e o `core` nasce no diretorio de trabalho.
    let filho = Filho(
        Command::new("sh")
            .args(["-c", "ulimit -c unlimited; exec \"$0\" \"$@\""])
            .arg(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(&*dir)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = porta_aberta(&erro_padrao);

    // Um volume cifrado de verdade: a tabela nova nasce com o diario cifrado,
    // e o `inserir` faz o cofre DERIVAR a chave dela.
    for linha in [
        r#"{"token":"t","op":"criar_database","database":"loja"}"#,
        r#"{"token":"t","op":"criar_tabela","database":"loja","tabela":"c","colunas":[{"nome":"n","tipo":"Int8"}]}"#,
        r#"{"token":"t","op":"inserir","database":"loja","tabela":"c","valores":{"n":1}}"#,
    ] {
        let r = pedir(porta, linha);
        assert!(r.contains("\"ok\":true"), "{linha} recusou: {r}");
    }

    let pid = filho.0.id();
    let filtro = std::fs::read_to_string(format!("/proc/{pid}/coredump_filter"))
        .unwrap_or_default()
        .trim()
        .to_string();

    let st = Command::new("kill")
        .args(["-ABRT", &pid.to_string()])
        .status()
        .expect("kill");
    assert!(st.success(), "o kill -ABRT falhou");
    let mut filho = filho;
    let ate = Instant::now() + Duration::from_secs(20);
    let saida = loop {
        if let Some(s) = filho.0.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < ate, "o phxsqld nao caiu com o SIGABRT");
        std::thread::sleep(Duration::from_millis(20));
    };
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(saida.signal(), Some(6), "nao caiu pelo SIGABRT: {saida:?}");
    }

    let padrao = std::fs::read_to_string("/proc/sys/kernel/core_pattern")
        .unwrap_or_default()
        .trim()
        .to_string();
    let core = achar_core(&dir);
    let na_memoria = match &core {
        Some(c) => {
            let bytes = std::fs::read(c).unwrap();
            let n = ocorrencias(&bytes, SENHA.as_bytes());
            eprintln!(
                "504: coredump_filter={filtro}; core de {} bytes; a senha do cofre \
                 aparece {n} vez(es)",
                bytes.len()
            );
            Some(n)
        }
        None => {
            eprintln!(
                "504: coredump_filter={filtro}; NENHUM core no diretorio \
                 (core_pattern={padrao:?}) -- a contagem no core NAO foi medida \
                 aqui, so o filtro"
            );
            None
        }
    };

    assert_eq!(
        filtro, "00000000",
        "o phxsqld subiu com o filtro de core {filtro}: um SIGABRT (a H5 do \
         pedido 451) poe a memoria do processo no disco, e nela a senha do cofre"
    );
    if let Some(n) = na_memoria {
        assert_eq!(
            n, 0,
            "a senha do cofre foi para o core {n} vez(es) -- quem leva o disco \
             leva a chave de todo volume cifrado"
        );
    }
}

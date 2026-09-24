//! A linha grande nao fica residente depois da resposta -- pedido 442, a
//! metade da MEMORIA, provada contra o sistema operacional.
//!
//! # O defeito
//!
//! O laco da conexao guardava a linha lida numa variavel declarada FORA dele
//! (`let mut linha;`), e a `String` velha so morria quando a leitura seguinte
//! DEVOLVIA. Entre uma e outra a conexao fica parada na leitura -- minutos,
//! horas --, e a linha anterior continua inteira na memoria. Medido pela
//! revisao de seguranca de 23/09/2026 (achado M1): VmRSS de 7.988 kB para
//! **73.680 kB ocioso** depois de um ping de 64 MiB, e so voltava a 8.140 kB
//! na linha seguinte. Quem manda uma linha grande e goteja a proxima segura a
//! memoria dela -- ja sem precisar do teto que a deixou entrar.
//!
//! # Por que um arquivo so para isto
//!
//! Porque o que se mede e o `VmRSS` do PROCESSO, e cada arquivo de `tests/` e
//! um binario proprio. Outro teste no mesmo binario, rodando em paralelo,
//! mandando a linha de 1 MiB dele, entraria na conta -- e a prova passaria ou
//! falharia pelo vizinho.
//!
//! # Por que o `/proc`, e nao contar a alocacao
//!
//! Porque a pergunta e se a memoria VOLTA, e isso e o alocador e o kernel
//! respondendo, nao o Rust. Uma `String` solta que o alocador nao devolve
//! seria um conserto que passa no teste unitario e nao muda nada na maquina.
//! O que depende do sistema operacional se prova contra o sistema
//! operacional.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-da-linha-residente";

/// O tamanho da linha: o mesmo da medicao da revisao, para os numeros se
/// compararem.
const LINHA_MIB: usize = 64;

/// Quanto o processo pode ficar acima do basal depois da resposta. A linha
/// ocupa 64 MiB; o que sobra acima do basal depois do conserto e o alocador
/// guardando pedaco pequeno, e fica bem abaixo de um quarto dela.
const FOLGA_KB: u64 = 16 * 1024;

fn vmrss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .expect("sem VmRSS no /proc/self/status")
}

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Servidor sem cadastro: o teto e o do registro para todos, e a linha de
/// 64 MiB entra -- que e o que se quer, porque a prova e do que acontece
/// DEPOIS que ela entrou.
fn subir(base: &std::path::Path) -> u16 {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let porta = porta_livre();
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:{porta}", "base": {b:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?}, "timeout_s": 60,
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        b = base.join("base").display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return porta;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

fn resposta(leitor: &mut BufReader<TcpStream>) -> Json {
    let mut r = String::new();
    leitor.read_line(&mut r).expect("sem resposta");
    Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
}

/// **Depois da resposta a uma linha de 64 MiB, a memoria volta perto do
/// basal -- sem esperar a linha seguinte.**
///
/// O teste manda a linha EM PEDACOS de 1 MiB, e nao montada inteira: o
/// cliente mora no mesmo processo, e uma `String` de 64 MiB do lado de ca
/// entraria na conta do `VmRSS` que se quer medir do lado de la.
#[test]
fn a_linha_grande_nao_fica_residente_depois_da_resposta() {
    let d = DirTemp::novo("linha-residente");
    let porta = subir(&d);
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);

    // O basal inclui a thread da conexao e os buffers dela: um ping pequeno
    // antes, pela mesma conexao.
    writeln!(escrita, r#"{{"token":"{TOKEN}","op":"ping"}}"#).unwrap();
    assert!(resposta(&mut leitor).booleano_ou("ok", false));
    std::thread::sleep(Duration::from_millis(200));
    let basal = vmrss_kb();

    let pedaco = vec![b'A'; 1024 * 1024];
    write!(escrita, r#"{{"token":"{TOKEN}","op":"ping","enchimento":""#).unwrap();
    for _ in 0..LINHA_MIB {
        escrita.write_all(&pedaco).unwrap();
    }
    escrita.write_all(b"\"}\n").unwrap();
    escrita.flush().unwrap();
    drop(pedaco);
    let r = resposta(&mut leitor);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());

    // A conexao fica PARADA na leitura seguinte -- e o estado ocioso que o
    // achado mediu. Sem mandar nada: e justamente a proxima linha que o
    // defeito esperava para soltar a memoria.
    let ate = Instant::now() + Duration::from_secs(5);
    let mut agora = vmrss_kb();
    while Instant::now() < ate && agora.saturating_sub(basal) > FOLGA_KB {
        std::thread::sleep(Duration::from_millis(100));
        agora = vmrss_kb();
    }
    eprintln!("VmRSS basal {basal} kB, ocioso depois da linha de {LINHA_MIB} MiB {agora} kB");
    assert!(
        agora.saturating_sub(basal) <= FOLGA_KB,
        "a linha de {LINHA_MIB} MiB ficou residente depois da resposta: VmRSS {basal} kB -> \
         {agora} kB ocioso, {} kB acima do basal (folga {FOLGA_KB} kB)",
        agora.saturating_sub(basal)
    );
}

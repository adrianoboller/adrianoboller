//! Pedido 758: o `phxsqld` morto por `SIGKILL` nao pode deixar a
//! `.phxsql.trava` presa num filho dele -- provado contra o sistema
//! operacional, com o `phxsqld` de verdade.
//!
//! # O defeito
//!
//! A trava e `flock`, e o `flock` e da DESCRICAO aberta: todo `spawn` copia o
//! descritor para o filho, que o segura ate o `exec`. O servidor que morre
//! nesse instante deixa o filho orfao com a trava na mao, e a abertura
//! gravavel seguinte ouve `InstanciaOcupada` apontando o pid do morto. No
//! servidor quem cria filho sozinho e a thread `vigia-disco`, que chama o `df`
//! logo no arranque: medido pelo SO, 37 de 300 quedas deixaram a trava presa,
//! e o detentor era um processo `vigia-disco` com `ppid` 1.
//!
//! # Como se prova
//!
//! O teste mata o servidor logo depois de uma gravacao (a trava tomada e
//! fixada), espera o processo, e abre a tabela para gravar pelo motor, como
//! o `commit-inteiro-na-queda` fazia quando caiu. Repetido [`VEZES`] vezes.
#![cfg(unix)]

mod comum;
use comum::{porta_no_texto, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "queda-nao-prende-a-trava";
/// Quedas por corrida. Com o `df` de volta ao `Command` direto, 6 corridas
/// de 100 quedas deram 6, 4, 6, 11, 7 e 13 recusas (vermelho nas seis); com
/// 40 quedas, uma corrida em tres passava sem ver o defeito.
const VEZES: u64 = 100;

/// O `phxsqld` com o erro padrao num cano: cada linha chega pelo canal assim
/// que ele a escreve.
fn subir(dir: &Path) -> (Filho, Receiver<String>, u16) {
    subir_com(dir, "")
}

/// O [`subir`] com campos a mais no `config.json` (`extra` comeca com `,`).
fn subir_com(dir: &Path, extra: &str) -> (Filho, Receiver<String>, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }}{extra}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        // Um segredo no ambiente do servidor: o gancho nao pode ve-lo.
        .env("PHXSQL_SEGREDO_DO_TESTE", "nao-pode-atravessar")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let erro = filho.0.stderr.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for linha in BufReader::new(erro).lines() {
            let Ok(linha) = linha else { break };
            if tx.send(linha).is_err() {
                break;
            }
        }
    });
    let porta = loop {
        let linha = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("o phxsqld nao abriu a porta em 20 s");
        if let Some(p) = porta_no_texto(&format!("{linha}\n")) {
            break p.unwrap_or_else(|e| panic!("{e}"));
        }
    };
    (filho, rx, porta)
}

/// Manda as linhas numa conexao so; `None` quando ela caiu sem resposta. A
/// resposta que nao e `ok` so e aceita na `inserir`: depois de um `SIGKILL`
/// o indice pode pedir reparo, e o que se mede aqui e a trava, que a
/// abertura gravavel ja tomou.
fn pedir(porta: u16, linhas: &[String]) -> Option<()> {
    let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    for l in linhas {
        writeln!(escrita, "{{\"token\":\"{TOKEN}\",{l}}}").ok()?;
        let mut r = String::new();
        match leitor.read_line(&mut r) {
            Ok(n) if n > 0 => {
                assert!(
                    r.contains("\"ok\":true") || l.contains("\"inserir\""),
                    "{l} -> {r}"
                );
            }
            _ => return None,
        }
    }
    Some(())
}

fn preparar(base: &Path) {
    let (filho, _rx, porta) = subir(base);
    pedir(
        porta,
        &[
            r#""op":"criar_database","database":"loja""#.to_string(),
            r#""op":"criar_tabela","database":"loja","tabela":"vendas",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        ],
    )
    .expect("a criacao nao respondeu");
    drop(filho);
}

/// Mata, espera, e abre `vendas` para gravar pelo motor. `Some` com a recusa.
fn matar_e_abrir(mut filho: Filho, dados: &Path) -> Option<String> {
    let _ = filho.0.kill();
    let _ = filho.0.wait();
    drop(filho);
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    db.abrir_qualificada("vendas")
        .err()
        .map(|e| format!("{e} [{}]", quem_segura(dados)))
}

/// Quem segura a trava, achado pelo SO: todo processo com um descritor no
/// `.phxsql.trava`, com o nome e o pai lidos do `/proc` na hora. A recusa
/// aponta o pid do MORTO (e o que a trava gravou); o detentor e outro.
fn quem_segura(dados: &Path) -> String {
    let mut achados = Vec::new();
    for p in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let pid = p.file_name().to_string_lossy().into_owned();
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let tem = std::fs::read_dir(p.path().join("fd"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|f| {
                std::fs::read_link(f.path())
                    .is_ok_and(|a| a.starts_with(dados) && a.ends_with(".phxsql.trava"))
            });
        if !tem {
            continue;
        }
        let comm = std::fs::read_to_string(p.path().join("comm")).unwrap_or_default();
        let ppid = std::fs::read_to_string(p.path().join("status"))
            .unwrap_or_default()
            .lines()
            .find_map(|l| l.strip_prefix("PPid:").map(|v| v.trim().to_string()))
            .unwrap_or_default();
        achados.push(format!("pid={pid} comm={} ppid={ppid}", comm.trim()));
    }
    achados.join("; ")
}

fn exigir_nenhuma(recusas: Vec<String>) {
    assert!(
        recusas.is_empty(),
        "{} de {VEZES} quedas deixaram a trava presa num processo que herdou o \
         descritor do morto:\n{}",
        recusas.len(),
        recusas.join("\n")
    );
}

/// O servidor comum, sem gancho de teste nenhum, morto logo depois de gravar
/// -- quando a `vigia-disco` pode estar chamando o `df`.
#[test]
fn o_sigkill_no_arranque_nao_deixa_a_trava_no_filho_do_df() {
    let base = DirTemp::novo("queda-vigia");
    preparar(&base.0);
    let dados = base.0.join("dados");
    let mut recusas = Vec::new();
    for vez in 1..=VEZES {
        let (filho, _rx, porta) = subir(&base.0);
        pedir(
            porta,
            &[format!(
                r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{vez}}}"#
            )],
        )
        .expect("a insercao nao respondeu");
        if let Some(e) = matar_e_abrir(filho, &dados) {
            recusas.push(format!("queda {vez}: {e}"));
        }
    }
    exigir_nenhuma(recusas);
}

/// Pedido 759: o irmao no gancho do operador. O canario da sonda vira um
/// DIRETORIO, e a primeira sonda (ja no arranque) falha com erro de E/S: o
/// carteiro chama o gancho logo depois de o servidor subir, quando o teste
/// grava e mata. Com o filho do gancho criado direto pelo servidor, a copia
/// da trava sobrava no orfao.
#[test]
fn o_sigkill_com_o_gancho_ligado_nao_deixa_a_trava_no_filho_do_gancho() {
    let base = DirTemp::novo("queda-gancho");
    preparar(&base.0);
    let dados = base.0.join("dados");
    std::fs::create_dir_all(dados.join(".saude-do-disco")).unwrap();
    // Cada execucao do gancho deixa uma linha aqui: sem ela, um gancho que
    // nunca rodasse passaria este teste sem provar nada.
    let marca = base.0.join("gancho-rodou");
    let extra = format!(
        r#",
        "alertas": {{ "disco": {{ "checar_segundos": 1, "repetir_minutos": 0 }},
          "gancho": {{ "ligado": true, "timeout_s": 5,
            "comando": ["/bin/sh", "-c", "cat >> {}"] }} }}"#,
        marca.display()
    );
    let mut recusas = Vec::new();
    for vez in 1..=VEZES {
        let (filho, _rx, porta) = subir_com(&base.0, &extra);
        pedir(
            porta,
            &[format!(
                r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{vez}}}"#
            )],
        )
        .expect("a insercao nao respondeu");
        if let Some(e) = matar_e_abrir(filho, &dados) {
            recusas.push(format!("queda {vez}: {e}"));
        }
    }
    // As execucoes em voo terminam no lancador (ou no orfao) logo depois.
    std::thread::sleep(Duration::from_millis(500));
    let rodou = std::fs::read_to_string(&marca)
        .unwrap_or_default()
        .lines()
        .count();
    eprintln!("o gancho rodou {rodou} vezes em {VEZES} quedas");
    assert!(
        rodou as u64 >= VEZES / 2,
        "o gancho rodou so {rodou} vezes em {VEZES} quedas: o teste nao exercitou o filho dele"
    );
    exigir_nenhuma(recusas);
}

/// Pedido 759, a forma do conserto: o filho do gancho e filho do LANCADOR (o
/// `phxsqld --lancador-de-ganchos`), nao do servidor -- e as garantias do
/// motor atravessam o lancador inteiras: ambiente limpo, as variaveis do
/// evento e a linha no stdin. Com o servidor morto, o lancador sai junto.
#[test]
fn o_filho_do_gancho_nasce_do_lancador_com_as_garantias_do_motor() {
    let base = DirTemp::novo("gancho-pelo-lancador");
    let dados = base.0.join("dados");
    std::fs::create_dir_all(dados.join(".saude-do-disco")).unwrap();
    let saida = base.0.join("o-que-o-gancho-viu");
    let extra = format!(
        r#",
        "alertas": {{ "gancho": {{ "ligado": true, "timeout_s": 5,
            "comando": ["/bin/sh", "-c", "{{ echo PAI=$PPID; env; cat; }} > {0}.tmp && mv {0}.tmp {0}"] }} }}"#,
        saida.display()
    );
    let (mut filho, _rx, _porta) = subir_com(&base.0, &extra);
    let ate = std::time::Instant::now() + Duration::from_secs(20);
    let visto = loop {
        if let Ok(t) = std::fs::read_to_string(&saida) {
            break t;
        }
        assert!(
            std::time::Instant::now() < ate,
            "o gancho nao rodou em 20 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let pai: u32 = visto
        .lines()
        .find_map(|l| l.strip_prefix("PAI="))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("sem o pai: {visto}"));
    assert_ne!(
        pai,
        filho.0.id(),
        "o gancho nasceu do servidor, com a trava na mao"
    );
    let linha_de_comando = std::fs::read(format!("/proc/{pai}/cmdline")).unwrap_or_default();
    assert!(
        String::from_utf8_lossy(&linha_de_comando).contains("--lancador-de-ganchos"),
        "o pai do gancho nao e o lancador: {:?}",
        String::from_utf8_lossy(&linha_de_comando)
    );
    assert!(visto.contains("PHXSQL_TIPO=entrada_saida"), "{visto}");
    assert!(
        visto.contains("PATH=/usr/local/bin:/usr/bin:/bin"),
        "{visto}"
    );
    assert!(
        !visto.contains("PHXSQL_SEGREDO"),
        "o ambiente do servidor atravessou: {visto}"
    );
    assert!(
        visto.contains("PhxSql"),
        "a linha do stdin nao chegou: {visto}"
    );

    let _ = filho.0.kill();
    let _ = filho.0.wait();
    drop(filho);
    // O lancador ve o fim da entrada e sai. Orfao, quem o colhe e o `init`;
    // zumbi ja nao roda nada, e conta como fora.
    let ate = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let estado = std::fs::read_to_string(format!("/proc/{pai}/status")).unwrap_or_default();
        let vivo = estado
            .lines()
            .find_map(|l| l.strip_prefix("State:"))
            .is_some_and(|s| !s.trim_start().starts_with('Z'));
        if !vivo {
            break;
        }
        assert!(
            std::time::Instant::now() < ate,
            "o lancador {pai} sobreviveu ao servidor por 10 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

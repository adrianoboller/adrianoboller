//! Pedido 483 -- comportamento VELHO: todo argumento que a bancada, o
//! `empacotar.sh`, o MANUAL.txt e os testes de integracao deste crate ja
//! passam ao `phxsqld` hoje continua aceito depois de a linha de comando
//! passar a recusar o que ela nao conhece.
//!
//! O inventario foi levantado por grep em `bancada/`, `testes-web/`,
//! `MANUAL.txt` e nos `crates/*/tests/*.rs` que chamam o binario (nao a
//! biblioteca): cada item abaixo nomeia de onde ele vem. Nenhuma combinacao
//! aqui precisa TERMINAR com sucesso -- a maioria falha por falta de um
//! `config.json` de verdade, que este teste nao monta de proposito. O que
//! importa e que a falha nunca seja "argumento desconhecido" ou "argumento
//! inesperado": essas duas frases so a conferencia nova do pedido 483
//! produz, e sao exatamente o que apareceria se alguem tirasse uma flag do
//! registro (`FLAGS`, em `src/main.rs`) por engano.
//!
//! Prova por PROCESSO, como o arquivo irmao `argumento-desconhecido.rs`: a
//! conferencia e uma funcao privada do `main`, e so se prova de fora rodando
//! o binario de verdade.

mod comum;
use comum::DirTemp;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Mata o processo no `Drop` -- se alguma combinacao aqui vier a abrir um
/// servidor de verdade (o que seria, por si so, uma regressao), o teste nao
/// pode deixar o `phxsqld` escutando a porta.
struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Roda com um prazo: nenhuma combinacao do inventario deveria virar
/// servidor (todas usam um `--config` que nao existe, ou nao pedem config
/// nenhum), mas o prazo protege o `cargo test` de pendurar se uma abrir --
/// e e o mesmo raciocinio do teste irmao para a flag que NAO esta na lista.
fn roda(args: &[&str], cwd: &Path) -> String {
    let saida_arq = cwd.join("saida.txt");
    let erro_arq = cwd.join("erro.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::fs::File::create(&saida_arq).unwrap()))
            .stderr(Stdio::from(std::fs::File::create(&erro_arq).unwrap()))
            .spawn()
            .expect("nao consegui rodar o phxsqld"),
    );
    let ate = Instant::now() + Duration::from_secs(5);
    loop {
        if filho.0.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < ate,
            "phxsqld {args:?} ainda rodava depois de 5 s -- nenhuma flag \
             deste inventario deveria subir servidor sem um config.json real"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut texto = std::fs::read_to_string(&saida_arq).unwrap_or_default();
    texto.push_str(&std::fs::read_to_string(&erro_arq).unwrap_or_default());
    let _ = std::fs::remove_file(&saida_arq);
    let _ = std::fs::remove_file(&erro_arq);
    texto
}

fn confere_nao_rejeitada(args: &[&str], texto: &str) {
    assert!(
        !texto.contains("argumento desconhecido") && !texto.contains("argumento inesperado"),
        "a flag {args:?}, em uso hoje pela bancada/testes/MANUAL, foi recusada \
         como se fosse nova (comportamento velho quebrado):\n{texto}"
    );
}

/// Uma flag do inventario por vez, cada uma comentada com de onde ela vem.
#[test]
fn cada_flag_em_uso_hoje_continua_aceita() {
    let dir = DirTemp::novo("flags-conhecidas");
    let inexistente = "nao-existe.json";

    let casos: &[&[&str]] = &[
        // crates/phxsql-server/tests/versao.rs; primeira linha de todo roteiro
        &["--version"],
        &["-V"],
        // uso interativo direto
        &["--help"],
        &["-h"],
        // empacotar.sh, bancada/usuarios/provar.py, bancada/diretivas/*.py,
        // bancada/comparativo/medir.py, testes-web/*.mjs -- sempre pelo cano
        &["--senha"],
        // MANUAL.txt secao do 2o fator
        &["--gerar-chave"],
        // bancada/cifra-do-fio/prova.py, testes-web/exercitar-fio-e-rele.mjs
        &["--config", inexistente, "--chave-do-fio"],
        // MANUAL.txt: o Centro de Controle como arquivo unico
        &["--pagina"],
        // empacotar.sh, MANUAL.txt 7.1
        &["--exemplo", "1"],
        &["--exemplo", "2"],
        &["--exemplo", "3"],
        // config-phz.rs, mcp_stdio.rs, caminhos-do-config.rs -- todo teste de
        // integracao deste crate sobe assim
        &["--config", inexistente],
        // bancada/diretivas/provar.py: "phxsqld --config <c> --usuarios"
        &["--config", inexistente, "--usuarios"],
        // MANUAL.txt (bloqueios/desbloquear); bancada/seguranca/porta.py,
        // bancada/seguranca/injecao.py, bancada/replicacao/credencial-recusada.py
        &["--config", inexistente, "--bloqueios"],
        &["--desbloquear", "203.0.113.9", "--config", inexistente],
        // MANUAL.txt secao do log de acessos
        &["--config", inexistente, "--acessos"],
        // crates/phxsql-server/tests/mcp_stdio.rs
        &["--config", inexistente, "--mcp"],
        &["--config", inexistente, "--mcp", "--escrita"],
        &["--mcp", "--usuario", "adriano", "--config", inexistente],
        // empacotar.sh (a migracao de verdade), config-phz.rs
        &["--empacotar-config", "--config", inexistente],
        &["--desempacotar-config", "--config", inexistente],
    ];

    for args in casos {
        let texto = roda(args, &dir);
        confere_nao_rejeitada(args, &texto);
    }
}

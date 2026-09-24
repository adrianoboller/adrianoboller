//! Pedido 483 -- o `phxsqld` reagia so as flags que conhecia e IGNORAVA o
//! resto. Achado pela frente do 478 (cognicao
//! `binario-velho-ignora-flag-nova-e-vira-servidor`): um `phxsqld` de antes
//! de `--empacotar-config` existir recebeu essa flag, nao a reconheceu, e
//! caiu direto no arranque do servidor usando o `--config` que sobrou --
//! quatro threads, `acessos.log` aberto e a porta de dados em `accept()`.
//!
//! Os tres motores maduros (`postgres`, `mysqld`, `mariadbd`) recusam opcao
//! desconhecida saindo com erro; e a prova aqui e PELO PROCESSO, nao pela
//! funcao, porque o defeito so aparece no `main` real: um teste unitario da
//! biblioteca nunca chega a decidir se sobe servidor.

mod comum;
use comum::DirTemp;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Mata o processo no `Drop`, inclusive quando uma assercao falha no meio --
/// senao um teste que falha deixa o `phxsqld` escutando a porta para sempre.
struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Um `config.json` minimo e valido -- o bastante para o arranque normal
/// continuar depois da conferencia de argumentos. E o que reproduz o
/// incidente de verdade: sozinha, uma flag desconhecida ja cai no
/// `Config::ler` do "config.json" padrao ausente e falha por OUTRO motivo;
/// com um `--config` valido do lado (como o `empacotar.sh` sempre passa), a
/// flag desconhecida sozinha bastava para o binario velho completar o
/// arranque.
fn escrever_config_valido(dir: &Path) {
    std::fs::write(
        dir.join("config.json"),
        "{\n  \"bind\": \"127.0.0.1:0\",\n  \"token\": \"t\",\n  \
         \"web\": { \"ligado\": false }\n}\n",
    )
    .unwrap();
}

/// Roda o `phxsqld` com um PRAZO: se ele nao voltar ate `prazo`, mata o
/// processo e devolve `None` -- e o teste falha dizendo que o binario ainda
/// rodava, em vez de o `cargo test` pendurar para sempre. A prova real dos
/// DOIS lados mora neste prazo: com o conserto, o binario volta em
/// milissegundos; com o defeito reposto (a conferencia removida), ele nunca
/// volta, porque virou um servidor de verdade no `accept()` -- medido a mao
/// antes desta bateria existir: sem a conferencia, o mesmo comando abriu
/// `acessos.log`, criou `dados/` e ficou escutando ate ser morto.
fn rodar_com_prazo(args: &[&str], cwd: &Path, prazo: Duration) -> Option<(bool, String, Duration)> {
    let saida_arq = cwd.join("prova483-saida.txt");
    let erro_arq = cwd.join("prova483-erro.txt");
    let inicio = Instant::now();
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
    let ate = inicio + prazo;
    let status = loop {
        if let Some(s) = filho.0.try_wait().unwrap() {
            break s;
        }
        if Instant::now() >= ate {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut texto = std::fs::read_to_string(&saida_arq).unwrap_or_default();
    texto.push_str(&std::fs::read_to_string(&erro_arq).unwrap_or_default());
    let _ = std::fs::remove_file(&saida_arq);
    let _ = std::fs::remove_file(&erro_arq);
    Some((status.success(), texto, inicio.elapsed()))
}

/// **A prova real do 483, nos dois sentidos.** Com o conserto: recusa em
/// menos de 1 s, sem abrir porta nem criar `acessos.log`/`dados/`. Com o
/// defeito reposto (remova a chamada a `conferir_argumentos` no `main`): o
/// processo vira servidor, nunca volta, e o `unwrap` do prazo falha dizendo
/// isso -- provado a mao antes desta bateria, com `timeout 3` matando o
/// processo pendurado e `dados/`/`acessos.log` la, exatamente como no
/// incidente do `empacotar.sh`.
#[test]
fn flag_desconhecida_com_config_valido_recusa_sem_subir_servidor() {
    let dir = DirTemp::novo("flag-desconhecida");
    escrever_config_valido(&dir);

    let resultado = rodar_com_prazo(
        &["--flag-que-nao-existe-483", "--config", "config.json"],
        &dir,
        Duration::from_secs(5),
    );
    let (ok, texto, decorrido) = resultado.unwrap_or_else(|| {
        panic!(
            "phxsqld --flag-que-nao-existe-483 ainda rodava depois de 5 s: \
             subiu como servidor (defeito do pedido 483 reposto)"
        )
    });

    assert!(
        !ok,
        "flag desconhecida devia sair com erro; saida:\n{texto}"
    );
    assert!(
        decorrido < Duration::from_secs(1),
        "devia recusar em menos de 1 s, levou {decorrido:?}: {texto}"
    );
    assert!(
        texto.contains("--flag-que-nao-existe-483"),
        "a mensagem devia nomear o argumento recusado: {texto}"
    );
    assert!(
        texto.contains("--help") && texto.contains("--mcp"),
        "a mensagem devia apontar --help e listar o que conhece: {texto}"
    );

    // O sintoma do incidente original: nem o log de acessos, nem a base
    // de dados nascem quando a recusa acontece antes do arranque.
    assert!(
        !dir.join("acessos.log").exists(),
        "acessos.log nao devia nascer -- a recusa e ANTES do arranque"
    );
    assert!(
        !dir.join("dados").exists(),
        "a base 'dados' nao devia nascer -- a recusa e ANTES do arranque"
    );
}

/// A mesma flag, agora sozinha (sem `--config` nenhum do lado): tem de
/// recusar rapido e sem tocar em disco, mesmo sem um `config.json` por
/// perto -- e o caso mais simples que o pedido pede explicitamente.
#[test]
fn flag_desconhecida_sozinha_recusa_rapido_sem_efeito_colateral() {
    let dir = DirTemp::novo("flag-desconhecida-sozinha");

    let inicio = Instant::now();
    let saida = Command::new(env!("CARGO_BIN_EXE_phxsqld"))
        .arg("--outra-flag-que-nao-existe")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .expect("nao consegui rodar o phxsqld");
    let decorrido = inicio.elapsed();

    assert!(!saida.status.success(), "devia sair com erro (codigo != 0)");
    assert!(
        decorrido < Duration::from_secs(1),
        "devia recusar em menos de 1 s, levou {decorrido:?}"
    );
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        erro.contains("--outra-flag-que-nao-existe"),
        "a mensagem devia nomear o argumento: {erro}"
    );

    // Diretorio continua vazio: nenhum arquivo nasceu so de tentar subir.
    let restante: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert!(
        restante.is_empty(),
        "nenhum arquivo devia nascer so de recusar o argumento: {restante:?}"
    );
}

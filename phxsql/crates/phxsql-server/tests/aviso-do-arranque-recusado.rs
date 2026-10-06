//! Pedido 573 -- o arranque que acha a sentinela do 509 AVISA o operador
//! pelo carteiro do 249 antes de recusar subir.
//!
//! # Por que o `phxsqld` de verdade
//!
//! O `fsync` recusado derruba o processo (509) e o carteiro morre junto: o
//! unico que ainda pode avisar e o arranque seguinte, e ele tambem nao sobe.
//! O que se prova e o PROCESSO -- o binario sai com erro, diz o porque no
//! erro padrao, e o gancho do operador rodou antes de ele sair.
//!
//! A sentinela e plantada com o `boot_id` DESTE boot, que e o caso que
//! recusa (outro boot sobe e apaga a sentinela).
//!
//! O vermelho: sem a chamada de `avisar_o_arranque_recusado` no
//! `conferir_sentinela_509`, o arranque recusa igual e o gancho nunca roda.

mod comum;
use comum::{DirTemp, Filho};

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
#[test]
fn a_sentinela_do_509_avisa_pelo_gancho_e_o_arranque_recusa_dizendo_por_que() {
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .expect("este teste precisa do boot_id do linux")
        .trim()
        .to_string();
    let d = DirTemp::novo("aviso-do-arranque-recusado");
    let base = d.join("base");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join(".fsync-recusado"),
        format!(
            "boot_id={boot}\ncaminho=/dados/loja/clientes.reg\nerro=No space left on device (os error 28)\nquando_ms=1\n"
        ),
    )
    .unwrap();
    let aviso = d.join("aviso.txt");
    let config = d.join("config.json");
    let bar = |p: &std::path::Path| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &config,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "t",
              "web": {{ "ligado": false }},
              "alertas": {{ "gancho": {{ "ligado": true, "timeout_s": 10,
                "comando": ["/bin/sh", "-c", "cat > {}"] }} }}
            }}"#,
            bar(&base),
            bar(&aviso),
        ),
    )
    .unwrap();
    let erro_padrao = d.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(&*d)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let ate = Instant::now() + Duration::from_secs(30);
    let saida = loop {
        if let Some(st) = filho.0.try_wait().unwrap() {
            break st;
        }
        assert!(
            Instant::now() < ate,
            "o phxsqld subiu com a sentinela do 509 deste boot: {}",
            std::fs::read_to_string(&erro_padrao).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
    assert!(
        !saida.success(),
        "o arranque recusado saiu com sucesso: {texto}"
    );
    assert!(
        texto.contains("NAO sobe") && texto.contains("pedido 509"),
        "a recusa nao diz o porque: {texto}"
    );
    // O aviso SAIU, pelo gancho -- e antes de o processo sair, porque o
    // carteiro do arranque espera o gancho terminar.
    let linha = std::fs::read_to_string(&aviso)
        .unwrap_or_else(|e| panic!("o gancho do operador nao rodou ({e}); erro padrao:\n{texto}"));
    assert!(
        linha.contains("arranque") && linha.contains("PhxSql"),
        "a linha do gancho nao diz do que se trata: {linha}"
    );
    // A linha do gancho e a do SMS: sem caminho (atravessa a operadora).
    assert!(!linha.contains("/dados"), "{linha}");
    assert!(
        texto.contains("SAUDE DO DISCO") && texto.contains("arranque"),
        "o evento nao passou pelo carteiro: {texto}"
    );
}

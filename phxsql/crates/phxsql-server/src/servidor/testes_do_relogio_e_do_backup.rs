//! O relogio de jobs que morre (irmao do pedido 452) e o backup agendado que
//! falha calado (pedido 510) -- as duas threads de servico que ninguem via
//! morrer ou errar.
use super::*;
use crate::apoio_teste::{rele_falso, DirTemp};

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.join("dados"),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    c
}

/// **O relogio que morre desliga o «no ar».**
///
/// Com o defeito (a marca sem `Drop`): o relogio morre na primeira volta,
/// o `relogio_no_ar` segue verdadeiro, e o job vencido NUNCA aparece como
/// parado -- o vigia de e-mail nao tem o que avisar. Com o conserto: a
/// marca sai com a thread, e o job aparece parado.
#[test]
fn relogio_de_jobs_que_morre_nao_continua_dizendo_que_esta_no_ar() {
    let dir = DirTemp::novo("relogio-452");
    std::fs::write(
        dir.join("jobs.json"),
        r#"{"jobs":[{"nome":"eco","ligado":true,"cada_minutos":60,"usuario":"",
                "pedido":{"op":"ping"}}]}"#,
    )
    .unwrap();
    let s = Servidor::novo(config_base(&dir)).unwrap();
    s.panico_no_relogio_de_jobs_de_teste
        .store(true, Ordering::SeqCst);
    s.subir_jobs();
    let ate = Instant::now() + Duration::from_secs(3);
    while s.relogio_de_jobs_no_ar() && Instant::now() < ate {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !s.relogio_de_jobs_no_ar(),
        "o relogio de jobs MORREU e o servidor continua dizendo que ele esta \
             no ar -- 3 s depois"
    );
    let r = s
        .executar("jobs", &Json::objeto(vec![]), &Sessao::default())
        .unwrap();
    let job = r
        .campo("jobs")
        .and_then(Json::lista)
        .and_then(|l| l.first())
        .cloned()
        .unwrap();
    assert_eq!(
        job.campo("parado").and_then(Json::booleano),
        Some(true),
        "o job vencido sem relogio tinha de aparecer PARADO: {}",
        job.escrever()
    );
}

/// **O amostrador que morre desliga o «no ar» do retrato** -- o mesmo
/// irmao do 452. Com o defeito: `amostrador: true` 3 s depois da morte.
#[test]
fn amostrador_que_morre_nao_continua_dizendo_que_esta_no_ar() {
    let dir = DirTemp::novo("amostrador-452");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    s.panico_no_amostrador_de_teste
        .store(true, Ordering::SeqCst);
    s.subir_amostrador();
    let ate = Instant::now() + Duration::from_secs(3);
    while s.telemetria.amostrador_no_ar() && Instant::now() < ate {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !s.telemetria.amostrador_no_ar(),
        "o amostrador MORREU e o retrato continua dizendo que ele esta no ar"
    );
}

/// **C1 do parecer do DBA, o irmao do backup:** a lapide com hora no
/// FUTURO nao vira a ultima corrida. Sem o `min`: o arranque devolve a
/// hora de daqui a um ano, e o backup de 24 h so voltaria depois dela.
#[test]
fn lapide_do_backup_no_futuro_nao_vira_a_ultima_corrida() {
    let dir = DirTemp::novo("lapide-backup-futuro");
    let mut c = config_base(&dir);
    c.backup.agendado = true;
    c.backup.destino = dir.join("backups");
    c.backup.cada_horas = 24;
    let s = Servidor::novo(c).unwrap();
    let um_ano: i64 = 365 * 86_400_000;
    let futuro = crate::agora_ms() + um_ano;
    std::fs::create_dir_all(dir.join("backups")).unwrap();
    std::fs::write(
        dir.join("backups").join(LAPIDE_DO_BACKUP),
        Json::objeto(vec![
            ("quando_ms", Json::de_i64(futuro)),
            ("base", Json::texto_de(s.config.base.display().to_string())),
        ])
        .escrever(),
    )
    .unwrap();
    let ultimo = s.lapide_do_backup_no_arranque();
    assert!(
        ultimo > 0 && ultimo <= crate::agora_ms(),
        "a lapide do futuro virou a ultima corrida do backup: {ultimo} (futuro {futuro})"
    );
}

/// **C2 do parecer do DBA:** a corrida de job que o arranque fecha como
/// FALHOU avisa por e-mail, como a falha comum. Sem a chamada no
/// `subir_jobs`: nenhum e-mail em 10 s (medido pelo DBA: 0 contra 1).
#[test]
fn corrida_de_job_interrompida_avisa_por_email() {
    let dir = DirTemp::novo("interrompida-avisa");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    c.alertas.email.ligado = true;
    c.alertas.email.avisar_jobs = true;
    c.alertas.email.servidor = "127.0.0.1".into();
    c.alertas.email.porta = porta;
    c.alertas.email.de = "phxsql@exemplo.com".into();
    c.alertas.email.para = vec!["admin@exemplo.com".into()];
    c.alertas.email.timeout_s = 5;
    std::fs::write(
        dir.join("jobs.json"),
        r#"{"jobs":[{"nome":"noturno","ligado":true,"cada_minutos":60,"usuario":"",
                "pedido":{"op":"ping"}}]}"#,
    )
    .unwrap();
    {
        let mut r = crate::jobs::Registro::abrir(&dir.join("jobs.json")).unwrap();
        r.registrar_inicio("noturno", "ping", "", crate::agora_ms() - 10_000);
    }
    let s = Servidor::novo(c).unwrap();
    s.subir_jobs();
    let bruto = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("a corrida interrompida virou FALHOU e ninguem foi avisado em 10 s");
    let (cabecalho, texto) = corpo(&bruto);
    assert!(cabecalho.contains("job noturno falhou"), "{cabecalho}");
    assert!(texto.contains("nunca terminou"), "{texto}");
}

/// O corpo de um e-mail do rele falso, decodificado.
fn corpo(bruto: &str) -> (String, String) {
    let (cabecalho, corpo) = bruto.split_once("\r\n\r\n").unwrap();
    let texto = phxsql_core::base64::decodificar_texto(&corpo.replace("\r\n", "")).unwrap();
    (cabecalho.to_string(), texto)
}

/// Um servidor com o backup agendado em `destino`, o e-mail no rele
/// falso, a sonda ligada (o carteiro) e o backup no ar.
fn backup_para(
    dir: &std::path::Path,
    destino: std::path::PathBuf,
) -> (Arc<Servidor>, std::sync::mpsc::Receiver<String>) {
    let (porta, caixa) = rele_falso();
    let mut c = config_base(dir);
    c.backup.agendado = true;
    c.backup.destino = destino;
    c.backup.hora = String::new();
    c.backup.cada_horas = 24;
    c.alertas.email.ligado = true;
    c.alertas.email.servidor = "127.0.0.1".into();
    c.alertas.email.porta = porta;
    c.alertas.email.de = "phxsql@exemplo.com".into();
    c.alertas.email.para = vec!["admin@exemplo.com".into()];
    c.alertas.email.timeout_s = 5;
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    s.subir_backup_agendado();
    (s, caixa)
}

/// **O pedido 510: o backup agendado que falha AVISA.**
///
/// A falha e do sistema operacional, e nao fabricada: o destino fica
/// DENTRO de um arquivo comum, e o `mkdir` recusa com `ENOTDIR`. (Destino
/// sem permissao nao serve de prova aqui: o conteiner roda como root, e
/// root atravessa a permissao.) O aviso chega ao rele falso pelo soquete,
/// pelo carteiro da saude do disco.
///
/// Com o defeito (so o erro padrao): nenhum e-mail em 10 s. Com o
/// conserto: um, que diz que o backup FALHOU, o destino e o erro do
/// sistema.
#[test]
fn backup_agendado_que_falha_avisa_pelo_carteiro() {
    let dir = DirTemp::novo("backup-510");
    std::fs::write(dir.join("arquivo-comum"), b"nao sou pasta").unwrap();
    let destino = dir.join("arquivo-comum").join("backups");
    let (_s, caixa) = backup_para(&dir, destino.clone());
    let bruto = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o backup agendado FALHOU e ninguem foi avisado em 10 s");
    let (cabecalho, texto) = corpo(&bruto);
    assert!(
        cabecalho.contains("backup agendado FALHOU"),
        "o assunto nao diz o que houve: {cabecalho}"
    );
    assert!(
        texto.contains(&destino.display().to_string()) && texto.contains("os error 20"),
        "o corpo nao diz o destino nem o erro do sistema: {texto}"
    );
    assert!(
        !texto.contains("disco onde o banco grava"),
        "o aviso do backup saiu com o texto da saude do disco: {texto}"
    );
}

/// **O mesmo aviso com o disco CHEIO de verdade** -- um `tmpfs` de 64 KiB
/// montado para a prova, e uma base maior que ele. Precisa de `mount`, e
/// por isso nao roda na suite: roda-se a mao, como root, e o vermelho e o
/// verde dele estao no `docs/SAUDE-DO-DISCO.md` §4.4.
#[test]
#[ignore = "precisa montar um tmpfs (root); roda-se a mao -- pedido 510"]
fn backup_agendado_em_disco_cheio_avisa_pelo_carteiro() {
    let dir = DirTemp::novo("backup-510-cheio");
    let ponto = dir.join("cheio");
    std::fs::create_dir_all(&ponto).unwrap();
    let montou = std::process::Command::new("mount")
        .args(["-t", "tmpfs", "-o", "size=64k", "tmpfs"])
        .arg(&ponto)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(
        montou,
        "NAO MEDIDO: o mount do tmpfs falhou (precisa de root)"
    );
    struct Desmonta(std::path::PathBuf);
    impl Drop for Desmonta {
        fn drop(&mut self) {
            let _ = std::process::Command::new("umount").arg(&self.0).status();
        }
    }
    let _desmonta = Desmonta(ponto.clone());
    // Uma base maior que o tmpfs, em bytes que nao comprimem.
    std::fs::create_dir_all(dir.join("dados")).unwrap();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let lastro: Vec<u8> = (0..256 * 1024)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    std::fs::write(dir.join("dados").join("lastro.bin"), lastro).unwrap();
    let (_s, caixa) = backup_para(&dir, ponto.join("backups"));
    let bruto = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o backup agendado FALHOU em disco cheio e ninguem foi avisado");
    let (_, texto) = corpo(&bruto);
    assert!(texto.contains("os error 28"), "nao e o ENOSPC: {texto}");
}

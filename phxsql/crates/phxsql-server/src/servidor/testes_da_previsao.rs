//! Pedido 496, fatia F5: a previsao de esgotamento contra o SISTEMA
//! OPERACIONAL, e o caminho dela ate o sedimento do aquario.
//!
//! O unitario da conta mora em `previsao.rs`. Aqui se prova o que o
//! unitario nao alcanca: o `df` de verdade, num tmpfs de 64 MiB enchendo a
//! 8 MiB/s, avisando ANTES do `ENOSPC` -- e o aviso chegando pelo produtor
//! unico (`telemetria::sinal`), nao so voltando da funcao.
//!
//! RED do produtor: tirar o `sinal` do `Servidor::prever` -> a pedra do
//! sedimento nao aparece e os dois testes daqui caem; tirar a chamada do
//! `amostrar_e_prever` -> nenhum aviso antes do `ENOSPC`.
use super::*;
use crate::aquario::Alarme;
use std::io::Write;
use std::time::Instant;

/// Quantas vezes um alarme ja foi ao sedimento neste processo.
fn vezes(alarme: Alarme) -> u64 {
    crate::aquario::alarme::sedimento()
        .into_iter()
        .find(|p| p.alarme == alarme)
        .map_or(0, |p| p.vezes)
}

fn servidor_vigiando(base: &std::path::Path, extra: &std::path::Path) -> Arc<Servidor> {
    let mut c = Config {
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O piso no zero: o alvo e o proprio `ENOSPC`, que e o que se mede.
    c.alertas.livre_minimo_percentual = 0.0;
    c.alertas.livre_minimo_mb = 0;
    c.alertas.caminhos = vec![extra.to_path_buf()];
    Servidor::novo(c).unwrap()
}

/// O caminho do servidor, sem SO: uma serie que desce depressa vira a pedra
/// critica do sedimento pelo produtor unico, com o recurso no aviso.
#[test]
fn a_previsao_chega_ao_sedimento_pelo_produtor_unico() {
    let dir = DirTemp::novo("previsao-sedimento");
    let s = servidor_vigiando(&dir.0, &dir.0);
    let antes = vezes(Alarme::EsgotamentoIminente);
    let mut ultimo = Vec::new();
    for i in 0..6i64 {
        let l = vec![crate::previsao::Leitura {
            recurso: "descritores".into(),
            resta: 1_000 - 100 * i as u64,
            piso: 0,
        }];
        ultimo = s.prever(i * 60_000, &l);
    }
    assert_eq!(ultimo.len(), 1, "{ultimo:?}");
    assert_eq!(ultimo[0].recurso, "descritores");
    assert!(
        vezes(Alarme::EsgotamentoIminente) >= antes + 3,
        "a previsao nao chegou ao sedimento"
    );
}

const AMBIENTE: &str = "PHX_F5_TMPFS";
const TAXA_BYTES_S: f64 = 8.0 * 1024.0 * 1024.0;
const BLOCO: usize = 256 * 1024;
const AMOSTRA: Duration = Duration::from_millis(500);

/// A prova contra o SO. Preferencia: `unshare -m` (o tmpfs nasce e morre
/// num espaco de montagem privado, e nada vaza para a maquina); sem ele, o
/// tmpfs montado pelo proprio teste, como a A6. Sem nenhum dos dois, diz
/// NAO PROVADO -- nunca verde por omissao.
#[cfg(target_os = "linux")]
#[test]
fn previsao_contra_o_so_avisa_antes_do_enospc() {
    if let Ok(ponto) = std::env::var(AMBIENTE) {
        corpo_contra_o_so(std::path::Path::new(&ponto), "unshare -m");
        return;
    }
    let dir = DirTemp::novo("previsao-so");
    let ponto = dir.0.join("tmpfs");
    std::fs::create_dir_all(&ponto).unwrap();
    let nome = format!(
        "{}::previsao_contra_o_so_avisa_antes_do_enospc",
        module_path!().split_once("::").map_or("", |(_, r)| r)
    );
    let exe = std::env::current_exe().unwrap();
    let saida = std::process::Command::new("unshare")
        .args(["-m", "--propagation", "private", "sh", "-c"])
        .arg(r#"mount -t tmpfs -o size=64m tmpfs "$1" && exec "$2" --exact "$3" --nocapture --test-threads=1"#)
        .arg("sh")
        .arg(&ponto)
        .arg(&exe)
        .arg(&nome)
        .env(AMBIENTE, &ponto)
        .output();
    if let Ok(o) = &saida {
        let texto =
            String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
        if texto.contains("F5-INICIO") {
            eprintln!("{texto}");
            assert!(
                o.status.success() && texto.contains("F5-PROVA OK"),
                "a prova dentro do unshare falhou:\n{texto}"
            );
            return;
        }
        eprintln!("unshare nao subiu a prova: {}", texto.trim());
    }
    // Sem `unshare`: o tmpfs montado aqui mesmo.
    let montou = std::process::Command::new("mount")
        .args(["-t", "tmpfs", "-o", "size=64m", "tmpfs"])
        .arg(&ponto)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !montou {
        eprintln!("NAO PROVADO: sem unshare e sem mount (precisa de CAP_SYS_ADMIN)");
        return;
    }
    struct Desmonta(std::path::PathBuf);
    impl Drop for Desmonta {
        fn drop(&mut self) {
            let _ = std::process::Command::new("umount")
                .arg("-l")
                .arg(&self.0)
                .output();
        }
    }
    let _desmonta = Desmonta(ponto.clone());
    corpo_contra_o_so(&ponto, "tmpfs montado pelo teste");
}

/// O tmpfs ja montado em `ponto`: um escritor a 8 MiB/s ate o `ENOSPC`, e o
/// vigia amostrando a cada 0,5 s pelo MESMO `amostrar_e_prever` da thread
/// do servidor.
fn corpo_contra_o_so(ponto: &std::path::Path, via: &str) {
    eprintln!("F5-INICIO via {via}");
    let base = DirTemp::novo("previsao-so-base");
    let s = servidor_vigiando(&base.0, ponto);
    let recurso = format!("disco:{}", ponto.display());
    let pedras_antes = vezes(Alarme::EsgotamentoIminente) + vezes(Alarme::EsgotamentoPrevisto);

    let t0 = Instant::now();
    let arquivo = ponto.join("cresce.log");
    let escritor = std::thread::spawn(move || -> (f64, std::io::ErrorKind) {
        let mut f = std::fs::File::create(&arquivo).unwrap();
        let bloco = vec![b'x'; BLOCO];
        let por_bloco = BLOCO as f64 / TAXA_BYTES_S;
        for k in 0u64.. {
            // Pelo relogio, e nao por `sleep` fixo: o tempo da propria
            // escrita nao pode baixar a taxa.
            let alvo = t0 + Duration::from_secs_f64(k as f64 * por_bloco);
            if let Some(falta) = alvo.checked_duration_since(Instant::now()) {
                std::thread::sleep(falta);
            }
            if let Err(e) = f.write_all(&bloco) {
                return (t0.elapsed().as_secs_f64(), e.kind());
            }
        }
        unreachable!()
    });

    // (instante da rodada em s, cruzamento previsto em s), so do disco.
    let mut previsoes: Vec<(f64, f64)> = Vec::new();
    while !escritor.is_finished() {
        let t = t0.elapsed().as_secs_f64();
        let avisos = s.amostrar_e_prever(crate::agora_ms());
        if let Some(a) = avisos.iter().find(|a| a.recurso == recurso) {
            previsoes.push((t, t + a.previsao.horas * 3_600.0));
        }
        std::thread::sleep(AMOSTRA);
    }
    let (enospc, tipo) = escritor.join().unwrap();
    assert_eq!(
        tipo,
        std::io::ErrorKind::StorageFull,
        "o escritor parou por outro motivo"
    );
    let Some(&(primeiro, _)) = previsoes.first() else {
        panic!("nenhum aviso de disco antes do ENOSPC em {enospc:.2} s");
    };
    let erros: Vec<f64> = previsoes
        .iter()
        .map(|&(_, p)| (p - enospc) / enospc)
        .collect();
    let medio = erros.iter().map(|e| e.abs()).sum::<f64>() / erros.len() as f64;
    let pior = erros.iter().fold(0.0f64, |m, e| m.max(e.abs()));
    let pior_otimista = erros.iter().fold(f64::MIN, |m, &e| m.max(e));
    let antecedencia = (enospc - primeiro) / enospc;
    eprintln!(
        "F5-MEDIDA via {via}: ENOSPC em {enospc:.2} s; 1o aviso em {primeiro:.2} s \
         ({:.2} s antes, {:.0}% de antecedencia); {} previsoes; erro medio {:.1}%, \
         pior {:.1}%, pior otimista {:+.1}%",
        enospc - primeiro,
        antecedencia * 100.0,
        previsoes.len(),
        medio * 100.0,
        pior * 100.0,
        pior_otimista * 100.0
    );
    assert!(primeiro < enospc);
    assert!(
        antecedencia >= 0.5,
        "antecedencia {:.0}% < 50%",
        antecedencia * 100.0
    );
    // O aviso saiu pelo produtor unico, nao so voltou da funcao.
    let pedras = vezes(Alarme::EsgotamentoIminente) + vezes(Alarme::EsgotamentoPrevisto);
    assert!(
        pedras >= pedras_antes + previsoes.len() as u64,
        "o aviso nao chegou ao sedimento ({pedras_antes} -> {pedras})"
    );
    eprintln!("F5-PROVA OK");
}

// ------------------------------------------------------------------ F6
//
// A contagem regressiva ate o teto duro, pelo servidor de verdade: tabelas
// paginadas num diretorio real, abertas SO para ler pelo vigia, e o aviso
// chegando ao sedimento pelo produtor unico.
//
// RED: o `>` da razao trocado por `>=` (previsao.rs, `degrau_da_razao`) ->
// `a_fronteira_de_80_por_cento_e_estrita` cai no 80% exato; tirar o `sinal`
// do `contar_regressivo` -> `a_tabela_a_81_por_cento...` cai no sedimento.
use crate::previsao::{Contagem, Degrau};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

/// Uma tabela paginada de capacidade 1.000 (100 por arquivo, 10 arquivos)
/// com `linhas` gravadas, e sincronizada: o vigia a abre por outra porta.
fn tabela_paginada(dir: &std::path::Path, nome: &str, linhas: i64, bytes_log: Option<u64>) {
    let inst = phxsql_store::catalogo::Instancia::nova(dir).unwrap();
    let db = inst.garantir_database("loja").unwrap();
    let mut p = Paginacao::nova(100, 10).unwrap();
    if let Some(b) = bytes_log {
        p = p.com_bytes_por_arquivo(b).unwrap();
    }
    let e = Schema::new(
        nome,
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![],
    )
    .unwrap()
    .com_paginacao(p)
    .unwrap();
    let mut t = db.criar_tabela(None, e).unwrap();
    for i in 0..linhas {
        t.inserir(&[Value::Int(i)]).unwrap();
    }
    t.sincronizar().unwrap();
}

fn servidor_simples(base: &std::path::Path) -> Arc<Servidor> {
    let mut c = Config {
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    Servidor::novo(c).unwrap()
}

/// **A prova do aceite.** Tabela a 81% da capacidade avisa; a 79% nada; e o
/// aviso chega ao sedimento pelo produtor unico, nomeando a tabela.
#[test]
fn a_tabela_a_81_por_cento_avisa_e_a_79_nao() {
    let d = DirTemp::novo("contagem-tabela");
    let s = servidor_simples(&d.0);
    tabela_paginada(&d.0, "cheia", 810, None);
    tabela_paginada(&d.0, "folgada", 790, None);
    let antes = vezes(Alarme::EsgotamentoPrevisto);
    let avisos = s.amostrar_e_contar(crate::agora_ms());
    let da_cheia = avisos.iter().find(|a| a.recurso == "tabela:loja.cheia");
    let a = da_cheia.unwrap_or_else(|| panic!("81% sem aviso: {avisos:?}"));
    assert_eq!(a.degrau, Degrau::Aviso, "{a:?}");
    assert_eq!((a.usado, a.teto), (810, 1_000));
    assert!(
        avisos.iter().all(|a| a.recurso != "tabela:loja.folgada"),
        "79% nao e noticia: {avisos:?}"
    );
    assert!(
        vezes(Alarme::EsgotamentoPrevisto) > antes,
        "o aviso nao chegou ao sedimento pelo produtor unico"
    );
}

/// O diario que rola por tamanho chega ao teto de VOLUMES: com 20 bytes por
/// volume, cada evento vira um volume e 9 linhas gastam 9 dos 10 arquivos.
#[test]
fn o_diario_perto_do_teto_de_volumes_avisa() {
    let d = DirTemp::novo("contagem-diario");
    let s = servidor_simples(&d.0);
    tabela_paginada(&d.0, "diaria", 9, Some(20));
    tabela_paginada(&d.0, "calma", 1, Some(20));
    let avisos = s.amostrar_e_contar(crate::agora_ms());
    let a = avisos
        .iter()
        .find(|a| a.recurso == "diario:loja.diaria")
        .unwrap_or_else(|| panic!("diario sem aviso: {avisos:?}"));
    assert!(a.usado >= 9 && a.teto == 10, "{a:?}");
    assert!(
        avisos.iter().all(|a| a.recurso != "diario:loja.calma"),
        "{avisos:?}"
    );
}

/// O backup que nunca rodou no periodo avisa; o que rodou, nao. Tempo
/// injetado: o teste vive horas num instante.
#[test]
fn o_backup_que_nunca_rodou_no_periodo_avisa() {
    let d = DirTemp::novo("contagem-backup");
    let destino = d.0.join("destino-vazio");
    std::fs::create_dir_all(&destino).unwrap();
    let mut c = Config {
        base: d.0.join("dados"),
        log_acessos: d.0.join("acessos.log"),
        blacklist: d.0.join("blacklist.json"),
        dblink: d.0.join("dblink.json"),
        jobs: d.0.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    c.backup.agendado = true;
    c.backup.cada_horas = 1;
    c.backup.destino = destino;
    let s = Servidor::novo(c).unwrap();
    let t0 = 1_800_000_000_000i64; // um instante fixo
    let hora = 3_600_000i64;
    let idade = |s: &Servidor, quando: i64| {
        s.amostrar_e_contar(quando)
            .into_iter()
            .find(|a| a.recurso == "backup:idade")
    };
    assert!(idade(&s, t0).is_none(), "recem-subido nao deve backup");
    assert!(
        idade(&s, t0 + hora * 11 / 10).is_none(),
        "1,1 periodo: ainda cedo"
    );
    let a = idade(&s, t0 + hora * 16 / 10).expect("1,6 periodo sem backup tem de avisar");
    assert_eq!(a.degrau, Degrau::Critico, "{a:?}");
    // Controle: depois de um backup bom a idade zera e o aviso some.
    s.backup_marcas.deu_certo(t0 + hora * 16 / 10, 1_000);
    assert!(idade(&s, t0 + hora * 17 / 10).is_none());
}

/// O irmao do agendado: um job de `op: backup` DESLIGADO nunca roda, e a
/// ausencia dele e o caso da C6.
#[test]
fn o_job_de_backup_desligado_avisa() {
    let d = DirTemp::novo("contagem-job");
    let s = servidor_simples(&d.0);
    let job = crate::jobs::Job::de_json(
        &Json::analisar(
            r#"{"nome":"noturno","ligado":false,"cada_minutos":60,
                "pedido":{"op":"backup","destino":"x"}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    s.jobs.lock().unwrap().jobs.push(job);
    let t0 = 1_800_000_000_000i64;
    assert!(s.amostrar_e_contar(t0).is_empty());
    let avisos = s.amostrar_e_contar(t0 + 3_600_000 * 2);
    assert!(
        avisos.iter().any(|a| a.recurso == "backup:idade:noturno"),
        "{avisos:?}"
    );
}

/// A fronteira, pelo caminho do servidor: 80% EXATOS nao avisam; um slot a
/// mais avisa. E o teste do RED do `>` contra o `>=`.
#[test]
fn a_fronteira_de_80_por_cento_e_estrita() {
    let d = DirTemp::novo("contagem-fronteira");
    let s = servidor_simples(&d.0);
    let c = |usado| {
        vec![Contagem {
            recurso: "tabela:x.y".into(),
            usado,
            teto: 1_000,
            tendencia: false,
        }]
    };
    assert!(s.contar_regressivo(0, &c(800)).is_empty(), "80% exatos");
    let a = s.contar_regressivo(1, &c(801));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].degrau, Degrau::Aviso);
    let a = s.contar_regressivo(2, &c(951));
    assert_eq!(a[0].degrau, Degrau::Critico);
    assert_eq!(s.contar_regressivo(3, &c(950))[0].degrau, Degrau::Aviso);
}

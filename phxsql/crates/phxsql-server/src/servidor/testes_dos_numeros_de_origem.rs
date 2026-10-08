//! O registro dos numeros de origem -- pedido 329, com as condicoes do parecer
//! do DBA de 01/10/2026 (§3): o registro vai ao disco ANTES de o par ser
//! aceito, e o arquivo ilegivel recusa em vez de virar vazio. A prova do
//! laco inteiro, com tres servidores, e `tests/identidade-do-bidirecional.rs`.
use super::*;

fn servidor(nome: &str, preparar: impl FnOnce(&Path)) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("numeros-{nome}"));
    preparar(&dir);
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.replicacao.id_servidor = "central".into();
    (Servidor::novo(c).unwrap(), dir)
}

/// **O disco antes de aceitar.** Com o registro sem como ir ao disco (o
/// caminho dele e uma PASTA), o par novo e recusado -- e recusado de NOVO
/// na segunda chamada, porque a memoria nao recebeu o par que o disco
/// nao guardou.
///
/// **Defeito reposto** (a memoria atualizada antes da gravacao, que era o
/// desenho da primeira versao desta frente): a segunda chamada acha o par
/// «ja conhecido», volta `Ok` sem nunca ter gravado, e a asercao cai.
#[test]
fn o_par_so_e_aceito_depois_de_o_registro_ir_ao_disco() {
    let (s, dir) = servidor("disco", |_| {});
    // Primeiro arranque, registro ausente: o primeiro par grava e passa.
    s.conferir_numero_do_par(40, "caixa-a").unwrap();
    assert!(dir.join("replicacao-numeros.json").is_file());

    // O disco passa a recusar DEPOIS do arranque: o registro foi lido
    // ausente (vazio, legitimo), e so a gravacao do par novo falha. Com o
    // caminho ja tomado por uma pasta no arranque, a recusa seria a do
    // registro ilegivel -- e o teste passaria por outro motivo.
    let (s, dir) = servidor("sem-disco", |_| {});
    std::fs::create_dir_all(dir.join("replicacao-numeros.json")).unwrap();
    assert!(s.conferir_numero_do_par(40, "caixa-a").is_err());
    assert!(
        s.conferir_numero_do_par(40, "caixa-a").is_err(),
        "o par entrou na memoria sem ter ido ao disco"
    );
    drop(dir);
}

/// **O numero nunca e reatribuido**, nem depois de o servidor reiniciar:
/// o registro relido do disco recusa outro id no mesmo numero, nomeando
/// os dois.
#[test]
fn o_numero_nao_e_reatribuido_a_outro_id_nem_depois_do_reinicio() {
    let (s, dir) = servidor("reatribui", |_| {});
    s.conferir_numero_do_par(40, "caixa-a").unwrap();
    drop(s);
    let base = dir.to_path_buf();
    let mut c = Config {
        base: base.clone(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.replicacao.id_servidor = "central".into();
    let s = Servidor::novo(c).unwrap();
    let e = s
        .conferir_numero_do_par(40, "caixa-b")
        .unwrap_err()
        .to_string();
    assert!(e.contains("caixa-a") && e.contains("caixa-b"), "{e}");
    // O dono de sempre continua passando.
    s.conferir_numero_do_par(40, "caixa-a").unwrap();
}

/// **Ilegivel recusa.** O registro torto nao vira vazio no arranque: todo
/// par e recusado nomeando o arquivo, ate alguem consertar.
#[test]
fn o_registro_ilegivel_recusa_todo_par() {
    let (s, _dir) = servidor("ilegivel", |d| {
        std::fs::write(d.join("replicacao-numeros.json"), "{ torto").unwrap();
    });
    let e = s
        .conferir_numero_do_par(40, "caixa-a")
        .unwrap_err()
        .to_string();
    assert!(e.contains("replicacao-numeros.json"), "{e}");
}

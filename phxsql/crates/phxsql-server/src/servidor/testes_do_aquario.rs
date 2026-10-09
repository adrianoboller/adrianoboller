//! O esqueleto do aquario (pedido 707, fatia A2): quem pode perguntar, e o
//! portao da telemetria antes do `anotar`.
//!
//! O que estes testes travam:
//!
//! 1. **op sem direito declarado cai** -- as duas ops pedem `monitorar`, e
//!    quem tem so `monitorar` passa; sem a linha no `da_operacao` elas cairiam
//!    no `_ => Administrar` e este usuario seria barrado em silencio;
//! 2. **ver nao e matar** -- `monitorar` nao alcanca `telemetria_encerrar` nem
//!    `encerrar_sessao`;
//! 3. **o campo que o portao le e o furo** -- `monitorar` numa base so nao
//!    vira o servidor inteiro mandando `"database"`;
//! 4. **comportamento velho** -- quem administra continua vendo, e sem
//!    cadastro nada muda;
//! 5. **telemetria desligada, aquario nao anota nada**.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("aq-{nome}"))
}

fn cadastro(nivel: &str, bases: &str) -> Cadastro {
    Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{"usuarios":[{{"login":"ana","id":9,"nivel":"{nivel}",
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
        ))
        .unwrap(),
    )
    .unwrap()
}

fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade -- os dois
/// portoes, o geral e o de dentro, no caminho.
fn pede(s: &Arc<Servidor>, c: &Cadastro, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: c.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

const AS_TRES: [&str; 3] = [
    r#""op":"aquario_log""#,
    r#""op":"aquario_contagens""#,
    r#""op":"aquario_retrato""#,
];

/// Passou do portao: o que volta e a resposta -- as series da contagem (A8),
/// as linhas do log (A6) --, ou o «ainda nao existe» do log ainda fechado; e
/// nunca o `ACESSO_NEGADO`. Conferir o NOME do erro e nao so `is_err` e o que
/// impede o teste de passar por engano.
fn passou_do_portao(r: Result<Json>, corpo: &str) {
    if corpo.contains("aquario_contagens") {
        let j = r.unwrap_or_else(|e| panic!("{corpo}: {e}"));
        assert!(j.campo("series").is_some(), "{corpo}: {}", j.escrever());
        return;
    }
    if corpo.contains("aquario_retrato") {
        let j = r.unwrap_or_else(|e| panic!("{corpo}: {e}"));
        assert!(j.campo("tarefas").is_some(), "{corpo}: {}", j.escrever());
        return;
    }
    match r {
        Ok(j) => assert!(j.campo("linhas").is_some(), "{corpo}: {}", j.escrever()),
        Err(e) => {
            assert_eq!(e.nome(), "NAO_ENCONTRADO", "{corpo}: {e}");
            assert!(e.to_string().contains("ainda nao"), "{corpo}: {e}");
        }
    }
}

fn barrado(r: Result<Json>, corpo: &str) {
    let e = r.expect_err(corpo);
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
}

/// O RED da A2. Leitor com `monitorar` no curinga, e nada mais de servidor:
/// passa nas duas. Tirar a linha `"aquario_log" | "aquario_contagens"` do
/// `da_operacao` as joga no `_ => Administrar`, e este teste cai.
#[test]
fn quem_tem_monitorar_passa_nas_duas() {
    let dir = dir_temp("monitor");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in AS_TRES {
        passou_do_portao(pede(&s, &c, corpo), corpo);
    }
    assert_eq!(
        Atividade::da_operacao("aquario_log"),
        Some(Atividade::Monitorar)
    );
    assert_eq!(
        Atividade::da_operacao("aquario_contagens"),
        Some(Atividade::Monitorar)
    );
    assert_eq!(
        Atividade::da_operacao("aquario_retrato"),
        Some(Atividade::Monitorar)
    );
}

#[test]
fn leitor_sem_monitorar_nao_ve_o_aquario() {
    let dir = dir_temp("leitor");
    let c = cadastro("leitor", r#"{"*":{"ler":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in AS_TRES {
        barrado(pede(&s, &c, corpo), corpo);
    }
}

/// Ver nao e matar: a convergencia dos tres maduros que fez o direito nascer.
#[test]
fn monitorar_nao_mata() {
    let dir = dir_temp("nao-mata");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in [
        r#""op":"telemetria_encerrar","id":"dados:1""#,
        r#""op":"encerrar_sessao","id":1,"tipo":"conexao""#,
    ] {
        barrado(pede(&s, &c, corpo), corpo);
    }
}

/// `monitorar` so na base `loja`: o portao geral, que le `"database"`,
/// deixaria passar o pedido que nomeia `loja`. O de dentro pergunta na regra
/// do servidor e recusa, com o motivo.
#[test]
fn monitorar_numa_base_so_nao_ve_o_servidor_inteiro() {
    let dir = dir_temp("uma-base");
    let c = cadastro("leitor", r#"{"loja":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in [
        r#""op":"aquario_log","database":"loja""#,
        r#""op":"aquario_contagens","database":"loja""#,
        r#""op":"aquario_log""#,
    ] {
        let e = pede(&s, &c, corpo).expect_err(corpo);
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
    }
    let e = pede(&s, &c, r#""op":"aquario_log","database":"loja""#).unwrap_err();
    assert!(e.to_string().contains("monitorar este servidor"), "{e}");
}

/// O comportamento VELHO: quem administrava ja via tudo, e continua vendo --
/// pelo nivel e pela regra curinga, sem nenhum `"monitorar": true` escrito.
#[test]
fn quem_administra_continua_vendo() {
    for (nivel, bases) in [
        ("admin", r#"{}"#),
        ("leitor", r#"{"*":{"ler":true,"administrar":true}}"#),
    ] {
        let dir = dir_temp(&format!("adm-{nivel}"));
        let c = cadastro(nivel, bases);
        let s = servidor(&dir, c.clone());
        for corpo in AS_TRES {
            passou_do_portao(pede(&s, &c, corpo), corpo);
        }
    }
}

#[test]
fn sem_cadastro_nada_muda() {
    let dir = dir_temp("sem-cadastro");
    let s = servidor(&dir, Cadastro::default());
    for corpo in AS_TRES {
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        passou_do_portao(r, corpo);
    }
}

/// Instrumentacao desligada custa zero, e o portao vem ANTES do trabalho:
/// com a telemetria desligada o aquario nao ve pedido nenhum. Tirar o
/// `then_some` do `aquario_se_ligada` (devolver sempre o aquario) faz este
/// teste cair.
#[test]
fn telemetria_desligada_o_aquario_nao_anota() {
    let dir = dir_temp("desligada");
    let s = servidor(&dir, Cadastro::default());
    let acesso = Acesso {
        op: "ping".into(),
        ok: true,
        ..Acesso::default()
    };
    s.telemetria.desligar();
    let antes = s.telemetria.aquario().anotados();
    for _ in 0..5 {
        s.anotar(&acesso);
    }
    assert_eq!(
        s.telemetria.aquario().anotados(),
        antes,
        "desligada e anotou"
    );

    s.telemetria.ligar(crate::agora_ms());
    s.anotar(&acesso);
    assert_eq!(
        s.telemetria.aquario().anotados(),
        antes + 1,
        "ligada e nao anotou: o gancho do anotar sumiu"
    );
}

// ---------------------------------------------------------------- A6: aquario.log

fn lento(op: &str, ms: u64) -> Acesso {
    Acesso {
        quando_ms: crate::agora_ms(),
        ip: "198.51.100.9".into(),
        usuario: "ana".into(),
        op: op.into(),
        ok: true,
        duracao_ms: ms,
        database: "loja".into(),
        tabela: "vendas".into(),
        ..Acesso::default()
    }
}

fn linhas_do_aquario(s: &Arc<Servidor>, corpo: &str) -> Json {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t","op":"aquario_log"{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r.unwrap()
}

/// Grava SEM ninguem perguntar: o pedido lento que termina deixa a linha no
/// disco pelo `anotar` do servidor, e o arquivo nasce ao lado do
/// `acessos.log`. Tirar a chamada `tarefa_terminou` do `anotar` faz este
/// teste cair.
#[test]
fn o_pedido_lento_estoura_no_aquario_log_sem_ninguem_perguntar() {
    let dir = dir_temp("grava");
    let s = servidor(&dir, Cadastro::default());
    s.anotar(&lento("varrer", 2_500));
    s.anotar(&lento("ping", 3));
    let texto = std::fs::read_to_string(dir.join("aquario.log")).unwrap();
    assert_eq!(texto.lines().count(), 1, "{texto}");
    assert!(texto.contains("\"estourou\"") && texto.contains("\"varrer\""));
    assert!(!texto.contains("ana") && !texto.contains("198.51.100.9"));
    let r = linhas_do_aquario(&s, "");
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].inteiro_ou("ms", 0), 2_500);
}

/// O aquario.log mora atras do mesmo portao: telemetria desligada, nada se
/// grava (instrumentacao desligada custa zero).
#[test]
fn telemetria_desligada_nao_grava_o_aquario_log() {
    let dir = dir_temp("log-desligado");
    let s = servidor(&dir, Cadastro::default());
    s.telemetria.desligar();
    s.anotar(&lento("varrer", 9_000));
    assert_eq!(s.telemetria.aquario().log().gravadas(), 0);
}

/// O tmpfs pequeno do teste de disco cheio. Desmonta ao sair, antes de o
/// `DirTemp` apagar a pasta (a ordem de declaracao decide).
struct Tmpfs(std::path::PathBuf);

impl Tmpfs {
    /// Precisa de root. Sem ele devolve `None`, e o teste diz que pulou --
    /// fingir com uma pasta 0500 nao serve, porque o bit de permissao nao
    /// vale para o uid 0 e o teste passaria por engano.
    fn montar(ponto: &std::path::Path, tamanho: &str) -> Option<Tmpfs> {
        std::fs::create_dir_all(ponto).ok()?;
        let ok = std::process::Command::new("mount")
            .args(["-t", "tmpfs", "-o", &format!("size={tamanho}"), "tmpfs"])
            .arg(ponto)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        ok.then(|| Tmpfs(ponto.to_path_buf()))
    }
}

impl Drop for Tmpfs {
    fn drop(&mut self) {
        // `-l`: o servidor do teste ainda segura o `aquario.log` aberto, e o
        // `umount` sem ele falharia com EBUSY e deixaria o tmpfs montado.
        let _ = std::process::Command::new("umount")
            .arg("-l")
            .arg(&self.0)
            .output();
    }
}

/// RED do disco cheio, contra o SISTEMA OPERACIONAL: um tmpfs de 16 KiB
/// cheio de verdade. O servidor nao cai, AVISA pelos dois caminhos (a saude
/// do disco conta o erro de E/S; a consulta do aquario devolve a falha), e
/// quando o disco volta a primeira linha nova se le inteira.
///
/// Prova real, nos tres sentidos:
/// * tirar o `evento_de_disco` do `anotar` -> `erros_es` nao sobe;
/// * tirar o `falhas.fetch_add` do `gravar` -> `falhas_de_escrita` fica 0;
/// * tirar a quebra do `linha_partida` (`acesso.rs`) -> a linha do
///   marcador se cola na metade cortada e some da consulta.
#[cfg(target_os = "linux")]
#[test]
fn disco_cheio_de_verdade_o_servidor_nao_cai_e_avisa() {
    let dir = dir_temp("disco-cheio");
    let s = servidor(&dir, Cadastro::default());
    let ponto = dir.join("tmpfs");
    let Some(_tmpfs) = Tmpfs::montar(&ponto, "16k") else {
        eprintln!("PULADO: nao consegui montar tmpfs (precisa de root)");
        return;
    };
    let log = s.telemetria.aquario().log();
    log.abrir(ponto.join("aquario.log")).unwrap();
    // Uma linha antes de encher: o arquivo ganha a pagina dele, e a falha de
    // depois corta uma linha ao meio no limite da pagina -- o pior caso.
    s.anotar(&lento("antes", 1_500));
    assert_eq!(log.gravadas(), 1);
    let enchimento = ponto.join("enchimento");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&enchimento).unwrap();
        let bloco = vec![b'x'; 1024];
        while f.write_all(&bloco).is_ok() {}
    }
    let erros_antes = s.saude.erros_es();
    let mut tentativas = 0;
    while log.falhas() == 0 {
        tentativas += 1;
        assert!(tentativas < 1_000, "o tmpfs nunca encheu");
        s.anotar(&lento(&format!("enchendo-{tentativas}"), 1_500));
    }
    assert!(
        s.saude.erros_es() > erros_antes,
        "o disco cheio do aquario.log nao chegou a saude do disco"
    );
    // Nao caiu: responde, e a consulta conta a falha.
    let r = linhas_do_aquario(&s, "");
    assert!(
        r.inteiro_ou("falhas_de_escrita", 0) >= 1,
        "{}",
        r.escrever()
    );
    assert!(
        r.texto_ou("ultima_falha", "").contains("space")
            || r.texto_ou("ultima_falha", "").contains("espaco"),
        "{}",
        r.escrever()
    );
    // O disco volta.
    std::fs::remove_file(&enchimento).unwrap();
    s.anotar(&lento("marcador", 1_500));
    let r = linhas_do_aquario(&s, "");
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(
        l.last().map(|j| j.texto_ou("op", "")),
        Some("marcador"),
        "a primeira linha depois do disco cheio se perdeu: {}",
        r.escrever()
    );
    // E o que se le e linha inteira: a metade cortada ficou sozinha.
    let texto = std::fs::read_to_string(ponto.join("aquario.log")).unwrap();
    let ilegiveis = texto.lines().filter(|t| Json::analisar(t).is_err()).count();
    assert!(ilegiveis <= 1, "{ilegiveis} linhas ilegiveis:\n{texto}");
}

/// A4: a base do aquario tambem fica atras do portao -- desligada, nenhuma
/// amostra entra nela; ligada, entra. E a prova de «desligada custa 0» no
/// lugar onde o trabalho de verdade mora (hash, trava, `ln`). Chamar o
/// `base.anotar` fora do `aquario_se_ligada` faz este teste cair.
#[test]
fn telemetria_desligada_a_base_nao_soma() {
    let dir = dir_temp("base-desligada");
    let s = servidor(&dir, Cadastro::default());
    let acesso = Acesso {
        op: "inserir".into(),
        ok: true,
        database: "loja".into(),
        tabela: "pedidos".into(),
        us: 900,
        ..Acesso::default()
    };
    s.telemetria.desligar();
    for _ in 0..5 {
        s.anotar(&acesso);
    }
    assert_eq!(s.telemetria.aquario().base().atualizacoes(), 0);
    s.telemetria.ligar(crate::agora_ms());
    s.anotar(&acesso);
    assert_eq!(s.telemetria.aquario().base().atualizacoes(), 1);
}

/// A espera pela trava chega ao `Acesso` de verdade: com outra thread
/// segurando a trava de dados 150 ms, o pedido desta thread acha ≥ 100 ms de
/// espera para descontar. Tirar o `somar_espera` do `contar_espera` deixa a
/// espera em zero -- e a vitima da fila voltaria a ser anormal.
#[test]
fn a_espera_pela_trava_vai_ao_pedido_que_esperou() {
    let dir = dir_temp("espera");
    let s = servidor(&dir, Cadastro::default());
    assert!(s.telemetria.ligada(), "a telemetria nasce ligada");
    let (tx, rx) = std::sync::mpsc::channel();
    let segura = {
        let s = Arc::clone(&s);
        std::thread::spawn(move || {
            let _g = s.dados.write().unwrap();
            tx.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(150));
        })
    };
    rx.recv().unwrap();
    crate::aquario::base::tomar_espera();
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"criar_database","database":"m"}"#,
        &mut ses,
        "127.0.0.1",
    );
    r.unwrap();
    let espera = crate::aquario::base::tomar_espera();
    segura.join().unwrap();
    assert!(espera >= 100_000, "espera medida: {espera} µs");
}

// ------------------------------------------------- a contagem (fatia A8)

/// Uma tabela `c` em `b`, com a coluna de exclusao suave (o padrao).
fn com_tabela(s: &Arc<Servidor>) {
    let ses = Sessao::default();
    s.executar(
        "criar_database",
        &Json::analisar(r#"{"database":"b"}"#).unwrap(),
        &ses,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &Json::analisar(
            r#"{"database":"b","tabela":"c",
                "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        )
        .unwrap(),
        &ses,
    )
    .unwrap();
}

fn parcial(s: &Arc<Servidor>) -> Json {
    let r = s.telemetria.aquario().contagens(&Json::Nulo).unwrap();
    r.campo("minuto_parcial")
        .and_then(|m| m.campo("c"))
        .cloned()
        .unwrap_or_else(|| {
            // Antes da primeira virada nao ha minuto armado; arma e relê.
            s.telemetria.aquario().contagem().virar(crate::agora_ms());
            let r = s.telemetria.aquario().contagens(&Json::Nulo).unwrap();
            r.campo("minuto_parcial")
                .unwrap()
                .campo("c")
                .unwrap()
                .clone()
        })
}

/// RED da A8, pelo caminho de verdade (o executor local: `despachar` e
/// `anotar`, os mesmos da porta de dados): o excluir FISICO conta como
/// fisico, pelo `modo` que a resposta devolve. Trocar o `"fisico"` do
/// `da_resposta` pelo padrao suave, ou nao passar o desfecho ao `Acesso`,
/// faz este teste cair.
#[test]
fn excluir_fisico_conta_como_fisico_pelo_caminho_de_verdade() {
    let dir = dir_temp("conta-excluir");
    let s = servidor(&dir, Cadastro::default());
    com_tabela(&s);
    s.telemetria.aquario().contagem().virar(crate::agora_ms());
    let ex = ExecutorLocal::novo(Arc::clone(&s), "(teste)");
    let pede = |txt: &str| {
        let mut p = Json::analisar(txt).unwrap();
        p.definir("token", Json::texto_de("t"));
        ex.executar(&p)
    };
    use crate::mcp::Executor;
    for id in 1..=3 {
        pede(&format!(
            r#"{{"op":"inserir","database":"b","tabela":"c","linha":{{"id":{id}}}}}"#
        ))
        .unwrap();
    }
    let r =
        pede(r#"{"op":"excluir","database":"b","tabela":"c","rowid":1,"fisico":true}"#).unwrap();
    assert_eq!(r.texto_ou("modo", ""), "fisico");
    pede(r#"{"op":"excluir","database":"b","tabela":"c","rowid":2}"#).unwrap();
    pede(r#"{"op":"varrer","database":"b","tabela":"c"}"#).unwrap();
    let c = parcial(&s);
    assert_eq!(c.inteiro_ou("insert", -1), 3, "{}", c.escrever());
    assert_eq!(c.inteiro_ou("excluir_fisico", -1), 1, "{}", c.escrever());
    assert_eq!(c.inteiro_ou("excluir_suave", -1), 1, "{}", c.escrever());
    assert_eq!(c.inteiro_ou("select", -1), 1, "{}", c.escrever());
    assert_eq!(c.inteiro_ou("erro", -1), 0, "{}", c.escrever());
}

/// RED da A8: o backup que falhou sobe so `erro`, pelo caminho de verdade.
#[test]
fn backup_que_falhou_sobe_so_erro_pelo_caminho_de_verdade() {
    let dir = dir_temp("conta-backup");
    let s = servidor(&dir, Cadastro::default());
    com_tabela(&s);
    s.telemetria.aquario().contagem().virar(crate::agora_ms());
    // Um ARQUIVO no lugar do destino: o backup nao tem onde escrever.
    let bloqueio = dir.join("bloqueio");
    std::fs::write(&bloqueio, b"x").unwrap();
    let ex = ExecutorLocal::novo(Arc::clone(&s), "(teste)");
    use crate::mcp::Executor;
    let mut p = Json::analisar(r#"{"op":"backup"}"#).unwrap();
    p.definir("token", Json::texto_de("t"));
    p.definir(
        "destino",
        Json::texto_de(bloqueio.join("dentro").display().to_string()),
    );
    assert!(ex.executar(&p).is_err(), "o backup devia ter falhado");
    let c = parcial(&s);
    assert_eq!(c.inteiro_ou("backup", -1), 0, "{}", c.escrever());
    assert_eq!(c.inteiro_ou("erro", -1), 1, "{}", c.escrever());
}

/// Telemetria desligada, acumulador parado -- o portao vem antes da conta.
#[test]
fn telemetria_desligada_a_contagem_fica_parada() {
    let dir = dir_temp("conta-desligada");
    let s = servidor(&dir, Cadastro::default());
    s.telemetria.aquario().contagem().virar(crate::agora_ms());
    let ex = ExecutorLocal::novo(Arc::clone(&s), "(teste)");
    use crate::mcp::Executor;
    // Falha sempre: sem base nem tabela.
    let falha = Json::analisar(r#"{"op":"varrer","token":"t"}"#).unwrap();
    s.telemetria.desligar();
    assert!(ex.executar(&falha).is_err());
    let c = parcial(&s);
    assert_eq!(c.inteiro_ou("erro", -1), 0, "{}", c.escrever());
    s.telemetria.ligar(crate::agora_ms());
    assert!(ex.executar(&falha).is_err());
    let c = parcial(&s);
    assert_eq!(
        c.inteiro_ou("erro", -1),
        1,
        "ligada e nao contou: {}",
        c.escrever()
    );
}

/// O servidor grava as horas ao lado do `acessos.log`, e a consulta diz onde.
#[test]
fn o_arquivo_de_horas_mora_ao_lado_do_acessos_log() {
    let dir = dir_temp("conta-arquivo");
    let s = servidor(&dir, Cadastro::default());
    let r = s.telemetria.aquario().contagens(&Json::Nulo).unwrap();
    assert_eq!(
        r.texto_ou("arquivo_de_horas", ""),
        dir.join(crate::aquario::contagem::ARQUIVO_DE_HORAS)
            .display()
            .to_string()
    );
}

// ------------------------------------- as ligacoes entre as fatias A3/A4/A6/A8

use crate::aquario::contagem::{HORA_MS, MINUTO_MS};

/// Um minuto ja fechado e a virada que o fecha, os dois na MESMA hora: a
/// virada que cruzasse a hora gravaria o `aquario-horas.jsonl` ali mesmo, e o
/// teste da retomada passaria sem retomada nenhuma.
fn minuto_fechavel() -> (i64, i64) {
    let agora = crate::agora_ms();
    let este = agora - agora.rem_euclid(MINUTO_MS);
    let m0 = if este.rem_euclid(HORA_MS) >= MINUTO_MS {
        este - MINUTO_MS
    } else {
        este - 2 * MINUTO_MS
    };
    (m0, m0 + MINUTO_MS)
}

fn pedido_que_falhou() -> Acesso {
    Acesso {
        quando_ms: crate::agora_ms(),
        op: "varrer".into(),
        ok: false,
        codigo: 2001,
        ..Acesso::default()
    }
}

/// Um minuto com UM erro contado, fechado pela virada do servidor.
fn fechar_um_minuto_com_um_erro(s: &Arc<Servidor>) -> i64 {
    let (m0, vira) = minuto_fechavel();
    s.telemetria.aquario().contagem().virar(m0);
    s.anotar(&pedido_que_falhou());
    assert!(s.virar_a_contagem(vira).is_some(), "o minuto nao fechou");
    m0
}

/// Ligacao 1 (A8 -> A6): a linha do minuto que a virada fecha chega ao
/// `aquario.log` pelo escritor da A6, e a consulta a le. Tirar o `gravar` do
/// `virar_a_contagem` faz este teste cair.
#[test]
fn a_virada_do_minuto_grava_a_contagem_no_aquario_log() {
    let dir = dir_temp("liga-contagem-log");
    let s = servidor(&dir, Cadastro::default());
    let m0 = fechar_um_minuto_com_um_erro(&s);
    let r = linhas_do_aquario(&s, "");
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    let contagens: Vec<&Json> = l
        .iter()
        .filter(|j| j.texto_ou("evento", "") == "contagem")
        .collect();
    assert_eq!(contagens.len(), 1, "{}", r.escrever());
    let dados = contagens[0].campo("dados").unwrap();
    assert_eq!(dados.inteiro_ou("minuto_ms", 0), m0);
    assert_eq!(dados.campo("c").unwrap().inteiro_ou("erro", -1), 1);
}

/// Os erros que a contagem tem, somando a hora corrente e as fechadas.
fn erros_contados(s: &Arc<Servidor>) -> i64 {
    let r = s.telemetria.aquario().contagens(&Json::Nulo).unwrap();
    let mut total = 0;
    if let Some(ms) = r
        .campo("hora_corrente")
        .and_then(|h| h.campo("minutos"))
        .and_then(Json::lista)
    {
        total += ms.iter().map(|m| m.inteiro_ou("erro", 0)).sum::<i64>();
    }
    if let Some(hs) = r.campo("horas").and_then(Json::lista) {
        total += hs
            .iter()
            .filter_map(|h| h.campo("c"))
            .map(|c| c.inteiro_ou("erro", 0))
            .sum::<i64>();
    }
    total
}

/// Ligacao 2 (A6 -> A8): o processo novo refaz a contagem das linhas
/// `contagem` que o velho deixou no `aquario.log`, pelo arranque de verdade.
/// Tirar o `retomar_a_contagem` do `Servidor::novo` faz este teste cair: o
/// processo novo nasce sem o erro que o velho contou.
#[test]
fn o_arranque_retoma_a_contagem_do_aquario_log() {
    let dir = dir_temp("liga-retomar");
    {
        let velho = servidor(&dir, Cadastro::default());
        fechar_um_minuto_com_um_erro(&velho);
        assert_eq!(erros_contados(&velho), 1);
    }
    let novo = servidor(&dir, Cadastro::default());
    // A primeira virada do amostrador, que no servidor no ar vem em 1 s: so
    // depois dela a hora corrente tem um minuto em curso para mostrar.
    novo.virar_a_contagem(crate::agora_ms());
    assert_eq!(
        erros_contados(&novo),
        1,
        "o minuto do processo velho nao voltou: {}",
        novo.telemetria
            .aquario()
            .contagens(&Json::Nulo)
            .unwrap()
            .escrever()
    );
}

/// Ligacao 3 (A4 -> A3 -> A6): o pedido de 10 s depois de vinte de 1 ms na
/// mesma operacao e tabela acende `fora_do_habitual` na TAREFA, pelo
/// produtor unico, e deixa a linha `mudou` no `aquario.log`. Tirar o
/// `sinalizar_desvio` do `anotar` derruba as duas metades; tirar so o
/// `sinal_em` derruba o bit; tirar so o `gravar` derruba a linha.
#[test]
fn fora_do_habitual_acende_o_bit_e_grava_mudou() {
    use crate::aquario::Alarme;
    let dir = dir_temp("liga-desvio");
    let s = servidor(&dir, Cadastro::default());
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:1", "dados", "127.0.0.1", 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let pedido = |us: u64| Acesso {
        quando_ms: crate::agora_ms(),
        op: "inserir".into(),
        ok: true,
        database: "loja".into(),
        tabela: "pedidos".into(),
        duracao_ms: us / 1_000,
        us,
        ..Acesso::default()
    };
    for _ in 0..20 {
        a.comecou_pedido("inserir", "root", "loja", "pedidos", agora);
        s.anotar(&pedido(1_000));
        assert_eq!(a.alarmes(), 0, "o habitual alarmou");
    }
    a.comecou_pedido("inserir", "root", "loja", "pedidos", agora);
    s.anotar(&pedido(10_000_000));
    assert_ne!(
        a.alarmes() & Alarme::ForaDoHabitual.bit(),
        0,
        "o desvio nao chegou a tarefa"
    );
    let r = linhas_do_aquario(&s, "");
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    let mudou: Vec<&Json> = l
        .iter()
        .filter(|j| j.texto_ou("evento", "") == "mudou")
        .collect();
    assert_eq!(mudou.len(), 1, "{}", r.escrever());
    assert_eq!(mudou[0].texto_ou("alarme", ""), "fora_do_habitual");
    assert_eq!(mudou[0].texto_ou("tabela", ""), "pedidos");
    assert!(mudou[0].campo("dados").unwrap().inteiro_ou("n", 0) >= 20);
}

// ------------------------------------------------- A5: a classificar unica

fn retrato(s: &Arc<Servidor>) -> Json {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"aquario_retrato"}"#,
        &mut ses,
        "127.0.0.1",
    );
    r.unwrap()
}

/// **O RED da A5: o log e o retrato dao a MESMA cor para a mesma tarefa.**
///
/// Cada caso pinta uma tarefa viva (o retrato, pela op `aquario_retrato`) e
/// depois a faz terminar pelo `anotar` do servidor (a linha `estourou` do
/// `aquario.log`). Cor e motivo tem de bater, caso a caso. Dar ao log uma
/// regra propria -- por exemplo `ok` = verde e erro = vermelho no
/// `classe_do_fim`, em vez da `classificar` -- faz cair o «integridade» (o
/// log diria verde) e o «tempo fixo»; trocar o `nivel` do painel por uma
/// regra propria cai no teste do painel logo abaixo.
///
/// O caso por tempo usa `alto_uso_ms` = 1 ms para caber num teste: a tarefa
/// viva tem poucos ms e a terminada tem 1,5 s, e as duas passam do limiar
/// pela MESMA regra (sem habitual formado, acima do tempo fixo).
#[test]
fn o_log_e_o_retrato_pintam_a_mesma_tarefa_da_mesma_cor() {
    use crate::aquario::Alarme;
    use crate::config::Painel;
    let dir = dir_temp("mesma-cor");
    let s = servidor(&dir, Cadastro::default());
    let casos: [(&str, Option<Alarme>, u64, &str); 5] = [
        (
            "reentrante",
            Some(Alarme::TravaReentrante),
            2_000,
            "vermelho",
        ),
        (
            "corrompido",
            Some(Alarme::DadoCorrompido),
            2_000,
            "vermelho",
        ),
        (
            "integridade",
            Some(Alarme::IntegridadeRecusada),
            2_000,
            "amarelo",
        ),
        ("tempo fixo", None, 1, "amarelo"),
        ("calma", None, 2_000, "verde"),
    ];
    for (i, (nome, alarme, alto_uso_ms, cor)) in casos.into_iter().enumerate() {
        s.telemetria.definir_pintura(Painel {
            alto_uso_ms,
            ..Painel::default()
        });
        let n = 100 + i as u64;
        let chave = format!("dados:{n}");
        let a = s
            .telemetria
            .entrar(&chave, "dados", "198.51.100.9", n, crate::agora_ms())
            .unwrap();
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        a.comecou_pedido("varrer", "ana", "loja", "vendas", crate::agora_ms());
        if let Some(al) = alarme {
            crate::telemetria::sinal(al, "teste");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));

        let r = retrato(&s);
        let prefixo = format!("{chave}#");
        let bolha = r
            .campo("tarefas")
            .and_then(Json::lista)
            .and_then(|l| {
                l.iter()
                    .find(|t| t.texto_ou("tarefa", "").starts_with(&prefixo))
            })
            .unwrap_or_else(|| panic!("{nome}: a tarefa nao esta no retrato: {}", r.escrever()))
            .clone();

        s.anotar(&lento("varrer", 1_500));
        let log = linhas_do_aquario(&s, "");
        let linha = log
            .campo("linhas")
            .and_then(Json::lista)
            .and_then(|l| l.last())
            .unwrap()
            .clone();
        assert_eq!(linha.texto_ou("evento", ""), "estourou", "{nome}");

        for campo in ["cor", "motivo"] {
            assert_eq!(
                bolha.texto_ou(campo, "?"),
                linha.texto_ou(campo, "!"),
                "{nome}: o {campo} do retrato e o do log divergem\nretrato: {}\nlog: {}",
                bolha.escrever(),
                linha.escrever()
            );
        }
        assert_eq!(
            bolha.texto_ou("cor", ""),
            cor,
            "{nome}: {}",
            bolha.escrever()
        );
        if let Some(al) = alarme {
            let nomes = |j: &Json| j.campo("alarmes").map(Json::escrever).unwrap_or_default();
            assert!(nomes(&bolha).contains(al.nome()), "{nome}");
            assert_eq!(nomes(&bolha), nomes(&linha), "{nome}");
        }
        a.terminou_pedido("ana");
        s.telemetria.sair(&chave);
    }
}

/// A TV ve operacao, tabela e cor -- nunca login nem IP (decisao do dono,
/// 09/10). Quem administra (aqui: o token, sem usuario) ve os dois. Tirar o
/// `if completo` do `bolha_do_aquario` faz a primeira metade cair.
#[test]
fn a_tv_ve_operacao_tabela_e_cor_e_nunca_login_nem_ip() {
    let dir = dir_temp("tv");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    let a = s
        .telemetria
        .entrar("dados:77", "dados", "198.51.100.77", 77, crate::agora_ms())
        .unwrap();
    a.comecou_pedido(
        "varrer",
        "login-secreto",
        "loja",
        "vendas",
        crate::agora_ms(),
    );
    // Uma ociosa ao lado: conexao sem pedido nao e tarefa, e nao vira bolha.
    s.telemetria
        .entrar("dados:78", "dados", "198.51.100.78", 78, crate::agora_ms())
        .unwrap();

    let r = pede(&s, &c, r#""op":"aquario_retrato""#).unwrap();
    let t = r.escrever();
    assert_eq!(r.campo("completo"), Some(&Json::Bool(false)), "{t}");
    let tarefas = r.campo("tarefas").and_then(Json::lista).unwrap();
    assert_eq!(tarefas.len(), 1, "{t}");
    for esperado in [
        "\"op\":\"varrer\"",
        "\"tabela\":\"vendas\"",
        "\"cor\":\"verde\"",
    ] {
        assert!(t.contains(esperado), "{esperado}: {t}");
    }
    for proibido in ["login-secreto", "198.51.100.77", "\"usuario\"", "\"ip\""] {
        assert!(!t.contains(proibido), "{proibido} vazou para a TV: {t}");
    }

    // O administrador logado, no mesmo servidor, com a mesma tarefa no ar.
    let dir_adm = dir_temp("tv-adm");
    let c_adm = cadastro("admin", "{}");
    let s_adm = servidor(&dir_adm, c_adm.clone());
    let b = s_adm
        .telemetria
        .entrar("dados:77", "dados", "198.51.100.77", 77, crate::agora_ms())
        .unwrap();
    b.comecou_pedido(
        "varrer",
        "login-secreto",
        "loja",
        "vendas",
        crate::agora_ms(),
    );
    let r = pede(&s_adm, &c_adm, r#""op":"aquario_retrato""#).unwrap();
    let t = r.escrever();
    assert_eq!(r.campo("completo"), Some(&Json::Bool(true)), "{t}");
    assert!(
        t.contains("login-secreto") && t.contains("198.51.100.77"),
        "{t}"
    );
}

/// Os bits da A3 aparecem no retrato da TELEMETRIA tambem, e o `nivel` do
/// painel e a projecao da mesma cor: o reentrante pinta `stress` la e
/// `vermelho` aqui. Uma regra propria no `nivel` (a de antes da A5, que nao
/// via bit nenhum) diria `normal`.
#[test]
fn o_painel_ve_os_bits_e_o_nivel_e_a_mesma_cor() {
    let dir = dir_temp("painel-bits");
    let s = servidor(&dir, Cadastro::default());
    let a = s
        .telemetria
        .entrar("dados:90", "dados", "198.51.100.90", 90, crate::agora_ms())
        .unwrap();
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("inserir", "ana", "loja", "vendas", crate::agora_ms());
    crate::telemetria::sinal(crate::aquario::Alarme::TravaReentrante, "teste");
    let painel = s.telemetria.para_json(crate::agora_ms(), 1);
    let bolha = painel
        .campo("atividades")
        .and_then(Json::lista)
        .and_then(|l| l.iter().find(|x| x.texto_ou("id", "") == "dados:90"))
        .unwrap()
        .clone();
    assert_eq!(
        bolha.texto_ou("nivel", ""),
        "stress",
        "{}",
        bolha.escrever()
    );
    assert_eq!(bolha.texto_ou("cor", ""), "vermelho");
    assert_eq!(
        bolha.texto_ou("motivo", ""),
        "aquario.motivo.trava_reentrante"
    );
    assert!(bolha
        .campo("alarmes")
        .map(Json::escrever)
        .unwrap_or_default()
        .contains("trava_reentrante"));
}

// ------------------------------------------- a digital do `sql` (F1 do 495)

/// O `sql` entra na linha de base pela DIGITAL, pelo caminho de verdade (o
/// executor local: `despachar` e `anotar`). O mesmo `SELECT` com outro
/// literal soma na mesma linha; outra forma abre outra. Tirar o
/// `digital_do_sql` do `op_sql`, ou o `tomar_digital` do `Acesso`, deixa o
/// `sql` fora da base e este teste cai.
#[test]
fn o_sql_entra_na_base_pela_digital() {
    let dir = dir_temp("digital-sql");
    let s = servidor(&dir, Cadastro::default());
    assert!(s.telemetria.ligada(), "a telemetria nasce ligada");
    com_tabela(&s);
    let ex = ExecutorLocal::novo(Arc::clone(&s), "(teste)");
    use crate::mcp::Executor;
    let sql = |texto: &str| {
        let mut p = Json::analisar(r#"{"op":"sql","database":"b"}"#).unwrap();
        p.definir("token", Json::texto_de("t"));
        p.definir("texto", Json::texto_de(texto));
        ex.executar(&p).unwrap();
    };
    let digital = |texto: &str| {
        phxsql_sql::digital(&phxsql_sql::lexico::analisar_com_comentarios(texto).unwrap())
    };
    let base = s.telemetria.aquario().base();
    let antes = base.chaves();
    for id in 1..=3 {
        sql(&format!("SELECT * FROM c WHERE id = {id}"));
    }
    sql("SELECT id FROM c");
    let a = base
        .ler(crate::aquario::base::Chave::Digital(digital(
            "SELECT * FROM c WHERE id = 99",
        )))
        .expect("o SELECT por id tem linha propria na base");
    assert_eq!(a.n, 3);
    let b = base
        .ler(crate::aquario::base::Chave::Digital(digital(
            "SELECT id FROM c",
        )))
        .expect("a outra forma tem a sua");
    assert_eq!(b.n, 1);
    assert_eq!(base.chaves(), antes + 2);
    // O pedido seguinte nao herda a digital: ela se toma uma vez.
    assert_eq!(crate::aquario::base::tomar_digital(), None);
}

/// Desligada, o `op_sql` nao le o texto para a digital: o portao vem antes
/// do lexico.
#[test]
fn telemetria_desligada_o_sql_nao_calcula_digital() {
    let dir = dir_temp("digital-desligada");
    let s = servidor(&dir, Cadastro::default());
    com_tabela(&s);
    s.telemetria.desligar();
    crate::aquario::base::tomar_digital();
    let mut ses = Sessao::default();
    let pedido = r#"{"token":"t","op":"sql","database":"b","texto":"SELECT * FROM c"}"#;
    let (_, _, r) = s.despachar(pedido, &mut ses, "127.0.0.1");
    r.unwrap();
    assert_eq!(crate::aquario::base::tomar_digital(), None);
    s.telemetria.ligar(crate::agora_ms());
    let (_, _, r) = s.despachar(pedido, &mut ses, "127.0.0.1");
    r.unwrap();
    assert!(crate::aquario::base::tomar_digital().is_some());
}

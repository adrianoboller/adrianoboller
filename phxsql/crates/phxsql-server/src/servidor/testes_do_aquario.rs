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

const AS_DUAS: [&str; 2] = [r#""op":"aquario_log""#, r#""op":"aquario_contagens""#];

/// Passou do portao: o que volta e a resposta (A6 ja entrou no
/// `aquario_log`), ou o «ainda nao existe» da fatia que falta -- e nunca o
/// `ACESSO_NEGADO`. Conferir o NOME do erro e nao so `is_err` e o que impede o
/// teste de passar por engano.
fn passou_do_portao(r: Result<Json>, corpo: &str) {
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
    for corpo in AS_DUAS {
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
}

#[test]
fn leitor_sem_monitorar_nao_ve_o_aquario() {
    let dir = dir_temp("leitor");
    let c = cadastro("leitor", r#"{"*":{"ler":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in AS_DUAS {
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
        for corpo in AS_DUAS {
            passou_do_portao(pede(&s, &c, corpo), corpo);
        }
    }
}

#[test]
fn sem_cadastro_nada_muda() {
    let dir = dir_temp("sem-cadastro");
    let s = servidor(&dir, Cadastro::default());
    for corpo in AS_DUAS {
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

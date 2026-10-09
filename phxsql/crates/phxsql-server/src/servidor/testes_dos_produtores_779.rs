//! Pedido 779: os tres alarmes que estavam declarados e sem produtor fora de
//! teste -- `ForcaBruta`, `SenhaEmClaro`, `IntegridadeRecusada` --, os tres
//! ramos `MarcaNaoResolvida` do bidirecional que nenhum teste alcancava, e a
//! replica agendada que nao passava pelo `Ritmo`.
//!
//! # Como cada teste mede
//!
//! Pela ocorrencia no correio DESTE servidor, com uma atividade dele
//! amarrada a thread -- e nao pelo sedimento, que e do processo: outro teste
//! do mesmo binario passando pelo mesmo produtor faria o `vezes() > antes`
//! passar com o produtor tirado (o limite que o `testes_dos_produtores_769`
//! deixou escrito). Cada servidor tem a propria camada e o proprio silencio.
//!
//! # RED
//!
//! Cada teste nomeia a linha que, tirada, o derruba.

use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("p779-{rotulo}"))
}

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.to_path_buf(),
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

/// O prefixo da tarefa das atividades amarradas por este arquivo: so o que
/// nasceu nelas conta. O alarme de servidor sem atividade de outro teste do
/// binario cai na camada do processo, que pode ser a de um servidor daqui
/// (medido no `testes_da_protecao_766` em 09/10/2026).
const TAREFA: &str = "dados:p779-";

fn ocorrencias_de(s: &Arc<Servidor>, alarme: Alarme) -> Vec<crate::ocorrencias::Ocorrencia> {
    s.ocorrencias
        .correio()
        .retirar(
            std::time::Duration::from_millis(1),
            |c| matches!(c, Carta::Ocorrencia(o) if o.alarme == alarme && o.tarefa.starts_with(TAREFA)),
        )
        .into_iter()
        .filter_map(|c| match c {
            Carta::Ocorrencia(o) => Some(o),
            Carta::Saude(_) => None,
        })
        .collect()
}

fn amarrada(s: &Arc<Servidor>, ip: &str) -> Arc<crate::telemetria::Atividade> {
    s.telemetria
        .entrar(
            &format!("dados:p779-{ip}"),
            "dados",
            ip,
            1,
            crate::agora_ms(),
        )
        .expect("a telemetria nasce ligada")
}

// ------------------------------------------------------------- ForcaBruta

/// Quatro senhas erradas nao alarmam; a quinta (o limite da politica leve)
/// alarma UMA vez. RED: tirar o `sinal` do `violacao_de_credencial` -> zero.
#[test]
fn a_quinta_credencial_errada_e_forca_bruta_e_a_quarta_nao() {
    let dir = dir_temp("forca");
    let mut c = config_base(&dir);
    // A senha em claro aceita: o que se mede e a credencial errada, e nao a
    // recusa do fio (que tem alarme proprio, logo abaixo).
    c.cifra_fio.senha_em_claro_pela_rede = true;
    let s = Servidor::novo(c).unwrap();
    let ip = "203.0.113.81";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    for tentativa in 1..=5 {
        let mut sessao = Sessao::default();
        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"login","usuario":"ana","senha":"senha-errada-779"}"#,
            &mut sessao,
            ip,
        );
        assert!(r.is_err());
        let vistas = ocorrencias_de(&s, Alarme::ForcaBruta);
        if tentativa < 5 {
            assert!(vistas.is_empty(), "a tentativa {tentativa} alarmou");
        } else {
            assert_eq!(vistas.len(), 1, "{vistas:?}");
            assert_eq!(vistas[0].ip, ip);
            assert!(
                !vistas[0].dados.escrever().contains("senha-errada-779"),
                "a senha foi parar no alarme"
            );
            assert!(
                a.alarmes() & Alarme::ForcaBruta.bit() != 0,
                "o bit nao subiu"
            );
        }
    }
}

/// O IRMAO do REST: o token do REST errado conta pelo MESMO motor. Pelo
/// metodo, e nao pela porta HTTP: o que se prova e que os tres pontos da
/// credencial chamam a mesma regua.
#[test]
fn o_token_da_porta_errado_tambem_e_forca_bruta() {
    let dir = dir_temp("forca-token");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "203.0.113.82";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    for _ in 0..5 {
        let mut sessao = Sessao::default();
        let _ = s.despachar(r#"{"token":"nao","op":"ping"}"#, &mut sessao, ip);
    }
    assert_eq!(ocorrencias_de(&s, Alarme::ForcaBruta).len(), 1);
}

/// O comportamento VELHO: o que nao e credencial -- o IP fora da lista de
/// permitidos, contando pela mesma politica leve -- bloqueia sem virar
/// forca bruta. RED: o `sinal` posto no `violacao_leve` (para todo motivo)
/// -> 1 ocorrencia aqui.
#[test]
fn a_tentativa_leve_que_nao_e_credencial_nao_e_forca_bruta() {
    let dir = dir_temp("forca-nao");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "203.0.113.83";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    for _ in 0..5 {
        let _ = s.violacao_leve(ip, "conexao", "ip fora da lista de permitidos");
    }
    assert!(
        s.barrado(ip, crate::agora_ms()).is_some(),
        "premissa: bloqueou"
    );
    assert!(ocorrencias_de(&s, Alarme::ForcaBruta).is_empty());
}

// ----------------------------------------------------------- SenhaEmClaro

/// A senha que chega por fio em claro, de fora do loopback, e recusada
/// (pedido 667) e agora alarma -- sem a senha no alarme. RED: tirar o
/// `sinal` do `conferir_o_fio_da_senha` -> zero.
#[test]
fn a_senha_em_claro_pela_rede_alarma_sem_levar_a_senha() {
    let dir = dir_temp("claro");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "203.0.113.84";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let mut sessao = Sessao {
        ip: ip.into(),
        entrada: Entrada::Dados,
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"login","usuario":"ana","senha":"vazou-779-em-claro"}"#,
        &mut sessao,
        ip,
    );
    // A recusa e a do fio (pedido 667), e nao a da credencial: a frase
    // vem da MESMA chave que o portao usa.
    let e = r.unwrap_err().to_string();
    let do_fio = s.msg("erro.senha_em_claro_pela_rede", &[]);
    assert!(e.contains(&do_fio), "{e}");
    let vistas = ocorrencias_de(&s, Alarme::SenhaEmClaro);
    assert_eq!(vistas.len(), 1, "{vistas:?}");
    let texto = vistas[0].dados.escrever();
    assert!(
        !texto.contains("vazou-779"),
        "a senha foi parar no alarme: {texto}"
    );
}

/// O comportamento VELHO: no loopback a senha em claro e aceita (nunca
/// atravessou a rede) e nao alarma; sem senha no pedido (o desafio) nada
/// acontece. RED: o `sinal` antes do `fio_aceita_credencial` -> alarma.
#[test]
fn a_senha_pelo_loopback_nao_alarma() {
    let dir = dir_temp("claro-velho");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "127.0.0.1";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let mut sessao = Sessao {
        ip: ip.into(),
        entrada: Entrada::Dados,
        ..Sessao::default()
    };
    let _ = s.despachar(
        r#"{"token":"t","op":"login","usuario":"ana","senha":"qualquer"}"#,
        &mut sessao,
        ip,
    );
    assert!(ocorrencias_de(&s, Alarme::SenhaEmClaro).is_empty());
    // De fora, mas SEM senha no pedido (o desafio): nada a alarmar.
    let fora = "203.0.113.87";
    let b = amarrada(&s, fora);
    let _b = crate::telemetria::amarrar(Some(Arc::clone(&b)));
    let mut sessao = Sessao {
        ip: fora.into(),
        entrada: Entrada::Dados,
        ..Sessao::default()
    };
    let _ = s.despachar(
        r#"{"token":"t","op":"desafio","usuario":"ana"}"#,
        &mut sessao,
        fora,
    );
    assert!(ocorrencias_de(&s, Alarme::SenhaEmClaro).is_empty());
}

// ---------------------------------------------------- IntegridadeRecusada

/// Os tres codigos sao os dos `PhxError` de verdade -- o numero nao deriva
/// calado.
#[test]
fn os_codigos_de_integridade_sao_os_do_erro() {
    assert_eq!(
        super::servico_avisos_01::CODIGOS_DE_INTEGRIDADE,
        [
            PhxError::Duplicado(String::new()).codigo(),
            PhxError::Conflito(String::new()).codigo(),
            PhxError::Integridade(String::new()).codigo(),
        ]
    );
}

/// A recusa de unicidade, de verdade, pelo `despachar`, e o desfecho dela no
/// `anotar` -- o sumidouro unico: o bit amarelo na tarefa e UMA ocorrencia.
/// O `NaoEncontrado` (3001) nao alarma. RED: tirar o gancho do `anotar` ->
/// zero.
#[test]
fn a_recusa_de_unicidade_vira_integridade_recusada() {
    let dir = dir_temp("integridade");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "203.0.113.85";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let dono = Sessao::default();
    let pede = |op: &str, corpo: &str| s.executar(op, &Json::analisar(corpo).unwrap(), &dono);
    pede("criar_database", r#"{"database":"loja"}"#).unwrap();
    pede(
        "criar_tabela",
        r#"{"database":"loja","tabela":"cli",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
            "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
    )
    .unwrap();
    let inserir = r#"{"database":"loja","tabela":"cli","linha":{"id":1}}"#;
    pede("inserir", inserir).unwrap();
    let e = pede("inserir", inserir).unwrap_err();
    assert_eq!(e.codigo(), 3002, "premissa: a recusa e de unicidade: {e}");
    a.comecou_pedido("inserir", "", "loja", "cli", crate::agora_ms());
    s.anotar(&Acesso {
        quando_ms: crate::agora_ms(),
        ip: ip.into(),
        op: "inserir".into(),
        database: "loja".into(),
        tabela: "cli".into(),
        erro: Some(e.to_string()),
        codigo: e.codigo(),
        ..Acesso::default()
    });
    assert!(a.alarmes() & Alarme::IntegridadeRecusada.bit() != 0);
    assert_eq!(ocorrencias_de(&s, Alarme::IntegridadeRecusada).len(), 1);
    // O comportamento VELHO: «nao achei» nao e recusa de integridade.
    let ip2 = "203.0.113.86";
    let b = amarrada(&s, ip2);
    let _b = crate::telemetria::amarrar(Some(Arc::clone(&b)));
    b.comecou_pedido("ler", "", "loja", "cli", crate::agora_ms());
    s.anotar(&Acesso {
        quando_ms: crate::agora_ms(),
        ip: ip2.into(),
        op: "ler".into(),
        erro: Some("nao achei".into()),
        codigo: PhxError::NaoEncontrado(String::new()).codigo(),
        ..Acesso::default()
    });
    assert_eq!(b.alarmes() & Alarme::IntegridadeRecusada.bit(), 0);
    assert!(ocorrencias_de(&s, Alarme::IntegridadeRecusada).is_empty());
}

// ------------------------------------------- MarcaNaoResolvida, bidirecional

/// Uma marca do bidirecional valida em `<base>/loja`, para a tabela `fantasma`.
fn plantar_marca_do_bidi(dir: &std::path::Path, database: &str) -> std::path::PathBuf {
    let pasta = dir.join(database);
    std::fs::create_dir_all(&pasta).unwrap();
    let imagem: Vec<u8> = Vec::new();
    crate::transacao::gravar_marca_do_bidi(
        &pasta,
        779,
        crate::agora_ms(),
        &[phxsql_store::marca::EventoDoGrupo {
            tabela: "fantasma",
            operacao: phxsql_store::log::Operacao::Inclusao,
            rowid: 1,
            carimbo_ms: crate::agora_ms(),
            origem: 7,
            posicao: 1,
            imagem: &imagem,
        }],
    )
    .unwrap()
}

/// O ramo `SemChave`: a marca de um binario MAIS NOVO (versao acima da que
/// este le) PARA e nao apaga -- e vira ocorrencia. RED: tirar o `sinal` do
/// ramo `SemChave` do `completar_marcas_do_bidi`.
#[test]
fn a_marca_do_bidi_sem_chave_vira_ocorrencia_e_fica() {
    let dir = dir_temp("bidi-sem-chave");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let marca = plantar_marca_do_bidi(&dir, "loja");
    let mut b = std::fs::read(&marca).unwrap();
    b[8..12].copy_from_slice(&999u32.to_le_bytes());
    std::fs::write(&marca, &b).unwrap();
    let a = amarrada(&s, "bidi-sem-chave");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    s.completar_marcas_do_bidi();
    assert!(marca.exists(), "a marca sem chave nao pode ser apagada");
    assert_eq!(ocorrencias_de(&s, Alarme::MarcaNaoResolvida).len(), 1);
}

/// O ramo da marca que NAO SE LE (E/S): fica, e vira ocorrencia. A falha e
/// a do gancho de teste do `store` (pedido 451, M4), armada nesta thread.
/// RED: tirar o `sinal` do ramo `Err` da leitura.
#[test]
fn a_marca_do_bidi_que_nao_se_le_vira_ocorrencia_e_fica() {
    let dir = dir_temp("bidi-ilegivel");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let marca = plantar_marca_do_bidi(&dir, "loja");
    let a = amarrada(&s, "bidi-ilegivel");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    phxsql_store::marca::falhar_a_proxima_leitura_de_teste();
    s.completar_marcas_do_bidi();
    assert!(
        marca.exists(),
        "a marca que nao se leu nao pode ser apagada"
    );
    assert_eq!(ocorrencias_de(&s, Alarme::MarcaNaoResolvida).len(), 1);
}

/// O ramo do grupo que NAO SE COMPLETA: a marca abre, mas o database dela nao
/// e um database (a pasta nao se abre como tal). Fica, e vira ocorrencia.
/// RED: tirar o `sinal` do ramo `Err` do `completar_grupo_bidi`.
#[test]
fn a_marca_do_bidi_que_nao_se_completa_vira_ocorrencia_e_fica() {
    let dir = dir_temp("bidi-incompleta");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let marca = plantar_marca_do_bidi(&dir, "nao-e-database");
    let a = amarrada(&s, "bidi-incompleta");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    s.completar_marcas_do_bidi();
    assert!(
        marca.exists(),
        "a marca que nao se completou nao pode ser apagada"
    );
    assert_eq!(ocorrencias_de(&s, Alarme::MarcaNaoResolvida).len(), 1);
}

/// O comportamento VELHO: a marca que nao CONFERE (um grupo que nunca
/// comecou) sai calada -- nem fica, nem alarma.
#[test]
fn a_marca_do_bidi_que_nao_confere_sai_sem_alarme() {
    let dir = dir_temp("bidi-nao-confere");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let marca = plantar_marca_do_bidi(&dir, "loja");
    std::fs::write(&marca, b"isto nao e uma marca").unwrap();
    let a = amarrada(&s, "bidi-nao-confere");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    s.completar_marcas_do_bidi();
    assert!(!marca.exists());
    assert!(ocorrencias_de(&s, Alarme::MarcaNaoResolvida).is_empty());
}

// ----------------------------------------- a replica agendada e o `Ritmo`

fn porta_fechada() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn origem_fechada(nome: &str) -> crate::config::Origem {
    crate::config::Origem {
        nome: nome.into(),
        host: "127.0.0.1".into(),
        porta: porta_fechada(),
        token: "x".into(),
        databases: Vec::new(),
        reconectar_em: 10,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 60,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
        espelho: false,
    }
}

/// A janela da replica agendada com a origem fora do ar ALEM do prazo vira a
/// pedra `origem_inalcancavel`, uma por episodio -- e antes do prazo, nada.
/// Antes do 779 a falha de rede do laco agendado so ia ao `stderr`. RED:
/// tirar o `registrar_a_falha` do `alcancar_na_janela`.
#[test]
fn a_replica_agendada_alarma_a_origem_inalcancavel_alem_do_prazo() {
    use crate::replica::Ritmo;
    let dir = dir_temp("agendada");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, "agendada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let origem = origem_fechada("p779-agendada");
    let mut ritmo = Ritmo::novo(Duration::from_secs(origem.reconectar_em));
    // Dentro do prazo: a janela que falha nao alarma.
    assert!(s.alcancar_na_janela(&origem, &mut ritmo));
    assert!(ocorrencias_de(&s, Alarme::OrigemInalcancavel).is_empty());
    // O episodio ja passou do prazo: a janela seguinte alarma, uma vez.
    ritmo.inalcancavel_ha(Ritmo::PRAZO_DE_INALCANCAVEL + Duration::from_secs(1));
    assert!(s.alcancar_na_janela(&origem, &mut ritmo));
    assert_eq!(ocorrencias_de(&s, Alarme::OrigemInalcancavel).len(), 1);
    assert!(s.alcancar_na_janela(&origem, &mut ritmo));
    assert!(
        !ritmo.cruzou_o_prazo(),
        "o episodio alarmou duas vezes na replica agendada"
    );
}

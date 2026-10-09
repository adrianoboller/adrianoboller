//! Pedidos 765/766, fatias P6 e P9 (`docs/propostas/protecao-765-desenho.md`
//! §6), pelo caminho que o servidor usa: o `despachar`, a memoria de IPs e a
//! lista negra.
//!
//! # O que cada grupo prova
//!
//! - **P6 (`IpNovo`):** o primeiro login com sucesso de um IP novo vira UMA
//!   ocorrencia; o seguinte, nenhuma -- inclusive depois de reiniciar, pela
//!   memoria em disco, e nao pelo silencio da camada de ocorrencias (que e
//!   por servidor e morre com ele); e o IP que ja estava no `acessos.log`
//!   nao e novo no dia em que a memoria nasce.
//! - **P9 (nao se trancar):** loopback, whitelist, IP de administrador e IP
//!   compartilhado chegam ao limite de tentativas e NAO sao bloqueados -- e
//!   a forca bruta vira ocorrencia mesmo assim. O comportamento VELHO junto:
//!   o IP desconhecido continua bloqueado, e `poupar_loopback: false`
//!   devolve o bloqueio do loopback a quem o quer.
//!
//! # RED
//!
//! Cada teste nomeia a linha que, tirada, o derruba. Registrado em
//! `docs/propostas/protecao-765-desenho.md` §6.1.

use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;

const SENHA: &str = "segredo-766-p6";

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("p766-{rotulo}"))
}

/// Tres usuarios: dois leitores e um administrador, com a mesma senha
/// (PBKDF2 de 1.000 voltas: o teste mede a guarda, nao o custo).
fn config_base(dir: &std::path::Path) -> Config {
    let hash = phxsql_core::senha::cifrar_com(SENHA, 1_000);
    let usuarios = format!(
        r#"{{"usuarios":[
            {{"login":"ana","senha_hash":"{hash}","nivel":"leitor"}},
            {{"login":"bia","senha_hash":"{hash}","nivel":"leitor"}},
            {{"login":"chefe","senha_hash":"{hash}","nivel":"admin"}}]}}"#
    );
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        cadastro: crate::usuarios::Cadastro::de_json(&Json::analisar(&usuarios).unwrap()).unwrap(),
        ..Config::default()
    };
    // O ESCAPE ESCRITO: o que se mede e a memoria e a guarda, e o `despachar`
    // daqui nao tem fio cifrado para a senha atravessar.
    c.cifra_fio.exigir = false;
    c.cifra_fio.senha_em_claro_pela_rede = true;
    c
}

/// O prefixo da tarefa das atividades amarradas por este arquivo.
///
/// O alarme de SERVIDOR sem atividade amarrada vai a camada do PROCESSO, que
/// e a do ultimo servidor criado neste binario -- pode ser o de um teste
/// daqui. Medido na suite de 09/10/2026: um `FirewallBloqueou` de outro teste
/// caiu no correio deste e o `len() == 1` viu 2. Contar so o que nasceu nas
/// atividades daqui tira o vizinho da conta, e nao afrouxa nada.
const TAREFA: &str = "dados:p766-";

/// As ocorrencias de `alarme` que esperam o carteiro neste servidor, nascidas
/// nas atividades deste arquivo.
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

/// Uma atividade deste servidor amarrada a esta thread, como a conexao faz:
/// a ocorrencia vai ao correio DESTE servidor, e nenhum outro teste do
/// binario a ve ou a produz.
fn amarrada(s: &Arc<Servidor>, ip: &str) -> Arc<crate::telemetria::Atividade> {
    s.telemetria
        .entrar(
            &format!("dados:p766-{ip}"),
            "dados",
            ip,
            1,
            crate::agora_ms(),
        )
        .expect("a telemetria nasce ligada")
}

fn login(s: &Arc<Servidor>, usuario: &str, senha: &str, ip: &str) -> Result<Json> {
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t","op":"login","usuario":"{usuario}","senha":"{senha}"}}"#),
        &mut sessao,
        ip,
    );
    r
}

/// Cinco pedidos com o token errado (o padrao `tentativas_ate_bloquear`).
fn cinco_tokens_errados(s: &Arc<Servidor>, ip: &str) {
    for _ in 0..5 {
        let mut sessao = Sessao::default();
        let (_, _, r) = s.despachar(r#"{"token":"errado","op":"ping"}"#, &mut sessao, ip);
        assert!(r.is_err(), "o token errado tinha de ser recusado");
    }
}

// ----------------------------------------------------------------- P6

/// **O aceite da P6:** 1o login de IP novo -> 1 ocorrencia `ip_novo`; o 2o
/// -> 0, e depois de REINICIAR o servidor (silencio novo) continua 0: e a
/// memoria em disco que lembra, nao o silencio. RED: tirar a chamada do
/// `ip_visto_no_login` no `despachar` -> nenhuma ocorrencia; `registrar`
/// sem memoria (sempre novo) -> 1 ocorrencia depois do reinicio.
#[test]
fn o_primeiro_login_de_um_ip_novo_gera_uma_ocorrencia_e_o_segundo_nenhuma() {
    let dir = dir_temp("p6");
    let ip = "203.0.113.61";
    {
        let s = Servidor::novo(config_base(&dir)).unwrap();
        let a = amarrada(&s, ip);
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        login(&s, "ana", SENHA, ip).expect("o login tinha de dar certo");
        let vistas = ocorrencias_de(&s, Alarme::IpNovo);
        assert_eq!(vistas.len(), 1, "{vistas:?}");
        assert_eq!(vistas[0].ip, ip, "a ocorrencia leva o IP");
        assert!(
            !vistas[0].dados.escrever().contains(SENHA),
            "a senha foi parar na ocorrencia"
        );
        login(&s, "ana", SENHA, ip).unwrap();
        assert!(ocorrencias_de(&s, Alarme::IpNovo).is_empty());
    }
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    login(&s, "ana", SENHA, ip).unwrap();
    assert!(
        ocorrencias_de(&s, Alarme::IpNovo).is_empty(),
        "depois de reiniciar o IP ja visto voltou a ser novo"
    );
    // O MESMO IP e novo para OUTRO usuario: a chave e (usuario, base, IP).
    login(&s, "bia", SENHA, ip).unwrap();
    assert_eq!(ocorrencias_de(&s, Alarme::IpNovo).len(), 1);
}

/// A semente: o IP que ja estava no `acessos.log` nao gera ocorrencia no
/// arranque em que a memoria nasce. RED: a semente vazia (`Vec::new` no
/// lugar do `LogAcessos::ler` do `Servidor::novo`) -> 1 ocorrencia.
#[test]
fn o_ip_que_ja_esta_no_acessos_log_nao_gera_ocorrencia() {
    let dir = dir_temp("p6-semente");
    let ip = "203.0.113.62";
    {
        let mut log = LogAcessos::abrir(dir.join("acessos.log")).unwrap();
        log.registrar(&Acesso {
            quando_ms: crate::agora_ms() - 86_400_000,
            ip: ip.into(),
            op: "login".into(),
            usuario: "ana".into(),
            autenticado: true,
            ok: true,
            ..Acesso::default()
        })
        .unwrap();
    }
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    login(&s, "ana", SENHA, ip).unwrap();
    assert!(
        ocorrencias_de(&s, Alarme::IpNovo).is_empty(),
        "o IP do acessos.log foi acusado como novo"
    );
    assert!(dir.join(crate::ips_vistos::NOME_DO_ARQUIVO).exists());
}

/// O comportamento VELHO: o login que falha nao vai a memoria nem acusa IP
/// novo -- e o seguinte, que da certo, e o primeiro.
#[test]
fn o_login_que_falha_nao_e_ip_visto() {
    let dir = dir_temp("p6-falha");
    let ip = "203.0.113.63";
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    assert!(login(&s, "ana", "errada", ip).is_err());
    assert!(ocorrencias_de(&s, Alarme::IpNovo).is_empty());
    login(&s, "ana", SENHA, ip).unwrap();
    assert_eq!(ocorrencias_de(&s, Alarme::IpNovo).len(), 1);
}

// ----------------------------------------------------------------- P9

/// **O caso medido em 09/10/2026:** cinco tokens errados vindos de
/// `127.0.0.1` bloqueavam o proprio loopback por 60 minutos -- a tela e a
/// TV junto. Agora: nada na lista, o `ping` seguinte do loopback entra, e a
/// forca bruta vira UMA ocorrencia. RED: tirar o ramo do loopback da
/// `Blacklist::guarda` -> o loopback fica barrado.
#[test]
fn cinco_tokens_errados_do_loopback_nao_bloqueiam_o_loopback() {
    let dir = dir_temp("p9-loopback");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "127.0.0.1";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    cinco_tokens_errados(&s, ip);
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "o loopback se trancou para fora"
    );
    assert!(s.lista_negra.lock().unwrap().lista().is_empty());
    assert!(ocorrencias_de(&s, Alarme::FirewallBloqueou).is_empty());
    assert_eq!(ocorrencias_de(&s, Alarme::ForcaBruta).len(), 1);
    // A forma v6 do loopback e a mapeada tambem.
    for outro in ["::1", "::ffff:127.0.0.1"] {
        cinco_tokens_errados(&s, outro);
        assert!(s.barrado(outro, crate::agora_ms()).is_none(), "{outro}");
    }
    // E a porta de dados continua atendendo o loopback.
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(r#"{"token":"t","op":"ping"}"#, &mut sessao, ip);
    assert!(r.is_ok(), "{r:?}");
}

/// O comportamento VELHO, pedido: `poupar_loopback: false` devolve o
/// bloqueio do loopback (e e o que os testes de soquete usam). RED: o
/// interruptor ignorado -> o loopback nao bloqueia.
#[test]
fn com_poupar_loopback_desligado_o_loopback_bloqueia_como_antes() {
    let dir = dir_temp("p9-loopback-velho");
    let mut c = config_base(&dir);
    c.politica.poupar_loopback = false;
    let s = Servidor::novo(c).unwrap();
    // Amarrada para o alarme do bloqueio ficar NESTE servidor, e nao cair
    // na camada do processo, que pode ser a de um teste vizinho.
    let a = amarrada(&s, "127.0.0.1");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    cinco_tokens_errados(&s, "127.0.0.1");
    assert!(s.barrado("127.0.0.1", crate::agora_ms()).is_some());
}

/// O bloqueio que um binario ANTERIOR gravou para o loopback nao segura a
/// porta: a guarda vale tambem no `barrado()`, como a whitelist sempre
/// valeu.
#[test]
fn o_loopback_bloqueado_antes_da_guarda_entra_de_novo() {
    let dir = dir_temp("p9-loopback-antigo");
    {
        let mut bl = Blacklist::abrir(dir.join("blacklist.json")).unwrap();
        let p = crate::blacklist::Politica {
            poupar_loopback: false,
            ..crate::blacklist::Politica::default()
        };
        bl.bloquear(
            "127.0.0.1",
            "token invalido",
            "ping",
            5,
            &p,
            crate::agora_ms(),
        );
    }
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(s.barrado("127.0.0.1", crate::agora_ms()).is_none());
}

/// Whitelist, pelo caminho LEVE: o IP conta, chega ao limite, nao bloqueia
/// -- e a forca bruta aparece. Antes, o protegido saia sem contar e a forca
/// bruta vinda dele era invisivel. RED: o ramo `Whitelist` da guarda tirado
/// -> bloqueia.
#[test]
fn a_whitelist_chega_ao_limite_sem_bloquear_e_alarma_a_forca_bruta() {
    let dir = dir_temp("p9-whitelist");
    let ip = "203.0.113.70";
    let mut c = config_base(&dir);
    c.politica.whitelist = vec![ip.into()];
    let s = Servidor::novo(c).unwrap();
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    cinco_tokens_errados(&s, ip);
    assert!(s.barrado(ip, crate::agora_ms()).is_none());
    assert!(ocorrencias_de(&s, Alarme::FirewallBloqueou).is_empty());
    assert_eq!(ocorrencias_de(&s, Alarme::ForcaBruta).len(), 1);
}

/// IP de onde um ADMINISTRADOR entrou nas ultimas 24 h: nao bloqueia --
/// bloquea-lo trancaria para fora quem solta o bloqueio. RED: tirar o ramo
/// `Administrador` do `IpsVistos::guarda` -> bloqueia.
#[test]
fn o_ip_de_quem_administra_nao_e_bloqueado() {
    let dir = dir_temp("p9-admin");
    let ip = "203.0.113.71";
    let s = Servidor::novo(config_base(&dir)).unwrap();
    login(&s, "chefe", SENHA, ip).unwrap();
    cinco_tokens_errados(&s, ip);
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "o IP do administrador foi bloqueado"
    );
}

/// IP COMPARTILHADO -- dois usuarios distintos em 30 dias (NAT, proxy): nao
/// bloqueia, porque derrubaria quem nao fez nada. RED: tirar o ramo
/// `Compartilhado` do `IpsVistos::guarda` -> bloqueia.
#[test]
fn o_ip_de_dois_usuarios_nao_e_bloqueado() {
    let dir = dir_temp("p9-compartilhado");
    let ip = "203.0.113.72";
    let s = Servidor::novo(config_base(&dir)).unwrap();
    login(&s, "ana", SENHA, ip).unwrap();
    login(&s, "bia", SENHA, ip).unwrap();
    cinco_tokens_errados(&s, ip);
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "o IP compartilhado foi bloqueado"
    );
}

/// O comportamento VELHO, e o que impede a guarda de virar «nunca bloqueia
/// ninguem»: o IP desconhecido bloqueia, e o IP de UM usuario leitor
/// tambem. RED: a guarda devolvendo sempre `Some` -> nenhum dos dois
/// bloqueia.
#[test]
fn o_ip_desconhecido_e_o_de_um_usuario_so_continuam_bloqueando() {
    let dir = dir_temp("p9-velho");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let desconhecido = "203.0.113.73";
    let a = amarrada(&s, desconhecido);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    cinco_tokens_errados(&s, desconhecido);
    assert!(s.barrado(desconhecido, crate::agora_ms()).is_some());
    assert_eq!(ocorrencias_de(&s, Alarme::FirewallBloqueou).len(), 1);
    assert_eq!(ocorrencias_de(&s, Alarme::ForcaBruta).len(), 1);

    let de_um_so = "203.0.113.74";
    login(&s, "ana", SENHA, de_um_so).unwrap();
    cinco_tokens_errados(&s, de_um_so);
    assert!(s.barrado(de_um_so, crate::agora_ms()).is_some());
}

/// P10 pelo servidor: o segundo bloqueio do mesmo IP dura o dobro, e o
/// historico atravessa o reinicio pelo `blacklist.json`. RED: o
/// `historico` fora do `gravar` -> depois do reinicio o terceiro dura 60.
#[test]
fn o_bloqueio_reincidente_dobra_e_o_reinicio_preserva_a_conta() {
    let dir = dir_temp("p10");
    let ip = "203.0.113.75";
    let minutos = |s: &Arc<Servidor>| {
        let b = s.barrado(ip, crate::agora_ms()).expect("bloqueado");
        (b.ate_ms - b.desde_ms) / 60_000
    };
    // Amarrada em cada servidor, pelo motivo do teste do loopback velho: o
    // alarme do bloqueio fica no servidor que bloqueou.
    {
        let s = Servidor::novo(config_base(&dir)).unwrap();
        let a = amarrada(&s, ip);
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        cinco_tokens_errados(&s, ip);
        assert_eq!(minutos(&s), 60);
        assert!(s.lista_negra.lock().unwrap().desbloquear(ip).unwrap());
        cinco_tokens_errados(&s, ip);
        assert_eq!(minutos(&s), 120);
        assert!(s.lista_negra.lock().unwrap().desbloquear(ip).unwrap());
    }
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    cinco_tokens_errados(&s, ip);
    assert_eq!(minutos(&s), 240, "o reinicio zerou a reincidencia");
}

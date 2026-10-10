//! Pedidos 765/766, fatias P7 e P8 (`docs/propostas/protecao-765-desenho.md`
//! §4.5, §4.6 e §6), pelo caminho que o servidor usa: o `despachar`.
//!
//! - **P7 (perfil habitual, so observa):** 300 leituras em `clientes`, depois
//!   `excluir` em `folha` -> uma `ForaDoPerfil`; a mesma leitura -> nada; o
//!   usuario com menos de 200 pedidos -> nada; a telemetria desligada -> o
//!   perfil nem nasce (custo zero). A resposta nunca muda.
//! - **P8 (codigo malicioso conta para bloquear, pedido):** desligado, 50
//!   tautologias -> nenhum bloqueio (o comportamento velho); ligado, 5 do
//!   mesmo IP -> bloqueio pelo `violacao_leve` de sempre, e a sessao de quem
//!   mandou termina; o IP poupado pela guarda (loopback) nao vai a lista, e a
//!   sessao termina do mesmo jeito; e o empilhado nao conta duas vezes com o
//!   `contar_injecao_sql` velho ligado junto.
//!
//! # RED
//!
//! Cada teste nomeia a linha que, tirada, o derruba. Registrado no desenho
//! §6.3 e no catalogo de guardas.

use super::testes_do_observador::{do_arsenal, tautologia_do_arsenal};
use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;

const DIA_MS: i64 = 24 * 3_600_000;

fn cadastro() -> crate::Cadastro {
    let h = phxsql_core::senha::cifrar_com("p78-senha", 1);
    crate::Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{"usuarios":[
                {{"login":"ana","senha_hash":"{h}","nivel":"operador"}},
                {{"login":"bia","senha_hash":"{h}","nivel":"operador"}},
                {{"login":"chefe","senha_hash":"{h}","nivel":"admin"}}]}}"#
        ))
        .unwrap(),
    )
    .unwrap()
}

fn config(dir: &std::path::Path) -> Config {
    Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        cadastro: cadastro(),
        ..Config::default()
    }
}

/// `b.clientes` com duas linhas e `b.folha` com uma.
fn popular(s: &Servidor) {
    let dono = Sessao::default();
    let p = |t: &str| Json::analisar(t).unwrap();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    for t in ["clientes", "folha"] {
        s.executar(
            "criar_tabela",
            &p(&format!(
                r#"{{"database":"b","tabela":"{t}",
                   "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                              {{"nome":"nome","tipo":"Str(20)"}}],
                   "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                               "primario":true}}]}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    for (t, id, nome) in [
        ("clientes", 1, "Adriano"),
        ("clientes", 2, "Maria"),
        ("folha", 1, "Joana"),
    ] {
        s.executar(
            "inserir",
            &p(&format!(
                r#"{{"database":"b","tabela":"{t}","linha":{{"id":{id},"nome":"{nome}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
}

fn servidor(dir: &DirTemp, ajuste: impl FnOnce(&mut Config)) -> Arc<Servidor> {
    let mut c = config(dir);
    ajuste(&mut c);
    let s = Servidor::novo(c).unwrap();
    popular(&s);
    s
}

fn sessao(login: &str, ip: &str) -> Sessao {
    Sessao {
        usuario: cadastro().por_login(login).cloned(),
        ip: ip.into(),
        ..Sessao::default()
    }
}

const TAREFA: &str = "dados:p78-";

/// Um pedido como a conexao o faz: a atividade amarrada (a ocorrencia vai
/// ao correio DESTE servidor) e o `despachar`.
fn pede(s: &Arc<Servidor>, sessao: &mut Sessao, corpo: &str) -> Result<Json> {
    let ip = sessao.ip.clone();
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar(&format!("{TAREFA}{ip}"), "dados", &ip, 1, agora);
    let _amarrada = crate::telemetria::amarrar(a.clone());
    let login = sessao.login().to_string();
    if let Some(a) = &a {
        a.comecou_pedido("p78", &login, "b", "", agora);
    }
    let linha = format!(r#"{{"token":"t",{corpo}}}"#);
    let r = s.despachar(&linha, sessao, &ip).2;
    if let Some(a) = &a {
        a.terminou_pedido(&login);
    }
    r
}

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

/// O perfil com 7 dias e meio de historia e as 24 horas com massa: o que se
/// mede e a combinacao, e a hora do relogio de quem roda o teste nao pode
/// virar «hora rara» por acaso.
fn semear_perfil(dir: &DirTemp, login: &str) {
    let horas = vec!["10"; 24].join(",");
    std::fs::write(
        dir.join(crate::perfis::NOME_DO_ARQUIVO),
        format!(
            "{{\"usuario\":\"{login}\",\"primeiro_ms\":{},\"n\":0,\"horas\":[{horas}]}}\n",
            crate::agora_ms() - 8 * DIA_MS
        ),
    )
    .unwrap();
}

const LER_CLIENTES: &str = r#""op":"varrer","database":"b","tabela":"clientes""#;
const EXCLUIR_FOLHA: &str = r#""op":"excluir","database":"b","tabela":"folha","rowid":1"#;

// ----------------------------------------------------------------- P7

/// **O aceite da P7:** 300 leituras em `clientes`, depois `excluir` em
/// `folha` -> UMA `ForaDoPerfil`; a mesma leitura de novo -> nada. E a
/// resposta nao muda: o excluir executa. RED: tirar a chamada do
/// `perfil_do_pedido` no `executar_e_contar_escrita_local` -> nenhuma
/// ocorrencia.
#[test]
fn trezentas_leituras_e_um_excluir_em_outra_tabela_saem_do_perfil() {
    let dir = DirTemp::novo("p78-p7-aceite");
    semear_perfil(&dir, "ana");
    let s = servidor(&dir, |_| {});
    let mut ana = sessao("ana", "203.0.113.71");
    for _ in 0..300 {
        pede(&s, &mut ana, LER_CLIENTES).expect("a leitura executa");
    }
    assert!(ocorrencias_de(&s, Alarme::ForaDoPerfil).is_empty());

    pede(&s, &mut ana, EXCLUIR_FOLHA).expect("so observa: o excluir executa");
    let vistas = ocorrencias_de(&s, Alarme::ForaDoPerfil);
    assert_eq!(vistas.len(), 1, "{vistas:?}");
    assert_eq!(vistas[0].usuario, "ana");
    // O `dados` inteiro, por igualdade: o motivo, a categoria e a tabela --
    // e nada do valor da linha (o nome «Joana» da folha).
    assert_eq!(
        vistas[0].dados,
        Json::texto_de("combinacao_nova escrever b folha"),
        "{:?}",
        vistas[0]
    );

    pede(&s, &mut ana, LER_CLIENTES).unwrap();
    assert!(ocorrencias_de(&s, Alarme::ForaDoPerfil).is_empty());
}

/// O comando perigoso que a camada RECUSA tambem entra no perfil -- e o
/// pedido mais fora do habito que existe. RED: o perfil depois do
/// `protecao_do_pedido(...)?` -> o `?` sai antes e nada acusa.
#[test]
fn o_comando_perigoso_recusado_tambem_sai_do_perfil() {
    let dir = DirTemp::novo("p78-p7-recusado");
    semear_perfil(&dir, "chefe");
    let s = servidor(&dir, |_| {});
    let mut chefe = sessao("chefe", "203.0.113.75");
    for _ in 0..300 {
        pede(&s, &mut chefe, LER_CLIENTES).unwrap();
    }
    let r = pede(
        &s,
        &mut chefe,
        r#""op":"excluir_tabela","database":"b","tabela":"folha","confirmar":"folha""#,
    );
    match r {
        Err(e) => assert_eq!(e.codigo(), 4009, "recusou por outro motivo: {e}"),
        Ok(j) => panic!("executou sem a senha de execucao: {}", j.escrever()),
    }
    assert!(dir.join("b").join("folha.reg").exists());
    let vistas = ocorrencias_de(&s, Alarme::ForaDoPerfil);
    assert_eq!(vistas.len(), 1, "{vistas:?}");
    assert_eq!(
        vistas[0].dados,
        Json::texto_de("combinacao_nova administrar b folha"),
        "{:?}",
        vistas[0]
    );
}

/// O usuario com menos de 200 pedidos de historia nao acusa nada, mesmo
/// com 7 dias. RED: tirar o `p.n >= PISO_DE_PEDIDOS` do `maduro`.
#[test]
fn o_usuario_com_menos_de_200_pedidos_nao_sai_do_perfil() {
    let dir = DirTemp::novo("p78-p7-novato");
    semear_perfil(&dir, "bia");
    let s = servidor(&dir, |_| {});
    let mut bia = sessao("bia", "203.0.113.72");
    for _ in 0..50 {
        pede(&s, &mut bia, LER_CLIENTES).unwrap();
    }
    pede(&s, &mut bia, EXCLUIR_FOLHA).unwrap();
    assert!(ocorrencias_de(&s, Alarme::ForaDoPerfil).is_empty());
}

/// **Custo zero com a telemetria desligada:** o perfil nem nasce, e o
/// arquivo nao se escreve. RED: tirar o portao `self.telemetria.ligada()`
/// de antes do `perfil_do_pedido` -> o perfil da ana aparece.
#[test]
fn com_a_telemetria_desligada_o_perfil_nao_nasce() {
    let dir = DirTemp::novo("p78-p7-desligada");
    let s = servidor(&dir, |_| {});
    s.telemetria.desligar();
    let mut ana = sessao("ana", "203.0.113.73");
    for _ in 0..5 {
        pede(&s, &mut ana, LER_CLIENTES).unwrap();
    }
    assert_eq!(s.perfis.lock().unwrap().quantos(), 0);
    assert!(!dir.join(crate::perfis::NOME_DO_ARQUIVO).exists());
}

/// A op `sql` entra pelos passos que produz, com a op e a tabela de
/// verdade, e nao como combinacao propria; e o caminho SEM IP (job, rotina
/// interna, replica) fica fora. RED: tirar o `op == "sql"` do
/// `perfil_do_pedido` -> duas combinacoes; tirar o `sessao.ip.is_empty()`
/// -> o perfil da bia nasce.
#[test]
fn a_op_sql_entra_pelos_passos_e_o_caminho_sem_ip_fica_fora() {
    let dir = DirTemp::novo("p78-p7-sql");
    let s = servidor(&dir, |_| {});
    let mut ana = sessao("ana", "203.0.113.74");
    pede(
        &s,
        &mut ana,
        r#""op":"sql","database":"b","texto":"SELECT * FROM clientes""#,
    )
    .unwrap();
    {
        let m = s.perfis.lock().unwrap();
        assert_eq!(m.quantos(), 1);
        assert_eq!(m.combinacoes_de("ana"), 1, "so o passo, nao o sql");
    }
    let mut sem_ip = sessao("bia", "");
    let _ = s.despachar(
        &format!(r#"{{"token":"t",{LER_CLIENTES}}}"#),
        &mut sem_ip,
        "",
    );
    assert_eq!(s.perfis.lock().unwrap().quantos(), 1);
}

// ----------------------------------------------------------------- P8

/// **O comportamento velho:** `bloquear_por_codigo` nasce desligado, e 50
/// tautologias do mesmo IP nao bloqueiam ninguem -- a ocorrencia continua
/// nascendo (o observador do 495) e a sessao continua de pe. RED: o padrao
/// `bloquear_por_codigo: true` -> bloqueia na quinta.
#[test]
fn desligado_cinquenta_tautologias_nao_bloqueiam() {
    let dir = DirTemp::novo("p78-p8-desligado");
    let s = servidor(&dir, |_| {});
    assert!(!s.config.protecao.bloquear_por_codigo);
    let ip = "203.0.113.9";
    let mut ana = sessao("ana", ip);
    let corpo = sql_de(&tautologia_do_arsenal());
    for _ in 0..50 {
        pede(&s, &mut ana, &corpo).expect("a tautologia executa");
    }
    assert!(s.barrado(ip, crate::agora_ms()).is_none());
    assert!(s.lista_negra.lock().unwrap().lista().is_empty());
    assert!(ana.usuario.is_some(), "a sessao nao podia terminar");
    assert!(!ocorrencias_de(&s, Alarme::InjecaoSuspeita).is_empty());
}

/// **O aceite da P8:** ligado, 5 tautologias de 203.0.113.9 em 10 min ->
/// bloqueio, pelo mesmo `violacao_leve` (a quarta ainda nao); e a sessao de
/// quem as mandou termina. RED: tirar o `violacao_leve` do ramo da P8 no
/// `despachar_o_pedido` -> nao bloqueia.
#[test]
fn ligado_cinco_tautologias_do_mesmo_ip_bloqueiam() {
    let dir = DirTemp::novo("p78-p8-ligado");
    let s = servidor(&dir, |c| c.protecao.bloquear_por_codigo = true);
    let ip = "203.0.113.9";
    let mut ana = sessao("ana", ip);
    let corpo = sql_de(&tautologia_do_arsenal());
    for _ in 0..4 {
        pede(&s, &mut ana, &corpo).expect("a tautologia executa");
    }
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "bloqueou antes do limite"
    );
    assert!(ana.usuario.is_some());
    pede(&s, &mut ana, &corpo).unwrap();
    let b = s.barrado(ip, crate::agora_ms()).expect("a quinta bloqueia");
    assert!(b.motivo.contains("constante_sob_or"), "{}", b.motivo);
    assert!(
        ana.usuario.is_none(),
        "a sessao de quem atacou seguiu de pe"
    );
    assert!(ana.encerrada_pela_protecao);
    // O pedido seguinte da mesma conexao pede login.
    assert!(pede(&s, &mut ana, LER_CLIENTES).is_err());
}

/// **O que o 766 deixou para a P8:** o IP poupado pela guarda (aqui o
/// loopback) chega ao limite e NAO vai a lista -- mas a sessao de quem
/// mandou termina. RED: tirar o `sessao.encerrar_pela_protecao()` -> a
/// sessao segue autenticada.
#[test]
fn o_ip_poupado_nao_bloqueia_mas_a_sessao_termina() {
    let dir = DirTemp::novo("p78-p8-poupado");
    let s = servidor(&dir, |c| c.protecao.bloquear_por_codigo = true);
    let ip = "127.0.0.1";
    let mut ana = sessao("ana", ip);
    let mut bia = sessao("bia", ip);
    let corpo = sql_de(&tautologia_do_arsenal());
    for _ in 0..5 {
        pede(&s, &mut ana, &corpo).unwrap();
    }
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "o loopback se trancou"
    );
    assert!(ana.usuario.is_none(), "a sessao do IP poupado seguiu de pe");
    // Quem divide o IP nao perde nada.
    pede(&s, &mut bia, LER_CLIENTES).expect("a outra sessao do IP continua");
    assert!(bia.usuario.is_some());
}

/// Ligado com o observador do 495 DESLIGADO: a P8 abre a mesma
/// classificacao sozinha, e bloqueia do mesmo jeito. RED: a vez aberta so
/// pelo `observar_injecao_sql` -> nada se classifica e nada bloqueia.
#[test]
fn ligado_bloqueia_mesmo_com_o_observador_desligado() {
    let dir = DirTemp::novo("p78-p8-sem-observador");
    let s = servidor(&dir, |c| {
        c.protecao.bloquear_por_codigo = true;
        c.politica.observar_injecao_sql = false;
    });
    let ip = "203.0.113.13";
    let mut ana = sessao("ana", ip);
    let corpo = sql_de(&tautologia_do_arsenal());
    for _ in 0..5 {
        pede(&s, &mut ana, &corpo).unwrap();
    }
    assert!(s.barrado(ip, crate::agora_ms()).is_some());
}

/// A pergunta legitima nao conta: 20 leituras comuns com a P8 ligada nao
/// chegam perto do bloqueio. RED: contar sem olhar as classes (`sinais`
/// ignorado) -> bloqueia na quinta.
#[test]
fn ligado_a_pergunta_comum_nao_conta() {
    let dir = DirTemp::novo("p78-p8-comum");
    let s = servidor(&dir, |c| c.protecao.bloquear_por_codigo = true);
    let ip = "203.0.113.10";
    let mut ana = sessao("ana", ip);
    let corpo = sql_de("SELECT * FROM clientes WHERE id = 1");
    for _ in 0..20 {
        pede(&s, &mut ana, &corpo).unwrap();
    }
    assert!(s.barrado(ip, crate::agora_ms()).is_none());
    assert!(ana.usuario.is_some());
}

/// Uma politica so: com o `contar_injecao_sql` velho ligado junto, o
/// empilhado recusado conta UMA vez -- quatro nao bloqueiam. RED: tirar o
/// `!contado_pelo_codigo` do ramo do 215 -> conta duas vezes e bloqueia na
/// terceira.
#[test]
fn com_as_duas_ligadas_o_empilhado_conta_uma_vez() {
    let dir = DirTemp::novo("p78-p8-uma-vez");
    let s = servidor(&dir, |c| {
        c.protecao.bloquear_por_codigo = true;
        c.politica.contar_injecao_sql = true;
    });
    let ip = "203.0.113.11";
    let mut ana = sessao("ana", ip);
    let corpo = sql_de(&do_arsenal("segundo comando: DROP"));
    for _ in 0..4 {
        assert!(pede(&s, &mut ana, &corpo).is_err());
    }
    assert!(
        s.barrado(ip, crate::agora_ms()).is_none(),
        "contou duas vezes"
    );
    assert!(pede(&s, &mut ana, &corpo).is_err());
    assert!(
        s.barrado(ip, crate::agora_ms()).is_some(),
        "a quinta bloqueia"
    );
}

/// A porta HTTP guarda a sessao fora da copia do `despachar`: a sessao que a
/// protecao encerrou morre tambem no `http::Sessoes`. RED: o
/// `fim_da_sessao_web_pela_protecao` sem o `encerrar` -> o id continua
/// valendo.
#[test]
fn a_sessao_web_encerrada_pela_protecao_morre_no_navegador() {
    let dir = DirTemp::novo("p78-p8-web");
    let s = servidor(&dir, |_| {});
    let agora = crate::agora_ms();
    let dur = s.config.web.sessao_ms();
    let teto = s.config.web.sessao_teto_ms();
    let mut id = s.sessoes.lock().unwrap().nova("ana", dur, agora);
    let mut copia = sessao("ana", "203.0.113.12");
    // Sem a marca, nada muda.
    s.fim_da_sessao_web_pela_protecao(&copia, &mut id);
    assert!(s
        .sessoes
        .lock()
        .unwrap()
        .usar(&id, dur, teto, agora)
        .is_some());
    copia.encerrar_pela_protecao();
    let antigo = id.clone();
    s.fim_da_sessao_web_pela_protecao(&copia, &mut id);
    assert!(id.is_empty());
    assert!(s
        .sessoes
        .lock()
        .unwrap()
        .usar(&antigo, dur, teto, agora)
        .is_none());
}

fn sql_de(texto: &str) -> String {
    format!(
        r#""op":"sql","database":"b","texto":{}"#,
        Json::texto_de(texto).escrever()
    )
}

// ------------------------------------------------------- P15: a op `perfis`

/// O portao da op `perfis` (765, P15) e o UNICO, na regra do SERVIDOR: o
/// operador e recusado, o administrador de UMA base tambem -- mesmo pedindo
/// com `"database":"b"`, a base que ele administra --, e quem administra o
/// servidor le. RED: tirar `perfis` de `OPS_DO_SERVIDOR` deixa o dono da
/// base `b` ler o perfil de todo mundo (o furo do 756); tirar o ramo do
/// `da_operacao` deixa o operador ler.
#[test]
fn a_op_perfis_e_so_de_quem_administra_o_servidor() {
    let dir = DirTemp::novo("p78-p15-portao");
    let h = phxsql_core::senha::cifrar_com("p78-senha", 1);
    let cad = crate::Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{"usuarios":[
                {{"login":"ana","senha_hash":"{h}","nivel":"operador"}},
                {{"login":"donab","senha_hash":"{h}",
                  "bases":{{"b":{{"ler":true,"administrar":true}}}}}},
                {{"login":"chefe","senha_hash":"{h}","nivel":"admin"}}]}}"#
        ))
        .unwrap(),
    )
    .unwrap();
    let s = servidor(&dir, |c| c.cadastro = cad.clone());
    let de = |login: &str| Sessao {
        usuario: cad.por_login(login).cloned(),
        ip: "203.0.113.90".into(),
        ..Sessao::default()
    };
    for login in ["ana", "donab"] {
        for corpo in [r#""op":"perfis""#, r#""op":"perfis","database":"b""#] {
            match pede(&s, &mut de(login), corpo) {
                Err(e) => assert!(
                    matches!(e, PhxError::Autorizacao(_)),
                    "{login}: recusou por outro motivo: {e}"
                ),
                Ok(j) => panic!("{login} leu o perfil de todos: {}", j.escrever()),
            }
        }
    }
    let r = pede(&s, &mut de("chefe"), r#""op":"perfis""#).expect("o administrador le");
    assert!(
        r.campo("perfis").and_then(Json::lista).is_some(),
        "{}",
        r.escrever()
    );
}

/// O CONTEUDO da op `perfis` e so metadado: depois de um `inserir` com um
/// valor marcado, de uma varredura filtrada por ele e de um SQL com ele no
/// texto, o retrato nao traz o valor em lugar nenhum -- e cada perfil tem
/// EXATAMENTE os campos do arquivo mais o `maduro`. Traz, sim, a combinacao
/// (categoria, database, tabela) com a contagem, e o `usuario` filtra.
/// RED: acrescentar ao retrato qualquer campo novo (o texto do ultimo
/// pedido, por exemplo) derruba a conferencia das chaves.
#[test]
fn a_op_perfis_devolve_so_metadado() {
    let dir = DirTemp::novo("p78-p15-conteudo");
    let s = servidor(&dir, |_| {});
    let mut ana = sessao("ana", "203.0.113.91");
    pede(
        &s,
        &mut ana,
        r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":9,"nome":"SEGREDO-P15"}"#,
    )
    .expect("a insercao executa");
    pede(
        &s,
        &mut ana,
        r#""op":"varrer","database":"b","tabela":"clientes","onde":{"nome":"SEGREDO-P15"}"#,
    )
    .expect("a varredura executa");
    let _ = pede(
        &s,
        &mut ana,
        &sql_de("SELECT nome FROM clientes WHERE nome = 'SEGREDO-P15'"),
    );

    let mut chefe = sessao("chefe", "203.0.113.92");
    let r = pede(&s, &mut chefe, r#""op":"perfis","usuario":"ana""#).unwrap();
    let texto = r.escrever();
    assert!(
        !texto.contains("SEGREDO"),
        "valor de linha no perfil: {texto}"
    );
    assert!(!texto.contains("SELECT"), "texto de SQL no perfil: {texto}");

    let perfis = r.campo("perfis").and_then(Json::lista).unwrap();
    assert_eq!(perfis.len(), 1, "o filtro por usuario: {texto}");
    let um = &perfis[0];
    assert_eq!(um.texto_ou("usuario", ""), "ana");
    let Json::Objeto(pares) = um else {
        panic!("perfil nao e objeto: {texto}")
    };
    let mut chaves: Vec<&str> = pares.iter().map(|(k, _)| k.as_str()).collect();
    chaves.sort_unstable();
    assert_eq!(
        chaves,
        [
            "combinacoes",
            "coringa",
            "horas",
            "maduro",
            "n",
            "primeiro_ms",
            "usuario"
        ],
        "{texto}"
    );
    assert_eq!(um.campo("maduro"), Some(&Json::Bool(false)), "{texto}");
    let combinacoes = um.campo("combinacoes").and_then(Json::lista).unwrap();
    let escreveu = combinacoes.iter().any(|c| {
        c.lista().is_some_and(|c| {
            c.first().and_then(Json::texto) == Some("escrever")
                && c.get(1).and_then(Json::texto) == Some("b")
                && c.get(2).and_then(Json::texto) == Some("clientes")
                && c.get(3).and_then(Json::inteiro) == Some(1)
        })
    });
    assert!(escreveu, "a combinacao do inserir: {texto}");
    assert_eq!(
        r.campo("piso_de_pedidos").and_then(Json::inteiro),
        Some(crate::perfis::PISO_DE_PEDIDOS as i64)
    );

    // O filtro por um login sem perfil devolve a lista vazia, e nao erro.
    let r = pede(&s, &mut chefe, r#""op":"perfis","usuario":"ninguem""#).unwrap();
    assert_eq!(
        r.campo("perfis").and_then(Json::lista).map(<[Json]>::len),
        Some(0)
    );
}

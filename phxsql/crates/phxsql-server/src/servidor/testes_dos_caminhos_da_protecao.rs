//! Pedidos 765/767, fatias P3, P4 e P5: o plano largo com o numero, a DDL
//! acima do piso e os caminhos escondidos -- cada um pela MESMA camada
//! (`executar_e_contar_escrita_local` e `protecao_do_plano`).
//!
//! O RED de cada teste esta escrito acima dele. O par de sempre: a camada
//! LIGADA recusa com 4009 e NADA muda; a sessao liberada pela senha de
//! execucao executa; e onde o teste prova o caminho, a corrida com a camada
//! DESLIGADA (o interruptor de teste) executa -- a recusa vem dela.

use super::*;
use crate::Cadastro;

const IP: &str = "127.0.0.1";

fn p(t: &str) -> Json {
    Json::analisar(t).unwrap()
}

fn cadastro() -> Cadastro {
    Cadastro::de_json(&p(r#"{"usuarios":[{"login":"ana","id":9,"supervisor":true,
             "senha_hash":"pbkdf2-sha256$1000$00$00"}]}"#))
    .unwrap()
}

/// `ligada` = `protecao.ligada`; desligada so na corrida que prova que a
/// recusa vem da camada.
fn servidor(nome: &str, ligada: bool, linhas: u32) -> (Arc<Servidor>, DirTemp) {
    servidor_com(nome, ligada, linhas, cadastro())
}

/// Sem cadastro, o servidor inteiro entra pelo token -- e e assim que a
/// ponte MCP desta casa chega hoje (sem login por usuario, fase 1).
fn servidor_com(
    nome: &str,
    ligada: bool,
    linhas: u32,
    cadastro: Cadastro,
) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("caminhos-protecao-{nome}"));
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    };
    c.protecao.ligada = ligada;
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"c",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(20)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                           "primario":true},
                          {"nome":"porNome","colunas":["nome"]}]}"#),
        &dono,
    )
    .unwrap();
    for inicio in (1..=linhas).step_by(1_000) {
        let fim = (inicio + 1_000).min(linhas + 1);
        let lote: Vec<String> = (inicio..fim)
            .map(|i| format!(r#"{{"id":{i},"nome":"n{i}"}}"#))
            .collect();
        s.executar(
            "inserir_lote",
            &p(&format!(
                r#"{{"database":"b","tabela":"c","linhas":[{}]}}"#,
                lote.join(",")
            )),
            &dono,
        )
        .unwrap();
    }
    s.executar(
        "criar_visao",
        &p(r#"{"database":"b","nome":"v","sql":"SELECT id FROM c"}"#),
        &dono,
    )
    .unwrap();
    (s, dir)
}

fn ana() -> Sessao {
    Sessao {
        usuario: cadastro().por_login("ana").cloned(),
        ip: IP.into(),
        ..Sessao::default()
    }
}

fn liberada() -> Sessao {
    Sessao {
        execucao_liberada: Some(LiberacaoDeExecucao {
            login: "ana".into(),
            ip: IP.into(),
        }),
        ..ana()
    }
}

/// Com a atividade amarrada, como a conexao faz: a ocorrencia vai a camada
/// DESTE servidor, e nao a de outro teste em paralelo.
fn pede(s: &Servidor, sessao: &mut Sessao, corpo: &str) -> Result<Json> {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:caminhos", "dados", IP, 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("sql", "ana", "b", "c", agora);
    let linha = format!(r#"{{"token":"t",{corpo}}}"#);
    let r = s.despachar(&linha, sessao, IP).2;
    a.terminou_pedido("ana");
    r
}

fn sql(texto: &str) -> String {
    format!(r#""op":"sql","database":"b","texto":"{texto}""#)
}

fn recusou(r: &Result<Json>, onde: &str) -> String {
    match r {
        Err(e) => {
            assert_eq!(e.codigo(), 4009, "{onde}: recusou por outro motivo: {e}");
            e.to_string()
        }
        Ok(j) => panic!("{onde}: executou -- {}", j.escrever()),
    }
}

/// Quantas linhas tem `nome` igual a `valor`, contadas pelo dono.
fn quantas(s: &Servidor, valor: &str) -> i64 {
    let r = s
        .op_sql(
            &p(&format!(
                r#"{{"database":"b","texto":"SELECT COUNT(*) FROM c WHERE nome = '{valor}'"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    r.inteiro_ou("contagem", -1)
}

/// Alguma linha com `nome = valor` esta marcada como excluida? O DELETE do
/// SQL e SUAVE: a linha fica, e a prova de que ele andou e a marca (o
/// `COUNT` pelo indice conta a marcada tambem).
fn marcada(s: &Servidor, valor: &str) -> bool {
    s.op_sql(
        &p(&format!(
            r#"{{"database":"b","texto":"SELECT * FROM c WHERE nome = '{valor}'"}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .escrever()
    .contains(r#""softdeleted":true"#)
}

fn versao(s: &Servidor, rowid: u64) -> i64 {
    s.executar(
        "ler",
        &p(&format!(
            r#"{{"database":"b","tabela":"c","rowid":{rowid},"com_versao":true}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .inteiro_ou("versao", -1)
}

/// Os arquivos da tabela `c`, com o tamanho de cada um.
fn arquivos_de_c(d: &DirTemp) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(d.join("b"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().into_owned();
            nome.starts_with("c.")
                .then(|| (nome, e.metadata().map(|m| m.len()).unwrap_or(0)))
        })
        .collect();
    v.sort();
    v
}

// ------------------------------------------------------------------ P3

/// P3: `UPDATE ... WHERE id > 0` sobre 2.000 de 2.000 recusa ANTES da
/// primeira linha, e a recusa diz o numero do plano. Nada muda: nenhuma
/// linha com o valor novo e a versao da primeira e da ultima intacta (o
/// `rowstamp` nao serve de prova aqui -- ele nasce no `inserir` e nunca muda
/// no `atualizar`, `docs/FORMATO.md`). Liberada, as 2.000 mudam.
///
/// RED: sem o `protecao_do_plano` do `executar_dml_por_faixa`, o UPDATE da
/// sessao nao liberada grava as 2.000.
#[test]
fn o_update_largo_recusa_com_o_numero_e_nada_muda() {
    let (s, _d) = servidor("p3-update", true, 2_000);
    let antes = (versao(&s, 1), versao(&s, 2_000));
    let r = pede(&s, &mut ana(), &sql("UPDATE c SET nome = 'z' WHERE id > 0"));
    let msg = recusou(&r, "UPDATE largo");
    assert!(
        msg.contains("atualizar_por_faixa em b.c (alterar_em_massa, 2000 de 2000 linhas)"),
        "{msg}"
    );
    assert_eq!(quantas(&s, "z"), 0, "o UPDATE recusado gravou linhas");
    assert_eq!((versao(&s, 1), versao(&s, 2_000)), antes);

    let r = pede(
        &s,
        &mut liberada(),
        &sql("UPDATE c SET nome = 'z' WHERE id > 0"),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("afetadas", -1), 2_000, "{}", r.escrever());
    assert_eq!(quantas(&s, "z"), 2_000);
}

/// P3: abaixo do piso (999 de 2.000) nao e plano largo, e passa sem senha;
/// o DELETE largo diz o numero tambem.
#[test]
fn abaixo_do_piso_passa_e_o_delete_largo_diz_o_numero() {
    let (s, _d) = servidor("p3-piso", true, 2_000);
    let r = pede(
        &s,
        &mut ana(),
        &sql("UPDATE c SET nome = 'y' WHERE id < 1000"),
    );
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(quantas(&s, "y"), 999);
    let r = pede(&s, &mut ana(), &sql("DELETE FROM c WHERE id > 0"));
    let msg = recusou(&r, "DELETE largo");
    assert!(
        msg.contains("(apagar_em_massa, 2000 de 2000 linhas)"),
        "{msg}"
    );
    assert!(!marcada(&s, "y"), "o DELETE recusado apagou");
}

// ------------------------------------------------------------------ P4

/// P4: `excluir_tabela` de uma tabela de 1.000 linhas sem a sessao liberada:
/// TODOS os arquivos da tabela ficam, com o mesmo tamanho. Liberada, some.
/// E o `DROP TABLE` pelo SQL nao chega a camada porque a linguagem nao o
/// tem (`rotina.rs`: «DROP TABLE e a operacao excluir_tabela») -- recusa
/// tambem, e a tabela fica.
///
/// RED: sem o `protecao_do_pedido` no `executar_e_contar_escrita_local`, a
/// tabela some na primeira chamada.
#[test]
fn o_drop_de_tabela_grande_deixa_todos_os_arquivos() {
    let (s, d) = servidor("p4-drop", true, 1_000);
    let antes = arquivos_de_c(&d);
    assert!(antes.len() >= 5, "{antes:?}");
    let drop = r#""op":"excluir_tabela","database":"b","tabela":"c","confirmar":"c""#;
    let msg = recusou(&pede(&s, &mut ana(), drop), "excluir_tabela");
    assert!(
        msg.contains("excluir_tabela em b.c (destruir_estrutura)"),
        "{msg}"
    );
    assert_eq!(arquivos_de_c(&d), antes, "um arquivo da tabela mudou");

    let r = pede(&s, &mut ana(), &sql("DROP TABLE c"));
    assert!(r.is_err(), "DROP TABLE pelo SQL executou: {r:?}");
    assert_eq!(arquivos_de_c(&d), antes);

    pede(&s, &mut liberada(), drop).unwrap();
    assert!(!d.join("b").join("c.reg").exists(), "liberada e nao apagou");
}

/// P4: a REESCRITA acima do piso pede a senha com o tamanho; o
/// `ALTER TABLE ... ENCRYPT` pelo SQL chega a mesma camada (a diretiva da
/// cifra e pedido derivado); e tirar um indice de texto e DROP INDEX em
/// qualquer tamanho.
///
/// RED: com o `Classe::SeGrande` de `crate::protecao::classe` virando
/// `Classe::Livre`, as tres primeiras deixam de ser 4009.
#[test]
fn a_reescrita_acima_do_piso_pede_a_senha_com_o_tamanho() {
    let (s, _d) = servidor("p4-reescrita", true, 1_100);
    for (rotulo, corpo) in [
        (
            "migrar_esquema",
            r#""op":"migrar_esquema","database":"b","tabela":"c","confirmar":"c""#.to_string(),
        ),
        (
            "criptografar",
            r#""op":"criptografar","database":"b","tabela":"c""#.to_string(),
        ),
        ("ALTER TABLE ENCRYPT", sql("ALTER TABLE c ENCRYPT")),
    ] {
        let msg = recusou(&pede(&s, &mut ana(), &corpo), rotulo);
        assert!(
            msg.contains("(reescrever_tabela_grande, 1100 de 1100 linhas)"),
            "{rotulo}: {msg}"
        );
    }

    // O indice de texto: declarar e de graca; tirar e DROP INDEX.
    let (s, _d) = servidor("p4-indice", true, 3);
    let com = r#""op":"redeclarar_indices_texto","database":"b","tabela":"c","indices_texto":[{"nome":"porTexto","coluna":"nome"}]"#;
    pede(&s, &mut ana(), com).unwrap();
    let sem = r#""op":"redeclarar_indices_texto","database":"b","tabela":"c","indices_texto":[]"#;
    let msg = recusou(&pede(&s, &mut ana(), sem), "tirar indice de texto");
    assert!(msg.contains("(destruir_estrutura)"), "{msg}");
    pede(&s, &mut liberada(), sem).unwrap();
}

// ------------------------------------------------------------------ P5

/// A ponte MCP deste servidor, com a escrita liberada: a camada de protecao
/// e a prova destes testes, e a ponte de leitura recusaria antes dela (781).
fn mcp(s: &Arc<Servidor>, texto: &str) -> Json {
    mcp_da_ponte(s, texto, true)
}

/// A ponte MCP pelo caminho real (`ExecutorLocal` -> `despachar`). Sem
/// `escrita`, somente de leitura, como nasce.
fn mcp_da_ponte(s: &Arc<Servidor>, texto: &str, escrita: bool) -> Json {
    let ponte = crate::mcp::Ponte::nova(ExecutorLocal::novo(Arc::clone(s), "mcp:prova"))
        .com_escrita(escrita)
        .com_campo_fixo("token", Json::texto_de("t"));
    let pedido = Json::objeto(vec![
        ("jsonrpc", Json::texto_de("2.0")),
        ("id", Json::de_u64(1)),
        ("method", Json::texto_de("tools/call")),
        (
            "params",
            Json::objeto(vec![
                ("name", Json::texto_de("phx_sql")),
                (
                    "arguments",
                    Json::objeto(vec![
                        ("database", Json::texto_de("b")),
                        ("texto", Json::texto_de(texto)),
                    ]),
                ),
            ]),
        ),
    ]);
    Json::analisar(&ponte.atender(&pedido.escrever()).unwrap()).unwrap()
}

/// P5, MCP: a ferramenta `phx_sql` leva o `DROP VIEW` e o `DELETE` largo
/// ao `despachar`, e a camada recusa os dois. Sem a camada, os dois executam
/// -- pela ponte com escrita; a de leitura recusa antes (pedido 781, o teste
/// de baixo).
///
/// RED: a corrida com a camada desligada; e sem o `protecao_do_pedido` no
/// `executar_e_contar_escrita_local`, a visao some pela ponte.
#[test]
fn o_mcp_passa_pela_mesma_camada() {
    let (s, _d) = servidor_com("p5-mcp", true, 1_500, Cadastro::default());
    let r = mcp(&s, "DROP VIEW v").escrever();
    assert!(r.contains("erro 4009"), "{r}");
    let r = mcp(&s, "DELETE FROM c WHERE id > 0").escrever();
    assert!(r.contains("erro 4009"), "{r}");
    assert!(!marcada(&s, "n1"), "o DELETE pelo MCP apagou");
    assert!(s
        .op_sql(
            &p(r#"{"database":"b","texto":"SELECT id FROM v"}"#),
            &Sessao::default()
        )
        .is_ok());

    let (s, _d) = servidor_com("p5-mcp-desligada", false, 1_500, Cadastro::default());
    let r = mcp(&s, "DELETE FROM c WHERE id > 0").escrever();
    assert!(!r.contains("erro"), "{r}");
    assert!(
        r.contains(r#"\"afetadas\": 1500"#),
        "sem a camada o DELETE pelo MCP executa: {r}"
    );
}

/// P5, job: o job com comando perigoso NAO roda -- e a recusa e a 4009, a
/// mesma da rede (a senha propria do job e a P12, fora desta fatia).
#[test]
fn o_job_com_comando_perigoso_recusa_com_a_4009() {
    let (s, d) = servidor("p5-job", true, 3);
    let drop = r#""op":"excluir_tabela","database":"b","tabela":"c","confirmar":"c""#;
    pede(
        &s,
        &mut ana(),
        &format!(
            r#""op":"job_salvar","job":{{"nome":"limpa","usuario":"ana","cada_minutos":60,"pedido":{{{drop}}}}}"#
        ),
    )
    .unwrap();
    let r = pede(&s, &mut ana(), r#""op":"job_rodar","nome":"limpa""#).unwrap();
    assert!(!r.booleano_ou("ok", true), "{}", r.escrever());
    let esperado = crate::protecao::da_op_perigosa(
        "excluir_tabela",
        &p(&format!("{{{drop}}}")),
        crate::protecao::Categoria::DestruirEstrutura,
    )
    .para_a_sessao(false)
    .em_resultado()
    .unwrap_err();
    assert_eq!(esperado.codigo(), 4009);
    assert_eq!(r.texto_ou("detalhe", ""), esperado.to_string());
    assert!(d.join("b").join("c.reg").exists());
}

/// P5, rotina e gatilho AFTER: o corpo fala com o servidor SO pelo motor das
/// rotinas, e o motor entrega pelo `executar_derivado` -- a mesma camada. A
/// linguagem de hoje nao escreve DROP nem DELETE num corpo (as duas
/// primeiras asercoes travam isso: no dia em que escrever, este teste cai e
/// manda olhar), e a terceira prova que, quando escrever, a camada esta no
/// caminho: o `excluir_tabela` pelo motor recusa com 4009 e a tabela fica.
///
/// RED: com o `MotorDoServidor::operacao` chamando `executar` direto (o
/// atalho que a lei «funcao e comando vem do mesmo motor» proibe), a tabela
/// some pelo motor.
#[test]
fn rotina_e_gatilho_after_passam_pela_mesma_camada() {
    let (s, d) = servidor("p5-rotina", true, 3);
    for corpo in [
        "CREATE PROCEDURE limpa() BEGIN DROP VIEW v; END",
        "CREATE PROCEDURE limpa() BEGIN DELETE FROM c WHERE id > 0; END",
        "CREATE TRIGGER t AFTER INSERT ON c FOR EACH ROW BEGIN DELETE FROM c WHERE id > 0; END",
    ] {
        let r = pede(&s, &mut ana(), &sql(corpo));
        assert!(r.is_err(), "a linguagem passou a aceitar: {corpo}");
    }
    let drop = p(r#"{"database":"b","tabela":"c","confirmar":"c"}"#);
    let r = s.operacao_pelo_motor_de_rotina("b", "excluir_tabela", &drop, &ana());
    recusou(&r, "motor das rotinas");
    assert!(d.join("b").join("c.reg").exists());
    // O mesmo motor com a sessao liberada executa: a recusa e da camada, e
    // nao de o motor nao saber a op.
    s.operacao_pelo_motor_de_rotina("b", "excluir_tabela", &drop, &liberada())
        .unwrap();
    assert!(!d.join("b").join("c.reg").exists());
}

/// P5, replica: o `aplicar` que traz a exclusao de linha da origem nao
/// passa pela lista -- o comando ja passou pela camada LA. Uma sessao sem
/// liberacao nenhuma.
#[test]
fn o_aplicar_da_replica_nao_pede_a_senha() {
    let (s, _d) = servidor("p5-replica", true, 3);
    let evento =
        p(r#"{"database":"b","tabela":"c","eventos":[{"operacao":"exclusao","rowid":1}]}"#);
    assert_eq!(
        s.protecao_do_pedido("aplicar", &evento, &ana()).unwrap(),
        None
    );
}

/// A lei «funcao e comando vem do mesmo motor», com numero: as chamadas
/// diretas ao `executar` -- o que pula os tres irmaos e a camada -- nas
/// fontes do servidor. Nao e defeito cada uma (o `esquema` que a camada
/// mede, o direito por coluna DENTRO do irmao); e defeito a NOVA, que
/// ninguem conferiu. Catraca: so desce.
const TETO_EXECUTAR_DIRETO: usize = 24;

#[test]
fn nenhum_executar_direto_novo() {
    let n: usize = crate::servidor::FONTES_DO_SERVIDOR
        .iter()
        .filter(|(nome, _)| !nome.starts_with("servidor/testes_"))
        .map(|(_, fonte)| {
            // So o codigo de producao: o modulo de teste no fim de um
            // arquivo pode chamar o `executar` a vontade.
            let producao = fonte.split("#[cfg(test)]\nmod ").next().unwrap_or(fonte);
            producao.matches(".executar(").count()
        })
        .sum();
    assert!(
        n <= TETO_EXECUTAR_DIRETO,
        "{n} chamadas diretas ao executar (teto {TETO_EXECUTAR_DIRETO}): a nova pula a \
         camada de protecao e os portoes -- use o executar_derivado"
    );
    assert!(
        n >= TETO_EXECUTAR_DIRETO,
        "{n} < {TETO_EXECUTAR_DIRETO}: baixe a catraca no mesmo commit"
    );
}

/// O texto que a ponte devolve numa chamada, e se ela o marcou como erro.
fn texto_do_mcp(r: &Json) -> (String, bool) {
    let res = r.campo("result").expect("tools/call devolve result");
    let erro = res.booleano_ou("isError", false);
    let texto = res.campo("content").unwrap().lista().unwrap()[0]
        .texto_ou("text", "")
        .to_string();
    (texto, erro)
}

/// Pedido 781: a ponte MCP somente de leitura oferece o `phx_sql`, e ele
/// ESCREVIA -- medido, um DELETE apagou 1.500 linhas com a camada de
/// protecao desligada. Agora o `op_sql` recusa pelo analisador de cada ramo
/// tudo o que nao so le, ANTES do trabalho: o DELETE numa tabela que nem
/// existe volta com a recusa da ponte, e nao com «tabela nao existe».
///
/// Camada DESLIGADA de proposito: a recusa tem de vir da ponte, nao dela.
///
/// RED: com `CAMPO_SO_LEITURA` sem ser carimbado em `Ponte::tools_call` (ou
/// com o `so_le` de `sintaxe::Comando` devolvendo `true`), o DELETE volta com
/// `"afetadas": 1500` e a primeira asercao cai.
#[test]
fn a_ponte_de_leitura_recusa_o_sql_que_escreve_antes_do_trabalho() {
    let (s, _d) = servidor_com("781-mcp-leitura", false, 1_500, Cadastro::default());
    for texto in [
        "DELETE FROM c WHERE id > 0",
        "DELETE FROM nao_existe WHERE id = 1",
        "UPDATE c SET nome = 'x' WHERE id = 1",
        "INSERT INTO c (id, nome) VALUES (9999, 'novo')",
        "DROP VIEW v",
        "CREATE VIEW w AS SELECT id FROM c",
        "CREATE PROCEDURE poe() BEGIN INSERT INTO c (id, nome) VALUES (9998, 'x'); END",
        "CALL poe()",
        "BEGIN",
        "CREATE USER bia PASSWORD 'segredo123'",
        "ALTER TABLE c SET dicas.observacao = 'x'",
        "ALTER TABLE c ENCRYPT",
    ] {
        let (msg, erro) = texto_do_mcp(&mcp_da_ponte(&s, texto, false));
        assert!(erro, "{texto}: executou pela ponte de leitura -- {msg}");
        // A frase da `erro.mcp_so_leitura`, e nao «somente de leitura», que
        // outros caminhos do servidor tambem dizem.
        assert!(
            msg.contains("este comando SQL escreve"),
            "{texto}: recusou por outro motivo -- {msg}"
        );
    }
    assert!(!marcada(&s, "n1"), "o DELETE pela ponte de leitura apagou");
    assert_eq!(quantas(&s, "n1"), 1);
    assert_eq!(
        quantas(&s, "novo"),
        0,
        "o INSERT pela ponte de leitura gravou"
    );
    assert_eq!(quantas(&s, "x"), 0, "o UPDATE pela ponte de leitura gravou");
}

/// O comportamento VELHO, que o 781 nao pode tirar: pela ponte de leitura o
/// SELECT simples, o composto, a uniao, a visao e o SHOW continuam saindo.
/// E a ponte com escrita continua escrevendo pelo `phx_sql` -- a recusa e
/// da ponte de leitura, nao do `phx_sql`.
///
/// RED: com o `so_le` de `sintaxe::Comando` devolvendo `false` para
/// `Selecao`, o primeiro SELECT cai.
#[test]
fn a_ponte_de_leitura_continua_lendo_pelo_phx_sql() {
    let (s, _d) = servidor_com("781-mcp-le", false, 30, Cadastro::default());
    for texto in [
        "SELECT id, nome FROM c WHERE id = 7",
        "SELECT COUNT(*) FROM c",
        "SELECT id FROM c WHERE id IN (SELECT id FROM c WHERE id = 3)",
        "SELECT * FROM c UNION SELECT * FROM c",
        "SELECT id FROM v",
        "SHOW TRIGGERS",
        "SHOW TABLE c SETTINGS",
    ] {
        let (msg, erro) = texto_do_mcp(&mcp_da_ponte(&s, texto, false));
        assert!(!erro, "{texto}: a leitura parou -- {msg}");
    }
    let (msg, _) = texto_do_mcp(&mcp_da_ponte(&s, "SELECT nome FROM c WHERE id = 7", false));
    assert!(msg.contains("n7"), "{msg}");

    let (msg, erro) = texto_do_mcp(&mcp_da_ponte(&s, "DELETE FROM c WHERE id = 7", true));
    assert!(!erro, "a ponte com escrita parou de escrever: {msg}");
    assert!(marcada(&s, "n7"));
}

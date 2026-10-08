//! O agregado sobre a COMPOSICAO: `por`, `agregados` e `tendo` no `consultar`
//! -- a porta do `GROUP BY` sobre junção (pedido 394).
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("agregado-composto-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `clientes` (id, cidade, uf, salario) e `pedidos` (id, cliente_id,
/// total): tres clientes em duas UFs, quatro pedidos.
fn servidor(d: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        cadastro,
        max_linhas: 10_000,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"clientes","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"cidade","tipo":"Str(20)"},
                    {"nome":"uf","tipo":"Str(2)"},
                    {"nome":"salario","tipo":"Decimal(12,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                             "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"pedidos","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"cliente_id","tipo":"Int4"},
                    {"nome":"total","tipo":"Decimal(12,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                             "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for (id, cidade, uf, salario) in [
        (1, "Blumenau", "SC", "1000.00"),
        (2, "Itajai", "SC", "2000.00"),
        (3, "Curitiba", "PR", "3000.00"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"loja","tabela":"clientes","linha":{{"id":{id},
                        "cidade":"{cidade}","uf":"{uf}","salario":"{salario}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    // Os centavos sao de proposito: 10,00 + 0,01 tem de fechar em 10,01.
    for (id, cli, total) in [
        (1, 1, "10.00"),
        (2, 1, "0.01"),
        (3, 2, "5.00"),
        (4, 3, "99.00"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"loja","tabela":"pedidos","linha":{{"id":{id},
                        "cliente_id":{cli},"total":"{total}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    s
}

fn consultar(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "consultar",
        &pedido(&format!(r#"{{"database":"loja",{corpo}}}"#)),
        &Sessao::default(),
    )
}

/// O `de` e a junção, escritos uma vez: quase todo teste daqui parte deles.
const JUNCAO: &str = r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
        "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
                   "tipo":"interno",
                   "em":[{"esquerda":"p.cliente_id","direita":"c.id"}]}]"#;

const CONTA: &str = r#""agregados":[{"funcao":"contagem","apelido":"n"}]"#;

fn linhas(r: &Json) -> &[Json] {
    r.campo("linhas").and_then(Json::lista).unwrap()
}

/// **O CAMINHO FELIZ: agregado sobre junção.**
///
/// PROVA REAL, com o defeito reposto: tirando o portao
/// `if p.campo("por").is_some() || p.campo("agregados").is_some()` de
/// `op_consultar` -- que e o mesmo que ignorar os tres campos novos --,
/// a resposta volta a ser a linha juntada crua e este teste REPROVA logo
/// no primeiro numero (3 linhas de pedido, e nao 2 grupos de cidade).
#[test]
fn o_agregado_sobre_a_juncao_responde_por_grupo() {
    let d = dir("feliz");
    let s = servidor(&d, Cadastro::default());

    let r = consultar(
        &s,
        &format!(
            r#"{JUNCAO},"expressao":"c.uf = 'SC'","por":["c.cidade"],
                   "agregados":[{{"funcao":"contagem","apelido":"n"}},
                                {{"funcao":"soma","coluna":"p.total",
                                  "apelido":"faturado"}}],
                   "ordem":[{{"coluna":"faturado","desc":true}}]"#
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{}", r.escrever());
    let l = linhas(&r);
    assert_eq!(l[0].texto_ou("c.cidade", ""), "Blumenau", "{l:?}");
    assert_eq!(l[0].inteiro_ou("n", -1), 2, "{l:?}");
    // 10,00 + 0,01 = 10,01 EXATO. O `Decimal` viaja como TEXTO no JSON e
    // volta pelo TIPO do modelo (`valor_tipado`, o mesmo conversor da
    // `expressao`); somando pelo FORMATO do JSON ele seria texto, o
    // acumulador nao o somaria e a resposta sairia NULA.
    assert_eq!(l[0].texto_ou("faturado", ""), "10.01", "{l:?}");
    assert_eq!(l[1].texto_ou("c.cidade", ""), "Itajai", "{l:?}");
    assert_eq!(l[1].texto_ou("faturado", ""), "5.00", "{l:?}");

    // O modelo da resposta traz o tipo do AGREGADO, e nao o da coluna --
    // do mesmo `tipo_do_agregado` que o `agrupar` usa, um lugar e nao dois.
    let colunas = r.campo("colunas").and_then(Json::lista).unwrap();
    assert_eq!(colunas.len(), 3, "{colunas:?}");
    assert_eq!(colunas[0].texto_ou("nome", ""), "c.cidade");
    assert_eq!(colunas[1].texto_ou("tipo", ""), "UInt8", "{colunas:?}");
    assert!(
        colunas[2].texto_ou("tipo", "").contains("Decimal"),
        "{colunas:?}"
    );

    // O `tendo` peneira o que ja esta agregado, e a projecao vem por
    // ultimo -- com os apelidos, na ordem pedida.
    let r = consultar(
        &s,
        &format!(
            r#"{JUNCAO},"expressao":"c.uf = 'SC'","por":["c.cidade"],
                   "agregados":[{{"funcao":"contagem","apelido":"n"}},
                                {{"funcao":"soma","coluna":"p.total",
                                  "apelido":"faturado"}}],
                   "tendo":"n > 1","colunas":["c.cidade","n","faturado"]"#
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("c.cidade", ""), "Blumenau");
    assert_eq!(linhas(&r)[0].texto_ou("faturado", ""), "10.01");
    let _ = std::fs::remove_dir_all(&d);
}

/// **O TESTE QUE MAIS IMPORTA: sem os tres campos, o `consultar` responde
/// o que respondia.**
///
/// Guarda nova entra PEDIDA, nao imposta -- e o teste que trava isso e o
/// do comportamento VELHO, que tem de passar nos dois estados do codigo:
/// com a agregacao e sem ela.
#[test]
fn sem_os_tres_campos_o_consultar_responde_o_que_respondia() {
    let d = dir("velho");
    let s = servidor(&d, Cadastro::default());

    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"expressao":"c.uf = 'SC'","ordem":[{{"coluna":"p.id"}}]"#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 3, "{}", r.escrever());
    let l = linhas(&r);
    assert_eq!(l[0].texto_ou("c.cidade", ""), "Blumenau", "{l:?}");
    assert_eq!(l[0].texto_ou("p.total", ""), "10.00", "{l:?}");
    // A linha sai INTEIRA, dos dois lados, como sempre saiu.
    assert!(l[0].campo("c.uf").is_some(), "{l:?}");

    // E o caminho sem junção nenhuma, que e o mais usado de todos.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},"ordem":[{"coluna":"id"}],
               "colunas":["cidade","uf"]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 3, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(linhas(&r)[2].texto_ou("uf", ""), "PR");
    let _ = std::fs::remove_dir_all(&d);
}

/// **O `tendo` que cita coluna CRUA recusa nomeando.**
///
/// `p.total > 10` num grupo de mil linhas nao quer dizer nada: o `tendo`
/// fala da linha AGREGADA. E a recusa lista o que ele PODE ver, senao
/// quem escreveu tem de adivinhar.
///
/// PROVA REAL: trocando a conferencia de `tendo_do_pedido` por um `Ok`
/// direto, o pedido passa e a expressao estoura depois -- ou, pior, o
/// grupo cai calado.
#[test]
fn o_tendo_que_cita_coluna_crua_recusa_nomeando() {
    let d = dir("tendo");
    let s = servidor(&d, Cadastro::default());
    let e = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.cidade"],{CONTA},"tendo":"p.total > 10""#),
    )
    .expect_err("o tendo sobre a coluna crua passou");
    let t = e.to_string();
    assert!(t.contains("p.total") && t.contains("tendo"), "{t}");
    assert!(t.contains("c.cidade, n"), "nao lista o que ha: {t}");

    // E o `tendo` que cita o que existe continua filtrando.
    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.cidade"],{CONTA},"tendo":"n > 1""#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    let _ = std::fs::remove_dir_all(&d);
}

/// **Agregar a coluna NEGADA recusa nomeando -- sem peneira nova.**
///
/// O ganho de graca da via escolhida: `modelo_da_tabela` monta o modelo do
/// sub-pedido pulando `colunas_sem_leitura`, entao a coluna negada NAO
/// EXISTE no modelo e o agregado sobre ela cai em `resolver_ou_recusar`.
/// Onde o `agrupar` recusa a operacao inteira para quem tem regra de
/// coluna, esta porta deixa agregar o que ele PODE ler.
///
/// PROVA REAL: tirando o `resolver_ou_recusar` do agregado -- isto e,
/// usando o nome como veio em vez de resolve-lo contra o modelo --, a
/// resposta vira `{"uf":"SC","x":null}` com `ok: true`: resposta errada
/// CALADA, que e pior que a recusa. Medido nesta rodada.
///
/// E o diagnostico que eu errei primeiro, registrado porque a lei da casa
/// manda: eu escrevi que bastaria tirar o `negadas.iter().any(...)` de
/// `modelo_da_tabela` para repor o defeito. **Medido, nao repoe** -- o
/// teste continua passando, porque a coluna negada e tirada em DOIS
/// lugares independentes: a peneira ja a tirou da LINHA, e
/// `linhas_do_sub_pedido` monta o modelo pela primeira linha quando ha
/// linha. Um diagnostico plausivel nao e um diagnostico medido.
#[test]
fn o_agregado_sobre_coluna_negada_recusa_nomeando() {
    let d = dir("coluna-negada");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"clientes":{"ler":true,
                   "colunas":{"salario":{"ler":false}}}}}}}]}"#,
    ))
    .unwrap();
    let s = servidor(&d, cadastro.clone());
    let mut ses = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let corpo = |ag: &str| {
        format!(
            r#"{{"token":"t","op":"consultar","database":"loja",
                     "de":{{"op":"varrer","tabela":"clientes"}},
                     "por":["uf"],"agregados":[{ag}]}}"#
        )
    };

    // O controle: agregar o que ela PODE ler passa, e conta certo.
    let (_, _, ok) = s.despachar(
        &corpo(r#"{"funcao":"contagem","apelido":"n"}"#),
        &mut ses,
        "127.0.0.1",
    );
    let ok = ok.expect("a contagem tinha de passar");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 2, "{}", ok.escrever());

    // E a NEGADA recusa NOMEANDO, sem devolver valor nenhum dela.
    let (_, _, negado) = s.despachar(
        &corpo(r#"{"funcao":"soma","coluna":"salario","apelido":"x"}"#),
        &mut ses,
        "127.0.0.1",
    );
    let e = negado.expect_err("agregou a coluna negada");
    let t = e.to_string();
    assert!(t.contains("salario") && t.contains("soma"), "{t}");
    assert!(!t.contains("3000"), "o valor da coluna negada vazou: {t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A ordem dos passos: `expressao` ANTES de agrupar, `tendo` DEPOIS.**
///
/// E a ordem dos quatro motores maduros, e ela se prova pelos dois lados:
/// a `expressao` nao enxerga o apelido do agregado (ele ainda nao existe)
/// e o `tendo` nao enxerga a coluna crua (ela ja nao existe).
///
/// PROVA REAL: movendo o bloco da agregacao para ANTES da `expressao`, a
/// terceira parte deste teste passa a devolver 2 grupos em vez de 1 -- e a
/// quarta deixa de recusar.
#[test]
fn a_expressao_filtra_antes_de_agrupar_e_o_tendo_depois() {
    let d = dir("ordem");
    let s = servidor(&d, Cadastro::default());

    // Sem filtro nenhum: duas UFs, 1 pedido no PR e 3 em SC.
    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.uf"],{CONTA},"ordem":[{{"coluna":"c.uf"}}]"#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("c.uf", ""), "PR");
    assert_eq!(linhas(&r)[0].inteiro_ou("n", -1), 1);
    assert_eq!(linhas(&r)[1].inteiro_ou("n", -1), 3);

    // A `expressao` filtra a linha CRUA: o grupo do PR nem chega a nascer.
    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"expressao":"c.uf = 'PR'","por":["c.uf"],{CONTA}"#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("c.uf", ""), "PR");

    // O `tendo` filtra DEPOIS: o grupo do PR nasce e cai por ter n = 1.
    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.uf"],{CONTA},"tendo":"n > 2""#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("c.uf", ""), "SC");

    // E o outro lado da mesma ordem: a `expressao` NAO ve o apelido do
    // agregado, porque roda antes de ele existir.
    let e = consultar(
        &s,
        &format!(r#"{JUNCAO},"expressao":"n > 2","por":["c.uf"],{CONTA}"#),
    )
    .expect_err("a expressao viu o agregado antes de ele existir");
    assert!(e.to_string().contains("\"n\""), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A coluna NAO agregada recusa na projecao -- o motor ERRA, como o
/// PostgreSQL e o MySQL.**
///
/// Decisao ja tomada por media ponderada nesta casa (PG 4 + MySQL 2 = 6
/// contra MariaDB 3 + SQLite 1 = 4). Ela sai de graca da substituicao do
/// modelo: depois de agrupar so existem `por` e os apelidos, entao pedir
/// `p.id` cai no `resolver_ou_recusar` e recusa nomeando o que ha.
#[test]
fn a_coluna_fora_do_por_recusa_na_projecao() {
    let d = dir("nao-agregada");
    let s = servidor(&d, Cadastro::default());
    let e = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.uf"],{CONTA},"colunas":["c.uf","p.id","n"]"#),
    )
    .expect_err("a coluna nao agregada saiu na projecao");
    let t = e.to_string();
    assert!(t.contains("p.id") && t.contains("projecao"), "{t}");
    assert!(t.contains("c.uf, n"), "nao lista o que ha: {t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`por` vazio com `agregados` e UMA linha: o agregado global.**
///
/// E o `SELECT COUNT(*) FROM a JOIN b`. Sobre resultado vazio nao nasce
/// grupo nenhum -- somar nada nao da zero, da resposta nenhuma --, e e a
/// mesma regra do `agrupar`, do mesmo `crate::agrupar`.
#[test]
fn por_vazio_com_agregados_e_o_agregado_global() {
    let d = dir("global");
    let s = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        &format!(
            r#"{JUNCAO},"por":[],
                   "agregados":[{{"funcao":"contagem","apelido":"n"}},
                                {{"funcao":"soma","coluna":"p.total",
                                  "apelido":"faturado"}}]"#
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].inteiro_ou("n", -1), 4);
    assert_eq!(linhas(&r)[0].texto_ou("faturado", ""), "114.01");

    // Composicao vazia: nenhum grupo, e o modelo continua dizendo a forma.
    let r = consultar(
        &s,
        &format!(r#"{JUNCAO},"expressao":"c.uf = 'RS'","por":[],{CONTA}"#),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 0, "{}", r.escrever());
    let colunas = r.campo("colunas").and_then(Json::lista).unwrap();
    assert_eq!(colunas[0].texto_ou("nome", ""), "n", "{colunas:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **O apelido padrao, o apelido repetido e a `coluna` que falta.**
///
/// Tres recusas e um padrao, todos do mesmo contrato do `agrupar`. O
/// apelido padrao usa o ULTIMO segmento do nome (`p.total` -> `soma_total`)
/// porque `soma_p.total` teria um ponto, e o resolvedor le ponto como
/// qualificacao: `total` passaria a ser ambiguo entre a coluna e o
/// agregado dela.
#[test]
fn o_apelido_padrao_nao_carrega_o_ponto_do_nome_qualificado() {
    let d = dir("apelido");
    let s = servidor(&d, Cadastro::default());

    let r = consultar(
        &s,
        &format!(
            r#"{JUNCAO},"por":["c.uf"],
                   "agregados":[{{"funcao":"soma","coluna":"p.total"}}],
                   "ordem":[{{"coluna":"c.uf"}}]"#
        ),
    )
    .unwrap();
    assert!(
        linhas(&r)[0].campo("soma_total").is_some(),
        "{}",
        r.escrever()
    );
    assert_eq!(linhas(&r)[0].texto_ou("soma_total", ""), "99.00");

    // Dois agregados com o mesmo apelido RECUSAM: um deles ficaria
    // invisivel na resposta, que e numero errado calado.
    let e = consultar(
        &s,
        &format!(
            r#"{JUNCAO},"por":["c.uf"],
                   "agregados":[{{"funcao":"soma","coluna":"p.total"}},
                                {{"funcao":"media","coluna":"p.total",
                                  "apelido":"soma_total"}}]"#
        ),
    )
    .expect_err("dois apelidos iguais passaram");
    assert!(e.to_string().contains("soma_total"), "{e}");

    // E o agregado que precisa de valor sem `coluna` recusa dizendo por que.
    let e = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.uf"],"agregados":[{{"funcao":"soma"}}]"#),
    )
    .expect_err("a soma sem coluna passou");
    assert!(e.to_string().contains("contagem"), "{e}");

    // Coluna que nao existe no resultado composto recusa NOMEANDO.
    let e = consultar(
        &s,
        &format!(r#"{JUNCAO},"por":["c.uf"],"agregados":[{{"funcao":"soma","coluna":"zzz"}}]"#),
    )
    .expect_err("o agregado sobre coluna inexistente passou");
    assert!(e.to_string().contains("zzz"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Sem junção, o apelido de fora se descarta -- e `por` segue a mesma
/// regra da `ordem` e da projecao (pedido 240).**
///
/// Quem escreve SQL escreve `GROUP BY c.uf` mesmo quando ha uma tabela so,
/// e ali a linha nao foi prefixada. Fazer o `por` divergir da `ordem`
/// neste ponto seria a mesma consulta funcionando num campo e recusando no
/// irmao.
#[test]
fn sem_juncao_o_por_aceita_o_apelido_de_fora() {
    let d = dir("apelido-de-fora");
    let s = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
               "por":["c.uf"],
               "agregados":[{"funcao":"soma","coluna":"c.salario","apelido":"folha"}],
               "ordem":[{"coluna":"c.uf"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("uf", ""), "PR", "{}", r.escrever());
    assert_eq!(linhas(&r)[1].texto_ou("folha", ""), "3000.00");
    let _ = std::fs::remove_dir_all(&d);
}

/// **O IRMAO do SQL de relatorio: agregar sobre uma VISAO.**
///
/// Outra porta, o MESMO pedido `consultar` -- montado por
/// `phxsql_sql::planejar_sobre`, que e um segundo construtor daquele
/// pedido. Ate 23/09/2026 ele recusava `COUNT(*)`/`GROUP BY` e mandava
/// «componha por fora», saida que nao existia. Se um dia so uma das duas
/// portas souber agregar, e este teste que cai.
#[test]
fn agregar_sobre_uma_visao_tambem_atravessa() {
    let d = dir("visao");
    let s = servidor(&d, Cadastro::default());
    s.executar(
        "criar_visao",
        &pedido(
            r#"{"database":"loja","nome":"v_sc",
                    "sql":"SELECT id, cidade, uf FROM clientes"}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    let r = s
        .executar(
            "sql",
            &pedido(r#"{"database":"loja","texto":"SELECT COUNT(*) FROM v_sc"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        linhas(&r)[0].inteiro_ou("contagem", -1),
        3,
        "{}",
        r.escrever()
    );

    let r = s
        .executar(
            "sql",
            &pedido(
                r#"{"database":"loja","texto":"SELECT uf, COUNT(*) AS n FROM v_sc GROUP BY uf HAVING n > 1"}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].texto_ou("uf", ""), "SC");
    assert_eq!(linhas(&r)[0].inteiro_ou("n", -1), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// **AS DUAS METADES SE ENCONTRAM: o SQL inteiro, do texto ao numero.**
///
/// O tradutor (`phxsql-sql`) e o motor (aqui) foram escritos no mesmo
/// passo, e cada um tem a sua prova. Esta e a unica que falha quando os
/// dois estao certos e o CONTRATO entre eles esta errado -- um nome de
/// campo diferente, a ordem dos passos trocada, o apelido que uma metade
/// emite e a outra nao resolve. E a prova que so existe no encontro.
///
/// PROVA REAL: com o `por` fora do pedido emitido pelo tradutor, a
/// resposta vem com as linhas cruas da junção e o `unwrap` da projecao
/// recusa; com o agregado emitido e o motor sem o bloco de agregacao,
/// recusa na projecao por `n` nao existir.
#[test]
fn o_sql_de_relatorio_atravessa_as_duas_metades() {
    let d = dir("sql-inteiro");
    let s = servidor(&d, Cadastro::default());
    let r = s
        .executar(
            "sql",
            // O texto do SQL numa linha so: JSON nao aceita quebra crua
            // dentro de uma string.
            &pedido(
                r#"{"database":"loja","texto":"SELECT c.cidade, COUNT(*) AS n, SUM(p.total) AS faturado FROM pedidos p JOIN clientes c ON p.cliente_id = c.id WHERE c.uf = 'SC' GROUP BY c.cidade HAVING n > 1 ORDER BY faturado DESC LIMIT 100"}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{}", r.escrever());
    let l = linhas(&r);
    assert_eq!(l[0].texto_ou("c.cidade", ""), "Blumenau", "{l:?}");
    assert_eq!(l[0].inteiro_ou("n", -1), 2, "{l:?}");
    assert_eq!(l[0].texto_ou("faturado", ""), "10.01", "{l:?}");

    // `SELECT COUNT(*)` sobre junção: o agregado GLOBAL, sem GROUP BY.
    let r = s
        .executar(
            "sql",
            &pedido(
                r#"{"database":"loja","texto":"SELECT COUNT(*) FROM pedidos p JOIN clientes c ON p.cliente_id = c.id"}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        linhas(&r)[0].inteiro_ou("contagem", -1),
        4,
        "{}",
        r.escrever()
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **As DUAS portas respondem numeros diferentes, e e por isso que o
/// `agrupar` nao virou acucar desta aqui.**
///
/// O `agrupar` flui do disco e ve a tabela INTEIRA; a composicao agrega o
/// que ela materializou, e o que ela materializa tem o teto de
/// `recursos.max_linhas` -- que nasce 1.000. Com o teto em 2 e quatro
/// pedidos, a mesma pergunta responde **4** por uma porta e **2** pela
/// outra. Transformar o `agrupar` em acucar do `consultar` trocaria o 4
/// pelo 2 calado, e e a medida que matou aquela proposta (parecer do papel
/// J, 23/09/2026). O contrato diz qual porta responde o que -- e este
/// teste e o que trava a diferenca em vez de deixa-la virar surpresa.
///
/// O teto de GRUPOS do `crate::agrupar` continua valendo aqui, e nunca
/// dispara: grupo nenhum passa do numero de linhas, e as linhas ja pararam
/// no teto da composicao. Quem precisa do agregado da tabela inteira usa o
/// `agrupar`.
#[test]
fn o_agregado_da_composicao_conta_o_que_a_composicao_viu() {
    let d = dir("duas-portas");
    let s = servidor(&d, Cadastro::default());
    drop(s);
    // O MESMO diretorio, com o teto em 2.
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        max_linhas: 2,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();

    let pela_composicao = s
        .executar(
            "consultar",
            &pedido(&format!(
                r#"{{"database":"loja","de":{{"op":"varrer","tabela":"pedidos"}},
                        "apelido":"p","por":[],{CONTA}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        linhas(&pela_composicao)[0].inteiro_ou("n", -1),
        2,
        "{}",
        pela_composicao.escrever()
    );

    let pelo_agrupar = s
        .executar(
            "agrupar",
            &pedido(
                r#"{"database":"loja","tabela":"pedidos","por":[],
                        "agregados":[{"funcao":"contagem","apelido":"n"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        linhas(&pelo_agrupar)[0].inteiro_ou("n", -1),
        4,
        "{}",
        pelo_agrupar.escrever()
    );
    let _ = std::fs::remove_dir_all(&d);
}

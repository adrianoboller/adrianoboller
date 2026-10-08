//! O `inserir` com `se_existir` -- o upsert pelo protocolo.
use super::*;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("upsert-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(d: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let ses = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    // Uma tabela com DOIS indices unicos e nenhum primario: a ambigua.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"pessoas","colunas":[
                    {"nome":"cpf","tipo":"Str(14)"},
                    {"nome":"email","tipo":"Str(40)"},
                    {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porCpf","colunas":["cpf"],"unico":true},
                            {"nome":"porEmail","colunas":["email"],"unico":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s
}

fn inserir(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "inserir",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"clientes",{corpo}}}"#
        )),
        &Sessao::default(),
    )
}

fn nome_gravado(s: &Arc<Servidor>, rowid: u64) -> String {
    s.executar(
        "ler",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"clientes","rowid":{rowid}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .texto_ou("nome", "")
    .to_string()
}

fn registros(s: &Arc<Servidor>) -> i64 {
    s.executar(
        "varrer",
        &pedido(r#"{"database":"b","tabela":"clientes"}"#),
        &Sessao::default(),
    )
    .unwrap()
    .inteiro_ou("visiveis", -1)
}

/// **O teste que mais importa, e e o do comportamento VELHO.**
///
/// Sem `se_existir`, a chave repetida continua RECUSANDO. Guarda nova
/// entra pedida: um cliente escrito antes disto nao pode passar a
/// sobrescrever linha nenhuma por causa de um campo que ele nao mandou.
#[test]
fn sem_se_existir_a_chave_repetida_continua_recusando() {
    let d = dir("velho");
    let s = servidor(&d);
    inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();
    let e = inserir(&s, r#""linha":{"id":1,"nome":"outra"}"#).expect_err("a chave repetida passou");
    assert_eq!(e.nome(), "DUPLICADO", "{e}");
    assert_eq!(nome_gravado(&s, 1), "ana", "a linha foi sobrescrita");
    assert_eq!(registros(&s), 1);
    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL do `ignorar`: nada muda no disco, e o rowid que volta e
/// o de QUEM JA ESTAVA LA.**
///
/// A prova mede o dado gravado e a contagem, e nao o veredito: um
/// `ignorar` que inserisse uma segunda linha tambem responderia `ok`.
#[test]
fn ignorar_devolve_o_rowid_de_quem_ja_estava_la() {
    let d = dir("ignorar");
    let s = servidor(&d);
    let r = inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();
    assert_eq!(r.inteiro_ou("rowid", -1), 1);
    assert!(r.campo("ignorada").is_none(), "a primeira nao foi ignorada");

    let r = inserir(
        &s,
        r#""linha":{"id":1,"nome":"outra"},"se_existir":"ignorar""#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("rowid", -1), 1, "devolveu outro rowid");
    assert!(
        r.booleano_ou("ignorada", false),
        "a resposta nao diz que ignorou"
    );
    assert_eq!(nome_gravado(&s, 1), "ana", "ignorar gravou por cima");
    assert_eq!(registros(&s), 1, "ignorar inseriu uma segunda linha");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL do `atualizar`: a linha muda e NAO nasce outra.**
#[test]
fn atualizar_grava_por_cima_sem_criar_linha() {
    let d = dir("atualizar");
    let s = servidor(&d);
    inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();
    let r = inserir(
        &s,
        r#""linha":{"id":1,"nome":"nova"},"se_existir":"atualizar""#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("rowid", -1), 1);
    assert!(
        r.booleano_ou("atualizada", false),
        "a resposta nao diz que atualizou"
    );
    assert_eq!(nome_gravado(&s, 1), "nova", "nao gravou por cima");
    assert_eq!(registros(&s), 1, "criou uma segunda linha");

    // E a chave que NAO existe entra como insercao normal, sem marca.
    let r = inserir(
        &s,
        r#""linha":{"id":2,"nome":"bia"},"se_existir":"atualizar""#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("rowid", -1), 2);
    assert!(
        r.campo("atualizada").is_none(),
        "insercao marcada como alteracao"
    );
    assert_eq!(registros(&s), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// Dois indices unicos e nenhum primario: RECUSA nomeando os candidatos,
/// e dizer qual resolve.
#[test]
fn indice_ambiguo_recusa_nomeando_os_candidatos() {
    let d = dir("ambiguo");
    let s = servidor(&d);
    let pede = |corpo: &str| {
        s.executar(
            "inserir",
            &pedido(&format!(r#"{{"database":"b","tabela":"pessoas",{corpo}}}"#)),
            &Sessao::default(),
        )
    };
    pede(r#""linha":{"cpf":"1","email":"a@x","nome":"ana"}"#).unwrap();
    let e = pede(r#""linha":{"cpf":"1","email":"b@x","nome":"nova"},"se_existir":"atualizar""#)
        .expect_err("escolheu o indice sozinho");
    let t = e.to_string();
    assert!(t.contains("porCpf") && t.contains("porEmail"), "{t}");

    // Dito qual, funciona -- e casa pelo CPF, e nao pelo email.
    let r = pede(
        r#""linha":{"cpf":"1","email":"b@x","nome":"nova"},
               "se_existir":"atualizar","indice":"porCpf""#,
    )
    .unwrap();
    assert!(r.booleano_ou("atualizada", false), "{r:?}");
    assert_eq!(r.inteiro_ou("rowid", -1), 1);

    // Indice que existe mas nao e unico nao sabe dizer se a linha existe.
    let e = inserir(
        &s,
        r#""linha":{"id":9},"se_existir":"ignorar","indice":"nao_existe""#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("nao_existe"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `se_existir` com palavra que nao existe RECUSA, listando as que valem
/// -- em vez de cair no padrao e gravar de um jeito que ninguem pediu.
#[test]
fn se_existir_invalido_recusa_listando() {
    let d = dir("palavra");
    let s = servidor(&d);
    let e = inserir(&s, r#""linha":{"id":1},"se_existir":"talvez""#).unwrap_err();
    let t = e.to_string();
    assert!(t.contains("talvez"), "{t}");
    assert!(t.contains("ignorar") && t.contains("atualizar"), "{t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **DENTRO DE TRANSACAO, o upsert empilha COMO A OP QUE ELE VIROU.**
///
/// A alternativa seria empilhar sempre como `inserir` e deixar o commit
/// descobrir a duplicata: a transacao inteira cairia no fim por causa de
/// uma linha que o pedido mandava justamente sobrescrever.
///
/// A prova mede a `acao` empilhada E o dado depois do commit -- um teste
/// que so olhasse a resposta do empilhar nao veria a gravacao errada.
#[test]
fn dentro_da_transacao_o_upsert_empilha_a_op_que_ele_virou() {
    let d = dir("transacao");
    let s = servidor(&d);
    inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();

    let ses = Sessao {
        ligacao: 7,
        ..Sessao::default()
    };
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();

    // A chave que JA existe vira um `atualizar` empilhado.
    let r = s
        .executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"clientes",
                        "linha":{"id":1,"nome":"nova"},"se_existir":"atualizar"}"#,
            ),
            &ses,
        )
        .unwrap();
    assert!(r.booleano_ou("empilhada", false), "{r:?}");
    assert_eq!(
        r.texto_ou("acao", ""),
        "atualizar",
        "empilhou como insercao"
    );
    assert!(r.booleano_ou("atualizada", false), "{r:?}");
    assert_eq!(r.inteiro_ou("rowid", -1), 1);

    // A que NAO existe continua sendo insercao.
    let r = s
        .executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"clientes",
                        "linha":{"id":2,"nome":"bia"},"se_existir":"atualizar"}"#,
            ),
            &ses,
        )
        .unwrap();
    assert_eq!(r.texto_ou("acao", ""), "inserir");

    s.executar("commit", &Json::objeto(vec![]), &ses).unwrap();
    assert_eq!(
        nome_gravado(&s, 1),
        "nova",
        "o commit nao gravou a alteracao"
    );
    assert_eq!(registros(&s), 2);
    let _ = std::fs::remove_dir_all(&d);
}

fn pessoa(s: &Arc<Servidor>, rowid: u64) -> Json {
    s.executar(
        "ler",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"pessoas","rowid":{rowid}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

/// **O `atualizar` (o SET do `ON CONFLICT DO UPDATE`) grava a linha LIDA
/// com o SET por cima -- e o `VALUES` fica de fora.** O motor passou uma
/// rodada ignorando o campo calado: o tradutor punha o SET em
/// `atualizar`, ninguem lia, e `... DO UPDATE SET nome = 'B'` gravava o
/// VALUES por cima da linha, com NULL nas colunas que o VALUES nao trazia.
///
/// A prova mede as TRES colunas: a do SET mudou, a que so o VALUES trazia
/// NAO mudou, e a que ninguem citou continua. E mede a linha nova: quando
/// a chave nao existe, o VALUES entra e o SET nao.
#[test]
fn o_atualizar_grava_a_lida_com_o_set_por_cima_e_nao_o_values() {
    let d = dir("set");
    let s = servidor(&d);
    let ins = |corpo: &str| {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"pessoas","indice":"porCpf",{corpo}}}"#
            )),
            &Sessao::default(),
        )
    };
    ins(r#""linha":{"cpf":"1","email":"a@x","nome":"um"}"#).unwrap();
    let r = ins(r#""linha":{"cpf":"1","email":"b@x","nome":"dois"},
               "se_existir":"atualizar","atualizar":{"nome":"B"}"#)
    .unwrap();
    assert!(r.booleano_ou("atualizada", false), "{r:?}");
    let l = pessoa(&s, 1);
    assert_eq!(l.texto_ou("nome", ""), "B", "o SET nao foi honrado: {l:?}");
    assert_eq!(
        l.texto_ou("email", ""),
        "a@x",
        "o VALUES entrou na linha existente: {l:?}"
    );
    assert_eq!(l.texto_ou("cpf", ""), "1");

    // A chave que NAO existe: entra o VALUES, e o SET nao.
    let r = ins(r#""linha":{"cpf":"2","email":"c@x","nome":"tres"},
               "se_existir":"atualizar","atualizar":{"nome":"Z"}"#)
    .unwrap();
    assert!(!r.booleano_ou("atualizada", false), "{r:?}");
    let l = pessoa(&s, 2);
    assert_eq!(l.texto_ou("nome", ""), "tres", "{l:?}");
    assert_eq!(l.texto_ou("email", ""), "c@x");

    // Pelo SQL, que e de onde o campo vem.
    s.executar(
        "sql",
        &pedido(
            r#"{"database":"b","texto":"INSERT INTO pessoas (cpf, email, nome) VALUES ('1', 'z@x', 'quatro') ON CONFLICT (cpf) DO UPDATE SET nome = 'SQL'"}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    let l = pessoa(&s, 1);
    assert_eq!(l.texto_ou("nome", ""), "SQL", "{l:?}");
    assert_eq!(l.texto_ou("email", ""), "a@x", "{l:?}");

    // Dentro de transacao: o que empilha e a lida mesclada.
    let ses = Sessao {
        ligacao: 7,
        ..Sessao::default()
    };
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    let r = s
        .executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"pessoas","indice":"porCpf",
                        "linha":{"cpf":"1","email":"t@x","nome":"tx"},
                        "se_existir":"atualizar","atualizar":{"nome":"TX"}}"#,
            ),
            &ses,
        )
        .unwrap();
    assert_eq!(r.texto_ou("acao", ""), "atualizar", "{r:?}");
    s.executar("commit", &Json::objeto(vec![]), &ses).unwrap();
    let l = pessoa(&s, 1);
    assert_eq!(l.texto_ou("nome", ""), "TX", "{l:?}");
    assert_eq!(
        l.texto_ou("email", ""),
        "a@x",
        "o VALUES entrou pela transacao: {l:?}"
    );

    // Coluna que nao existe recusa nomeando, e nada muda.
    let e = ins(r#""linha":{"cpf":"1","email":"e@x","nome":"x"},
               "se_existir":"atualizar","atualizar":{"zzz":"1"}"#)
    .expect_err("gravou coluna que nao existe");
    assert!(e.to_string().contains("zzz"), "{e}");
    assert_eq!(pessoa(&s, 1).texto_ou("nome", ""), "TX");
    let _ = std::fs::remove_dir_all(&d);
}

/// **CONTRATO, nao defeito -- e a guarda dele** (pedido 245, O3).
///
/// O upsert SEM o campo `atualizar` grava a LINHA INTEIRA por cima, e a
/// coluna que o pedido nao trouxe fica NULA. Medido em 16/09/2026: com
/// `{"cpf":"1","nome":"dois"}`, o `email` gravado virou nulo.
///
/// **Isto nao vira uma mescla**, e a razao esta no IRMAO e nao no gosto:
/// o `crate::upsert` e o mesmo caminho da sincronia do DbLink
/// (`dblink/sincronia.rs::aplicar_para_ca`). Mesclar aqui tiraria dela a
/// unica forma de gravar NULO num destino -- uma sincronia que nao
/// consegue apagar um campo deixa o destino diferente da origem, calada,
/// que e exatamente o que ela existe para impedir. E mudaria o
/// significado do `inserir` de todo cliente escrito antes.
///
/// A saida existe e e a do SQL: o campo `atualizar` (o SET), provado no
/// teste irmao acima. O que faltava era o catalogo dize-lo -- o campo nao
/// estava listado la, entao quem lia o protocolo nao achava a forma
/// segura.
///
/// Reponha o «conserto» fazendo o upsert mesclar: a primeira asserção
/// (o nulo) cai, e com ela a garantia do DbLink.
#[test]
fn o_upsert_sem_o_set_grava_a_linha_inteira_e_isso_e_contrato() {
    let d = dir("contrato");
    let s = servidor(&d);
    let ins = |corpo: &str| {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"pessoas","indice":"porCpf",{corpo}}}"#
            )),
            &Sessao::default(),
        )
    };
    ins(r#""linha":{"cpf":"1","email":"a@x","nome":"um"}"#).unwrap();

    // A forma PERIGOSA, que e o contrato: o que nao veio vira nulo.
    ins(r#""linha":{"cpf":"1","nome":"dois"},"se_existir":"atualizar""#).unwrap();
    let l = pessoa(&s, 1);
    assert!(
        l.campo("email").map(Json::e_nulo).unwrap_or(false),
        "o upsert parcial deixou de zerar a coluna ausente: {l:?}"
    );
    assert_eq!(l.texto_ou("nome", ""), "dois");

    // A forma SEGURA, no mesmo pedido, sobre a mesma linha: o que nao veio
    // fica como estava. As duas lado a lado sao o que faz a diferenca
    // entre elas ser contrato e nao acaso.
    ins(r#""linha":{"cpf":"1","email":"b@x","nome":"tres"}"#).unwrap_err();
    ins(r#""linha":{"cpf":"1","email":"b@x"},
             "se_existir":"atualizar","atualizar":{"nome":"quatro"}"#)
    .unwrap();
    let l = pessoa(&s, 1);
    assert_eq!(l.texto_ou("nome", ""), "quatro");
    assert!(
        l.campo("email").map(Json::e_nulo).unwrap_or(false),
        "o VALUES do pedido entrou na linha que ja existia: {l:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **`atualizar` fora do modo `atualizar` RECUSA em vez de ser ignorado.**
/// Ignorar calado e o que o motor fez por uma rodada inteira, e nenhum
/// erro apareceu porque nenhum foi emitido.
#[test]
fn o_atualizar_fora_do_modo_atualizar_recusa() {
    let d = dir("set-modo");
    let s = servidor(&d);
    inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();
    for corpo in [
        r#""linha":{"id":1,"nome":"x"},"atualizar":{"nome":"B"}"#,
        r#""linha":{"id":1,"nome":"x"},"se_existir":"ignorar","atualizar":{"nome":"B"}"#,
        r#""linha":{"id":1,"nome":"x"},"se_existir":"atualizar","atualizar":"nome""#,
    ] {
        let e = inserir(&s, corpo).expect_err("o atualizar passou calado");
        assert!(e.to_string().contains("atualizar"), "{corpo}: {e}");
    }
    assert_eq!(nome_gravado(&s, 1), "ana", "alguma gravou");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`ignorar` dentro de transacao nao empilha nada**, e a resposta diz
/// isso: nao ha o que gravar, entao nao ha o que confirmar.
#[test]
fn ignorar_dentro_da_transacao_nao_empilha() {
    let d = dir("tx-ignorar");
    let s = servidor(&d);
    inserir(&s, r#""linha":{"id":1,"nome":"ana"}"#).unwrap();
    let ses = Sessao {
        ligacao: 8,
        ..Sessao::default()
    };
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    let r = s
        .executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"clientes",
                        "linha":{"id":1,"nome":"nao"},"se_existir":"ignorar"}"#,
            ),
            &ses,
        )
        .unwrap();
    assert!(!r.booleano_ou("empilhada", true), "{r:?}");
    assert!(r.booleano_ou("ignorada", false), "{r:?}");
    s.executar("commit", &Json::objeto(vec![]), &ses).unwrap();
    assert_eq!(nome_gravado(&s, 1), "ana");
    let _ = std::fs::remove_dir_all(&d);
}

/// A chave que ESTA TRANSACAO ja empilhou recusa nomeando: a linha
/// pendente ainda nao tem rowid em disco para atualizar, e escolher um dos
/// dois caminhos seria adivinhar.
#[test]
fn a_chave_pendente_na_mesma_transacao_recusa_nomeando() {
    let d = dir("tx-pendente");
    let s = servidor(&d);
    let ses = Sessao {
        ligacao: 9,
        ..Sessao::default()
    };
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    let corpo = r#"{"database":"b","tabela":"clientes",
                        "linha":{"id":5,"nome":"x"},"se_existir":"atualizar"}"#;
    s.executar("inserir", &pedido(corpo), &ses).unwrap();
    let e = s
        .executar("inserir", &pedido(corpo), &ses)
        .expect_err("a chave pendente passou");
    assert_eq!(e.nome(), "DUPLICADO", "{e}");
    assert!(e.to_string().contains("DISCO"), "a recusa nao explica: {e}");
    s.executar("rollback", &Json::objeto(vec![]), &ses).unwrap();
    let _ = std::fs::remove_dir_all(&d);
}

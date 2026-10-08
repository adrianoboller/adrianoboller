//! As quatro sondas de efeito do `bancada/comparativo/medir.py` --
//! DEFAULT, CHECK, coluna calculada e indice parcial -- mais a do indice
//! por expressao, feitas AQUI com os mesmos pedidos que o medidor manda,
//! para a celula virar TEM pelo medidor e nao por edicao.
use super::*;

fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    s.executar(
        "criar_database",
        &Json::analisar(r#"{"database":"cmp"}"#).unwrap(),
        &Sessao::default(),
    )
    .unwrap();
    s
}

fn roda(s: &Arc<Servidor>, op: &str, txt: &str) -> Result<Json> {
    s.executar(op, &Json::analisar(txt).unwrap(), &Sessao::default())
}

fn linha(s: &Arc<Servidor>, tabela: &str, rowid: u64) -> Json {
    roda(
        s,
        "ler",
        &format!(r#"{{"database":"cmp","tabela":"{tabela}","rowid":{rowid}}}"#),
    )
    .unwrap()
}

#[test]
fn default_a_linha_nasce_com_7() {
    let d = DirTemp::novo("regras-default");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_def",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"v","tipo":"Int8","padrao":7}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_def","linha":{"id":1}}"#,
    )
    .unwrap();
    assert_eq!(
        linha(&s, "t_def", 1).campo("v").and_then(Json::inteiro),
        Some(7)
    );
    // E o esquema DIZ que a coluna tem padrao -- campo que nao se le mente.
    let e = roda(&s, "esquema", r#"{"database":"cmp","tabela":"t_def"}"#).unwrap();
    let cols = e.campo("colunas").and_then(Json::lista).unwrap();
    let v = cols.iter().find(|c| c.texto_ou("nome", "") == "v").unwrap();
    assert_eq!(v.texto_ou("padrao", ""), "7");
}

#[test]
fn check_recusa_menos_5_e_aceita_5() {
    let d = DirTemp::novo("regras-check");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_ck",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"v","tipo":"Int8","check":"v > 0"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    let proibido = roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_ck","linha":{"id":1,"v":-5}}"#,
    );
    let e = proibido.unwrap_err().to_string();
    assert!(e.contains("CHECK da coluna v"), "{e}");
    // O CONTROLE: o permitido passa na mesma tabela.
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_ck","linha":{"id":2,"v":5}}"#,
    )
    .unwrap();
}

#[test]
fn calculada_b_sai_6() {
    let d = DirTemp::novo("regras-calculada");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_gc",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"a","tipo":"Int8"},
                       {"nome":"b","tipo":"Int8","calculada":"a*2"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_gc","linha":{"id":1,"a":3}}"#,
    )
    .unwrap();
    assert_eq!(
        linha(&s, "t_gc", 1).campo("b").and_then(Json::inteiro),
        Some(6)
    );
}

#[test]
fn indice_parcial_guarda_a_incluida_e_nao_a_filtrada() {
    let d = DirTemp::novo("regras-parcial");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_ip",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"v","tipo":"Int8"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true},
                       {"nome":"so_positivo","colunas":["v"],"onde":"v > 0"}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_ip","linha":{"id":1,"v":-5}}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_ip","linha":{"id":2,"v":7}}"#,
    )
    .unwrap();
    let fora = roda(
        &s,
        "buscar",
        r#"{"database":"cmp","tabela":"t_ip","indice":"so_positivo","chave":[-5]}"#,
    )
    .unwrap();
    let dentro = roda(
        &s,
        "buscar",
        r#"{"database":"cmp","tabela":"t_ip","indice":"so_positivo","chave":[7]}"#,
    )
    .unwrap();
    assert_eq!(
        dentro.campo("encontrados").and_then(Json::inteiro),
        Some(1),
        "{dentro:?}"
    );
    assert_eq!(
        fora.campo("encontrados").and_then(Json::inteiro),
        Some(0),
        "{fora:?}"
    );
}

#[test]
fn indice_por_expressao_nasce_e_acha_pelo_valor_baixo() {
    let d = DirTemp::novo("regras-expressao");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_ie",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"nome","tipo":"Str(40)"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true},
                       {"nome":"por_baixo","colunas":["lower(nome)"]}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_ie","linha":{"id":1,"nome":"Ana"}}"#,
    )
    .unwrap();
    let acha = roda(
        &s,
        "buscar",
        r#"{"database":"cmp","tabela":"t_ie","indice":"por_baixo","chave":["ana"]}"#,
    )
    .unwrap();
    assert_eq!(
        acha.campo("encontrados").and_then(Json::inteiro),
        Some(1),
        "{acha:?}"
    );
    let e = roda(&s, "esquema", r#"{"database":"cmp","tabela":"t_ie"}"#).unwrap();
    let idx = e.campo("indices").and_then(Json::lista).unwrap();
    let pb = idx
        .iter()
        .find(|i| i.texto_ou("nome", "") == "por_baixo")
        .unwrap();
    let col = &pb.campo("colunas").and_then(Json::lista).unwrap()[0];
    assert_eq!(col.texto_ou("expressao", ""), "lower(nome)");
    // Expressao de duas colunas recusa na declaracao, nomeando.
    let e = roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_ie2",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"nome","tipo":"Str(40)"}],
            "indices":[{"nome":"x","colunas":["concat(nome, id)"]}]}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("exatamente uma coluna"), "{e}");
}

/// Campo desconhecido continua desconhecido: quem manda `default` em vez
/// de `padrao` nao ganha padrao -- e o esquema mostra que nao ganhou.
#[test]
fn a_declaracao_recusa_coluna_inexistente_na_expressao() {
    let d = DirTemp::novo("regras-recusa");
    let s = servidor(&d.0);
    let e = roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_x",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"v","tipo":"Int8","check":"w > 0"}]}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("check de v") && e.contains("\"w\""), "{e}");
}

/// **Pedido 245, O2b: a calculada acrescentada PREENCHE a linha velha** --
/// parecer do papel C (`docs/propostas/parecer-dba-check-e-calculada-contra-linha-velha-2026-09.md`
/// §4.2 e §9): os quatro motores convergem em que a linha velha nunca le
/// nulo numa calculada computavel, e o preenchimento acontece no ALTER.
///
/// Medido antes do conserto (16/09/2026): `b = a*2` sobre a = 3 e 7 lia
/// NULO nas duas, e `SUM(b)` devolvia 10 sobre uma tabela cuja soma e 24
/// depois que UMA delas era tocada. Aqui: as duas linhas leem 6 e 14, a
/// excluida suave tambem ganha o valor (volta pelo `restaurar`), a
/// resposta nao traz mais o aviso de NULA, e tres recusas da declaracao
/// deixam a tabela como estava -- padrao numa calculada, a conta que nao
/// cabe no tipo (nomeando o rowid) e o CHECK julgado com o valor
/// CALCULADO.
///
/// Reponha o defeito passando `None` no lugar do `preencher` do
/// `acrescentar_coluna_fase_a_recusando`: as leituras de 6 e 14 caem.
#[test]
fn acrescentar_calculada_preenche_a_linha_velha() {
    let d = DirTemp::novo("regras-o2b");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_av",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"a","tipo":"Int8"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    for (id, a) in [(1, 3), (2, 7), (3, 100)] {
        roda(
            &s,
            "inserir",
            &format!(r#"{{"database":"cmp","tabela":"t_av","linha":{{"id":{id},"a":{a}}}}}"#),
        )
        .unwrap();
    }
    let x = roda(
        &s,
        "excluir",
        r#"{"database":"cmp","tabela":"t_av","rowid":3,"motivo":"teste"}"#,
    )
    .unwrap();
    assert_eq!(x.texto_ou("modo", ""), "suave", "{}", x.escrever());
    let colunas = |s: &Arc<Servidor>| {
        roda(s, "esquema", r#"{"database":"cmp","tabela":"t_av"}"#)
            .unwrap()
            .campo("colunas")
            .and_then(Json::lista)
            .map(|l| l.len())
            .unwrap()
    };
    let antes = colunas(&s);

    // RECUSA 1: padrao numa calculada e um valor que a proxima gravacao
    // apaga (o `default 999` sobre `a*3` do parecer, §3).
    let e = roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_av",
                "coluna":{"nome":"c","tipo":"Int8","calculada":"a*3"},"default":999}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("calculada") && e.contains("padrao"), "{e}");

    // RECUSA 2: a conta que nao cabe no tipo, na linha que ja existe --
    // 100 * 2 = 200 nao cabe num Int1 -- nomeia o rowid, e recusa ANTES
    // de reescrever: a excluida suave conta.
    let e = roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_av",
                "coluna":{"nome":"c","tipo":"Int1","calculada":"a*2"}}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("linha 3") && e.contains("calculada c"), "{e}");

    // RECUSA 3: o CHECK da propria calculada e julgado com o valor que a
    // linha VAI ter. a*2 > 10 falha em a = 3 (6) -- e so em a = 3.
    let e = roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_av",
                "coluna":{"nome":"c","tipo":"Int8","calculada":"a*2","check":"c > 10"}}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("1 das 3"), "{e}");
    assert_eq!(antes, colunas(&s), "uma recusa tocou no esquema");

    // O caso: entra, sem aviso, e a linha velha le o valor calculado.
    let r = roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_av",
                "coluna":{"nome":"b","tipo":"Int8","calculada":"a*2"}}"#,
    )
    .unwrap();
    assert!(r.campo("avisos").is_none(), "{}", r.escrever());
    assert_eq!(
        linha(&s, "t_av", 1).campo("b").and_then(Json::inteiro),
        Some(6)
    );
    assert_eq!(
        linha(&s, "t_av", 2).campo("b").and_then(Json::inteiro),
        Some(14)
    );
    roda(
        &s,
        "restaurar",
        r#"{"database":"cmp","tabela":"t_av","rowid":3}"#,
    )
    .unwrap();
    assert_eq!(
        linha(&s, "t_av", 3).campo("b").and_then(Json::inteiro),
        Some(200),
        "a excluida suave volta pelo `restaurar` sem passar pela calculada"
    );

    // CONTROLE (o comportamento velho): coluna SEM regra nenhuma continua
    // nascendo nula e sem campo novo na resposta.
    let r = roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_av","coluna":{"nome":"c","tipo":"Int8"}}"#,
    )
    .unwrap();
    assert!(r.campo("avisos").is_none(), "{}", r.escrever());
    assert!(linha(&s, "t_av", 1).campo("c").unwrap().e_nulo());
}

/// **Pedido 245, O2a: o CHECK que linha velha viola recusa a coluna,
/// dizendo quantas linhas** -- decisao do dono de 17/09/2026 05:41.
///
/// Pelo `despachar`. Quatro linhas, `a` = 3, 20, 40 e 1, a ultima
/// excluida suave; o `CHECK a > 18` numa coluna nova e violado por DUAS.
/// A recusa diz «2 das 4» -- a excluida suave conta, porque volta pelo
/// `restaurar` sem passar pelo CHECK -- e a tabela fica com as colunas que
/// tinha.
///
/// O controle (comportamento velho): o MESMO comando com um CHECK que as
/// tres cumprem (`a > 0`) continua entrando.
///
/// **Defeito reposto** (a contagem tirada da fase A do `Table`, que era o
/// codigo de antes do O2a): a coluna entra, e o `unwrap_err` cai.
#[test]
fn o_check_que_a_linha_velha_viola_recusa_a_coluna_dizendo_quantas() {
    let d = DirTemp::novo("regras-o2a");
    let s = servidor(&d.0);
    let pede = |corpo: &str| -> Result<Json> {
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        r
    };
    pede(
        r#""op":"criar_tabela","database":"cmp","tabela":"t_o2",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"a","tipo":"Int8"}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true}]"#,
    )
    .unwrap();
    for (id, a) in [(1, 3), (2, 20), (3, 40), (4, 1)] {
        pede(&format!(
            r#""op":"inserir","database":"cmp","tabela":"t_o2",
                   "linha":{{"id":{id},"a":{a}}}"#
        ))
        .unwrap();
    }
    // A quarta (a = 1) sai pela exclusao SUAVE: continua no `.reg`.
    let x = pede(
        r#""op":"excluir","database":"cmp","tabela":"t_o2","rowid":4,
               "motivo":"teste""#,
    )
    .unwrap();
    assert_eq!(x.texto_ou("modo", ""), "suave", "{}", x.escrever());
    let antes = pede(r#""op":"esquema","database":"cmp","tabela":"t_o2""#)
        .unwrap()
        .campo("colunas")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap();

    let e = pede(
        r#""op":"acrescentar_coluna","database":"cmp","tabela":"t_o2",
               "coluna":{"nome":"nota","tipo":"Int8","check":"a > 18"}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("2 das 4"),
        "a recusa tem de dizer QUANTAS linhas violam (2 das 4, a excluida \
             suave inclusive): {e}"
    );
    assert!(e.contains("nota") && e.contains("a > 18"), "{e}");
    let depois = pede(r#""op":"esquema","database":"cmp","tabela":"t_o2""#)
        .unwrap()
        .campo("colunas")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap();
    assert_eq!(antes, depois, "a recusa tocou no esquema");

    // CONTROLE: a declaracao SEM violacao continua passando.
    let r = pede(
        r#""op":"acrescentar_coluna","database":"cmp","tabela":"t_o2",
               "coluna":{"nome":"nota","tipo":"Int8","check":"a > 0"}"#,
    )
    .unwrap();
    assert!(r.campo("avisos").is_none(), "{}", r.escrever());
}

/// Pedido 475: a linha VELHA tem de receber o MESMO valor que o padrao
/// promete as linhas novas -- nao o JSON cru de "padrao" relido feito um
/// `inserir`. Antes do conserto, `"padrao":"'ativo'"` (a forma correta,
/// porque `padrao` e EXPRESSAO, MANUAL.txt:398) gravava a linha velha
/// com o texto `'ativo'`, aspas e tudo -- as aspas exigidas para o
/// EXPRESSAO passar entravam na LINHA, porque o backfill nao evaluava a
/// expressao: so recodificava o JSON.
#[test]
fn a_linha_velha_recebe_o_padrao_avaliado_e_nao_o_json_cru() {
    let d = DirTemp::novo("regras-padrao-texto");
    let s = servidor(&d.0);
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_pd",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_pd","linha":{"id":1}}"#,
    )
    .unwrap();
    // A forma que o F-NUCLEO exige para um texto: aspas simples.
    roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_pd",
                "nome":"situacao","tipo":"Str(12)","padrao":"'ativo'"}"#,
    )
    .unwrap();
    // A linha VELHA ganha o texto "ativo" -- sem as aspas da expressao.
    assert_eq!(linha(&s, "t_pd", 1).texto_ou("situacao", ""), "ativo");
    // E quem insere uma linha NOVA sem citar a coluna ganha o mesmo
    // padrao, pelo caminho de sempre (`aplicar_regras`, na tabela) --
    // e o dois tem de bater, porque e o MESMO padrao.
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_pd","linha":{"id":2}}"#,
    )
    .unwrap();
    assert_eq!(linha(&s, "t_pd", 2).texto_ou("situacao", ""), "ativo");

    // O escape continua vivo: SO "default" (sem "padrao") pede um valor
    // CRU, sem passar pelo crivo de expressao -- e nao cria DEFAULT
    // nenhum para linhas futuras.
    roda(
        &s,
        "criar_tabela",
        r#"{"database":"cmp","tabela":"t_pd2",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true}]}"#,
    )
    .unwrap();
    roda(
        &s,
        "inserir",
        r#"{"database":"cmp","tabela":"t_pd2","linha":{"id":1}}"#,
    )
    .unwrap();
    roda(
        &s,
        "acrescentar_coluna",
        r#"{"database":"cmp","tabela":"t_pd2",
                "nome":"situacao","tipo":"Str(12)","default":"ativo"}"#,
    )
    .unwrap();
    assert_eq!(linha(&s, "t_pd2", 1).texto_ou("situacao", ""), "ativo");
    let e = roda(&s, "esquema", r#"{"database":"cmp","tabela":"t_pd2"}"#).unwrap();
    let cols = e.campo("colunas").and_then(Json::lista).unwrap();
    let v = cols
        .iter()
        .find(|c| c.texto_ou("nome", "") == "situacao")
        .unwrap();
    assert_eq!(v.texto_ou("padrao", ""), "");
}

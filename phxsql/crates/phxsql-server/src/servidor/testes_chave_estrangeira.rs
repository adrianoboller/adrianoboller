//! **Chave estrangeira pelo protocolo** -- o #127 dizia «pronto» e era meia
//! verdade: o formato as suporta e o `esquema` as reporta desde sempre, mas
//! NENHUMA operacao as criava. So dava para declarar uma pela API Rust.
use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("fk-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

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
        &pedido(r#"{"database":"b"}"#),
        &Sessao::default(),
    )
    .unwrap();
    s
}

/// Pelo `despachar`: e por onde o pedido entra de verdade.
fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// Cria `pedidos` apontando para `clientes`, e le a chave de volta.
fn com_fk(s: &Arc<Servidor>, extra: &str) -> Result<Json> {
    pede(
        s,
        &format!(
            r#""op":"criar_tabela","database":"b","tabela":"pedidos",
                   "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                              {{"nome":"cliente_id","tipo":"Int4"}}],
                   "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}}],
                   "chaves_estrangeiras":[{{"nome":"fk_cliente",
                                            "colunas":["cliente_id"],
                                            "tabela_ref":"clientes",
                                            "colunas_ref":["id"]{extra}}}]"#
        ),
    )
}

#[test]
fn criar_tabela_declara_a_chave_e_o_esquema_a_devolve() {
    let guarda = dir_temp("declara");
    let s = servidor(&guarda);
    com_fk(&s, r#","ao_excluir":"restringir","ao_alterar":"cascata""#).unwrap();

    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    let fks = e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(fks.len(), 1, "a chave nao voltou: {}", e.escrever());
    let fk = &fks[0];
    assert_eq!(fk.texto_ou("nome", ""), "fk_cliente");
    assert_eq!(fk.texto_ou("tabela_ref", ""), "clientes");
    assert_eq!(fk.textos("colunas"), vec!["cliente_id"]);
    assert_eq!(fk.textos("colunas_ref"), vec!["id"]);
    assert_eq!(fk.texto_ou("ao_excluir", ""), "Restringir");
    assert_eq!(fk.texto_ou("ao_alterar", ""), "Cascata");

    // E o papel da coluna, que e DERIVADO das chaves, acompanha: sem isto
    // a tela desenharia a coluna como uma qualquer.
    let coluna = e
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .find(|c| c.texto_ou("nome", "") == "cliente_id")
        .cloned()
        .unwrap();
    assert_eq!(coluna.campo("estrangeira").unwrap().booleano(), Some(true));
    assert_eq!(coluna.textos("nas_chaves_estrangeiras"), vec!["fk_cliente"]);
}

/// **O teste do comportamento VELHO, e e o que mais importa.** Um pedido
/// sem o campo tem de criar a tabela exatamente como sempre criou -- todo
/// cliente escrito antes desta versao manda pedido assim.
#[test]
fn sem_o_campo_a_tabela_nasce_igual_ao_que_sempre_foi() {
    let guarda = dir_temp("velho");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
    )
    .unwrap();
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"clientes""#).unwrap();
    assert!(e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());
    // E a linha entra como sempre entrou.
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":1}"#,
    )
    .unwrap();
}

/// Sem `colunas_ref`, referencia colunas de MESMO NOME. Escrever a lista
/// duas vezes e onde alguem troca a ordem sem perceber.
#[test]
fn sem_colunas_ref_vale_o_mesmo_nome() {
    let guarda = dir_temp("mesmo-nome");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"itens",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}],
               "chaves_estrangeiras":[{"nome":"fk_id","colunas":["id"],
                                       "tabela_ref":"outra"}]"#,
    )
    .unwrap();
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"itens""#).unwrap();
    let fk = &e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap()[0];
    assert_eq!(fk.textos("colunas_ref"), vec!["id"]);
    // O padrao de cada lado e o da REGRA PRIMORDIAL: restringir ao
    // excluir (nunca se mata o pai que tem filhos) e cascata ao alterar
    // (a chave do pai que muda leva as filhas junto).
    assert_eq!(fk.texto_ou("ao_excluir", ""), "Restringir");
    assert_eq!(fk.texto_ou("ao_alterar", ""), "Cascata");
}

/// A acao aceita o portugues, o SQL e a forma que o `esquema` DEVOLVE --
/// pela mesma razao do tipo da coluna: o que sai tem de poder voltar.
#[test]
fn a_acao_aceita_as_tres_escritas() {
    for (escrito, esperado) in [
        ("cascata", "Cascata"),
        ("CASCADE", "Cascata"),
        ("Cascata", "Cascata"),
        ("restringir", "Restringir"),
        ("RESTRICT", "Restringir"),
        ("Restringir", "Restringir"),
    ] {
        let guarda = dir_temp(&format!("acao-{}", escrito.replace(' ', "-")));
        let s = servidor(&guarda);
        com_fk(&s, &format!(r#","ao_alterar":"{escrito}""#)).unwrap();
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
        let fk = &e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()[0];
        assert_eq!(fk.texto_ou("ao_alterar", ""), esperado, "{escrito}");
    }
}

/// Chave mal escrita recusa dizendo O QUE esta errado, e a tabela NAO
/// nasce: meia tabela criada seria pior que nenhuma.
#[test]
fn chave_mal_escrita_recusa_e_a_tabela_nao_nasce() {
    let guarda = dir_temp("ruim");
    let s = servidor(&guarda);
    // Cada caso com um nome de tabela proprio: com o mesmo nome, o segundo
    // erro seria "ja existe" e o teste passaria pelo motivo errado.
    for (n, chave, esperado) in [
        (
            1,
            r#"{"nome":"fk","colunas":["nao_existe"],"tabela_ref":"c"}"#,
            "nao existe nesta tabela",
        ),
        (
            2,
            r#"{"nome":"fk","colunas":["id"],"tabela_ref":""}"#,
            "tabela_ref",
        ),
        (
            3,
            r#"{"nome":"fk","colunas":["id"],"tabela_ref":"c","ao_excluir":"talvez"}"#,
            "acao de integridade desconhecida",
        ),
        (4, r#"{"colunas":["id"],"tabela_ref":"c"}"#, "nome"),
        (5, r#"{"nome":"fk","tabela_ref":"c"}"#, "colunas"),
    ] {
        let e = pede(
            &s,
            &format!(
                r#""op":"criar_tabela","database":"b","tabela":"t{n}",
                       "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
                       "chaves_estrangeiras":[{chave}]"#
            ),
        )
        .unwrap_err();
        assert!(
            e.to_string().contains(esperado),
            "caso {n}: {e} (esperava {esperado:?})"
        );

        // E a tabela NAO nasceu: meia tabela criada seria pior que nenhuma,
        // porque o proximo pedido diria "ja existe" e ninguem entenderia.
        let t = pede(&s, r#""op":"tabelas","database":"b""#).unwrap();
        assert!(
            !t.escrever().contains(&format!("t{n}")),
            "caso {n}: a tabela nasceu mesmo com a chave recusada"
        );
    }
}

/// **Declarar nao e aplicar, e este teste existe para o documento nao
/// mentir.**
///
/// A chave fica gravada no esquema, o `esquema` a devolve e o diagrama a
/// desenha -- mas NENHUMA gravacao a consulta hoje. Uma linha filha
/// apontando para um pai que nao existe entra sem reclamacao.
///
/// O teste trava o comportamento REAL, e ele MUDOU por decisao do dono:
/// «padrao para chave NOVA». Antes ele afirmava que declarar nao impunha
/// nada; hoje afirma o contrario, e e o mesmo teste guardando o mesmo
/// lugar -- o que ele nunca deixou de fazer e impedir que alguem leia
/// `chaves_estrangeiras` no `criar_tabela` e suponha a garantia errada,
/// em qualquer dos dois sentidos.
///
/// A regra primordial diz «nunca se mata o pai que tem filhos», sem
/// condicao. Uma chave que precisa ser LEMBRADA de conferir nao honra um
/// «nunca» -- e por isso o padrao virou conferir.
///
/// Banco que ja existe NAO muda: o `PSCH` v7 grava o byte por chave, e o
/// esquema em disco volta com o que foi gravado nele.
#[test]
fn a_chave_declarada_nasce_conferida() {
    let guarda = dir_temp("nasce-conferida");
    let s = servidor(&guarda);
    com_fk(&s, "").unwrap();
    // `clientes` nem existe, e o pai 999 muito menos.
    let e = pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":999}"#,
    )
    .expect_err(
        "o orfao entrou numa chave declarada sem dizer `verificar`: o padrao \
             voltou a ser NAO conferir, e a regra primordial deixou de valer para \
             quem nao lembrou de pedir",
    );
    assert_eq!(e.codigo(), 3006, "erro deveria ser INTEGRIDADE: {e}");
}

/// E o caminho de quem ESCOLHE nao conferir continua existindo -- escrito,
/// e nao por esquecimento. Sem este teste, a decisao do dono teria tirado
/// a opcao junto com o padrao.
#[test]
fn quem_pede_para_nao_conferir_continua_podendo() {
    let guarda = dir_temp("opt-out");
    let s = servidor(&guarda);
    com_fk(&s, r#","verificar":false"#).unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":999}"#,
    )
    .expect("com `verificar:false` explicito o orfao tem de entrar como antes");
}

/// A garantia NOVA: quem pede `verificar` ganha a recusa do orfao.
///
/// Prova real: tirar `"verificar":true` faz este teste passar a inserir --
/// e e exatamente o que o teste irmao, `sem_verificar_o_orfao_ainda_entra`,
/// afirma que continua acontecendo.
#[test]
fn com_verificar_o_orfao_e_recusado() {
    let guarda = dir_temp("fk-orfao");
    let s = servidor(&guarda);
    criar_clientes(&s);
    com_fk(&s, r#","verificar":true"#).unwrap();
    let e = pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":999}"#,
    )
    .expect_err("o orfao entrou mesmo com a chave conferida");
    let txt = e.to_string();
    assert!(
        txt.contains("fk_cliente"),
        "o erro nao nomeia a chave: {txt}"
    );
    assert_eq!(e.codigo(), 3006, "familia errada: {txt}");
}

/// E a outra metade, sem a qual a de cima passaria por vacuidade: com a
/// mae no lugar, a MESMA insercao entra.
#[test]
fn com_verificar_a_linha_que_tem_mae_entra() {
    let guarda = dir_temp("fk-mae");
    let s = servidor(&guarda);
    criar_clientes(&s);
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":7}"#,
    )
    .unwrap();
    com_fk(&s, r#","verificar":true"#).unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":7}"#,
    )
    .expect("a linha com mae foi recusada");
}

/// NULO satisfaz a chave -- o `MATCH SIMPLE` da norma.
///
/// Conferir o nulo recusaria a filha que ainda nao tem pai, que e
/// justamente o caso que a coluna anulavel existe para permitir. Quem quer
/// o contrario declara a coluna obrigatoria, e ai quem recusa e a
/// obrigatoriedade -- outra regra, outro erro.
#[test]
fn o_nulo_satisfaz_a_chave_estrangeira() {
    let guarda = dir_temp("fk-nulo");
    let s = servidor(&guarda);
    criar_clientes(&s);
    com_fk(&s, r#","verificar":true"#).unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos","linha":{"id":1}"#,
    )
    .expect("a linha com cliente_id nulo foi recusada");
}

/// Sem indice na mae, a gravacao RECUSA dizendo qual indice falta.
///
/// A alternativa seria varrer a mae a cada linha filha gravada -- um custo
/// escondido dentro de um `inserir` que parece barato. *Erro que se le e
/// se conserta vale mais que lentidao que ninguem explica.*
#[test]
fn sem_indice_na_mae_a_recusa_diz_qual_indice_falta() {
    let guarda = dir_temp("fk-sem-ndx");
    let s = servidor(&guarda);
    // `clientes` nasce SEM indice em `id`.
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]"#,
    )
    .unwrap();
    com_fk(&s, r#","verificar":true"#).unwrap();
    let e = pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":1}"#,
    )
    .expect_err("gravou sem indice na mae -- vai varrer a tabela por linha");
    let txt = e.to_string();
    assert!(txt.contains("indice"), "o erro nao diz o que falta: {txt}");
    assert!(txt.contains("clientes"), "{txt}");
}

/// **A regra primordial: nunca se mata o pai que tem filhos.**
///
/// Palavra do dono, e do naipe da ordem de digitacao. `ao_excluir` aceita
/// SO `restringir`; cascata, anular e nada deixam de existir ali -- e o par
/// cascata/cascata some por CONSEQUENCIA, porque sem cascata no excluir nao
/// ha par com cascata dos dois lados.
///
/// A recusa e na DECLARACAO e nao na gravacao, de proposito: a tabela nasce
/// uma vez e grava um milhao de vezes. Recusar cedo custa um erro lido
/// enquanto se cria a tabela; recusar tarde custa um banco modelado errado,
/// descoberto no dia do primeiro `excluir`.
#[test]
fn ao_excluir_so_aceita_restringir() {
    for proibido in [
        "cascata",
        "cascade",
        "anular",
        "set null",
        "nada",
        "no action",
    ] {
        let guarda = dir_temp(&format!("proib-{}", proibido.replace(' ', "-")));
        let s = servidor(&guarda);
        let e = com_fk(&s, &format!(r#","ao_excluir":"{proibido}""#))
            .expect_err(&format!("{proibido:?} passou no ao_excluir"));
        let txt = e.to_string();
        assert!(
            txt.contains("sempre") && txt.contains("restringir"),
            "a recusa nao ensina a regra: {txt}"
        );
        assert!(
            txt.contains("cascata/cascata"),
            "a recusa nao diz que o par nao existe: {txt}"
        );
    }
}

/// `itens(id, x, cod_cliente = x + 0)` com a chave sobre a CALCULADA, e o
/// `ao_alterar` que o teste pedir (`extra` entra cru no objeto da chave).
fn itens_com_chave_calculada(s: &Arc<Servidor>, tabela: &str, extra: &str) -> Result<Json> {
    pede(
        s,
        &format!(
            r#""op":"criar_tabela","database":"b","tabela":"{tabela}",
                   "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                              {{"nome":"x","tipo":"Int4"}},
                              {{"nome":"cod_cliente","tipo":"Int4","calculada":"x + 0"}}],
                   "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}},
                              {{"nome":"porCliente","colunas":["cod_cliente"]}}],
                   "chaves_estrangeiras":[{{"nome":"fk_cliente","colunas":["cod_cliente"],
                                            "tabela_ref":"clientes","colunas_ref":["id"]{extra}}}]"#
        ),
    )
}

/// **Pedido 514, P1: chave sobre coluna CALCULADA nao aceita `ao_alterar`
/// que escreve nela.** A cascata levaria a chave nova da mae para uma
/// coluna que o motor recalcula a cada gravacao, e a filha ficaria orfa --
/// medido fora de transacao, com a mae ja gravada. PostgreSQL, MySQL e
/// MariaDB recusam o mesmo par na declaracao: aceite automatico.
///
/// O `ao_alterar` AUSENTE tambem cai, porque o padrao da casa e cascata --
/// e e ai que a recusa mais importa, porque quem nao escreveu nada nao
/// escolheu nada. As duas portas de declarar passam pelo mesmo leitor da
/// chave: o `criar_tabela` e o `declarar_fk`.
#[test]
fn chave_sobre_calculada_recusa_cascata_e_anular_na_declaracao() {
    for (i, extra) in [
        "",
        r#","ao_alterar":"cascata""#,
        r#","ao_alterar":"CASCADE""#,
        r#","ao_alterar":"anular""#,
    ]
    .into_iter()
    .enumerate()
    {
        let guarda = dir_temp(&format!("calc-cascata-{i}"));
        let s = servidor(&guarda);
        let e = itens_com_chave_calculada(&s, "itens", extra).expect_err(&format!(
            "{extra:?}: a chave calculada em cascata foi aceita"
        ));
        let txt = e.to_string();
        assert!(
            txt.contains("cod_cliente")
                && txt.contains("calculada")
                && txt.contains("\"restringir\""),
            "a recusa nao nomeia a coluna, a calculada e a saida: {txt}"
        );
    }

    // A segunda porta: a chave declarada numa tabela que ja existe.
    let guarda = dir_temp("calc-cascata-declarar");
    let s = servidor(&guarda);
    criar_clientes(&s);
    itens_com_chave_calculada(&s, "itens", r#","ao_alterar":"restringir""#).unwrap();
    let e = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"itens","nome":"fk_outra",
               "colunas":["cod_cliente"],"tabela_ref":"clientes","colunas_ref":["id"]"#,
    )
    .expect_err("o declarar_fk aceitou a chave calculada em cascata");
    assert!(e.to_string().contains("calculada"), "{e}");
}

/// O par do de cima: a recusa e do PAR, nao da coluna nem da cascata. A
/// chave calculada com `restringir` ou `nada` continua declarando -- os
/// tres maduros tambem as aceitam --, e a chave sobre coluna COMUM continua
/// nascendo em cascata. Sem isto, um portao que recusasse toda chave
/// calculada, ou toda cascata, passaria pelo teste de cima.
#[test]
fn chave_sobre_calculada_com_restringir_ou_nada_e_chave_comum_em_cascata_continuam() {
    for (i, extra) in [r#","ao_alterar":"restringir""#, r#","ao_alterar":"nada""#]
        .into_iter()
        .enumerate()
    {
        let guarda = dir_temp(&format!("calc-ok-{i}"));
        let s = servidor(&guarda);
        itens_com_chave_calculada(&s, "itens", extra)
            .unwrap_or_else(|e| panic!("{extra:?} foi recusado: {e}"));
    }
    let guarda = dir_temp("comum-cascata");
    let s = servidor(&guarda);
    com_fk(&s, "").expect("a chave comum com o padrao cascata foi recusada");
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    let fks = e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(fks[0].texto_ou("ao_alterar", ""), "Cascata");
}

/// O par do de cima, sem o qual ele passaria com um portao que recusa TUDO
/// -- e um portao assim tambem impediria criar qualquer chave.
#[test]
fn ao_excluir_aceita_restringir_escrito_de_tres_jeitos() {
    for escrito in ["restringir", "RESTRICT", "Restringir"] {
        let guarda = dir_temp(&format!("ok-{escrito}"));
        let s = servidor(&guarda);
        com_fk(&s, &format!(r#","ao_excluir":"{escrito}""#))
            .unwrap_or_else(|e| panic!("{escrito:?} foi recusado: {e}"));
    }
}

fn criar_clientes(s: &Arc<Servidor>) {
    pede(
        s,
        r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                           "primario":true}]"#,
    )
    .unwrap();
}

/// **`duplicar_tabela` preserva a chave.** Ele copia os arquivos byte a
/// byte, e o esquema mora no `.reg` -- mas isso e uma consequencia de como
/// ele foi feito, e nao uma promessa escrita. Este teste vira a promessa:
/// se um dia alguem trocar a copia por uma reinsercao linha a linha, a
/// chave sumiria em silencio.
#[test]
fn duplicar_tabela_preserva_a_chave_estrangeira() {
    let guarda = dir_temp("duplicar");
    let s = servidor(&guarda);
    com_fk(&s, r#","ao_alterar":"cascata""#).unwrap();
    pede(
        &s,
        r#""op":"duplicar_tabela","database":"b","tabela":"pedidos",
               "destino":"pedidos_copia""#,
    )
    .unwrap();

    let e = pede(
        &s,
        r#""op":"esquema","database":"b","tabela":"pedidos_copia""#,
    )
    .unwrap();
    let fks = e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(fks.len(), 1, "a copia perdeu a chave: {}", e.escrever());
    assert_eq!(fks[0].texto_ou("nome", ""), "fk_cliente");
    assert_eq!(fks[0].texto_ou("ao_alterar", ""), "Cascata");
}

/// **A chave entra numa tabela que JA existe** -- e o que o editor do
/// diagrama chama quando alguem liga duas colunas com o mouse. O bloco de
/// esquema cresce e mora antes do slot 1, entao o nome comprido forca o
/// caminho caro (reescrever o `.reg`): a linha gravada ANTES tem de
/// continuar legivel DEPOIS.
#[test]
fn declarar_fk_entra_numa_tabela_existente_sem_perder_linha() {
    let guarda = dir_temp("declara-depois");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"cliente_id","tipo":"Int4"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":7,"cliente_id":3}"#,
    )
    .unwrap();

    let r = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
               "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"],
               "ao_alterar":"restringir""#,
    )
    .unwrap();
    // A resposta diz a verdade que a tela precisa repetir -- e esta chave
    // nao mandou "verificar", entao nasce conferida (pedido 227: o
    // "imposta" da resposta tem de bater com o "verificar" do esquema).
    assert_eq!(r.campo("imposta").unwrap().booleano(), Some(true));

    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    let fks = e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(fks.len(), 1, "a chave nao entrou: {}", e.escrever());
    assert_eq!(fks[0].texto_ou("tabela_ref", ""), "clientes");
    assert_eq!(fks[0].texto_ou("ao_alterar", ""), "Restringir");
    assert_eq!(
        fks[0].campo("verificar").and_then(Json::booleano),
        Some(true),
        "o esquema tem de concordar com a resposta do declarar_fk"
    );
    // E o excluir e o da regra, mesmo sem ninguem ter pedido.
    assert_eq!(fks[0].texto_ou("ao_excluir", ""), "Restringir");

    // A linha de antes continua inteira -- declarar e catalogo, nao dado.
    let l = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"pedidos","rowid":1"#,
    )
    .unwrap();
    assert!(
        l.escrever().contains('7'),
        "a linha sumiu: {}",
        l.escrever()
    );

    // Duplicar o nome e recusado -- e a primeira declaracao fica.
    let e2 = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
               "colunas":["cliente_id"],"tabela_ref":"clientes""#,
    )
    .unwrap_err();
    assert!(e2.to_string().contains("ja esta declarada"), "{e2}");
}

/// Pedido 227: o `imposta` da resposta do `declarar_fk` tem de ser o
/// MESMO valor que o `esquema` devolve depois (`verificar`) -- nunca um
/// literal a parte. Antes do conserto havia um `Json::Bool(false)` fixo
/// no lugar deste campo, entao a tela ficava com duas respostas
/// diferentes sobre a mesma chave. Prova nos dois sentidos: a chave que
/// nasce conferida (padrao) responde `imposta:true`, e a que pede
/// `"verificar":false` de proposito responde `imposta:false` -- as duas
/// batendo com o que o `esquema` mostra.
#[test]
fn declarar_fk_responde_imposta_com_o_mesmo_verificar_do_esquema() {
    let guarda = dir_temp("declara-imposta-bate-com-verificar");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"cliente_a","tipo":"Int4"},
                          {"nome":"cliente_b","tipo":"Int4"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();

    // Sem "verificar": nasce conferida -- imposta:true, igual ao esquema.
    let r_padrao = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_a","colunas":["cliente_a"],"tabela_ref":"clientes""#,
    )
    .unwrap();
    assert_eq!(
        r_padrao.campo("imposta").and_then(Json::booleano),
        Some(true)
    );

    // Com "verificar":false explicito -- imposta:false, igual ao esquema.
    let r_solta = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_b","colunas":["cliente_b"],"tabela_ref":"clientes",
               "verificar":false"#,
    )
    .unwrap();
    assert_eq!(
        r_solta.campo("imposta").and_then(Json::booleano),
        Some(false)
    );

    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    let fks = e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap();
    let fk_a = fks
        .iter()
        .find(|f| f.texto_ou("nome", "") == "fk_a")
        .unwrap();
    let fk_b = fks
        .iter()
        .find(|f| f.texto_ou("nome", "") == "fk_b")
        .unwrap();
    assert_eq!(fk_a.campo("verificar").and_then(Json::booleano), Some(true));
    assert_eq!(
        fk_b.campo("verificar").and_then(Json::booleano),
        Some(false)
    );
}

/// O leitor da chave aceita `tabela` como apelido de `tabela_ref` -- mas
/// NESTE pedido `tabela` e a tabela que recebe a declaracao. Sem a recusa,
/// omitir `tabela_ref` viraria uma chave apontando para si mesma, em
/// silencio.
#[test]
fn declarar_fk_sem_tabela_ref_recusa_em_vez_de_apontar_para_si() {
    let guarda = dir_temp("sem-ref");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]"#,
    )
    .unwrap();
    let e = pede(
        &s,
        r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk","colunas":["id"]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("tabela_ref"), "{e}");
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    assert!(
        e.campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()
            .is_empty(),
        "a chave nasceu mesmo recusada"
    );
}

/// `excluir_fk` tira a declaracao e NADA mais: a linha fica, e um nome
/// que nao existe responde com a lista do que existe.
#[test]
fn excluir_fk_tira_a_declaracao_e_nada_mais() {
    let guarda = dir_temp("tira");
    let s = servidor(&guarda);
    com_fk(&s, "").unwrap();
    // Com a chave declarada -- e conferida, que e o padrao -- a gravacao
    // para, porque `clientes` nao existe. Este passo era um `unwrap()` que
    // provava nada; hoje ele e a METADE DE ANTES da prova, e sem ele o
    // teste nao distingue «a remocao funcionou» de «nunca houve nada».
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":2}"#,
    )
    .expect_err("a chave declarada tinha de estar conferindo antes de ser tirada");

    let e = pede(
        &s,
        r#""op":"excluir_fk","database":"b","tabela":"pedidos","nome":"fk_errada""#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("fk_cliente"), "{e}");

    pede(
        &s,
        r#""op":"excluir_fk","database":"b","tabela":"pedidos","nome":"fk_cliente""#,
    )
    .unwrap();
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
    assert!(e
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());
    // E a metade DE DEPOIS: sem a declaracao, a mesma gravacao entra.
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":2}"#,
    )
    .expect("tirada a chave, a gravacao tinha de voltar a entrar");
    let l = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"pedidos","rowid":1"#,
    )
    .unwrap();
    assert!(l.escrever().contains("cliente_id"), "{}", l.escrever());
}

/// O que o `esquema` devolve tem de poder voltar como `criar_tabela`. E o
/// caminho de recriar uma tabela noutro servidor, e uma chave que so sai e
/// nao entra quebraria justamente ele.
#[test]
fn o_que_o_esquema_devolve_volta_como_criar_tabela() {
    let guarda = dir_temp("ida-e-volta");
    let s = servidor(&guarda);
    com_fk(&s, r#","ao_alterar":"restringir","verificar":true"#).unwrap();
    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();

    let mut recriar = vec![
        ("op".to_string(), Json::texto_de("criar_tabela")),
        ("database".to_string(), Json::texto_de("b")),
        ("tabela".to_string(), Json::texto_de("pedidos2")),
        ("token".to_string(), Json::texto_de("t")),
    ];
    // As colunas de sistema saem de fora: elas entram sozinhas.
    let colunas: Vec<Json> = e
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter(|c| c.campo("sistema").and_then(Json::booleano) != Some(true))
        .cloned()
        .collect();
    recriar.push(("colunas".to_string(), Json::Lista(colunas)));
    recriar.push((
        "chaves_estrangeiras".to_string(),
        e.campo("chaves_estrangeiras").cloned().unwrap(),
    ));

    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(&Json::Objeto(recriar).escrever(), &mut ses, "127.0.0.1");
    r.expect("o esquema devolvido nao voltou como pedido");

    let e2 = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos2""#).unwrap();
    let fk = &e2
        .campo("chaves_estrangeiras")
        .and_then(Json::lista)
        .unwrap()[0];
    assert_eq!(fk.texto_ou("nome", ""), "fk_cliente");
    assert_eq!(fk.texto_ou("ao_alterar", ""), "Restringir");
    // O campo que a ida-e-volta perdia em silencio. Sem ele na resposta,
    // recriar a tabela a partir do que o servidor devolveu DESLIGA a
    // conferencia -- e ninguem seria avisado, porque o pedido continua
    // valido e a tabela continua nascendo.
    assert!(
        fk.booleano_ou("verificar", false),
        "a ida-e-volta desligou a conferencia: {}",
        fk.escrever()
    );
}

use super::*;
use crate::usuarios::{Cadastro, Nivel, Permissoes, Usuario};

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("excl-{nome}"))
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

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um banco com uma tabela de tres linhas.
fn com_dados(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let s = servidor(dir, cadastro);
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    for (id, nome) in [(1, "Adriano"), (2, "Maria"), (3, "Joao")] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"id":{id},"nome":"{nome}"}}}}"#
            )),
            &sessao,
        )
        .unwrap();
    }
    s
}

/// O padrao do protocolo e o caminho REVERSIVEL. Um cliente que manda
/// `excluir` sem dizer mais nada nao pode perder o dado.
#[test]
fn excluir_sem_dizer_nada_e_suave() {
    let dir = dir_temp("padrao");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();

    let r = s
        .executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":2,"motivo":"pedido"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.texto_ou("modo", ""), "suave");
    assert!(matches!(r.campo("reversivel"), Some(Json::Bool(true))));

    // Sumiu da varredura...
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 2);

    // ... e a lixeira continua vazia, porque nada foi apagado.
    let lx = s
        .executar(
            "lixeira",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(lx.inteiro_ou("total", -1), 0);

    // E volta.
    let r = s
        .executar(
            "restaurar",
            &pedido(r#"{"database":"b","tabela":"c","rowid":2,"motivo":"engano"}"#),
            &sessao,
        )
        .unwrap();
    assert!(matches!(r.campo("restaurado"), Some(Json::Bool(true))));
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 3);
}

#[test]
fn excluir_fisico_passa_pela_lixeira() {
    let dir = dir_temp("fisico");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();

    let r = s
        .executar(
            "excluir",
            &pedido(
                r#"{"database":"b","tabela":"c","rowid":2,
                        "fisico":true,"motivo":"duplicidade"}"#,
            ),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.texto_ou("modo", ""), "fisico");
    assert!(matches!(r.campo("na_lixeira"), Some(Json::Bool(true))));

    let lx = s
        .executar(
            "lixeira",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(lx.inteiro_ou("total", -1), 1);
    let itens = lx.campo("descartadas").and_then(Json::lista).unwrap();
    assert_eq!(itens[0].inteiro_ou("rowid", -1), 2);
    // A linha vem decodificada, com o esquema da tabela.
    let linha = itens[0].campo("linha").unwrap();
    assert_eq!(linha.texto_ou("nome", ""), "Maria");
    assert_eq!(itens[0].texto_ou("aviso", "x"), "");

    // E o motivo ficou registrado, com a identidade da linha.
    let m = s
        .executar(
            "motivos",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    let regs = m.campo("motivos").and_then(Json::lista).unwrap();
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].texto_ou("tipo", ""), "fisica");
    assert_eq!(regs[0].texto_ou("motivo", ""), "duplicidade");
    assert_eq!(regs[0].texto_ou("identidade", ""), "id=2");
}

/// O defeito que este teste protege: `atualizar` monta a linha inteira a
/// partir do JSON, e a coluna de sistema ausente virava `false`. Uma
/// edicao de rotina RESSUSCITARIA a linha, sem erro e sem aviso.
#[test]
fn atualizar_nao_ressuscita_linha_excluida() {
    let dir = dir_temp("ressuscita");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();

    s.executar(
        "excluir",
        &pedido(r#"{"database":"b","tabela":"c","rowid":2}"#),
        &sessao,
    )
    .unwrap();
    s.executar(
        "atualizar",
        &pedido(r#"{"database":"b","tabela":"c","rowid":2,"linha":{"id":2,"nome":"Outra"}}"#),
        &sessao,
    )
    .unwrap();

    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(
        v.inteiro_ou("devolvidas", -1),
        2,
        "a alteracao ressuscitou a linha excluida"
    );
}

#[test]
fn motivo_obrigatorio_vem_do_esquema() {
    let dir = dir_temp("obrigatorio");
    let s = servidor(&dir, Cadastro::default());
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c","motivo_obrigatorio":true,
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1}}"#),
        &sessao,
    )
    .unwrap();

    let e = s
        .executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(format!("{e}").contains("motivo"), "{e}");

    // A linha continua viva: a recusa veio antes de qualquer gravacao.
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 1);

    // E a tela sabe que a tabela exige, para pedir antes de mandar.
    let m = s
        .executar(
            "motivos",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert!(matches!(
        m.campo("motivo_obrigatorio"),
        Some(Json::Bool(true))
    ));
}

/// Esvaziar apaga sem volta -- e por isso exige a frase escrita, mesmo
/// numa tabela que nao exige motivo para excluir.
#[test]
fn esvaziar_exige_motivo_e_registra_antes_de_apagar() {
    let dir = dir_temp("esvaziar");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();
    s.executar(
        "excluir",
        &pedido(r#"{"database":"b","tabela":"c","rowid":1,"fisico":true}"#),
        &sessao,
    )
    .unwrap();

    let e = s
        .executar(
            "esvaziar_lixeira",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(format!("{e}").contains("motivo"), "{e}");

    let r = s
        .executar(
            "esvaziar_lixeira",
            &pedido(r#"{"database":"b","tabela":"c","motivo":"limpeza anual"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("apagadas", -1), 1);

    let lx = s
        .executar(
            "lixeira",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(lx.inteiro_ou("total", -1), 0);

    // O dado foi; o rastro de que foi, nao.
    let m = s
        .executar(
            "motivos",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    let regs = m.campo("motivos").and_then(Json::lista).unwrap();
    assert!(regs.iter().any(|r| r.texto_ou("tipo", "") == "expurgo"));
}

/// O portao de permissao: quem so le e escreve nao ve a lixeira nem os
/// motivos. E o requisito de "somente o administrador visualiza".
#[test]
fn lixeira_e_motivos_exigem_administrar() {
    for op in ["lixeira", "trash", "motivos", "reasons", "esvaziar_lixeira"] {
        assert_eq!(
            Atividade::da_operacao(op),
            Some(Atividade::Administrar),
            "{op:?} nao exige administrar"
        );
    }
    // Excluir e restaurar continuam no poder de excluir, que e o certo:
    // quem pode tirar da lista pode devolver.
    assert_eq!(Atividade::da_operacao("excluir"), Some(Atividade::Excluir));
    assert_eq!(
        Atividade::da_operacao("restaurar"),
        Some(Atividade::Excluir)
    );
}

/// A prova da paginacao por cursor: pedir pagina a pagina reconstroi
/// exatamente a tabela, sem repetir nem pular -- inclusive por cima dos
/// buracos que a exclusao deixa.
#[test]
fn o_cursor_reconstroi_a_tabela_inteira() {
    let dir = dir_temp("cursor");
    let s = servidor(&dir, Cadastro::default());
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    for id in 1..=25 {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"id":{id}}}}}"#
            )),
            &sessao,
        )
        .unwrap();
    }
    // Dois buracos: um marcado, um apagado de vez.
    s.executar(
        "excluir",
        &pedido(r#"{"database":"b","tabela":"c","rowid":7}"#),
        &sessao,
    )
    .unwrap();
    s.executar(
        "excluir",
        &pedido(r#"{"database":"b","tabela":"c","rowid":13,"fisico":true}"#),
        &sessao,
    )
    .unwrap();

    let mut vistos: Vec<i64> = Vec::new();
    let mut cursor = 0i64;
    let mut paginas = 0;
    loop {
        let r = s
            .executar(
                "varrer",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","max":7,"depois":{cursor}}}"#
                )),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.texto_ou("modo", ""), "cursor");
        let linhas: Vec<Json> = r.campo("linhas").and_then(Json::lista).unwrap().to_vec();
        if linhas.is_empty() {
            assert!(
                !matches!(r.campo("ha_mais"), Some(Json::Bool(true))),
                "disse que ha mais e devolveu vazio"
            );
            break;
        }
        paginas += 1;
        assert!(paginas < 20, "nao terminou -- o cursor nao anda");
        for l in &linhas {
            vistos.push(l.inteiro_ou("id", -1));
        }
        cursor = r.inteiro_ou("cursor_fim", 0);
    }

    let esperado: Vec<i64> = (1..=25).filter(|i| *i != 7 && *i != 13).collect();
    assert_eq!(vistos, esperado, "o cursor pulou ou repetiu linha");
    assert_eq!(paginas, 4, "23 linhas em paginas de 7 dao 4 paginas");
}

/// `registros` sai do cabecalho e nao de varredura: e o numero que a tela
/// mostra sem pagar por ele.
#[test]
fn varrer_nao_conta_a_tabela_para_responder() {
    let dir = dir_temp("sem-contar");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();
    let r = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c","max":2}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2);
    assert_eq!(r.inteiro_ou("registros", -1), 3);
    assert!(matches!(r.campo("ha_mais"), Some(Json::Bool(true))));
    assert!(matches!(r.campo("ha_antes"), Some(Json::Bool(false))));

    // E a pagina de tras devolve o que veio antes, em ordem crescente.
    let fim = r.inteiro_ou("cursor_fim", 0);
    let atras = s
        .executar(
            "varrer",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","max":5,"antes":{fim}}}"#
            )),
            &sessao,
        )
        .unwrap();
    let ids: Vec<i64> = atras
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    assert_eq!(ids, vec![1]);
}

/// O `rownum` chega na resposta e cresce com a ordem de digitacao.
#[test]
fn a_resposta_traz_o_numero_de_ordem() {
    let dir = dir_temp("rownum");
    let s = com_dados(&dir, Cadastro::default());
    let sessao = Sessao::default();
    let r = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    let nums: Vec<i64> = r
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.inteiro_ou("rownum", -1))
        .collect();
    assert_eq!(nums, vec![1, 2, 3]);
}

/// E o portao de verdade, com um usuario que tem tudo menos administrar.
#[test]
fn operador_sem_administrar_e_recusado_na_lixeira() {
    let dir = dir_temp("portao");
    let mut cadastro = Cadastro::default();
    let permissoes = Permissoes {
        ler: true,
        inserir: true,
        alterar: true,
        excluir: true,
        administrar: false,
        ..Permissoes::default()
    };
    cadastro.usuarios.push(Usuario {
        id: 7,
        nome: "Operador".into(),
        login: "op".into(),
        senha_hash: String::new(),
        email: String::new(),
        telefone: String::new(),
        supervisor: false,
        ativo: true,
        nivel: Nivel::Nenhum,
        chave_publica: None,
        bases: vec![("*".into(), permissoes)],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    });
    let usuario = cadastro.usuarios[0].clone();
    let s = com_dados(&dir, cadastro);

    let mut sessao = Sessao {
        usuario: Some(usuario),
        ..Sessao::default()
    };
    // Pelo `despachar`, que e por onde o pedido entra de verdade: e ali
    // que mora o portao de permissao, e nao no `executar`.
    let (_, _, r) = s.despachar(
        r#"{"op":"lixeira","token":"t","database":"b","tabela":"c"}"#,
        &mut sessao,
        "1.2.3.4",
    );
    let e = r.unwrap_err();
    assert!(
        format!("{e}").contains("administrar"),
        "o operador entrou na lixeira: {e}"
    );

    let (_, _, r) = s.despachar(
        r#"{"op":"motivos","token":"t","database":"b","tabela":"c"}"#,
        &mut sessao,
        "1.2.3.4",
    );
    assert!(r.is_err(), "o operador leu os motivos");

    // Mas excluir ele pode.
    let (_, _, r) = s.despachar(
        r#"{"op":"excluir","token":"t","database":"b","tabela":"c","rowid":1}"#,
        &mut sessao,
        "1.2.3.4",
    );
    assert!(r.is_ok(), "o operador nao conseguiu excluir: {r:?}");
}

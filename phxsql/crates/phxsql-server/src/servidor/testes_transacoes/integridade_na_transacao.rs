//! **Pedidos 491, 492, 515 e 516: a integridade DENTRO da transacao.**
//! Cada prova confere o DISCO depois do COMMIT (ou da recusa), e a que cabe
//! dos dois lados roda fora e dentro de transacao: a mesma sequencia tem
//! de dar o mesmo resultado nos dois.
use super::*;

fn escreve(s: &Arc<Servidor>, ses: &Sessao, corpo: &str) -> Json {
    pede(s, ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"))
}

/// A linha `rowid` de `tabela` com a coluna de sistema, ou `Nulo`.
fn linha(s: &Arc<Servidor>, ses: &Sessao, tabela: &str, rowid: u64) -> Json {
    escreve(
        s,
        ses,
        &format!(r#""op":"ler","database":"loja","tabela":"{tabela}","rowid":{rowid}"#),
    )
}

fn excluida(l: &Json) -> bool {
    l.booleano_ou(phxsql_core::schema::COLUNA_SOFTDELETED, false)
}

fn confirma(s: &Arc<Servidor>, ses: &Sessao) -> String {
    let r = pede(s, ses, r#""op":"commit""#);
    let veredito = match &r {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    };
    let r = r.unwrap_or_else(|_| panic!("lista valida recusada: {veredito}"));
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{veredito}"
    );
    veredito
}

// ------------------------------------------------------------- 491

/// `funcionarios(id, chefe_id, nome)`, `chefe_id -> funcionarios.id`
/// conferida, com indice dos dois lados.
fn hierarquia(s: &Arc<Servidor>, ses: &Sessao) {
    escreve(s, ses, r#""op":"criar_database","database":"loja""#);
    escreve(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"funcionarios",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"chefe_id","tipo":"Int8"},
                              {"nome":"nome","tipo":"Str(20)"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_chefe","colunas":["chefe_id"]}],
                   "chaves_estrangeiras":[{"nome":"fk_chefe","colunas":["chefe_id"],
                                           "tabela_ref":"funcionarios","colunas_ref":["id"]}]"#,
    );
}

fn funcionario(s: &Arc<Servidor>, ses: &Sessao, id: i64, chefe: Option<i64>) -> u64 {
    let chefe = chefe.map_or("null".to_string(), |c| c.to_string());
    let r = escreve(
        s,
        ses,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"funcionarios",
                       "linha":{{"id":{id},"chefe_id":{chefe},"nome":"f{id}"}}"#
        ),
    );
    r.inteiro_ou("rowid", -1) as u64
}

fn exclui(tabela: &str, rowid: u64, fisico: bool) -> String {
    format!(
        r#""op":"excluir","database":"loja","tabela":"{tabela}","rowid":{rowid},"fisico":{fisico}"#
    )
}

/// **Pedido 491, fora de transacao:** excluir o chefe que tem
/// subordinado -- de vez e suave -- respondia `Ok`, e o subordinado
/// ficava apontando para ninguem. E os dois lados que nao mudam: a linha
/// que aponta so para si mesma sai, e o chefe sai depois do subordinado.
#[test]
fn fora_da_transacao_o_chefe_com_subordinado_nao_sai() {
    let dir = dir_temp("491-fora");
    let s = servidor(&dir);
    let ses = sessao(4911);
    hierarquia(&s, &ses);
    let chefe = funcionario(&s, &ses, 1, None);
    let sub = funcionario(&s, &ses, 2, Some(1));
    for fisico in [true, false] {
        match pede(&s, &ses, &exclui("funcionarios", chefe, fisico)) {
            Err(e) => assert!(
                e.nome() == "INTEGRIDADE" && e.to_string().contains("fk_chefe"),
                "fisico={fisico}: {e}"
            ),
            Ok(j) => panic!(
                "fisico={fisico}: o chefe com subordinado saiu e o subordinado \
                         ficou orfao: {}",
                j.escrever()
            ),
        }
        let l = linha(&s, &ses, "funcionarios", chefe);
        assert!(
            !matches!(l, Json::Nulo) && !excluida(&l),
            "fisico={fisico}: o chefe mudou apesar da recusa: {}",
            l.escrever()
        );
    }
    let dono = funcionario(&s, &ses, 3, None);
    escreve(
        &s,
        &ses,
        &format!(
            r#""op":"atualizar","database":"loja","tabela":"funcionarios","rowid":{dono},
                       "linha":{{"id":3,"chefe_id":3,"nome":"f3"}}"#
        ),
    );
    escreve(&s, &ses, &exclui("funcionarios", dono, true));
    escreve(&s, &ses, &exclui("funcionarios", sub, true));
    escreve(&s, &ses, &exclui("funcionarios", chefe, true));
    assert_eq!(quantas(&s, &ses, "funcionarios"), 0);
}

/// **Pedido 491, dentro:** `[inserir 11->10, excluir 10]` confirmava
/// com o 11 apontando para um chefe excluido. Suave e de vez recusam no
/// COMMIT, antes da marca e com zero gravado.
#[test]
fn na_transacao_o_subordinado_novo_segura_o_chefe() {
    let dir = dir_temp("491-dentro");
    let s = servidor(&dir);
    let ses = sessao(4912);
    hierarquia(&s, &ses);
    let chefe = funcionario(&s, &ses, 10, None);
    for fisico in [false, true] {
        let antes = (
            quantas(&s, &ses, "funcionarios"),
            slots(&s, &ses, "funcionarios"),
        );
        escreve(&s, &ses, r#""op":"begin""#);
        funcionario(&s, &ses, 11, Some(10));
        escreve(&s, &ses, &exclui("funcionarios", chefe, fisico));
        let r = pede(&s, &ses, r#""op":"commit""#);
        let depois = (
            quantas(&s, &ses, "funcionarios"),
            slots(&s, &ses, "funcionarios"),
        );
        match r {
            Err(e) => {
                let texto = e.to_string();
                assert_eq!(e.nome(), "INTEGRIDADE", "fisico={fisico}: {texto}");
                assert!(
                    texto.contains("fk_chefe") && texto.contains("ANTES da marca"),
                    "fisico={fisico}: {texto}"
                );
                assert_eq!(depois, antes, "fisico={fisico}: (linhas, slots): {texto}");
            }
            Ok(j) => panic!(
                "fisico={fisico}: o COMMIT confirmou o subordinado de um chefe \
                         excluido: {} -- (linhas, slots) antes {antes:?}, depois {depois:?}",
                j.escrever()
            ),
        }
        let l = linha(&s, &ses, "funcionarios", chefe);
        assert!(!excluida(&l), "fisico={fisico}: {}", l.escrever());
    }
}

// ------------------------------------------------------------- 492

/// **Pedido 492:** `[excluir suave M, atualizar M.nome]`. Fora de
/// transacao M continua excluida; dentro, o `empilhar` copiava a marca
/// do DISCO para a linha empilhada, e o COMMIT ressuscitava M. A mesma
/// sequencia tem de dar o mesmo resultado -- e tambem com a linha que
/// nasceu na propria transacao, e com o upsert que virou `atualizar`
/// (o irmao que copia a mesma marca, pelas mesmas funcoes).
#[test]
fn alterar_a_linha_excluida_nao_a_ressuscita_fora_nem_dentro() {
    let dir = dir_temp("492");
    let s = servidor(&dir);
    let ses = sessao(4921);
    base(&s, &ses);
    let cli = |id: i64, nome: &str| {
        format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{id},"nome":"{nome}"}}"#
        )
    };
    let altera = |r: u64, nome: &str| {
        format!(
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{r},
                       "linha":{{"id":{r},"nome":"{nome}"}}"#
        )
    };
    let upsert = |r: u64, nome: &str| {
        format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{r},"nome":"{nome}"}},"se_existir":"atualizar""#
        )
    };
    // Fora: a referencia, pelo `atualizar` e pelo upsert.
    escreve(&s, &ses, &cli(1, "Ana"));
    escreve(&s, &ses, &exclui("clientes", 1, false));
    escreve(&s, &ses, &altera(1, "Ana II"));
    escreve(&s, &ses, &cli(2, "Bia"));
    escreve(&s, &ses, &exclui("clientes", 2, false));
    escreve(&s, &ses, &upsert(2, "Bia II"));
    // Dentro, a linha do disco.
    escreve(&s, &ses, &cli(3, "Cid"));
    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &exclui("clientes", 3, false));
    escreve(&s, &ses, &altera(3, "Cid II"));
    confirma(&s, &ses);
    // Dentro, a linha que nasceu na propria transacao.
    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &cli(4, "Dan"));
    escreve(&s, &ses, &exclui("clientes", 4, false));
    escreve(&s, &ses, &altera(4, "Dan II"));
    confirma(&s, &ses);
    // Dentro, o upsert que vira `atualizar`.
    escreve(&s, &ses, &cli(5, "Eva"));
    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &exclui("clientes", 5, false));
    escreve(&s, &ses, &upsert(5, "Eva II"));
    confirma(&s, &ses);
    // Dentro, o upsert com SET: a mescla parte da linha ATUAL.
    escreve(&s, &ses, &cli(6, "Fabi"));
    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &exclui("clientes", 6, false));
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes",
                   "linha":{"id":6,"nome":"nunca"},"se_existir":"atualizar",
                   "atualizar":{"nome":"Fabi II"}"#,
    );
    confirma(&s, &ses);

    let visto: Vec<(String, bool)> = (1..=6)
        .map(|r| {
            let l = linha(&s, &ses, "clientes", r);
            (l.texto_ou("nome", "?").to_string(), excluida(&l))
        })
        .collect();
    let esperado: Vec<(String, bool)> =
        ["Ana II", "Bia II", "Cid II", "Dan II", "Eva II", "Fabi II"]
            .iter()
            .map(|n| (n.to_string(), true))
            .collect();
    assert_eq!(
        visto, esperado,
        "(nome, excluida) de cada linha: alterada, e continua excluida"
    );
}

// ------------------------------------------------------------- 515

/// `clientes(id, codigo, nome)` com `codigo` unico, referenciado por
/// `pedidos.cod_cliente` com cascata no `ao_alterar` -- a chave que a
/// mae muda sem mudar de `id`.
fn base_codigo(s: &Arc<Servidor>, ses: &Sessao) {
    base_codigo_com(s, ses, "");
}

/// A mesma base, com `extra` no fim do `criar_tabela` da mae -- o
/// indice de texto do irmao do C1 do 540, sem uma segunda copia da
/// base inteira.
fn base_codigo_com(s: &Arc<Servidor>, ses: &Sessao, extra: &str) {
    escreve(s, ses, r#""op":"criar_database","database":"loja""#);
    let mae = String::from(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"codigo","tipo":"Int8"},
                              {"nome":"nome","tipo":"Str(20)"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_codigo","colunas":["codigo"],"unico":true}]"#,
    ) + extra;
    escreve(s, ses, &mae);
    escreve(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cod_cliente","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cliente","colunas":["cod_cliente"]}],
                   "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cod_cliente"],
                                           "tabela_ref":"clientes","colunas_ref":["codigo"]}]"#,
    );
}

fn cliente(id: i64, codigo: i64) -> String {
    format!(
        r#""op":"inserir","database":"loja","tabela":"clientes",
                   "linha":{{"id":{id},"codigo":{codigo},"nome":"c{id}"}}"#
    )
}

fn muda_cliente(rowid: u64, id: i64, codigo: i64) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{rowid},
                   "linha":{{"id":{id},"codigo":{codigo},"nome":"c{id}"}}"#
    )
}

fn pedido(id: i64, codigo: i64) -> String {
    format!(
        r#""op":"inserir","database":"loja","tabela":"pedidos",
                   "linha":{{"id":{id},"cod_cliente":{codigo}}}"#
    )
}

fn muda_pedido(rowid: u64, id: i64, codigo: i64) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":{rowid},
                   "linha":{{"id":{id},"cod_cliente":{codigo}}}"#
    )
}

/// **Pedido 515 (N2 da segunda revisao do 448):** o elo que o
/// `empilhar` planejava pelo DISCO sobrescrevia o que a propria lista
/// ja tinha escrito na filha. As tres listas medidas pelo papel C, com
/// o que o PostgreSQL da ao lado:
///
/// | lista | PG | antes |
/// |---|---|---|
/// | `[filha id 10->11, mae 5->6]` | id 11, cod 6 | id 10 |
/// | `[filha troca de mae 15->8, mae 15->16]` | cod 8 | cod 16 |
/// | `[excluir suave a filha, mae 25->26]` | excluida | ressuscita |
#[test]
fn o_elo_da_cascata_nao_desfaz_o_que_a_lista_escreveu_na_filha() {
    let dir = dir_temp("515");
    let s = servidor(&dir);
    let ses = sessao(5151);
    base_codigo(&s, &ses);
    for (id, codigo) in [(1, 5), (2, 15), (3, 25), (4, 8)] {
        escreve(&s, &ses, &cliente(id, codigo));
    }
    for (id, codigo) in [(10, 5), (20, 15), (30, 25)] {
        escreve(&s, &ses, &pedido(id, codigo));
    }

    // O que a transacao VE de cada filha, lido ANTES do COMMIT: o
    // elo que o `empilhar` planejou so pelo disco ja esta na lista, e
    // a leitura dentro da transacao o mostra desfazendo o que a lista
    // escreveu. O COMMIT refaz o elo (pedido 537) sobre a linha
    // atual e esconde o defeito -- medido em 06/10: com o plano do
    // `empilhar` reposto so-pelo-disco, a conferencia de depois do
    // COMMIT passava e a leitura de dentro da transacao caia.
    let ver = |rowid: u64| -> (i64, i64, bool) {
        let l = linha(&s, &ses, "pedidos", rowid);
        (
            l.inteiro_ou("id", -1),
            l.inteiro_ou("cod_cliente", -1),
            excluida(&l),
        )
    };

    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &muda_pedido(1, 11, 5));
    escreve(&s, &ses, &muda_cliente(1, 1, 6));
    assert_eq!(ver(1), (11, 6, false), "dentro da transacao, lista 1");
    confirma(&s, &ses);

    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &muda_pedido(2, 20, 8));
    escreve(&s, &ses, &muda_cliente(2, 2, 16));
    assert_eq!(ver(2), (20, 8, false), "dentro da transacao, lista 2");
    confirma(&s, &ses);

    escreve(&s, &ses, r#""op":"begin""#);
    escreve(&s, &ses, &exclui("pedidos", 3, false));
    escreve(&s, &ses, &muda_cliente(3, 3, 26));
    assert_eq!(ver(3), (30, 26, true), "dentro da transacao, lista 3");
    confirma(&s, &ses);

    let visto: Vec<(i64, i64, bool)> = (1..=3)
        .map(|r| {
            let l = linha(&s, &ses, "pedidos", r);
            (
                l.inteiro_ou("id", -1),
                l.inteiro_ou("cod_cliente", -1),
                excluida(&l),
            )
        })
        .collect();
    assert_eq!(
        visto,
        vec![(11, 6, false), (20, 8, false), (30, 26, true)],
        "(id, cod_cliente, excluida) de cada filha"
    );
}

// ---------------------------------------------------------- 540, C1

/// O laco da sincronia do DbLink sobre `clientes`, sem o motor de la:
/// `aplicar_para_ca`, uma closure igual a do `op_dblink_sincronizar`
/// (o `alterar_solto` e o aviso guardado) e o `t.sincronizar()` do
/// fim. O fio nao entra: ele so traz as linhas, e nao toca no punho.
/// A linha nova (id 2, codigo 7) vem PRIMEIRO e suja o `t`; depois a
/// mae 1 muda de 5 para 6. Devolve os avisos de recuperacao.
fn sincronizar_pelo_mesmo_punho(
    s: &Arc<Servidor>,
    ses: &Sessao,
    nome_da_nova: &str,
    nome_da_mae: &str,
) -> Vec<String> {
    let mut trava = s.travar_dados().unwrap();
    let ped = pedido_da_tabela("loja", "clientes");
    let mut t = s.abrir_travada(&trava, &ped, ses).unwrap();
    let mut avisos: Vec<String> = Vec::new();
    let mut alterar = |t: &mut Table, rowid: u64, nova: &[Value]| {
        let feita = s.alterar_solto(&mut trava, t, &ped, ses, rowid, nova)?;
        avisos.extend(feita.aviso);
        Ok(())
    };
    let linhas = vec![
        vec![
            Value::Int(2),
            Value::Int(7),
            Value::Str(nome_da_nova.into()),
        ],
        vec![Value::Int(1), Value::Int(6), Value::Str(nome_da_mae.into())],
    ];
    let (inseridas, alteradas) =
        crate::dblink::sincronia::aplicar_para_ca(&mut t, "pk", 0, &linhas, &mut alterar).unwrap();
    assert_eq!((inseridas, alteradas), (1, 1));
    t.sincronizar().unwrap();
    avisos
}

/// **Pedido 540, C1 do papel C: o terceiro irmao, a sincronia do
/// DbLink.** Ela grava pelo punho `t` -- a linha nova da rodada entra
/// ANTES -- e, no MESMO punho, altera a mae que tem filha. Pela
/// `op_atualizar` e pelo upsert o `t` chega limpo ao `alterar_solto`;
/// por aqui ele chega com pagina suja. O laco e o
/// [`sincronizar_pelo_mesmo_punho`].
///
/// # Prova real
///
/// Sem a descida antes da marca, o punho da passada acha o byte 52 em
/// 1 sem atestado, a recuperacao reindexa a mae pelo `.reg` e o `Drop`
/// do `t` velho grava a arvore VELHA por cima: `buscar por_codigo 6`
/// da 0 -- o vermelho medido, 5 de 5 aqui (com «indices reconstruidos
/// 1» no bloco da recuperacao) e 5 de 5 na sonda do papel C, que viu
/// tambem o codigo 6 repetido e a orfa no 5 entrarem.
#[test]
fn a_cascata_solta_depois_de_escrever_no_mesmo_punho_nao_perde_o_indice_da_mae() {
    let dir = dir_temp("540-c1");
    let s = servidor(&dir);
    let ses = sessao(5401);
    base_codigo(&s, &ses);
    escreve(&s, &ses, &cliente(1, 5));
    for id in [10, 11] {
        escreve(&s, &ses, &pedido(id, 5));
    }

    let avisos = sincronizar_pelo_mesmo_punho(&s, &ses, "c2", "c1");

    // O DANO primeiro: o indice unico da mae, e o que ele deixa entrar.
    let achados = |tabela: &str, indice: &str, chave: i64| {
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"buscar","database":"loja","tabela":"{tabela}",
                           "indice":"{indice}","chave":[{chave}]"#
            ),
        )
        .inteiro_ou("encontrados", -1)
    };
    for (codigo, esperados) in [(6, 1), (5, 0), (7, 1)] {
        assert_eq!(
            achados("clientes", "por_codigo", codigo),
            esperados,
            "o indice da mae pela chave {codigo} depois da sincronia"
        );
    }
    let repetido = pede(&s, &ses, &cliente(3, 6));
    assert_eq!(
        repetido.as_ref().err().map(PhxError::nome),
        Some("DUPLICADO"),
        "o codigo 6 e unico e ja e do cliente 1, e o servidor aceitou outro: {:?}",
        repetido.map(|j| j.escrever())
    );
    let orfa = pede(&s, &ses, &pedido(12, 5));
    assert_eq!(
        orfa.as_ref().err().map(PhxError::nome),
        Some("INTEGRIDADE"),
        "o cliente 5 virou 6, e o pedido no 5 entrou orfao: {:?}",
        orfa.map(|j| j.escrever())
    );
    // A cascata inteira, pelo indice da filha.
    for (codigo, esperados) in [(5, 0), (6, 2)] {
        assert_eq!(
            achados("pedidos", "por_cliente", codigo),
            esperados,
            "o indice da filha pela chave {codigo} depois da sincronia"
        );
    }
    // E o mecanismo por ultimo: a passada nao quebrou, entao nao ha
    // aviso de recuperacao nenhum.
    assert!(
        avisos.is_empty(),
        "a cascata da sincronia passou pela recuperacao: {avisos:?}"
    );
}

/// **O irmao do C1 no `.fts` (R1 da re-checagem do papel C).** A
/// descida do `t` leva o `.ndx` E o `.fts`; o teste de cima nao tem
/// indice de texto, e tirar so a metade do `.fts` passava por ele
/// verde. Aqui a mae tem indice de texto no `nome`, e a sincronia
/// troca a chave e o nome dela (`alfa` -> `zeta`) depois de inserir
/// `nome7` pelo mesmo punho.
///
/// # Prova real
///
/// Com a descida levando so o `.ndx`, o texto sai errado e calado --
/// o `por_codigo` certo e `procurar_texto` achando o nome VELHO:
/// `zeta` 0 e `alfa` 1, 5 de 5 na sonda do papel C e o vermelho
/// medido aqui.
#[test]
fn a_cascata_solta_depois_de_escrever_no_mesmo_punho_nao_perde_o_texto_da_mae() {
    let dir = dir_temp("540-c1-fts");
    let s = servidor(&dir);
    let ses = sessao(5402);
    base_codigo_com(
        &s,
        &ses,
        r#","indices_texto":[{"nome":"txt","coluna":"nome"}]"#,
    );
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes",
                   "linha":{"id":1,"codigo":5,"nome":"alfa"}"#,
    );
    for id in [10, 11] {
        escreve(&s, &ses, &pedido(id, 5));
    }

    let avisos = sincronizar_pelo_mesmo_punho(&s, &ses, "nome7", "zeta");

    let achados = |palavra: &str| {
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"procurar_texto","database":"loja","tabela":"clientes",
                           "indice":"txt","palavra":"{palavra}""#
            ),
        )
        .inteiro_ou("encontrados", -1)
    };
    for (palavra, esperados) in [("zeta", 1), ("alfa", 0), ("nome7", 1)] {
        assert_eq!(
            achados(palavra),
            esperados,
            "o indice de texto da mae pela palavra {palavra:?} depois da sincronia"
        );
    }
    assert!(
        avisos.is_empty(),
        "a cascata da sincronia passou pela recuperacao: {avisos:?}"
    );
}

// ------------------------------------------------------------- 516

/// **Pedido 516 (N3 da segunda revisao do 448):** T1 muda a chave da
/// mae sem filha nenhuma; depois nasce a filha, por outra sessao; T3
/// abre com leitura repetivel e le a filha. O COMMIT de T1 leva a
/// cascata IMPLICITA -- o elo que so a pre-conferencia descobre -- e
/// ele passava por cima da trava compartilhada de T3: T3 relia 6 onde
/// tinha lido 5. O elo implicito toma a trava como qualquer escrita
/// da transacao, e o COMMIT nao espera com a trava de dados na mao:
/// recusa com nada gravado, a transacao continua ativa, e o COMMIT
/// seguinte, depois de T3 terminar, confirma.
#[test]
fn o_elo_implicito_respeita_a_leitura_repetivel_de_outra_transacao() {
    let dir = dir_temp("516");
    let s = servidor(&dir);
    let t1 = sessao(5161);
    let outra = sessao(5162);
    let t3 = sessao(5163);
    base_codigo(&s, &t1);
    escreve(&s, &t1, &cliente(1, 5));

    escreve(&s, &t1, r#""op":"begin","database":"loja""#);
    let r = escreve(&s, &t1, &muda_cliente(1, 1, 6));
    assert_eq!(
        r.inteiro_ou("linhas", 0),
        1,
        "sem filha no empilhar, a lista nao leva elo: {}",
        r.escrever()
    );
    escreve(&s, &outra, &pedido(10, 5));
    escreve(
        &s,
        &t3,
        r#""op":"begin","database":"loja","leitura_repetivel":true"#,
    );
    let cod = |ses: &Sessao| linha(&s, ses, "pedidos", 1).inteiro_ou("cod_cliente", -1);
    assert_eq!(cod(&t3), 5, "a primeira leitura de T3");

    let r = pede(&s, &t1, r#""op":"commit""#);
    let veredito = match &r {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    };
    assert_eq!(
        cod(&t3),
        5,
        "a leitura repetivel de T3 releu outro valor: o COMMIT de T1 levou o \
                 elo implicito por cima da trava dela -- {veredito}"
    );
    let e = r.expect_err("o COMMIT de T1 nao podia confirmar com T3 lendo a filha");
    assert!(
        matches!(e, PhxError::EmTransacao(_)) && veredito.contains("pedidos"),
        "a recusa tem de ser de trava, nomeando a tabela do elo: {veredito}"
    );
    assert!(
        e.adianta_repetir(),
        "nada foi gravado: repetir o COMMIT tem de adiantar -- {veredito}"
    );

    escreve(&s, &t3, r#""op":"commit""#);
    confirma(&s, &t1);
    assert_eq!(cod(&outra), 6, "a cascata implicita nao acompanhou a mae");
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// **C1 do papel C ao 516: dois COMMITs que se barram nao giram para
/// sempre.** T1 muda a mae `a`, T2 a mae `b`; outra sessao poe a
/// filha `c -> a` e a `d -> b`; T1 altera `d`, T2 altera `c`. O elo
/// que o COMMIT de cada um descobre esta travado pelo outro. Sem o
/// desempate os dois eram mandados repetir, e repetiam: 1.870 rodadas
/// em 11 s na sonda do DBA. Com ele a MAIS NOVA cede (`repetir:
/// false`), qualquer que seja a que descobre o ciclo, e a mais velha
/// confirma -- nas duas ordens.
///
/// # Prova real
///
/// Sem o desempate (`ceder` sempre falso), as 20 rodadas terminam com
/// os dois ainda em `EM_TRANSACAO` -- o vermelho medido.
#[test]
fn dois_commits_que_se_barram_cedem_pela_mais_nova() {
    for (rodada_da_ordem, mais_nova_primeiro) in [(1, false), (2, true)] {
        let dir = dir_temp(&format!("516-ciclo-{rodada_da_ordem}"));
        let s = servidor(&dir);
        let velha = sessao(5171);
        let nova = sessao(5172);
        let t0 = sessao(5173);
        base_codigo(&s, &t0);
        escreve(&s, &t0, &cliente(1, 1));
        escreve(&s, &t0, &cliente(2, 2));
        escreve(&s, &velha, r#""op":"begin""#);
        escreve(&s, &velha, &muda_cliente(1, 1, 10));
        escreve(&s, &nova, r#""op":"begin""#);
        escreve(&s, &nova, &muda_cliente(2, 2, 20));
        escreve(&s, &t0, &pedido(100, 1));
        escreve(&s, &t0, &pedido(200, 2));
        escreve(&s, &velha, &muda_pedido(2, 201, 2));
        escreve(&s, &nova, &muda_pedido(1, 101, 1));

        let ordem: [(&str, &Sessao); 2] = if mais_nova_primeiro {
            [("nova", &nova), ("velha", &velha)]
        } else {
            [("velha", &velha), ("nova", &nova)]
        };
        let mut fim: HashMap<&str, std::result::Result<Json, PhxError>> = HashMap::new();
        let mut rodadas = 0;
        while fim.len() < 2 && rodadas < 20 {
            rodadas += 1;
            for (nome, ses) in ordem {
                if fim.contains_key(nome) {
                    continue;
                }
                match pede(&s, ses, r#""op":"commit""#) {
                    Err(PhxError::EmTransacao(_)) => {}
                    outro => {
                        fim.insert(nome, outro);
                    }
                }
            }
        }
        assert_eq!(
            fim.len(),
            2,
            "ordem {rodada_da_ordem}: depois de {rodadas} rodadas os dois COMMITs \
                     continuam se barrando, mandados repetir -- {fim:?}"
        );
        let v = fim.remove("velha").unwrap();
        assert_eq!(
            v.as_ref()
                .map(|j| j.texto_ou("transaction_state", "").to_string())
                .ok(),
            Some("COMMITTED".to_string()),
            "ordem {rodada_da_ordem}: a mais velha tinha de confirmar: {v:?}"
        );
        let e = fim
            .remove("nova")
            .unwrap()
            .expect_err("a mais nova tinha de ceder");
        assert!(
            matches!(e, PhxError::TransacaoAbortada(_))
                && !e.adianta_repetir()
                && e.to_string().contains("ciclo"),
            "ordem {rodada_da_ordem}: a mais nova cede sem mandar repetir, dizendo o \
                     ciclo: {e}"
        );
        assert!(rodadas <= 3, "ordem {rodada_da_ordem}: {rodadas} rodadas");
        escreve(&s, &nova, r#""op":"rollback""#);
        // O dado: a da mais velha inteira, cascata junto; a da mais nova,
        // nada.
        let cods = |r: u64| linha(&s, &t0, "pedidos", r).inteiro_ou("cod_cliente", -1);
        let ids = |r: u64| linha(&s, &t0, "pedidos", r).inteiro_ou("id", -1);
        assert_eq!(
            ((ids(1), cods(1)), (ids(2), cods(2))),
            ((100, 10), (201, 2)),
            "ordem {rodada_da_ordem}: (id, cod) das filhas"
        );
        assert_eq!(s.travas.travar().quantas(), 0);
    }
}

/// **R1 da re-checagem do papel C: a aresta velha nao faz ninguem
/// ceder.** A velha e barrada por T2 no COMMIT (o elo de `a` esbarra
/// na filha que T2 travou), volta ao SAVEPOINT -- a alteracao de `a`
/// sai da lista -- e passa a escrever so a outra filha. O COMMIT de
/// T2, barrado pela velha nessa filha, NAO e ciclo: a velha nao
/// precisa mais de nada de T2. Com a aresta velha, T2 cedia
/// («ciclo com T1») e a velha confirmava logo depois sem ela --
/// aborto sem ciclo. Sem ela, T2 espera, e as duas confirmam.
///
/// # Prova real
///
/// Tirar a limpeza da aresta do `rollback_para` faz T2 receber
/// `TRANSACAO_ABORTADA` aqui -- o vermelho medido.
#[test]
fn voltar_ao_savepoint_apaga_a_aresta_do_commit_barrado() {
    let dir = dir_temp("516-aresta-velha");
    let s = servidor(&dir);
    let velha = sessao(5181);
    let nova = sessao(5182);
    let t0 = sessao(5183);
    base_codigo(&s, &t0);
    escreve(&s, &t0, &cliente(1, 1));
    escreve(&s, &t0, &cliente(2, 2));
    escreve(&s, &velha, r#""op":"begin""#);
    escreve(&s, &velha, r#""op":"savepoint","nome":"sp""#);
    escreve(&s, &velha, &muda_cliente(1, 1, 10));
    escreve(&s, &nova, r#""op":"begin""#);
    escreve(&s, &nova, &muda_cliente(2, 2, 20));
    escreve(&s, &t0, &pedido(100, 1));
    escreve(&s, &t0, &pedido(200, 2));
    escreve(&s, &nova, &muda_pedido(1, 101, 1));

    let e = pede(&s, &velha, r#""op":"commit""#)
        .expect_err("o elo de `a` esbarra na filha que T2 travou");
    assert!(matches!(e, PhxError::EmTransacao(_)), "{e}");
    escreve(&s, &velha, r#""op":"rollback_para","nome":"sp""#);
    escreve(&s, &velha, &muda_pedido(2, 201, 2));

    match pede(&s, &nova, r#""op":"commit""#) {
        Err(PhxError::EmTransacao(_)) => {}
        outro => panic!(
            "a velha ja nao precisa de nada de T2, e T2 foi tratada como ciclo: \
                     {outro:?}"
        ),
    }
    confirma(&s, &velha);
    confirma(&s, &nova);
    let par = |r: u64| {
        let l = linha(&s, &t0, "pedidos", r);
        (l.inteiro_ou("id", -1), l.inteiro_ou("cod_cliente", -1))
    };
    assert_eq!(
        (par(1), par(2)),
        ((101, 1), (201, 20)),
        "(id, cod) das filhas: a velha sem a mae `a`, T2 com a cascata de `b`"
    );
    assert_eq!(s.travas.travar().quantas(), 0);
}

// ------------------------------------------------------------- 537

/// `clientes(id, codigo)` e `vendedores(id, codigo)`, os dois com
/// `codigo` unico, e `pedidos(id, cod_cliente, cod_vend, x)` filha das
/// DUAS, com cascata no `ao_alterar` e indice nas duas colunas.
fn base_duas_maes(s: &Arc<Servidor>, ses: &Sessao) {
    escreve(s, ses, r#""op":"criar_database","database":"loja""#);
    for mae in ["clientes", "vendedores"] {
        escreve(
            s,
            ses,
            &format!(
                r#""op":"criar_tabela","database":"loja","tabela":"{mae}",
                           "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                                      {{"nome":"codigo","tipo":"Int8"}}],
                           "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}},
                                      {{"nome":"por_codigo","colunas":["codigo"],"unico":true}}]"#
            ),
        );
    }
    escreve(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cod_cliente","tipo":"Int8"},
                              {"nome":"cod_vend","tipo":"Int8"},
                              {"nome":"x","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cliente","colunas":["cod_cliente"]},
                              {"nome":"por_vend","colunas":["cod_vend"]}],
                   "chaves_estrangeiras":[
                      {"nome":"fk_cliente","colunas":["cod_cliente"],
                       "tabela_ref":"clientes","colunas_ref":["codigo"]},
                      {"nome":"fk_vend","colunas":["cod_vend"],
                       "tabela_ref":"vendedores","colunas_ref":["codigo"]}]"#,
    );
    escreve(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"codigo":5}"#,
    );
    escreve(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"vendedores","linha":{"id":1,"codigo":3}"#,
    );
    escreve(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos",
                   "linha":{"id":10,"cod_cliente":5,"cod_vend":3,"x":0}"#,
    );
}

fn muda_mae(tabela: &str, codigo: i64) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"{tabela}","rowid":1,
                   "linha":{{"id":1,"codigo":{codigo}}}"#
    )
}

fn muda_x(cliente: i64, vend: i64, x: i64) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
                   "linha":{{"id":10,"cod_cliente":{cliente},"cod_vend":{vend},"x":{x}}}"#
    )
}

/// `(cod_cliente, cod_vend, x)` da filha.
fn a_filha(s: &Arc<Servidor>, ses: &Sessao) -> (i64, i64, i64) {
    let l = linha(s, ses, "pedidos", 1);
    (
        l.inteiro_ou("cod_cliente", -1),
        l.inteiro_ou("cod_vend", -1),
        l.inteiro_ou("x", -1),
    )
}

/// **Pedido 537, a trava:** o elo que o `empilhar` planeja trava a
/// LINHA da filha, e nao so o fim da tabela dela. Medido pelo papel C
/// antes: T1 muda a mae 5->6 com a filha em `x=0`; T2 grava `x=1` na
/// filha, solta ou em transacao, e confirma; o COMMIT de T1 regravava
/// `x=0`. Agora a escrita solta recusa na hora (`EM_TRANSACAO`,
/// `repetir`), a da transacao espera o `LOCK TIMEOUT` dela, e as duas
/// passam depois do COMMIT de T1 -- sobre a filha ja com a chave nova.
///
/// # Prova real
///
/// Sem a trava da linha na fase 2 do `empilhar_atualizar_com_cascata`,
/// a escrita solta de T2 sai `Ok` -- o vermelho medido.
#[test]
fn o_elo_do_empilhar_trava_a_linha_da_filha() {
    let dir = dir_temp("537-trava");
    let s = servidor(&dir);
    let t0 = sessao(5370);
    let t1 = sessao(5371);
    let t2 = sessao(5372);
    base_duas_maes(&s, &t0);
    escreve(&s, &t1, r#""op":"begin","database":"loja""#);
    let r = escreve(&s, &t1, &muda_mae("clientes", 6));
    assert_eq!(r.inteiro_ou("cascata", 0), 1, "{}", r.escrever());

    match pede(&s, &t2, &muda_x(5, 3, 1)) {
        Err(e) => assert!(
            matches!(e, PhxError::EmTransacao(_)) && e.adianta_repetir(),
            "a escrita solta na filha tem de esperar T1: {e}"
        ),
        Ok(j) => panic!(
            "a escrita solta passou por cima da filha que o elo de T1 leva, e o \
                     COMMIT dele a apagaria: {}",
            j.escrever()
        ),
    }
    escreve(
        &s,
        &t2,
        r#""op":"begin","database":"loja","lock_timeout_ms":30"#,
    );
    let e = pede(&s, &t2, &muda_x(5, 3, 2))
        .expect_err("a escrita de T2 em transacao tem de esperar a linha de T1");
    assert!(
        matches!(e, PhxError::EmTransacao(_)) && e.to_string().contains("LOCK TIMEOUT"),
        "{e}"
    );
    escreve(&s, &t2, r#""op":"rollback""#);

    confirma(&s, &t1);
    assert_eq!(a_filha(&s, &t0), (6, 3, 0));
    escreve(&s, &t2, &muda_x(6, 3, 1));
    assert_eq!(a_filha(&s, &t0), (6, 3, 1));
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// **Pedido 537, o refazer:** o COMMIT leva SO A CHAVE do elo, sobre
/// a linha atual. Quando o 537 entrou, a cascata SOLTA de outra mae da
/// mesma filha escrevia nela sem perguntar por trava de linha; desde o
/// 561 ela pergunta e espera, e o escritor que nao pergunta aqui e
/// simulado pelo `solto_sem_trava_de_linha` -- o refazer fica como
/// cinto, e continua provado. Aqui o vendedor 3 vira 4 fora de transacao, a filha
/// vai junto, e o elo de T1 -- planejado com a filha em `cod_vend 3` --
/// regravava a linha inteira: o COMMIT recusava pela FK (o vendedor 3
/// ja nao existe) ou, sem conferencia, deixaria a filha orfa. Com o
/// elo refeito, T1 confirma e a filha fica com as DUAS chaves novas.
///
/// # Prova real
///
/// Sem o `refazer_o_elo` na pre-conferencia, o COMMIT de T1 volta
/// `INTEGRIDADE` nomeando `fk_vend` -- o vermelho medido.
#[test]
fn o_commit_leva_so_a_chave_do_elo_sobre_a_linha_atual() {
    let dir = dir_temp("537-refazer");
    let s = servidor(&dir);
    let t0 = sessao(5373);
    let t1 = sessao(5374);
    base_duas_maes(&s, &t0);
    escreve(&s, &t1, r#""op":"begin","database":"loja""#);
    escreve(&s, &t1, &muda_mae("clientes", 6));
    s.solto_sem_trava_de_linha.store(true, Ordering::SeqCst);
    escreve(&s, &t0, &muda_mae("vendedores", 4));
    s.solto_sem_trava_de_linha.store(false, Ordering::SeqCst);
    assert_eq!(
        a_filha(&s, &t0),
        (5, 4, 0),
        "a cascata solta do vendedor alcanca a filha"
    );
    confirma(&s, &t1);
    assert_eq!(
        a_filha(&s, &t0),
        (6, 4, 0),
        "(cod_cliente, cod_vend, x): o elo de T1 desfez a cascata do vendedor"
    );
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// **Pedido 567:** a filha tem DUAS chaves na mesma coluna -- `fk_cli`
/// para clientes e `fk_vend` para vendedores. A mae 5->6 FORA de
/// transacao leva as filhas para 6, e vendedores nao tem 6. Medido
/// pelo papel C antes: `ParouNoMeio`, a mae em `[6]` e as filhas em
/// `[5, 5]` -- duas orfas. Agora a pre-conferencia do COMMIT roda
/// antes da marca, e a recusa sai com nada gravado.
///
/// # Prova real
///
/// Sem a `pre_conferir_a_lista` no `atualizar_com_a_marca`, a mae
/// fica em 6 e as filhas em 5 -- o vermelho medido.
#[test]
fn a_cascata_solta_recusa_antes_da_marca_a_fk_para_outra_mae() {
    let dir = dir_temp("567-duas-fks");
    let s = servidor(&dir);
    let t0 = sessao(5670);
    escreve(&s, &t0, r#""op":"criar_database","database":"loja""#);
    for mae in ["clientes", "vendedores"] {
        escreve(
            &s,
            &t0,
            &format!(
                r#""op":"criar_tabela","database":"loja","tabela":"{mae}",
                           "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                                      {{"nome":"codigo","tipo":"Int8"}}],
                           "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}},
                                      {{"nome":"por_codigo","colunas":["codigo"],"unico":true}}]"#
            ),
        );
        escreve(
            &s,
            &t0,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"{mae}","linha":{{"id":1,"codigo":5}}"#
            ),
        );
    }
    escreve(
        &s,
        &t0,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cod","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cod","colunas":["cod"]}],
                   "chaves_estrangeiras":[
                      {"nome":"fk_cli","colunas":["cod"],
                       "tabela_ref":"clientes","colunas_ref":["codigo"]},
                      {"nome":"fk_vend","colunas":["cod"],
                       "tabela_ref":"vendedores","colunas_ref":["codigo"]}]"#,
    );
    for id in [10, 11] {
        escreve(
            &s,
            &t0,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{id},"cod":5}}"#
            ),
        );
    }
    let e = pede(&s, &t0, &muda_mae("clientes", 6))
        .expect_err("a cascata leva as filhas para 6, que vendedores nao tem");
    assert!(matches!(e, PhxError::Integridade(_)), "{e}");
    assert!(e.to_string().contains("nada foi gravado"), "{e}");
    assert_eq!(linha(&s, &t0, "clientes", 1).inteiro_ou("codigo", -1), 5);
    for rowid in [1, 2] {
        assert_eq!(
            linha(&s, &t0, "pedidos", rowid).inteiro_ou("cod", -1),
            5,
            "a filha {rowid} ficou orfa ou saiu da chave velha"
        );
    }
}

/// **Pedido 561 (a):** T1 segura a FILHA (`x=1`, empilhado); a mae
/// muda de chave fora de transacao e a cascata solta levaria a filha
/// por cima do X de T1. Medido pelo papel C antes: a solta passava, e
/// o COMMIT de T1 recusava por `fk_vend` -- ou, com a chave renascida,
/// confirmava apontando para a mae errada. Agora a solta recusa
/// `EM_TRANSACAO` com `repetir`, com nada gravado, e passa depois.
///
/// # Prova real
///
/// Sem a pergunta pelas filhas do plano no `alterar_solto`, a solta
/// sai `Ok` -- o vermelho medido.
#[test]
fn a_cascata_solta_nao_passa_pela_trava_da_filha() {
    let dir = dir_temp("561-filha");
    let s = servidor(&dir);
    let t0 = sessao(5610);
    let t1 = sessao(5611);
    base_duas_maes(&s, &t0);
    escreve(&s, &t1, r#""op":"begin","database":"loja""#);
    escreve(&s, &t1, &muda_x(5, 3, 1));
    match pede(&s, &t0, &muda_mae("clientes", 6)) {
        Err(e) => assert!(
            matches!(e, PhxError::EmTransacao(_)) && e.adianta_repetir(),
            "a cascata solta tem de esperar a filha de T1: {e}"
        ),
        Ok(j) => panic!(
            "a cascata solta passou por cima da filha que T1 segura: {}",
            j.escrever()
        ),
    }
    assert_eq!(linha(&s, &t0, "clientes", 1).inteiro_ou("codigo", -1), 5);
    confirma(&s, &t1);
    assert_eq!(a_filha(&s, &t0), (5, 3, 1));
    escreve(&s, &t0, &muda_mae("clientes", 6));
    assert_eq!(a_filha(&s, &t0), (6, 3, 1));
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// **Pedido 561 (d):** o upsert solto altera uma linha que o pedido
/// nao nomeia por rowid, e o portao do `inserir` so pergunta pelo fim
/// da tabela. T1 segura a filha; o upsert solto nela respondia OK, e o
/// COMMIT de T1 o apagava -- update perdido.
///
/// # Prova real
///
/// Sem a pergunta pela linha da mae no `alterar_solto`, o upsert sai
/// `Ok` e o COMMIT de T1 deixa `x=1` -- o vermelho medido.
#[test]
fn o_upsert_solto_nao_passa_pela_trava_da_linha() {
    let dir = dir_temp("561-upsert");
    let s = servidor(&dir);
    let t0 = sessao(5612);
    let t1 = sessao(5613);
    base_duas_maes(&s, &t0);
    escreve(&s, &t1, r#""op":"begin","database":"loja""#);
    escreve(&s, &t1, &muda_x(5, 3, 1));
    let upsert = r#""op":"inserir","database":"loja","tabela":"pedidos",
                   "linha":{"id":10,"cod_cliente":5,"cod_vend":3,"x":2},"se_existir":"atualizar""#;
    match pede(&s, &t0, upsert) {
        Err(e) => assert!(
            matches!(e, PhxError::EmTransacao(_)) && e.adianta_repetir(),
            "o upsert solto tem de esperar a linha de T1: {e}"
        ),
        Ok(j) => panic!(
            "o upsert solto passou por cima da linha que T1 segura: {}",
            j.escrever()
        ),
    }
    confirma(&s, &t1);
    assert_eq!(a_filha(&s, &t0), (5, 3, 1));
    escreve(&s, &t0, upsert);
    assert_eq!(a_filha(&s, &t0), (5, 3, 2));
}

// ------------------------------------------------------------- 538

/// `estoque(id, qtd, delta)` com a linha 1 em `qtd 5`.
fn base_estoque(s: &Arc<Servidor>, ses: &Sessao, gatilho: &str) {
    escreve(s, ses, r#""op":"criar_database","database":"loja""#);
    escreve(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"estoque",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"qtd","tipo":"Int8"},
                              {"nome":"delta","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    escreve(
        s,
        ses,
        &format!(r#""op":"sql","database":"loja","texto":"{gatilho}""#),
    );
    escreve(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"estoque","linha":{"id":1,"qtd":5,"delta":0}"#,
    );
}

fn muda_qtd(rowid: u64, id: i64, qtd: i64) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"estoque","rowid":{rowid},
                   "linha":{{"id":{id},"qtd":{qtd},"delta":0}}"#
    )
}

fn qtd_e_delta(s: &Arc<Servidor>, ses: &Sessao, rowid: u64) -> (i64, Option<i64>) {
    let l = linha(s, ses, "estoque", rowid);
    (
        l.inteiro_ou("qtd", -1),
        l.campo("delta").and_then(Json::inteiro),
    )
}

/// **Pedido 538:** dentro da transacao, o OLD do BEFORE UPDATE e a
/// linha que a TRANSACAO ve. O gatilho de delta de estoque
/// (`NEW.qtd - OLD.qtd`) na transacao 5->3->1 dava -4 aqui, onde o
/// PostgreSQL 16 e o MySQL 8.0 dao -2 (medido pelo papel C) -- e fora
/// da transacao o nosso ja dava -2. Os irmaos que chamam o mesmo
/// gatilho na mesma instrucao: o upsert que vira alteracao, e a linha
/// nascida na propria transacao, que pelo disco nem tinha OLD.
///
/// # Prova real
///
/// Com o OLD de volta ao disco (`velha`) no ramo do `atualizar`, o
/// delta sai -4 -- o vermelho medido; no upsert, -1 onde e -3.
#[test]
fn o_old_do_before_update_e_a_linha_que_a_transacao_ve() {
    let dir = dir_temp("538-update");
    let s = servidor(&dir);
    let ses = sessao(5381);
    base_estoque(
        &s,
        &ses,
        "CREATE TRIGGER d BEFORE UPDATE ON estoque FOR EACH ROW \
                 SET NEW.delta = NEW.qtd - OLD.qtd",
    );
    // Fora de transacao, a referencia: 5 -> 3 -> 1 da -2.
    escreve(&s, &ses, &muda_qtd(1, 1, 3));
    escreve(&s, &ses, &muda_qtd(1, 1, 1));
    assert_eq!(qtd_e_delta(&s, &ses, 1), (1, Some(-2)));
    escreve(&s, &ses, &muda_qtd(1, 1, 5));

    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(&s, &ses, &muda_qtd(1, 1, 3));
    escreve(&s, &ses, &muda_qtd(1, 1, 1));
    confirma(&s, &ses);
    assert_eq!(
        qtd_e_delta(&s, &ses, 1),
        (1, Some(-2)),
        "(qtd, delta) na transacao 5->3->1: o OLD da segunda instrucao e 3"
    );

    // O upsert que vira alteracao: 1 -> 3 na lista, e o SET leva a 0.
    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(&s, &ses, &muda_qtd(1, 1, 3));
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"estoque","se_existir":"atualizar",
                   "linha":{"id":1,"qtd":0,"delta":0},"atualizar":{"qtd":0}"#,
    );
    confirma(&s, &ses);
    assert_eq!(
        qtd_e_delta(&s, &ses, 1),
        (0, Some(-3)),
        "(qtd, delta) do upsert: o OLD e o 3 da lista"
    );

    // A nascida na transacao: 7 -> 4 da -3.
    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"estoque","linha":{"id":2,"qtd":7,"delta":0}"#,
    );
    escreve(&s, &ses, &muda_qtd(2, 2, 4));
    confirma(&s, &ses);
    assert_eq!(
        qtd_e_delta(&s, &ses, 2),
        (4, Some(-3)),
        "(qtd, delta) da linha nascida na transacao"
    );
}

/// **Pedido 538, o irmao do DELETE:** o OLD do BEFORE DELETE tambem e
/// a linha que a transacao ve. O gatilho recusa excluir o estoque em
/// 3; a linha esta em 5 no disco e em 3 na lista -- e a nascida na
/// transacao nem disparava o gatilho, porque pelo disco nao existe.
///
/// # Prova real
///
/// Com o OLD de volta ao disco no ramo do `excluir`, as duas exclusoes
/// passam `Ok` -- o vermelho medido.
#[test]
fn o_old_do_before_delete_e_a_linha_que_a_transacao_ve() {
    let dir = dir_temp("538-delete");
    let s = servidor(&dir);
    let ses = sessao(5382);
    base_estoque(
        &s,
        &ses,
        "CREATE TRIGGER g BEFORE DELETE ON estoque FOR EACH ROW \
                 IF OLD.qtd = 3 THEN \
                   SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'estoque 3 nao sai'; \
                 END IF",
    );
    let exclui_estoque = |rowid: u64| exclui("estoque", rowid, false);

    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(&s, &ses, &muda_qtd(1, 1, 3));
    let r = pede(&s, &ses, &exclui_estoque(1));
    assert!(
        r.as_ref()
            .is_err_and(|e| e.to_string().contains("estoque 3 nao sai")),
        "a linha em 3 na lista saiu pelo OLD do disco: {r:?}"
    );
    escreve(&s, &ses, r#""op":"rollback""#);

    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"estoque","linha":{"id":2,"qtd":3,"delta":0}"#,
    );
    let r = pede(&s, &ses, &exclui_estoque(2));
    assert!(
        r.as_ref()
            .is_err_and(|e| e.to_string().contains("estoque 3 nao sai")),
        "a linha nascida na transacao saiu sem o gatilho: {r:?}"
    );
    escreve(&s, &ses, r#""op":"rollback""#);

    // O lado que nao muda: a linha em 5 sai, dentro e fora.
    escreve(&s, &ses, r#""op":"begin","database":"loja""#);
    escreve(&s, &ses, &exclui_estoque(1));
    confirma(&s, &ses);
    assert!(excluida(&linha(&s, &ses, "estoque", 1)));
}

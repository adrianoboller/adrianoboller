//! **Pedido 448, a revisao do DBA** (`docs/propostas/parecer-dba-448-2026-09-24.md`):
//! os tres ALTOS que bloquearam a integracao, e o NULL no indice unico. Os
//! cenarios sao os da sonda dele; cada prova confere o DISCO e a resposta.
use super::*;

/// `clientes(id, codigo, nome)` com `codigo` UNICO e referenciado por
/// `pedidos.cod_cliente` -- a chave que a mae MUDA sem mudar de `id`.
/// `codigo` entra com a declaracao que a prova pedir (DEFAULT,
/// `Sequence`, nada).
fn base_codigo(s: &Arc<Servidor>, ses: &Sessao, codigo: &str) {
    pede(s, ses, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        s,
        ses,
        &format!(
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                       "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},{codigo},
                                  {{"nome":"nome","tipo":"Str(20)"}}],
                       "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}},
                                  {{"nome":"por_codigo","colunas":["codigo"],"unico":true}}]"#
        ),
    )
    .unwrap();
    pede(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cod_cliente","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cliente","colunas":["cod_cliente"]}],
                   "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cod_cliente"],
                                           "tabela_ref":"clientes","colunas_ref":["codigo"]}]"#,
    )
    .unwrap();
}

fn escreve(s: &Arc<Servidor>, ses: &Sessao, corpo: &str) {
    pede(s, ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
}

/// A coluna `coluna` de cada linha viva de `tabela`, na ordem de digitacao.
fn coluna(s: &Arc<Servidor>, ses: &Sessao, tabela: &str, coluna: &str) -> Vec<i64> {
    pede(
        s,
        ses,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":1000"#),
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .map(|l| l.campo(coluna).and_then(Json::inteiro).unwrap_or(-1))
    .collect()
}

/// O COMMIT de uma lista VALIDA: `COMMITTED`, sem aviso, sem marca
/// sobrando depois de a janela fechar. Devolve o texto da resposta.
fn confirma(s: &Arc<Servidor>, ses: &Sessao, dir: &std::path::Path) -> String {
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
    assert!(
        r.campo("aviso").is_none(),
        "lista valida com aviso: {veredito}"
    );
    s.descarregar_sujas();
    assert_eq!(marcas_em(&dir.join("loja")), 0, "{veredito}");
    veredito
}

// ------------------------------------------------------------- A1

/// **A1:** `[inserir pedido->A, alterar codigo de B 2->3]`, com B SEM
/// filha. A passada planejava a cascata de B abrindo `pedidos` por um
/// segundo descritor -- sujo pela propria passada, que acabara de
/// inserir o pedido -- e a guarda do `.ndx` recusava responder mesmo
/// com o plano vazio: o pedido ficava gravado, B nao mudava, e a
/// resposta dizia «DEFEITO DO MOTOR». O planejador passa a ser UM, o
/// da pre-conferencia, e a passada aplica sem replanejar.
#[test]
fn a1_a_mae_sem_filha_muda_de_chave_depois_de_a_lista_escrever_na_filha() {
    let dir = dir_temp("448r-a1");
    let s = servidor(&dir);
    let ses = sessao(4501);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Int8"}"#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"codigo":1,"nome":"a"}"#,
    );
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"codigo":2,"nome":"b"}"#,
    );
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cod_cliente":1}"#,
    );
    escreve(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":2,
                   "linha":{"id":2,"codigo":3,"nome":"b"}"#,
    );
    confirma(&s, &ses, &dir);
    assert_eq!(
        (
            coluna(&s, &ses, "clientes", "codigo"),
            coluna(&s, &ses, "pedidos", "cod_cliente")
        ),
        (vec![1, 3], vec![1])
    );
}

/// **A1, o achado 4(a) da frente:** `[inserir filha->5, alterar a
/// chave da mae 5->6]`. PG, MySQL e MariaDB aceitam -- a filha da
/// propria transacao acompanha a mae pelo `ON UPDATE CASCADE` -- e a
/// primeira versao do 448 recusava mandando refazer, o que dava a
/// mesma recusa. Com o planejador unico, o elo da filha entra na lista
/// antes da marca, e duas rodadas seguidas confirmam.
#[test]
fn a1_a_filha_da_propria_lista_acompanha_a_chave_nova_da_mae() {
    let dir = dir_temp("448r-4a");
    let s = servidor(&dir);
    let ses = sessao(4502);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Int8"}"#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"codigo":5,"nome":"a"}"#,
    );
    for (rodada, de, para) in [(1, 5, 6), (2, 6, 7)] {
        pede(&s, &ses, r#""op":"begin""#).unwrap();
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"pedidos",
                           "linha":{{"id":{rodada},"cod_cliente":{de}}}"#
            ),
        );
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                           "linha":{{"id":1,"codigo":{para},"nome":"a"}}"#
            ),
        );
        confirma(&s, &ses, &dir);
    }
    assert_eq!(
        (
            coluna(&s, &ses, "clientes", "codigo"),
            coluna(&s, &ses, "pedidos", "cod_cliente")
        ),
        (vec![7], vec![7, 7]),
        "as filhas nao acompanharam a mae"
    );
}

// ------------------------------------------------------------- A2

/// **A2: o plano do `ao_alterar` nao copia a sobreposicao.** `n` filhas
/// na lista e DEPOIS `k` alteracoes de chave de mae: o
/// `MaesAbertas::prefixo` copiava a sobreposicao inteira de `pedidos` a
/// cada alteracao -- O(n*k) sob a trava global, 715 ms para 99,6 s com
/// n = k = 8.000 na sonda do DBA. A prova conta as copias, e nao o
/// tempo; o tempo mora na bancada (`custo-da-pre-conferencia chaves`).
#[test]
fn a2_o_plano_da_cascata_nao_copia_a_sobreposicao_da_filha() {
    let dir = dir_temp("448r-a2");
    let s = servidor(&dir);
    let ses = sessao(4503);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Int8"}"#);
    for i in 0..=5 {
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                           "linha":{{"id":{i},"codigo":{i},"nome":"m"}}"#
            ),
        );
    }
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    for i in 1..=20 {
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"pedidos",
                           "linha":{{"id":{i},"cod_cliente":0}}"#
            ),
        );
    }
    for i in 1..=5 {
        escreve(
            &s,
            &ses,
            &format!(
                r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{},
                           "linha":{{"id":{i},"codigo":{},"nome":"m"}}"#,
                i + 1,
                i + 100
            ),
        );
    }
    // A contagem vem ANTES do veredito: a copia acontece na
    // pre-conferencia, e um COMMIT que quebre depois dela nao pode
    // esconder o que ela custou.
    let antes = phxsql_store::table::copias_da_sobreposicao();
    let r = pede(&s, &ses, r#""op":"commit""#);
    let copias = phxsql_store::table::copias_da_sobreposicao() - antes;
    assert_eq!(
        copias,
        0,
        "o COMMIT copiou a sobreposicao {copias} vezes -- uma por alteracao de \
                 chave, O(n) cada. COMMIT: {:?}",
        r.as_ref().map(|j| j.escrever())
    );
    let r = r.expect("lista valida recusada");
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    assert!(r.campo("aviso").is_none(), "{}", r.escrever());
}

// ------------------------------------------------------------- A3

/// **A3, o DEFAULT:** `[mae com codigo pelo DEFAULT 7, filha->7]`
/// confirmava antes do 448. A sobreposicao guardava a linha CRUA do
/// `empilhar` (codigo nulo), e a pre-conferencia nao achava a mae 7
/// que a passada ia gravar.
#[test]
fn a3_a_mae_com_codigo_pelo_padrao_e_a_filha_confirmam() {
    let dir = dir_temp("448r-padrao");
    let s = servidor(&dir);
    let ses = sessao(4504);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Int8","padrao":"7"}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    );
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cod_cliente":7}"#,
    );
    confirma(&s, &ses, &dir);
    assert_eq!(
        (
            coluna(&s, &ses, "clientes", "codigo"),
            coluna(&s, &ses, "pedidos", "cod_cliente")
        ),
        (vec![7], vec![7])
    );
}

/// **A3, a `Sequence` mantida:** atualizar so o nome da mae, sem
/// mandar a `Sequence`, e pendurar uma filha no codigo dela. O store
/// MANTEM o numero; a linha crua o tinha nulo.
#[test]
fn a3_atualizar_a_mae_sem_a_sequencia_e_a_filha_confirmam() {
    let dir = dir_temp("448r-seq");
    let s = servidor(&dir);
    let ses = sessao(4505);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Sequence"}"#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    );
    let cod = coluna(&s, &ses, "clientes", "codigo")[0];
    assert!(cod > 0, "a sequencia nao numerou a mae");
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    escreve(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":1,"nome":"b"}"#,
    );
    escreve(
        &s,
        &ses,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"pedidos",
                       "linha":{{"id":1,"cod_cliente":{cod}}}"#
        ),
    );
    confirma(&s, &ses, &dir);
    assert_eq!(
        (
            coluna(&s, &ses, "clientes", "codigo"),
            coluna(&s, &ses, "pedidos", "cod_cliente")
        ),
        (vec![cod], vec![cod])
    );
}

/// **A3, o inverso:** `[atualizar a mae sem a Sequence, excluir de vez
/// a mae]`, com a filha no DISCO. A mae continua com o codigo, e tem
/// filha: a exclusao tem de recusar ANTES da marca. Com a linha crua,
/// o `empilhar` planejava uma cascata do codigo para NULO -- a filha
/// perdia a mae sem ninguem ter pedido -- e a mae saia.
#[test]
fn a3_atualizar_a_mae_sem_a_sequencia_e_excluir_de_vez_recusa_antes_da_marca() {
    let dir = dir_temp("448r-seq-exclui");
    let s = servidor(&dir);
    let ses = sessao(4506);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Sequence"}"#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    );
    let cod = coluna(&s, &ses, "clientes", "codigo")[0];
    escreve(
        &s,
        &ses,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"pedidos",
                       "linha":{{"id":1,"cod_cliente":{cod}}}"#
        ),
    );
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":1,"nome":"b"}"#,
    )
    .unwrap();
    assert_eq!(
        r.inteiro_ou("linhas", 0),
        1,
        "mudar so o nome nao cascateia: {}",
        r.escrever()
    );
    escreve(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,"fisico":true"#,
    );
    let e = pede(&s, &ses, r#""op":"commit""#).expect_err("a mae com filha saiu");
    let texto = e.to_string();
    assert_eq!(e.nome(), "INTEGRIDADE", "{texto}");
    assert!(texto.contains("ANTES da marca"), "{texto}");
    assert_eq!(
        (
            coluna(&s, &ses, "clientes", "codigo"),
            coluna(&s, &ses, "pedidos", "cod_cliente"),
            marcas_em(&dir.join("loja"))
        ),
        (vec![cod], vec![cod], 0),
        "(maes, filhas, marcas): a recusa mexeu no disco -- {texto}"
    );
}

/// **A3, a leitura:** dentro da transacao, a busca pela chave ve a
/// linha como o store vai grava-la -- o codigo do DEFAULT e a
/// `Sequence` mantida. A primeira versao do 448 regrediu as duas:
/// com o buraco (a) fechado, a linha alterada so e achada pela chave
/// que a troca lhe da, e a troca crua dava chave nula.
#[test]
fn a3_dentro_da_transacao_a_busca_ve_a_linha_como_sera_gravada() {
    let buscar = |s: &Arc<Servidor>, ses: &Sessao, cod: i64| -> usize {
        pede(
            s,
            ses,
            &format!(
                r#""op":"buscar","database":"loja","tabela":"clientes",
                           "indice":"por_codigo","chave":[{cod}]"#
            ),
        )
        .unwrap()
        .campo("linhas")
        .and_then(Json::lista)
        .map_or(0, |l| l.len())
    };
    let dir = dir_temp("448r-leitura");
    let s = servidor(&dir);
    let ses = sessao(4507);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Int8","padrao":"7"}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    );
    let padrao = buscar(&s, &ses, 7);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();

    let dir = dir_temp("448r-leitura-seq");
    let s = servidor(&dir);
    base_codigo(&s, &ses, r#"{"nome":"codigo","tipo":"Sequence"}"#);
    escreve(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    );
    let cod = coluna(&s, &ses, "clientes", "codigo")[0];
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    escreve(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":1,"nome":"b"}"#,
    );
    let mantida = buscar(&s, &ses, cod);
    assert_eq!(
        (padrao, mantida),
        (1, 1),
        "(pelo DEFAULT, pela Sequence mantida): a busca dentro da transacao nao \
                 achou a linha como ela vai ser gravada"
    );
}

// ------------------------------------------------------------- A4

/// **A4: NULL nao colide num indice unico** -- PG, MySQL, MariaDB e
/// SQLite aceitam varios. O store dava DUPLICADO no segundo NULL, e a
/// pre-conferencia pulava o NULL: a mesma decisao escrita duas vezes,
/// divergindo. Medido pelo DBA: com um NULL no disco, `[id=3 email=x,
/// id=4 email=NULL]` saia com 1 gravada e «DEFEITO DO MOTOR».
#[test]
fn a4_dois_nulos_no_indice_unico_nao_colidem() {
    let dir = dir_temp("448r-nulo");
    let s = servidor(&dir);
    let ses = sessao(4508);
    pede(&s, &ses, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"email","tipo":"Str(20)"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_email","colunas":["email"],"unico":true}]"#,
    )
    .unwrap();
    let insere = |id: i64, email: &str| {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                           "linha":{{"id":{id},"email":{email}}}"#
            ),
        )
    };
    insere(1, "null").unwrap();
    let fora = insere(2, "null").map(|_| ()).map_err(|e| e.to_string());
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    insere(3, r#""x""#).unwrap();
    insere(4, "null").unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#);
    let veredito = match &r {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    };
    let repetido = insere(5, r#""x""#).map(|_| ()).map_err(|e| e.nome());
    assert_eq!(
        (
            fora,
            r.is_ok() && !veredito.contains("aviso"),
            quantas(&s, &ses, "clientes"),
            repetido
        ),
        (Ok(()), true, 4, Err("DUPLICADO")),
        "(segundo NULL fora de transacao, COMMIT limpo, linhas, o nao-NULL \
                 repetido): COMMIT {veredito}"
    );
}

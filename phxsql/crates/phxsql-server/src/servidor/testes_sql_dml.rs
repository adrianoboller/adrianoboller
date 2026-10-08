//! INSERT, UPDATE e DELETE por chave, pela op `sql` -- o passo 2 do
//! roteiro de `docs/SQL.md`. A prova que mais importa e a da mescla:
//! o UPDATE nao pode zerar a coluna que o SET nao citou.
use super::*;

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("sqldml-{rotulo}"))
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
    Servidor::novo(c).unwrap()
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `b.c`: id (chave unica), nome e cidade -- a terceira coluna existe
/// para provar que o UPDATE nao a zera, e o indice comum em cidade para
/// provar que ele nao serve de chave.
fn com_dados(dir: &std::path::Path) -> Arc<Servidor> {
    let s = servidor(dir);
    let ses = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"cidade","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                               {"nome":"porCidade","colunas":["cidade"]}]}"#,
        ),
        &ses,
    )
    .unwrap();
    for (id, nome, cidade) in [
        (1, "Adriano", "Blumenau"),
        (2, "Maria", "Joinville"),
        (3, "Joao", "Blumenau"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c",
                        "valores":{{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    s
}

fn sql(s: &Arc<Servidor>, texto: &str) -> Result<Json> {
    s.executar(
        "sql",
        &pedido(&format!(
            r#"{{"database":"b","texto":{}}}"#,
            Json::texto_de(texto).escrever()
        )),
        &Sessao::default(),
    )
}

fn ler(s: &Arc<Servidor>, rowid: u64) -> Json {
    s.executar(
        "ler",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"c","rowid":{rowid}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

fn busca(s: &Arc<Servidor>, id: i64) -> Json {
    s.executar(
        "buscar",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"c","indice":"porId","chave":[{id}]}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

fn linhas_da(busca: &Json) -> usize {
    busca
        .campo("linhas")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap_or(0)
}

#[test]
fn insert_pela_camada_sql_grava_e_le_de_volta() {
    let dir = dir_temp("insert");
    let s = com_dados(&dir);
    let r = sql(
        &s,
        "INSERT INTO c (id, nome, cidade) VALUES (4, 'Bia', 'Pomerode')",
    )
    .unwrap();
    assert_eq!(r.texto_ou("op", ""), "inserir");
    assert_eq!(r.inteiro_ou("afetadas", -1), 1);
    assert_eq!(r.inteiro_ou("rowid", -1), 4);
    let l = ler(&s, 4);
    assert_eq!(l.inteiro_ou("id", -1), 4);
    assert_eq!(l.texto_ou("nome", ""), "Bia");
    assert_eq!(l.texto_ou("cidade", ""), "Pomerode");
    assert_eq!(linhas_da(&busca(&s, 4)), 1);
}

/// A PROVA REAL da mescla: tire-a do `pedido_de_atualizar` e `cidade`
/// volta NULL, porque o `atualizar` preenche com NULL toda coluna que
/// nao vem.
#[test]
fn update_por_chave_muda_so_a_coluna_do_set() {
    let dir = dir_temp("update");
    let s = com_dados(&dir);
    let antes = ler(&s, 2);
    let r = sql(&s, "UPDATE c SET nome = 'Mariana' WHERE id = 2").unwrap();
    assert_eq!(r.texto_ou("op", ""), "atualizar");
    assert_eq!(r.inteiro_ou("afetadas", -1), 1);
    assert_eq!(r.inteiro_ou("rowid", -1), 2);
    assert!(r.inteiro_ou("versao", -1) >= 1, "{}", r.escrever());
    let depois = ler(&s, 2);
    assert_eq!(depois.texto_ou("nome", ""), "Mariana");
    assert_eq!(
        depois.texto_ou("cidade", ""),
        "Joinville",
        "o SET nao citou cidade, e ela tinha de ficar"
    );
    assert_eq!(depois.inteiro_ou("id", -1), 2);
    // O numero de ordem e do motor e nao muda por um UPDATE.
    assert_eq!(
        depois.campo("rownum").map(Json::escrever),
        antes.campo("rownum").map(Json::escrever)
    );
    // E as outras linhas nao foram tocadas.
    assert_eq!(ler(&s, 1).texto_ou("nome", ""), "Adriano");
    assert_eq!(ler(&s, 3).texto_ou("cidade", ""), "Blumenau");
}

#[test]
fn delete_por_chave_e_suave_e_some_da_lista() {
    let dir = dir_temp("delete");
    let s = com_dados(&dir);
    let r = sql(&s, "DELETE FROM c WHERE id = 3").unwrap();
    assert_eq!(r.texto_ou("op", ""), "excluir");
    assert_eq!(r.inteiro_ou("afetadas", -1), 1);
    assert_eq!(r.texto_ou("modo", ""), "suave");
    assert!(r.booleano_ou("reversivel", false));
    // Some de quem LISTA -- o `varrer` e a grade...
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let ids: Vec<i64> = v
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    assert!(!ids.contains(&3), "{ids:?}");
    assert!(ids.contains(&1) && ids.contains(&2), "{ids:?}");
    // ...mas fica no arquivo, MARCADA: e o que `reversivel` promete, e o
    // indice continua a acha-la por isso mesmo -- o `restaurar` precisa.
    assert!(ler(&s, 3).booleano_ou("softdeleted", false));
    assert_eq!(linhas_da(&busca(&s, 1)), 1);
    assert_eq!(ler(&s, 1).texto_ou("nome", ""), "Adriano");
}

#[test]
fn chave_ausente_afeta_zero_linhas_sem_erro() {
    let dir = dir_temp("zero");
    let s = com_dados(&dir);
    let r = sql(&s, "UPDATE c SET nome = 'x' WHERE id = 999").unwrap();
    assert_eq!(r.inteiro_ou("afetadas", -1), 0);
    assert!(r.campo("rowid").is_none());
    let r = sql(&s, "DELETE FROM c WHERE id = 999").unwrap();
    assert_eq!(r.inteiro_ou("afetadas", -1), 0);
    assert_eq!(ler(&s, 1).texto_ou("nome", ""), "Adriano");
}

/// **O que era recusa agora e faixa.** Ate o UPDATE/DELETE por faixa,
/// `WHERE cidade = 'Blumenau'` sobre o indice COMUM era recusado com «NAO e
/// unico» -- o `=` so' passava por chave unica. Agora o mesmo `=` sobre
/// indice nao-unico cai no caminho POR FAIXA: `coletar_rowids` junta as
/// DUAS linhas de Blumenau e o laco atualiza as duas, deixando a de
/// Joinville intacta.
///
/// E a prova cobre um ramo que os testes de `>`/`<=` nao tocam: o `=` que
/// NAO acha chave unica. Se `tem_chave_unica` voltasse a mandar todo `=`
/// pela chave, este `=` erraria «NAO e unico» outra vez e o `unwrap`
/// entraria em panico. As duas cidades preservadas provam de quebra que a
/// coluna do filtro (fora do SET) nao foi zerada pela mescla.
#[test]
fn update_por_faixa_no_indice_comum_alcanca_todas_as_que_casam() {
    let dir = dir_temp("comum");
    let s = com_dados(&dir);
    let r = sql(&s, "UPDATE c SET nome = 'x' WHERE cidade = 'Blumenau'").unwrap();
    assert_eq!(
        r.inteiro_ou("afetadas", -1),
        2,
        "as duas linhas de Blumenau: {}",
        r.escrever()
    );
    // As duas de Blumenau viraram 'x'; a de Joinville nao.
    assert_eq!(ler(&s, 1).texto_ou("nome", ""), "x");
    assert_eq!(ler(&s, 3).texto_ou("nome", ""), "x");
    assert_eq!(ler(&s, 2).texto_ou("nome", ""), "Maria");
    // E a cidade -- coluna do filtro, fora do SET -- ficou de pe nas tres.
    assert_eq!(ler(&s, 1).texto_ou("cidade", ""), "Blumenau");
    assert_eq!(ler(&s, 2).texto_ou("cidade", ""), "Joinville");
}

/// O irmao do de cima pelo DELETE: `DELETE FROM c WHERE cidade =
/// 'Blumenau'` sobre o indice comum some com as DUAS de Blumenau (suave) e
/// deixa a de Joinville. Antes da faixa isto era recusado igual.
#[test]
fn delete_por_faixa_no_indice_comum_alcanca_todas_as_que_casam() {
    let dir = dir_temp("comum-del");
    let s = com_dados(&dir);
    let r = sql(&s, "DELETE FROM c WHERE cidade = 'Blumenau'").unwrap();
    assert_eq!(
        r.inteiro_ou("afetadas", -1),
        2,
        "as duas de Blumenau saem: {}",
        r.escrever()
    );
    assert!(ler(&s, 1).booleano_ou("softdeleted", false), "id 1 saiu");
    assert!(ler(&s, 3).booleano_ou("softdeleted", false), "id 3 saiu");
    assert!(
        !ler(&s, 2).booleano_ou("softdeleted", false),
        "a de Joinville ficou"
    );
}

/// Os pedidos dos passos 2 e 3 sao funcoes puras, e e aqui que a versao
/// se prova: tire a linha que a poe no pedido e este teste falha.
#[test]
fn os_pedidos_dos_passos_levam_a_linha_mesclada_e_a_versao() {
    let lida =
        pedido(r#"{"id":2,"nome":"Maria","cidade":"Joinville","softdeleted":false,"rownum":2}"#);
    let set = vec![("nome".to_string(), Json::texto_de("Mariana"))];
    let p = pedido_de_atualizar("b", "c", 2, &lida, &set, 7);
    assert_eq!(p.texto_ou("op", ""), "atualizar");
    assert_eq!(p.inteiro_ou("rowid", -1), 2);
    assert_eq!(
        p.inteiro_ou("versao", -1),
        7,
        "a versao lida tem de ir no pedido"
    );
    let v = p.campo("valores").unwrap();
    assert_eq!(v.texto_ou("nome", ""), "Mariana");
    assert_eq!(
        v.texto_ou("cidade", ""),
        "Joinville",
        "a coluna fora do SET vem da linha lida"
    );
    assert_eq!(v.inteiro_ou("id", -1), 2);
    assert_eq!(
        v.inteiro_ou("rownum", -1),
        2,
        "o numero de ordem volta como lido"
    );
    assert!(
        v.campo("softdeleted").is_none(),
        "a marca fica de fora: quem a preserva e o atualizar"
    );

    let p = pedido_de_excluir("b", "c", 2, 7);
    assert_eq!(p.inteiro_ou("versao", -1), 7);
    assert!(p.campo("fisico").is_none(), "suave por padrao");

    let p = pedido_de_ler("b", "c", 2);
    assert!(p.booleano_ou("com_versao", false));
}

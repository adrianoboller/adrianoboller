//! Paginacao por cursor, e a coluna de ordem que a sustenta.
//!
//! O que estes testes protegem:
//!
//! 1. `rownum` e atribuido sozinho, cresce de um em um e NUNCA reaproveita;
//! 2. alterar uma linha nao renumera;
//! 3. a pagina por cursor devolve exatamente a mesma coisa que a varredura
//!    inteira devolveria -- so que sem ler a tabela inteira;
//! 4. o cursor nao repete nem pula linha nas bordas.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema, COLUNA_ROWNUM};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Salto, Table, Visao};

const NOME: usize = 1;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// Linha curta de propósito: quem monta não sabe das colunas do motor.
fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("Cliente {i:04}"))]
}

fn com(n: i64, dir: &DirTemp) -> Table {
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t
}

fn rownum(t: &mut Table, rowid: u64) -> u64 {
    let i = t.esquema().coluna_rownum().unwrap();
    let linha = t.ler(rowid).unwrap().unwrap();
    match &linha[i] {
        Value::UInt(n) => *n,
        outro => panic!("rownum nao e UInt: {outro:?}"),
    }
}

#[test]
fn toda_tabela_nova_tem_rownum() {
    let dir = DirTemp::novo("tem-rownum");
    let t = Table::criar(&dir.0, esquema()).unwrap();
    let e = t.esquema();
    let i = e.coluna_rownum().expect("sem a coluna de ordem");
    assert_eq!(e.colunas()[i].nome, COLUNA_ROWNUM);
    assert_eq!(e.colunas()[i].ty, ColumnType::UInt8);
    assert!(!e.colunas()[i].nullable);
    // No fim, e depois da softdeleted.
    assert_eq!(i, e.colunas().len() - 1);
    assert_eq!(e.coluna_softdeleted(), Some(i - 1));
}

#[test]
fn o_motor_numera_de_um_em_um() {
    let dir = DirTemp::novo("numera");
    let mut t = com(5, &dir);
    for i in 1..=5u64 {
        assert_eq!(rownum(&mut t, i), i);
    }
    assert_eq!(t.rownum_atual(), 6);
}

/// A regra que faz o cursor valer: número usado não volta. Se voltasse, uma
/// linha nova apareceria ATRÁS de um cursor parado, e a paginação passaria a
/// pular registro sem avisar.
#[test]
fn excluir_nao_devolve_o_numero() {
    let dir = DirTemp::novo("nao-devolve");
    let mut t = com(5, &dir);
    t.excluir_de_vez(3, "").unwrap();
    t.excluir_suave(4, "").unwrap();

    let novo = t.inserir(&linha(6)).unwrap();
    assert_eq!(
        rownum(&mut t, novo),
        6,
        "o numero de uma linha morta voltou"
    );
    assert_eq!(t.rownum_atual(), 7);
}

#[test]
fn alterar_nao_renumera() {
    let dir = DirTemp::novo("nao-renumera");
    let mut t = com(3, &dir);
    let antes = rownum(&mut t, 2);

    // Linha curta, sem as colunas do motor -- o caso comum.
    t.atualizar(2, &[Value::Int(2), Value::Str("Outro nome".into())])
        .unwrap();
    assert_eq!(rownum(&mut t, 2), antes, "a alteracao renumerou a linha");
    assert_eq!(
        t.ler(2).unwrap().unwrap()[NOME],
        Value::Str("Outro nome".into())
    );
    // E o contador não andou por causa de uma alteração.
    assert_eq!(t.rownum_atual(), 4);
}

/// A prova que importa: a página por cursor devolve EXATAMENTE o que a
/// varredura inteira devolveria, pedaço por pedaço, sem repetir nem pular.
#[test]
fn as_paginas_reconstroem_a_varredura_inteira() {
    let dir = DirTemp::novo("reconstroi");
    let mut t = com(97, &dir);
    t.excluir_suave(10, "").unwrap();
    t.excluir_de_vez(20, "").unwrap();

    let inteira: Vec<u64> = t.varrer().unwrap().into_iter().map(|(r, _)| r).collect();

    let mut por_pagina = Vec::new();
    let mut cursor = 0u64;
    loop {
        let p = t.pagina_depois_de(cursor, 10, Visao::Ativas).unwrap();
        if p.is_empty() {
            break;
        }
        cursor = *p.last().unwrap();
        por_pagina.extend(p);
    }
    assert_eq!(por_pagina, inteira);
    assert_eq!(por_pagina.len(), 95);
}

#[test]
fn o_cursor_nao_devolve_a_propria_linha() {
    let dir = DirTemp::novo("nao-repete");
    let mut t = com(10, &dir);
    let p = t.pagina_depois_de(4, 3, Visao::Ativas).unwrap();
    assert_eq!(p, vec![5, 6, 7]);
    // E continuar do último devolve o seguinte, sem repetir o 7.
    let q = t.pagina_depois_de(7, 3, Visao::Ativas).unwrap();
    assert_eq!(q, vec![8, 9, 10]);
}

#[test]
fn cursor_no_fim_devolve_vazio() {
    let dir = DirTemp::novo("fim");
    let mut t = com(5, &dir);
    assert!(t.pagina_depois_de(5, 10, Visao::Ativas).unwrap().is_empty());
    assert!(t
        .pagina_depois_de(999, 10, Visao::Ativas)
        .unwrap()
        .is_empty());
}

#[test]
fn pagina_para_tras() {
    let dir = DirTemp::novo("tras");
    let mut t = com(20, &dir);
    // Volta em ordem crescente, como a de ir: quem chama não precisa saber
    // que a leitura foi de trás para a frente.
    assert_eq!(
        t.pagina_antes_de(10, 3, Visao::Ativas).unwrap(),
        vec![7, 8, 9]
    );
    assert_eq!(t.pagina_antes_de(3, 10, Visao::Ativas).unwrap(), vec![1, 2]);
    assert!(t.pagina_antes_de(1, 5, Visao::Ativas).unwrap().is_empty());
}

/// A página não enxerga linha marcada, como a varredura. Se enxergasse, a
/// exclusão suave não faria nada pela tela que mais importa.
#[test]
fn a_pagina_respeita_a_visao() {
    let dir = DirTemp::novo("visao");
    let mut t = com(10, &dir);
    for r in [3, 4, 5] {
        t.excluir_suave(r, "").unwrap();
    }
    assert_eq!(
        t.pagina_depois_de(0, 5, Visao::Ativas).unwrap(),
        vec![1, 2, 6, 7, 8]
    );
    assert_eq!(
        t.pagina_depois_de(0, 5, Visao::Excluidas).unwrap(),
        vec![3, 4, 5]
    );
    assert_eq!(
        t.pagina_depois_de(0, 5, Visao::Todas).unwrap(),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn pagina_por_posicao_ainda_funciona() {
    let dir = DirTemp::novo("posicao");
    let mut t = com(30, &dir);
    assert_eq!(t.pagina(0, 3, Visao::Ativas).unwrap(), vec![1, 2, 3]);
    assert_eq!(t.pagina(10, 3, Visao::Ativas).unwrap(), vec![11, 12, 13]);
    // Limite zero = sem teto.
    assert_eq!(t.pagina(0, 0, Visao::Ativas).unwrap().len(), 30);
}

/// O contador atravessa o fechamento da tabela. Se voltasse ao 1, a próxima
/// linha nasceria com um número que já existe.
#[test]
fn o_contador_atravessa_o_disco() {
    let dir = DirTemp::novo("atravessa");
    {
        let mut t = com(7, &dir);
        t.sincronizar().unwrap();
    }
    let mut t = Table::abrir(&dir.0, "clientes").unwrap();
    assert_eq!(t.rownum_atual(), 8);
    let novo = t.inserir(&linha(8)).unwrap();
    assert_eq!(rownum(&mut t, novo), 8);
}

/// A busca binária pelo número de ordem: é ela que faz a página custar
/// `log N` em vez de `N` quando o cursor é um rownum e não um rowid.
#[test]
fn acha_o_rowid_pelo_numero_de_ordem() {
    let dir = DirTemp::novo("bisseccao");
    let mut t = com(1000, &dir);

    for alvo in [1u64, 2, 500, 999, 1000] {
        let r = t.rowid_do_rownum(alvo).unwrap().unwrap();
        assert_eq!(rownum(&mut t, r), alvo, "alvo {alvo} caiu no rowid errado");
    }
    // Além do fim não existe.
    assert!(t.rowid_do_rownum(1001).unwrap().is_none());
    // Zero cai na primeira linha: nenhum rownum é menor que 1.
    assert_eq!(t.rowid_do_rownum(0).unwrap(), Some(1));
}

/// E ela continua certa com buracos: exclusão física deixa slot morto, e a
/// bissecção tem de andar por cima deles sem errar a resposta.
#[test]
fn a_bisseccao_atravessa_os_buracos() {
    let dir = DirTemp::novo("buracos");
    let mut t = com(200, &dir);
    for r in (2..=200).step_by(2) {
        t.excluir_de_vez(r, "").unwrap();
    }
    // Sobraram os ímpares: rowid 1,3,5… com rownum 1,3,5…
    for alvo in [1u64, 3, 101, 199] {
        let r = t.rowid_do_rownum(alvo).unwrap().unwrap();
        assert_eq!(rownum(&mut t, r), alvo);
    }
    // Um alvo que caiu num buraco devolve o PRÓXIMO vivo, e não nada.
    let r = t.rowid_do_rownum(100).unwrap().unwrap();
    assert_eq!(rownum(&mut t, r), 101);
}

#[test]
fn pagina_desde_o_numero_de_ordem_inclui_o_alvo() {
    let dir = DirTemp::novo("desde");
    let mut t = com(50, &dir);
    let p = t.pagina_desde_rownum(20, 3, Visao::Ativas).unwrap();
    assert_eq!(p, vec![20, 21, 22]);
    assert!(t
        .pagina_desde_rownum(999, 3, Visao::Ativas)
        .unwrap()
        .is_empty());
}

// ---------------------------------------------------------------------------
// O salto por posição — o `OFFSET` que deixou de andar até lá
// ---------------------------------------------------------------------------

/// A regra inteira em um teste: numa tabela intacta a posição É o `rownum`,
/// e por isso o salto pode ser uma bissecção.
#[test]
fn tabela_intacta_bisseta_a_posicao() {
    let dir = DirTemp::novo("bisseta");
    let mut t = com(500, &dir);

    assert!(t.posicao_e_rownum(Visao::Ativas));
    let (p, como) = t.pagina_por_posicao(300, 5, Visao::Ativas).unwrap();
    assert_eq!(como, Salto::Bissecao);
    assert_eq!(p, vec![301, 302, 303, 304, 305]);
    // E a resposta é IGUAL à de andar até lá — que é o ponto todo.
    assert_eq!(p, t.pagina(300, 5, Visao::Ativas).unwrap());
}

/// As quatro coisas que derrubam a igualdade entre posição e `rownum`.
/// Cada uma tem de derrubar sozinha.
#[test]
fn o_que_derruba_o_salto_derruba_um_de_cada_vez() {
    // 1. Exclusão física: o número saiu e não volta.
    let dir = DirTemp::novo("salto-fisica");
    let mut t = com(50, &dir);
    assert!(t.posicao_e_rownum(Visao::Ativas));
    t.excluir_de_vez(10, "").unwrap();
    assert!(!t.posicao_e_rownum(Visao::Ativas));

    // 2. Exclusão suave, na visão comum: a linha some da lista mas fica no
    //    arquivo, com o número dela.
    let dir = DirTemp::novo("salto-suave");
    let mut t = com(50, &dir);
    t.excluir_suave(10, "teste").unwrap();
    assert!(!t.posicao_e_rownum(Visao::Ativas));
    // Em `Todas` ela continua na lista, e a igualdade se mantém.
    assert!(t.posicao_e_rownum(Visao::Todas));
    // Restaurar devolve a igualdade: o contador desce junto.
    t.restaurar(10, "voltou").unwrap();
    assert!(t.posicao_e_rownum(Visao::Ativas));

    // 3. A visão das excluídas nunca: a décima marcada pode ser a linha três.
    assert!(!t.posicao_e_rownum(Visao::Excluidas));

    // 4. Tabela sem a coluna de sistema — sem número de ordem não há o que
    //    procurar. `do_disco` não acrescenta as colunas de sistema.
    let sem = Schema::do_disco(
        "sem_ordem",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![],
    )
    .unwrap();
    let dir = DirTemp::novo("salto-sem-coluna");
    let mut t2 = Table::criar(&dir.0, sem).unwrap();
    for i in 1..=10 {
        t2.inserir(&linha(i)).unwrap();
    }
    assert!(t2.esquema().coluna_rownum().is_none());
    assert!(!t2.posicao_e_rownum(Visao::Ativas));
    // E ainda assim a página sai certa — pelo caminho caro.
    let (p, como) = t2.pagina_por_posicao(3, 2, Visao::Ativas).unwrap();
    assert_eq!(como, Salto::Passo);
    assert_eq!(p, vec![4, 5]);
}

/// Com buracos, os dois caminhos têm de dar a MESMA página. É o que garante
/// que trocar de caminho seja uma decisão de preço e não de resultado.
#[test]
fn com_buraco_o_caminho_muda_e_a_pagina_nao() {
    let dir = DirTemp::novo("buraco-mesma-pagina");
    let mut t = com(200, &dir);
    for r in [3u64, 17, 88, 150] {
        t.excluir_de_vez(r, "").unwrap();
    }
    t.excluir_suave(40, "sumiu").unwrap();

    for pular in [0u64, 1, 50, 120, 190, 300] {
        let (p, como) = t.pagina_por_posicao(pular, 7, Visao::Ativas).unwrap();
        assert_eq!(como, Salto::Passo, "pular {pular}");
        assert_eq!(
            p,
            t.pagina(pular, 7, Visao::Ativas).unwrap(),
            "pular {pular}"
        );
    }
}

/// A primeira página nunca bisseta: não há o que pular, e procurar o começo
/// custaria uma busca para achar o que já se sabe.
#[test]
fn a_primeira_pagina_nao_procura_nada() {
    let dir = DirTemp::novo("primeira");
    let mut t = com(100, &dir);
    let (p, como) = t.pagina_por_posicao(0, 3, Visao::Ativas).unwrap();
    assert_eq!(como, Salto::Passo);
    assert_eq!(p, vec![1, 2, 3]);
}

/// Passar do fim devolve página vazia pelos dois caminhos, e não erro.
#[test]
fn passar_do_fim_devolve_pagina_vazia() {
    let dir = DirTemp::novo("do-fim");
    let mut t = com(20, &dir);
    let (p, como) = t.pagina_por_posicao(999, 10, Visao::Ativas).unwrap();
    assert_eq!(como, Salto::Bissecao);
    assert!(p.is_empty());
}

// ---------------------------------------------------------------------------
// Contar sem varrer
// ---------------------------------------------------------------------------

/// `contar` sai de dois números do cabeçalho. Tem de bater com a varredura
/// nas três visões, depois de exclusão suave, restauração e exclusão física.
#[test]
fn contar_sem_varrer_bate_com_a_varredura() {
    let dir = DirTemp::novo("contar");
    let mut t = com(60, &dir);

    let confere = |t: &mut Table, onde: &str| {
        for visao in [Visao::Ativas, Visao::Excluidas, Visao::Todas] {
            let varrido = t.varrer_com(visao).unwrap().len() as u64;
            assert_eq!(
                t.contar(visao),
                varrido,
                "{onde}: {visao:?} contou diferente da varredura"
            );
        }
    };

    confere(&mut t, "recém-criada");
    for r in [2u64, 5, 9, 30] {
        t.excluir_suave(r, "teste").unwrap();
    }
    confere(&mut t, "com quatro marcadas");
    t.restaurar(5, "voltou").unwrap();
    confere(&mut t, "depois de restaurar uma");
    // Excluir de vez uma que estava marcada tem de baixar os DOIS contadores.
    t.excluir_de_vez(9, "some").unwrap();
    confere(&mut t, "marcada que saiu de vez");
    // E uma que não estava marcada baixa só o de registros.
    t.excluir_de_vez(11, "some").unwrap();
    confere(&mut t, "ativa que saiu de vez");
    // Marcar duas vezes não conta duas vezes.
    t.excluir_suave(2, "de novo").unwrap();
    confere(&mut t, "marcar de novo o que já estava");
}

/// O contador mora no cabeçalho: tem de sobreviver a fechar e reabrir.
#[test]
fn o_contador_de_marcadas_volta_do_disco() {
    let dir = DirTemp::novo("marcadas-disco");
    {
        let mut t = com(30, &dir);
        for r in [4u64, 8, 15] {
            t.excluir_suave(r, "teste").unwrap();
        }
        t.sincronizar().unwrap();
        assert_eq!(t.marcadas(), 3);
    }
    let t = Table::abrir(&dir.0, "clientes").unwrap();
    assert_eq!(t.marcadas(), 3);
    assert_eq!(t.contar(Visao::Ativas), 27);
    assert!(!t.posicao_e_rownum(Visao::Ativas));
}

/// `verificar` reconta varrendo em vez de acreditar no cabeçalho — é assim
/// que um contador de cache continua confiável.
#[test]
fn a_conferencia_reconta_as_marcadas() {
    let dir = DirTemp::novo("reconta");
    let mut t = com(40, &dir);
    for r in [1u64, 2, 3, 4, 5] {
        t.excluir_suave(r, "teste").unwrap();
    }
    let r = t.verificar().unwrap();
    assert_eq!(r.marcadas, 5);
    assert_eq!(r.marcadas, t.marcadas());
    assert_eq!(t.recontar_marcadas().unwrap(), 5);
}

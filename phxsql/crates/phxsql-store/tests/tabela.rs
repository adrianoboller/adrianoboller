//! Testes de integracao da tabela completa: os quatro arquivos juntos.

mod comum;

use comum::{DirTemp, Rng};

use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const ID: usize = 0;
const NOME: usize = 1;
const CIDADE: usize = 2;
const LIMITE: usize = 3;
const CADASTRO: usize = 4;
const FOTO: usize = 5;
const FICHA: usize = 6;

fn esquema() -> Schema {
    Schema::new(
        "cadastroClientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("cadastro", ColumnType::Date),
            Column::new("foto", ColumnType::Bin),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(ID)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(NOME).sem_caixa()]),
            // Cidade crescente, limite decrescente: o maior cliente da cidade
            // aparece primeiro.
            IndexDef::new(
                "porCidadeLimite",
                vec![IndexColumn::asc(CIDADE), IndexColumn::desc(LIMITE)],
            ),
        ],
    )
    .unwrap()
}

fn cliente(id: i64, nome: &str, cidade: &str, limite: i128) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(nome.into()),
        Value::Str(cidade.into()),
        Value::Decimal(limite),
        Value::Date(20_000),
        Value::Null,
        Value::Null,
    ]
}

#[test]
fn os_quatro_arquivos_formam_a_tabela() {
    let dir = DirTemp::novo("quarteto");
    let t = Table::criar(&dir.0, esquema()).unwrap();
    drop(t);

    for ext in ["reg", "ndx", "bin", "memo"] {
        let c = dir.0.join(format!("cadastroClientes.{ext}"));
        assert!(c.exists(), "faltou {}", c.display());
    }

    // Criar por cima e recusado, para nunca sobrescrever dados.
    assert!(Table::criar(&dir.0, esquema()).is_err());
}

#[test]
fn ciclo_completo_com_memo_e_binario() {
    let dir = DirTemp::novo("ciclo");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    let foto: Vec<u8> = (0..5_000u32).map(|i| (i % 251) as u8).collect();
    let ficha = "Cliente antigo. ".repeat(500);

    // Decimal(15,2): 150_000 = R$ 1.500,00
    let mut linha = cliente(1, "Adriano Boller", "Blumenau", 150_000);
    linha[FOTO] = Value::Bin(foto.clone());
    linha[FICHA] = Value::Memo(ficha.clone());

    let rowid = t.inserir(&linha).unwrap();
    assert_eq!(rowid, 1);

    let lida = t.ler(rowid).unwrap().unwrap();
    assert_eq!(lida[ID], Value::Int(1));
    assert_eq!(lida[NOME], Value::Str("Adriano Boller".into()));
    assert_eq!(lida[CIDADE], Value::Str("Blumenau".into()));
    assert_eq!(lida[LIMITE], Value::Decimal(150_000));
    assert_eq!(lida[CADASTRO], Value::Date(20_000));
    assert_eq!(lida[FOTO], Value::Bin(foto));
    assert_eq!(lida[FICHA], Value::Memo(ficha));

    t.verificar().unwrap();
}

#[test]
fn reg_guarda_a_ordem_de_digitacao_e_o_ndx_a_ordem_da_chave() {
    let dir = DirTemp::novo("ordem");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    // Digitados fora de ordem alfabetica de proposito.
    for (id, nome) in [
        (10, "Zuleica"),
        (20, "Adriano"),
        (30, "Marcos"),
        (40, "Beatriz"),
    ] {
        t.inserir(&cliente(id, nome, "Blumenau", 100)).unwrap();
    }

    // O .reg devolve na ordem em que foram digitados.
    let digitados: Vec<String> = t
        .varrer()
        .unwrap()
        .iter()
        .map(|(_, l)| l[NOME].como_str().unwrap().to_string())
        .collect();
    assert_eq!(digitados, ["Zuleica", "Adriano", "Marcos", "Beatriz"]);

    // O .ndx devolve na ordem da chave.
    let por_nome = t.varrer_indice("porNome").unwrap();
    assert_eq!(por_nome, vec![2, 4, 3, 1]);

    t.verificar().unwrap();
}

#[test]
fn busca_por_indice_unico_e_composto() {
    let dir = DirTemp::novo("busca");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    t.inserir(&cliente(1, "Alfa", "Blumenau", 300)).unwrap();
    t.inserir(&cliente(2, "Bravo", "Blumenau", 900)).unwrap();
    t.inserir(&cliente(3, "Charlie", "Blumenau", 600)).unwrap();
    t.inserir(&cliente(4, "Delta", "Joinville", 500)).unwrap();

    assert_eq!(t.buscar("porId", &[Value::Int(3)]).unwrap(), vec![3]);
    assert!(t.buscar("porId", &[Value::Int(99)]).unwrap().is_empty());

    // NOCASE: procura em minusculas acha o registro gravado com maiuscula.
    assert_eq!(
        t.buscar("porNome", &[Value::Str("bravo".into())]).unwrap(),
        vec![2]
    );

    // Indice composto (cidade ASC, limite DESC): dentro de Blumenau, o maior
    // limite vem primeiro.
    let ordem = t.varrer_indice("porCidadeLimite").unwrap();
    assert_eq!(ordem, vec![2, 3, 1, 4]);

    // Intervalo por nome.
    let faixa = t
        .intervalo(
            "porNome",
            Some(&[Value::Str("BRAVO".into())]),
            Some(&[Value::Str("DELTA".into())]),
        )
        .unwrap();
    assert_eq!(faixa, vec![2, 3, 4]);

    t.verificar().unwrap();
}

#[test]
fn alteracao_move_a_chave_no_indice() {
    let dir = DirTemp::novo("altera");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    let rowid = t.inserir(&cliente(1, "Antigo", "Blumenau", 100)).unwrap();

    assert_eq!(
        t.buscar("porNome", &[Value::Str("Antigo".into())]).unwrap(),
        vec![1]
    );

    t.atualizar(rowid, &cliente(1, "Novo", "Blumenau", 100))
        .unwrap();

    assert!(t
        .buscar("porNome", &[Value::Str("Antigo".into())])
        .unwrap()
        .is_empty());
    assert_eq!(
        t.buscar("porNome", &[Value::Str("Novo".into())]).unwrap(),
        vec![1]
    );
    // O registro nao mudou de lugar no .reg.
    assert_eq!(t.slots(), 1);
    assert_eq!(t.registros(), 1);
    t.verificar().unwrap();
}

#[test]
fn exclusao_tira_do_indice_e_libera_o_memo() {
    let dir = DirTemp::novo("exclui");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    let mut linha = cliente(1, "Some", "Blumenau", 100);
    linha[FICHA] = Value::Memo("texto grande ".repeat(200));
    t.inserir(&linha).unwrap();
    t.inserir(&cliente(2, "Fica", "Blumenau", 100)).unwrap();

    assert!(t.excluir(1).unwrap());
    assert!(!t.excluir(1).unwrap(), "excluir de novo devolve false");

    assert!(t.ler(1).unwrap().is_none());
    assert!(t.buscar("porId", &[Value::Int(1)]).unwrap().is_empty());
    assert_eq!(t.registros(), 1);
    assert_eq!(t.slots(), 2, "o slot excluido nao e reaproveitado");

    // O proximo insert vai para o slot 3.
    assert_eq!(t.inserir(&cliente(3, "Novo", "Itajai", 100)).unwrap(), 3);
    t.verificar().unwrap();
}

#[test]
fn indice_unico_e_not_null_sao_impostos() {
    let dir = DirTemp::novo("regras");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    t.inserir(&cliente(1, "Primeiro", "Blumenau", 100)).unwrap();

    let e = t
        .inserir(&cliente(1, "Repetido", "Blumenau", 100))
        .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "erro foi {e}");
    // A tentativa recusada nao pode ter deixado lixo.
    assert_eq!(t.registros(), 1);
    assert_eq!(t.slots(), 1);

    let mut sem_nome = cliente(2, "x", "Blumenau", 100);
    sem_nome[NOME] = Value::Null;
    assert!(matches!(
        t.inserir(&sem_nome).unwrap_err(),
        PhxError::Tipo(_)
    ));

    // Aridade errada.
    assert!(t.inserir(&[Value::Int(9)]).is_err());

    t.verificar().unwrap();
}

#[test]
fn valores_nulos_indexam_antes_dos_preenchidos() {
    let dir = DirTemp::novo("nulos");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    let mut sem_cidade = cliente(1, "Sem cidade", "", 100);
    sem_cidade[CIDADE] = Value::Null;
    t.inserir(&sem_cidade).unwrap();
    t.inserir(&cliente(2, "Com cidade", "Blumenau", 100))
        .unwrap();

    let ordem = t.varrer_indice("porCidadeLimite").unwrap();
    assert_eq!(ordem, vec![1, 2], "NULL deve vir primeiro em ASC");

    let lida = t.ler(1).unwrap().unwrap();
    assert_eq!(lida[CIDADE], Value::Null);
    t.verificar().unwrap();
}

#[test]
fn fecha_reabre_e_continua() {
    let dir = DirTemp::novo("reabre");
    {
        let mut t = Table::criar(&dir.0, esquema()).unwrap();
        for i in 1..=50i64 {
            let mut l = cliente(i, &format!("Cliente {i:03}"), "Blumenau", (i as i128) * 100);
            l[FICHA] = Value::Memo(format!("ficha do cliente {i}"));
            t.inserir(&l).unwrap();
        }
        t.sincronizar().unwrap();
    }

    let mut t = Table::abrir(&dir.0, "cadastroClientes").unwrap();
    assert_eq!(t.registros(), 50);
    assert_eq!(t.esquema().nome(), "cadastroClientes");
    // Sete declaradas mais a coluna de sistema, que atravessou o disco.
    assert_eq!(t.esquema().colunas().len(), 8);
    assert_eq!(t.esquema().coluna_softdeleted(), Some(7));
    assert_eq!(
        t.ler(7).unwrap().unwrap()[FICHA],
        Value::Memo("ficha do cliente 7".into())
    );
    assert_eq!(t.buscar("porId", &[Value::Int(42)]).unwrap(), vec![42]);

    t.inserir(&cliente(51, "Depois de reabrir", "Itajai", 1))
        .unwrap();
    assert_eq!(t.registros(), 51);
    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, 51);
    assert_eq!(rel.indices.len(), 3);
    for (_, qtd) in &rel.indices {
        assert_eq!(*qtd, 51);
    }
}

#[test]
fn carga_de_dois_mil_registros_com_alteracoes_e_exclusoes() {
    let dir = DirTemp::novo("carga");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();

    const N: i64 = 2_000;
    let mut ids: Vec<i64> = (1..=N).collect();
    Rng::nova(2026).embaralhar(&mut ids);

    for id in &ids {
        let mut l = cliente(
            *id,
            &format!("Cliente {id:05}"),
            if id % 2 == 0 { "Blumenau" } else { "Joinville" },
            (*id as i128) * 37,
        );
        if id % 10 == 0 {
            l[FICHA] = Value::Memo(format!("observacao longa {id} ").repeat(20));
        }
        t.inserir(&l).unwrap();
    }
    assert_eq!(t.registros(), N as u64);
    t.verificar().unwrap();

    // Toda chave e encontrada pelo indice unico.
    for id in &ids {
        assert_eq!(t.buscar("porId", &[Value::Int(*id)]).unwrap().len(), 1);
    }

    // A varredura pelo indice sai ordenada por id.
    let por_id = t.varrer_indice("porId").unwrap();
    let ordenado: Vec<u64> = {
        let mut pares: Vec<(i64, u64)> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i as u64 + 1))
            .collect();
        pares.sort();
        pares.into_iter().map(|(_, r)| r).collect()
    };
    assert_eq!(por_id, ordenado);

    // Altera um em cada cinco.
    for rowid in (1..=N as u64).step_by(5) {
        let atual = t.ler(rowid).unwrap().unwrap();
        let id = atual[ID].como_i64().unwrap();
        t.atualizar(
            rowid,
            &cliente(id, &format!("Alterado {id:05}"), "Itajai", 1),
        )
        .unwrap();
    }
    t.verificar().unwrap();

    // Exclui um em cada sete.
    let mut excluidos = 0u64;
    for rowid in (1..=N as u64).step_by(7) {
        if t.excluir(rowid).unwrap() {
            excluidos += 1;
        }
    }
    assert_eq!(t.registros(), N as u64 - excluidos);
    assert_eq!(
        t.slots(),
        N as u64,
        "slots excluidos nao sao reaproveitados"
    );

    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, N as u64 - excluidos);
    for (nome, qtd) in &rel.indices {
        assert_eq!(*qtd, rel.registros, "indice {nome} fora de sincronia");
    }

    // A varredura do .reg continua na ordem de digitacao, pulando os buracos.
    let vivos = t.varrer().unwrap();
    assert_eq!(vivos.len(), rel.registros as usize);
    let rowids: Vec<u64> = vivos.iter().map(|(r, _)| *r).collect();
    let mut ordenados = rowids.clone();
    ordenados.sort_unstable();
    assert_eq!(rowids, ordenados);
}

/// Duas aberturas da MESMA tabela nao podem gravar as duas.
///
/// Este teste documenta um contrato, e nao um defeito: abrir uma tabela LE o
/// cabecalho, e o cabecalho traz `slot_count` e `proxima_sequencia` -- os dois
/// contadores que decidem onde a proxima linha vai. Duas aberturas guardam o
/// mesmo numero, e as duas gravam no mesmo rowid.
///
/// **Quem abre e responsavel por serializar.** O servidor faz isso segurando a
/// trava de dados do abrir ate o gravar, num bloco so. Ja fez errado: a trava
/// era tomada e solta na abertura, e duas operacoes simultaneas sobrescreviam
/// uma a outra em silencio -- a tabela ficava com uma linha a menos e ninguem
/// era avisado.
///
/// O teste existe para que a propriedade fique escrita: se um dia alguem
/// achar que pode abrir duas vezes e gravar nas duas, este teste mostra o que
/// acontece.
#[test]
fn duas_aberturas_da_mesma_tabela_disputam_o_mesmo_rowid() {
    let dir = DirTemp::novo("duas-aberturas");
    let esquema = Schema::new(
        "t",
        vec![
            Column::new("quem", ColumnType::Str(10)),
            Column::new("n", ColumnType::Int8),
        ],
        vec![],
    )
    .unwrap();
    {
        let mut t = Table::criar(dir.0.as_path(), esquema).unwrap();
        t.inserir(&[Value::Str("inicial".into()), Value::Int(0)])
            .unwrap();
        t.sincronizar().unwrap();
    }

    let mut a = Table::abrir(dir.0.as_path(), "t").unwrap();
    let mut b = Table::abrir(dir.0.as_path(), "t").unwrap();

    let ra = a.inserir(&[Value::Str("A".into()), Value::Int(1)]).unwrap();
    a.sincronizar().unwrap();
    let rb = b.inserir(&[Value::Str("B".into()), Value::Int(2)]).unwrap();
    b.sincronizar().unwrap();

    assert_eq!(
        ra, rb,
        "as duas aberturas escolhem o mesmo rowid -- e por isso quem abre \
         precisa serializar"
    );

    // E a prova de que uma sobrescreveu a outra: tres insercoes, dois slots.
    let mut c = Table::abrir(dir.0.as_path(), "t").unwrap();
    assert_eq!(c.slots(), 2);
    match &c.ler(2).unwrap().unwrap()[0] {
        Value::Str(s) => assert_eq!(s, "B", "a segunda gravacao ficou por cima"),
        outro => panic!("esperava texto, veio {outro:?}"),
    }
}

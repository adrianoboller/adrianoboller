//! Replicação: o diário com a imagem da linha, e a réplica que a aplica.
//!
//! O que estes testes protegem:
//!
//! 1. a imagem carrega o **conteúdo** dos anexos, e não os ponteiros — os
//!    offsets do `.bin` do source não valem no `.bin` da réplica;
//! 2. aplicar todos os eventos na ordem reproduz a tabela **byte a byte**,
//!    rowid por rowid, sem transmitir nem negociar rowid nenhum;
//! 3. uma réplica que divergiu **para**, em vez de espalhar a divergência;
//! 4. o diário sem imagem continua funcionando, e um evento sem imagem é
//!    recusado na aplicação com a mensagem que diz o que ligar.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::table::{Table, Visao};

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 12,
                    escala: 2,
                },
            ),
            Column::new("ficha", ColumnType::Memo),
            Column::new("foto", ColumnType::Bin),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:04}")),
        Value::Decimal(i as i128 * 150),
        Value::Memo(format!(
            "ficha longa do cliente {i}, com texto que mora no .memo"
        )),
        Value::Bin(vec![(i % 251) as u8; 40 + (i as usize % 17)]),
    ]
}

/// Um source com a imagem ligada, e uma réplica vazia com o mesmo esquema.
fn par(dir_s: &DirTemp, dir_r: &DirTemp) -> (Table, Table) {
    let s = Table::criar(&dir_s.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    let r = Table::criar(&dir_r.0, esquema()).unwrap();
    (s, r)
}

/// Aplica na réplica tudo o que o source registrou a partir de `desde`.
fn replicar(source: &mut Table, replica: &mut Table, desde: u64) -> u64 {
    let eventos = source.diario_com_imagem(desde, 0).unwrap();
    let n = eventos.len() as u64;
    for (e, imagem) in eventos {
        replica
            .aplicar_evento(e.operacao, e.rowid, &imagem)
            .unwrap();
    }
    desde + n
}

/// O teste que vale por todos: aplicar os eventos na ordem produz a mesma
/// tabela, com os mesmos rowids, sem ninguém combinar rowid nenhum.
#[test]
fn a_replica_reproduz_o_source_linha_por_linha() {
    let ds = DirTemp::novo("rep-source");
    let dr = DirTemp::novo("rep-replica");
    let (mut s, mut r) = par(&ds, &dr);

    for i in 1..=50 {
        s.inserir(&linha(i)).unwrap();
    }
    // Alterações e exclusões entram na mesma sequência.
    s.atualizar(7, &{
        let mut l = linha(7);
        l[1] = Value::Str("Cliente ALTERADO".into());
        l[3] = Value::Memo(
            "ficha trocada, bem maior que a de antes, para o bloco mudar de lugar".into(),
        );
        l
    })
    .unwrap();
    s.excluir_suave(9, "sumiu").unwrap();
    s.excluir_de_vez(11, "some de vez").unwrap();

    replicar(&mut s, &mut r, 0);

    assert_eq!(r.registros(), s.registros(), "contagem de registros");
    assert_eq!(r.marcadas(), s.marcadas(), "contagem de marcadas");

    let ls = s.varrer_com(Visao::Todas).unwrap();
    let lr = r.varrer_com(Visao::Todas).unwrap();
    assert_eq!(ls.len(), lr.len(), "quantidade de linhas");
    for ((rs, vs), (rr, vr)) in ls.iter().zip(lr.iter()) {
        assert_eq!(rs, rr, "o rowid saiu diferente sem ninguém combinar rowid");
        assert_eq!(vs, vr, "a linha {rs} saiu diferente");
    }
}

/// Os anexos são o caso em que copiar o ponteiro daria bloco errado: os
/// offsets do `.bin` do source não valem no `.bin` da réplica. A imagem leva o
/// CONTEÚDO, e este teste prova que o conteúdo está dentro dela.
#[test]
fn os_anexos_atravessam_com_conteudo_e_nao_com_ponteiro() {
    let ds = DirTemp::novo("rep-anexo-s");
    let dr = DirTemp::novo("rep-anexo-r");
    let (mut s, mut r) = par(&ds, &dr);

    // Anexos antes, para a linha que interessa cair longe do começo do `.bin`
    // e do `.memo` do source — e o ponteiro dela não ser um número pequeno.
    for i in 1..=6 {
        s.inserir(&linha(i)).unwrap();
    }
    let foto = vec![0xABu8; 5000];
    let ficha = "memo grande ".repeat(500);
    s.inserir(&[
        Value::Int(7),
        Value::Str("Com anexo".into()),
        Value::Decimal(0),
        Value::Memo(ficha.clone()),
        Value::Bin(foto.clone()),
    ])
    .unwrap();

    // A imagem do último evento: o conteúdo tem de estar DENTRO dela.
    let eventos = s.diario_com_imagem(6, 0).unwrap();
    assert_eq!(eventos.len(), 1);
    let (_, imagem) = &eventos[0];
    let (payload, externos) = Table::abrir_imagem(imagem).unwrap();
    assert!(
        imagem.len() > payload.len() + foto.len() + ficha.len(),
        "a imagem tem {} bytes: cabe o ponteiro, não cabe o anexo",
        imagem.len()
    );
    assert_eq!(externos.len(), 2, "as duas colunas externas");
    let memo = externos.iter().find(|(c, _)| *c == 3).unwrap();
    let bin = externos.iter().find(|(c, _)| *c == 4).unwrap();
    assert_eq!(memo.1, ficha.as_bytes(), "o memo não é o conteúdo");
    assert_eq!(bin.1, foto, "o binário não é o conteúdo");

    // E do outro lado a linha volta inteira, com blocos gravados aqui.
    replicar(&mut s, &mut r, 0);
    let l = r.ler(7).unwrap().unwrap();
    assert_eq!(l[3], Value::Memo(ficha));
    assert_eq!(l[4], Value::Bin(foto));
    r.verificar().unwrap();
}

/// Réplica que divergiu para na hora. É a garantia que o rowid dá de graça.
#[test]
fn replica_que_divergiu_para_em_vez_de_espalhar() {
    let ds = DirTemp::novo("rep-div-s");
    let dr = DirTemp::novo("rep-div-r");
    let (mut s, mut r) = par(&ds, &dr);

    // Alguém escreveu na réplica — o que `somente_leitura` existe para impedir.
    r.inserir(&linha(999)).unwrap();

    s.inserir(&linha(1)).unwrap();
    let eventos = s.diario_com_imagem(0, 0).unwrap();
    let (e, imagem) = &eventos[0];
    let erro = r.aplicar_evento(e.operacao, e.rowid, imagem).unwrap_err();
    let texto = erro.to_string();
    assert!(texto.contains("divergiu"), "mensagem sem o motivo: {texto}");
    assert!(
        texto.contains("rowid 1"),
        "mensagem sem o rowid do source: {texto}"
    );
}

/// Sem o interruptor, o diário continua com 44 bytes por evento e a aplicação
/// recusa dizendo o que ligar — em vez de gravar linha vazia.
#[test]
fn evento_sem_imagem_e_recusado_com_o_motivo() {
    let ds = DirTemp::novo("rep-sem-s");
    let dr = DirTemp::novo("rep-sem-r");
    let mut s = Table::criar(&ds.0, esquema()).unwrap(); // imagem DESLIGADA
    let mut r = Table::criar(&dr.0, esquema()).unwrap();

    s.inserir(&linha(1)).unwrap();
    let eventos = s.diario_com_imagem(0, 0).unwrap();
    assert_eq!(eventos.len(), 1);
    assert!(
        eventos[0].1.is_empty(),
        "o diário gravou imagem sem ninguém pedir"
    );
    assert_eq!(eventos[0].0.tam_imagem, 0);

    let erro = r.aplicar_evento(Operacao::Inclusao, 1, &[]).unwrap_err();
    assert!(
        erro.to_string().contains("imagem_da_linha"),
        "a mensagem não diz o que ligar: {erro}"
    );
}

/// Replicar em duas rodadas dá o mesmo que replicar tudo de uma vez: é o que
/// permite a réplica guardar UM número por tabela e continuar de onde parou.
#[test]
fn retomar_da_posicao_da_o_mesmo_resultado() {
    let ds = DirTemp::novo("rep-ret-s");
    let dr = DirTemp::novo("rep-ret-r");
    let (mut s, mut r) = par(&ds, &dr);

    for i in 1..=20 {
        s.inserir(&linha(i)).unwrap();
    }
    let posicao = replicar(&mut s, &mut r, 0);
    assert_eq!(posicao, 20);

    for i in 21..=35 {
        s.inserir(&linha(i)).unwrap();
    }
    s.atualizar(3, &linha(300)).unwrap();
    let posicao = replicar(&mut s, &mut r, posicao);
    assert_eq!(posicao, 36);

    assert_eq!(
        s.varrer_com(Visao::Todas).unwrap(),
        r.varrer_com(Visao::Todas).unwrap()
    );
    // Pedir de novo a partir da posição atual não traz nada — e não repete.
    assert!(s.diario_com_imagem(posicao, 0).unwrap().is_empty());
}

/// A imagem é o payload cru: decimal, data e hora atravessam sem reencodar.
#[test]
fn a_imagem_leva_o_payload_cru_e_nao_o_texto() {
    let ds = DirTemp::novo("rep-cru-s");
    let dr = DirTemp::novo("rep-cru-r");
    let (mut s, mut r) = par(&ds, &dr);

    // Um decimal que perde precisão se passar por f64.
    s.inserir(&[
        Value::Int(1),
        Value::Str("Centavos".into()),
        Value::Decimal(999_999_999_999i128),
        Value::Null,
        Value::Null,
    ])
    .unwrap();
    replicar(&mut s, &mut r, 0);
    assert_eq!(
        r.ler(1).unwrap().unwrap()[2],
        Value::Decimal(999_999_999_999)
    );
}

// ------------------------------------------- a marca de posicao do diário

/// Ler em lotes com a marca tem de dar exatamente o mesmo que ler de uma vez.
///
/// A marca é uma otimização num caminho onde errar não dá erro — dá **evento
/// errado**, aplicado como se fosse o certo. Este teste é o que trava isso.
#[test]
fn a_marca_da_exatamente_os_mesmos_eventos() {
    let dir = DirTemp::novo("marca-igual");
    let mut t = Table::criar(dir.0.join("s"), esquema()).unwrap();
    t.ligar_imagem_no_diario(true);
    for i in 1..=2_000 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();

    // De uma vez, sem marca nenhuma.
    t.definir_marca_do_diario(None);
    let inteiro = t.diario_com_imagem(0, 0).unwrap();
    assert_eq!(inteiro.len(), 2_000);

    // Em lotes de 137 — número que não divide 2.000, para o último lote ser
    // parcial e a marca ter de sobreviver a isso.
    t.definir_marca_do_diario(None);
    let mut em_lotes = Vec::new();
    let mut desde = 0u64;
    loop {
        let lote = t.diario_com_imagem(desde, 137).unwrap();
        if lote.is_empty() {
            break;
        }
        desde += lote.len() as u64;
        em_lotes.extend(lote);
    }
    assert_eq!(em_lotes.len(), inteiro.len());
    for (i, (a, b)) in inteiro.iter().zip(em_lotes.iter()).enumerate() {
        assert_eq!(a.0, b.0, "evento {i} veio diferente");
        assert_eq!(a.1, b.1, "imagem do evento {i} veio diferente");
    }
}

#[test]
fn a_marca_e_so_uma_dica_e_a_de_tras_ainda_serve() {
    let dir = DirTemp::novo("marca-tras");
    let mut t = Table::criar(dir.0.join("s"), esquema()).unwrap();
    t.ligar_imagem_no_diario(true);
    for i in 1..=500 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();

    // Avança até 300 e guarda onde parou.
    t.definir_marca_do_diario(None);
    let _ = t.diario_com_imagem(0, 300).unwrap();
    let marca = t
        .marca_do_diario()
        .expect("a leitura devia ter deixado marca");
    assert_eq!(marca.evento, 300);

    // Uma réplica atrasada pede de 100: a marca de 300 está À FRENTE e não
    // pode ser usada, senão ela receberia os eventos errados.
    t.definir_marca_do_diario(Some(marca));
    let atrasada = t.diario_com_imagem(100, 10).unwrap();
    t.definir_marca_do_diario(None);
    let sem_marca = t.diario_com_imagem(100, 10).unwrap();
    assert_eq!(atrasada.len(), 10);
    assert_eq!(atrasada[0].0, sem_marca[0].0);
    assert_eq!(atrasada[0].1, sem_marca[0].1);

    // E uma que pede exatamente de onde a marca aponta usa o atalho e recebe
    // o mesmo que receberia varrendo tudo.
    t.definir_marca_do_diario(Some(marca));
    let com = t.diario_com_imagem(300, 10).unwrap();
    t.definir_marca_do_diario(None);
    let sem = t.diario_com_imagem(300, 10).unwrap();
    assert_eq!(com.len(), 10);
    for (a, b) in com.iter().zip(sem.iter()) {
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
    }
}

#[test]
fn a_marca_atravessa_a_troca_de_volume() {
    // Volumes pequenos: a marca tem de continuar valendo quando a leitura
    // passa de um arquivo para o seguinte, que é onde o `qtd_eventos` do
    // cabeçalho deixa de poder pular o volume inteiro.
    let dir = DirTemp::novo("marca-volume");
    let esq = esquema()
        .com_paginacao(phxsql_core::paginacao::Paginacao::nova(200, 500).unwrap())
        .unwrap();
    let mut t = Table::criar(dir.0.join("s"), esq).unwrap();
    t.ligar_imagem_no_diario(true);
    for i in 1..=1_200 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();

    t.definir_marca_do_diario(None);
    let inteiro = t.diario_com_imagem(0, 0).unwrap();

    t.definir_marca_do_diario(None);
    let mut em_lotes = Vec::new();
    let mut desde = 0u64;
    while let Ok(lote) = t.diario_com_imagem(desde, 50) {
        if lote.is_empty() {
            break;
        }
        desde += lote.len() as u64;
        em_lotes.extend(lote);
    }
    assert_eq!(em_lotes.len(), inteiro.len());
    for (i, (a, b)) in inteiro.iter().zip(em_lotes.iter()).enumerate() {
        assert_eq!(a.0, b.0, "evento {i} diferente ao trocar de volume");
    }
}

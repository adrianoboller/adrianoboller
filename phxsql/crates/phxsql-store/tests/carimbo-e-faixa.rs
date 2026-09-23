//! O `PSCH` v10: o carimbo de criacao por linha, a faixa da `Sequence` e a
//! recusa da tabela-cadeia.
//!
//! Pedidos 289 (carimbo), 290 (`passo` da sequencia) e 314 (ledger fica no
//! formato anterior). Cada prova aqui tem o defeito que ela repoe escrito ao
//! lado -- teste que passa por engano e pior que teste que falta.

mod comum;

use comum::DirTemp;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema, COLUNA_ROWSTAMP, COLUNA_ROWTIME};
use phxsql_core::types::ColumnType;
use phxsql_core::uuid::Uuid;
use phxsql_core::value::Value;
use phxsql_store::ledger::Verificacao;
use phxsql_store::{no, preparar_bloco, verificar_cadeia, Table};

// ------------------------------------------------------------ esquemas

fn esquema_maes() -> Schema {
    Schema::new(
        "maes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn esquema_filhas() -> Schema {
    Schema::new(
        "filhas",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("mae", ColumnType::Int8).obrigatoria(),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porMae", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

/// O esquema como ele sai do disco de uma tabela ANTERIOR ao v10.
///
/// `do_disco` e o caminho de LER: ele monta exatamente as colunas dadas e nao
/// acrescenta nenhuma. E o que um motor v10 obtem ao abrir um bloco v9 -- as
/// duas colunas de sistema de antes, e nenhuma das de carimbo.
fn esquema_antes_do_v10() -> Schema {
    Schema::do_disco(
        "antigas",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new(phxsql_core::schema::COLUNA_SOFTDELETED, ColumnType::Bool).obrigatoria(),
            Column::new(phxsql_core::schema::COLUNA_ROWNUM, ColumnType::UInt8).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn esquema_blocos() -> Schema {
    Schema::new(
        "blocos",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("hash", ColumnType::Uuid256).obrigatoria(),
            Column::new("anterior", ColumnType::Uuid256),
            Column::new("altura", ColumnType::Sequence),
            Column::new("autor", ColumnType::Str(60)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porHash", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porAltura", vec![IndexColumn::asc(3)]).unico(),
        ],
    )
    .unwrap()
}

fn carimbo_de(t: &mut Table, rowid: u64) -> u64 {
    let i = t.esquema().coluna_rowstamp().expect("sem rowstamp");
    match &t.ler(rowid).unwrap().unwrap()[i] {
        Value::UInt(n) => *n,
        outro => panic!("o carimbo nao e UInt: {outro:?}"),
    }
}

fn relogio_de(t: &mut Table, rowid: u64) -> i64 {
    let i = t.esquema().coluna_rowtime().expect("sem rowtime");
    match &t.ler(rowid).unwrap().unwrap()[i] {
        Value::DateTime(ms) => *ms,
        outro => panic!("o relogio nao e DateTime: {outro:?}"),
    }
}

// ------------------------------------------ 289: a ordem de criacao

/// **A prova que o pedido 289 existe para dar.**
///
/// O pai e a filha estao em tabelas diferentes -- estao sempre, porque a chave
/// estrangeira e declarada na filha. Gravados um em seguida do outro, o
/// carimbo do pai tem de ser ESTRITAMENTE menor.
///
/// # O defeito que ela repoe
///
/// Um contador por tabela, em vez de por processo: as duas tabelas emitiriam
/// `1` e o teste veria `1 == 1`. Foi por isso que o contador foi para um
/// `AtomicU64` do processo, e nao para um campo do `Table`.
#[test]
fn o_pai_nunca_tem_o_mesmo_carimbo_da_filha() {
    let d = DirTemp::novo("pai-filha");
    let mut maes = Table::criar(&d.0, esquema_maes()).unwrap();
    let mut filhas = Table::criar(&d.0, esquema_filhas()).unwrap();

    for i in 1..=50i64 {
        let pai = maes
            .inserir(&[Value::Int(i), Value::Str(format!("mae {i}"))])
            .unwrap();
        let filha = filhas.inserir(&[Value::Int(i), Value::Int(i)]).unwrap();
        let (cp, cf) = (carimbo_de(&mut maes, pai), carimbo_de(&mut filhas, filha));
        assert!(
            cp < cf,
            "o pai {cp} nao veio antes da filha {cf} na rodada {i}"
        );
    }
}

/// **O avanco forcado: N linhas no mesmo instante de relogio, N carimbos.**
///
/// Mede quantas linhas caem no MESMO milissegundo de `rowtime` e exige que
/// nenhuma delas empate no `rowstamp`. E a medida que mata a proposta de
/// carimbar pelo relogio: com resolucao de milissegundo a coluna empataria
/// tantas vezes quantas o numero impresso abaixo.
///
/// # O defeito que ela repoe
///
/// Trocar o contador pelo relogio (`agora_ms()` nas duas colunas) faz o
/// `distintos` cair para o numero de milissegundos distintos, e a asserção
/// falha com o numero medido na mensagem.
#[test]
fn mil_linhas_no_mesmo_instante_saem_com_mil_carimbos() {
    const N: i64 = 1000;
    let d = DirTemp::novo("mesmo-instante");
    let mut t = Table::criar(&d.0, esquema_maes()).unwrap();

    let mut carimbos = Vec::with_capacity(N as usize);
    let mut relogios = Vec::with_capacity(N as usize);
    for i in 1..=N {
        let r = t.inserir(&[Value::Int(i), Value::Str("x".into())]).unwrap();
        carimbos.push(carimbo_de(&mut t, r));
        relogios.push(relogio_de(&mut t, r));
    }

    let mut distintos = carimbos.clone();
    distintos.sort_unstable();
    distintos.dedup();
    assert_eq!(
        distintos.len(),
        N as usize,
        "o carimbo empatou em {} linhas",
        N as usize - distintos.len()
    );
    for j in 1..carimbos.len() {
        assert!(
            carimbos[j] > carimbos[j - 1],
            "o carimbo recuou entre a linha {j} e a anterior"
        );
    }

    // E o lado que prova que o relogio NAO servia: quantos milissegundos
    // distintos houve para as mesmas N linhas.
    let mut ms = relogios.clone();
    ms.sort_unstable();
    ms.dedup();
    println!(
        "{N} linhas: {} carimbos distintos, {} milissegundos distintos \
         (o maior empate de relogio tem {} linhas)",
        distintos.len(),
        ms.len(),
        ms.iter()
            .map(|m| relogios.iter().filter(|r| *r == m).count())
            .max()
            .unwrap_or(0)
    );
    assert!(
        ms.len() < N as usize,
        "nesta maquina o relogio nao empatou nenhuma vez em {N} linhas; \
         a medicao que motivou o pedido 289 nao se reproduziu aqui"
    );
}

/// A alteracao MANTEM o carimbo, e a linha anterior ao v10 fica em zero.
///
/// # O defeito que ela repoe
///
/// Renovar o carimbo no `atualizar` (chamar `carimbar_linha` com `anterior:
/// None`). Um pai alterado depois da filha ficaria com carimbo maior que o
/// dela, e a pergunta «o pai veio antes?» passaria a responder errado sobre
/// dado que esta certo no disco.
#[test]
fn a_alteracao_nao_renova_o_carimbo() {
    let d = DirTemp::novo("alterar");
    let mut t = Table::criar(&d.0, esquema_maes()).unwrap();
    let r = t
        .inserir(&[Value::Int(1), Value::Str("antes".into())])
        .unwrap();
    let antes = carimbo_de(&mut t, r);
    let relogio_antes = relogio_de(&mut t, r);

    // Outras linhas andam o contador no meio do caminho.
    for i in 2..=10i64 {
        t.inserir(&[Value::Int(i), Value::Str("x".into())]).unwrap();
    }
    t.atualizar(r, &[Value::Int(1), Value::Str("depois".into())])
        .unwrap();

    assert_eq!(
        carimbo_de(&mut t, r),
        antes,
        "a alteracao renovou o carimbo"
    );
    assert_eq!(
        relogio_de(&mut t, r),
        relogio_antes,
        "a alteracao renovou o relogio de criacao"
    );
}

/// A marca d'agua do carimbo volta do DISCO ao reabrir a tabela.
///
/// # O defeito que ela repoe
///
/// Nao gravar os bytes 116..124 do cabecalho (tirar o `por_u64(&mut buf, 116,
/// ...)` do `montar_cabecalho`): a marca volta 0 e um no que reiniciasse
/// emitiria carimbo menor que o de linha ja gravada -- a garantia morrendo
/// calada, no caso que ela existe para cobrir.
#[test]
fn a_marca_dagua_do_carimbo_atravessa_o_disco() {
    let d = DirTemp::novo("marca-dagua");
    let maior = {
        let mut t = Table::criar(&d.0, esquema_maes()).unwrap();
        let mut maior = 0;
        for i in 1..=20i64 {
            let r = t.inserir(&[Value::Int(i), Value::Str("x".into())]).unwrap();
            maior = carimbo_de(&mut t, r);
        }
        t.sincronizar().unwrap();
        maior
    };

    let t = Table::abrir(&d.0, "maes").unwrap();
    assert_eq!(
        t.ultimo_carimbo(),
        maior,
        "a marca d'agua do carimbo nao voltou do cabecalho"
    );
    assert!(
        no::ultimo_carimbo() >= maior,
        "abrir a tabela nao empurrou o contador do processo"
    );
}

/// A replica HONRA o carimbo que veio, e empurra o contador local.
///
/// # O defeito que ela repoe
///
/// Carimbar na replica como se fosse escrita local (tirar o ramo
/// `self.como_replica` do `carimbar_linha`). O carimbo local difere do da
/// origem, o retrato SHA-256 de source e replica diverge, e e exatamente a
/// doenca do `rownum` do pedido 291.
#[test]
fn a_replica_honra_o_carimbo_que_veio() {
    let d = DirTemp::novo("replica");
    let mut t = Table::criar(&d.0, esquema_maes()).unwrap();
    let i = t.esquema().coluna_rowstamp().unwrap();
    let j = t.esquema().coluna_rowtime().unwrap();

    // Um carimbo bem a frente do contador local, como o de um source movimentado.
    let do_source = no::ultimo_carimbo() + 1_000_000;
    let mut linha = vec![
        Value::Int(1),
        Value::Str("da origem".into()),
        Value::Bool(false),
        Value::UInt(1),
        Value::UInt(0),
        Value::DateTime(0),
    ];
    linha[i] = Value::UInt(do_source);
    linha[j] = Value::DateTime(1_700_000_000_000);

    let r = t.inserir_replicado(&linha).unwrap();
    assert_eq!(
        carimbo_de(&mut t, r),
        do_source,
        "a replica gerou carimbo proprio em vez de honrar o que veio"
    );
    assert_eq!(
        relogio_de(&mut t, r),
        1_700_000_000_000,
        "a replica trocou o relogio da origem pelo dela"
    );

    // E o contador local foi empurrado: a proxima escrita LOCAL nao pode sair
    // atras de uma linha que ja esta na tabela.
    let local = t
        .inserir(&[Value::Int(2), Value::Str("daqui".into())])
        .unwrap();
    assert!(
        carimbo_de(&mut t, local) > do_source,
        "a escrita local saiu atras do evento que chegou da origem"
    );
}

// ------------------------------------------ 290: a faixa da `Sequence`

/// O `passo` atravessa o disco, e a ausencia dele vale 1.
///
/// # O defeito que ela repoe
///
/// Nao gravar o bloco v10 no `serializar` (ou nao le-lo no `desserializar`):
/// o passo volta 1 e os dois nos numeram a mesma faixa outra vez.
#[test]
fn o_passo_atravessa_o_disco() {
    let com_faixa = Schema::new(
        "numerada",
        vec![
            Column::new("id", ColumnType::Sequence),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_passo_da_sequencia(3)
    .unwrap();
    assert_eq!(com_faixa.passo_da_sequencia(), 3);

    let volta = Schema::desserializar(&com_faixa.serializar()).unwrap();
    assert_eq!(volta.passo_da_sequencia(), 3);
    assert_eq!(volta, com_faixa, "o bloco v10 nao fecha na ida e volta");

    // Sem declarar nada, o passo e 1 -- e e o que toda tabela ja gravada tem.
    let sem = Schema::new(
        "simples",
        vec![Column::new("id", ColumnType::Sequence)],
        vec![],
    )
    .unwrap();
    assert_eq!(sem.passo_da_sequencia(), 1);
    assert_eq!(
        Schema::desserializar(&sem.serializar())
            .unwrap()
            .passo_da_sequencia(),
        1
    );
}

/// **A guarda do comportamento VELHO, que e a que mais importa.**
///
/// Um bloco v9 -- os bytes de versao trocados e a cauda do v10 cortada --
/// continua abrindo, com passo 1 e SEM as colunas de carimbo. Banco que existe
/// nao quebra.
///
/// # O defeito que ela repoe
///
/// Ler o bloco v10 sem o portao `if versao >= 10`: o `u16` da contagem seria
/// lido do fim do bloco v9, que nao existe, e a tabela antiga nao abriria mais.
#[test]
fn bloco_v9_continua_abrindo_com_passo_um_e_sem_carimbo() {
    let v10 = esquema_antes_do_v10();
    let bytes10 = v10.serializar();

    // Um v9 de verdade: versao 9 e sem a lista da faixa no fim. A tabela nao
    // tem coluna `Sequence`, entao a lista do v10 e so o `u16` do zero.
    let mut v9 = bytes10.clone();
    v9[4..6].copy_from_slice(&9u16.to_le_bytes());
    v9.truncate(bytes10.len() - 2);

    let lido = Schema::desserializar(&v9).unwrap();
    assert_eq!(lido, v10, "o v9 nao voltou igual ao que foi gravado");
    assert_eq!(lido.passo_da_sequencia(), 1);
    assert!(
        lido.coluna_rowstamp().is_none() && lido.coluna_rowtime().is_none(),
        "a leitura de um v9 INVENTOU coluna de carimbo -- cada linha passaria \
         a ser lida com os offsets deslocados"
    );
}

/// Bloco v10 TRUNCADO no meio da faixa e erro, e nao «fica sem».
///
/// Um `passo` que sumisse calado viraria faixa 1, e dois nos voltariam a
/// numerar a mesma faixa -- a colisao que a faixa existe para impedir,
/// produzida pelo conserto e em silencio.
#[test]
fn bloco_v10_truncado_e_erro() {
    let s = Schema::new(
        "numerada",
        vec![Column::new("id", ColumnType::Sequence)],
        vec![],
    )
    .unwrap()
    .com_passo_da_sequencia(4)
    .unwrap();
    let bytes = s.serializar();
    for cortar in 1..=8 {
        let e = Schema::desserializar(&bytes[..bytes.len() - cortar]).unwrap_err();
        let texto = e.to_string();
        assert!(
            texto.contains("truncado"),
            "cortando {cortar} bytes a recusa nao nomeia o truncamento: {texto}"
        );
    }
}

/// Duas faixas numeram sem NUNCA repetir um numero, de ponta a ponta.
///
/// # O defeito que ela repoe
///
/// `proxima_da_sequencia` ignorando a faixa (`self.proxima_sequencia += 1`
/// sem o `na_faixa`): os dois nos entregam 1, 2, 3... e o teste acha a
/// interseccao inteira.
#[test]
fn dois_nos_com_faixas_diferentes_nunca_repetem_numero() {
    fn numeros_do_no(inicio: u64, quantos: i64) -> Vec<u64> {
        let d = DirTemp::novo(&format!("faixa-{inicio}"));
        no::definir_inicio_da_sequencia(inicio);
        let esq = Schema::new(
            "numerada",
            vec![
                Column::new("id", ColumnType::Sequence),
                Column::new("nome", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
        .com_passo_da_sequencia(2)
        .unwrap();
        let mut t = Table::criar(&d.0, esq).unwrap();
        let mut saida = Vec::new();
        for _ in 0..quantos {
            let r = t.inserir(&[Value::Null, Value::Str("x".into())]).unwrap();
            match &t.ler(r).unwrap().unwrap()[0] {
                Value::UInt(n) => saida.push(*n),
                outro => panic!("a sequencia nao e UInt: {outro:?}"),
            }
        }
        saida
    }

    let a = numeros_do_no(0, 200);
    let b = numeros_do_no(1, 200);
    // A identidade do no e do PROCESSO: devolve ao padrao para nao contaminar
    // os testes vizinhos, que rodam na mesma thread deste arquivo.
    no::definir_inicio_da_sequencia(0);

    assert!(
        a.iter().all(|n| n % 2 == 0),
        "o no 0 saiu da faixa dele: {a:?}"
    );
    assert!(
        b.iter().all(|n| n % 2 == 1),
        "o no 1 saiu da faixa dele: {b:?}"
    );
    assert!(
        !a.iter().any(|n| b.contains(n)),
        "os dois nos entregaram o mesmo numero"
    );
}

// ---------------------------------- 314: a tabela-cadeia fica no formato anterior

/// **Lado A do pedido 314: a ledger COM cadeia recusa, e diz o motivo.**
///
/// # O defeito que ela repoe
///
/// Deixar a migracao passar (tirar a guarda do `migrar_para_psch_v10`): cada
/// slot de uma historia assinada seria reescrito por uma troca de arquivo.
/// E se a recusa fosse seca -- um `PhxError` sem texto --, quem a recebesse
/// procuraria defeito em vez de ler uma decisao.
#[test]
fn ledger_com_cadeia_recusa_a_migracao_nomeando_o_motivo() {
    let d = DirTemp::novo("ledger-com-cadeia");
    // Nasce sem as colunas de carimbo, como uma tabela anterior ao v10.
    let mut esq = esquema_blocos();
    esq = Schema::do_disco(
        esq.nome().to_string(),
        esq.colunas()
            .iter()
            .filter(|c| c.nome != COLUNA_ROWSTAMP && c.nome != COLUNA_ROWTIME)
            .cloned()
            .collect(),
        esq.indices().to_vec(),
    )
    .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    assert!(t.esquema().coluna_rowstamp().is_none());

    for i in 0..3 {
        let bruto = vec![
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Str(format!("mineirador-{i}")),
        ];
        let pronto = preparar_bloco(&mut t, bruto).unwrap();
        t.inserir(&pronto).unwrap();
    }
    t.sincronizar().unwrap();
    assert!(matches!(
        verificar_cadeia(&mut t).unwrap(),
        Verificacao::Integra { .. }
    ));

    let e = t.migrar_para_psch_v10().unwrap_err().to_string();
    for pedaco in ["ledger", "3 bloco", "historia assinada", "de proposito"] {
        assert!(e.contains(pedaco), "a recusa nao diz {pedaco:?}: {e}");
    }
    // E a tabela nao foi tocada: a cadeia continua verificavel.
    assert!(t.esquema().coluna_rowstamp().is_none());
    assert!(matches!(
        verificar_cadeia(&mut t).unwrap(),
        Verificacao::Integra { .. }
    ));
}

/// **Lado B do pedido 314: a ledger NASCIDA no v10 funciona normalmente.**
///
/// O hash do bloco exclui coluna de sistema (`coluna_no_hash_do_ledger`),
/// entao o carimbo entra na linha e fica fora da prova.
///
/// # O defeito que ela repoe
///
/// Tirar as duas colunas novas do `e_coluna_de_sistema`: elas entrariam no
/// conteudo canonico, e como o carimbo e local de cada no, dois servidores
/// calculariam hashes diferentes para o mesmo bloco.
#[test]
fn ledger_nascida_no_v10_funciona() {
    let d = DirTemp::novo("ledger-v10");
    let mut t = Table::criar(&d.0, esquema_blocos()).unwrap();
    assert!(t.esquema().coluna_rowstamp().is_some());

    let mut carimbos = Vec::new();
    for i in 0..5 {
        let bruto = vec![
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Str(format!("mineirador-{i}")),
        ];
        let pronto = preparar_bloco(&mut t, bruto).unwrap();
        let r = t.inserir(&pronto).unwrap();
        carimbos.push(carimbo_de(&mut t, r));
    }
    t.sincronizar().unwrap();

    let v = verificar_cadeia(&mut t).unwrap();
    assert!(
        matches!(v, Verificacao::Integra { blocos: 5, .. }),
        "a cadeia nascida no v10 nao verifica: {v:?}"
    );
    assert!(
        carimbos.windows(2).all(|p| p[0] < p[1]),
        "os blocos nao saiu em ordem de carimbo: {carimbos:?}"
    );
    // O hash NAO cobre o carimbo: recalcular o hash de um bloco com o carimbo
    // trocado da o mesmo valor.
    let linha = t.ler(1).unwrap().unwrap();
    let mut com_outro_carimbo = linha.clone();
    let i = t.esquema().coluna_rowstamp().unwrap();
    com_outro_carimbo[i] = Value::UInt(999_999);
    assert_eq!(
        phxsql_store::hash_do_bloco(t.esquema(), &linha),
        phxsql_store::hash_do_bloco(t.esquema(), &com_outro_carimbo),
        "o hash do bloco passou a cobrir o carimbo, que e local de cada no"
    );
}

// ---------------------------------------------------- a migracao em si

/// A tabela anterior ao v10 migra, e a linha velha fica com ZERO.
///
/// Zero quer dizer «esta linha nasceu antes de a coluna existir», que e a
/// verdade sobre ela. Carimbo retroativo passaria no CRC e ninguem mais o
/// distinguiria de um medido.
///
/// # O defeito que ela repoe
///
/// Preencher a linha velha com o carimbo de hoje: as dez linhas antigas
/// sairiam com carimbo MAIOR que o das linhas que ja existiam depois delas, e
/// a ordem gravada viraria ficcao.
#[test]
fn a_tabela_anterior_ao_v10_migra_e_a_linha_velha_fica_em_zero() {
    let d = DirTemp::novo("migrar");
    let mut t = Table::criar(&d.0, esquema_antes_do_v10()).unwrap();
    assert!(t.esquema().coluna_rowstamp().is_none());
    for i in 1..=10i64 {
        t.inserir(&[Value::Int(i), Value::Str(format!("linha {i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();

    let slots = t.migrar_para_psch_v10().unwrap();
    assert_eq!(slots, 10);
    assert_eq!(t.esquema().coluna_rowstamp(), Some(4));
    assert_eq!(t.esquema().coluna_rowtime(), Some(5));

    for i in 1..=10u64 {
        assert_eq!(
            carimbo_de(&mut t, i),
            0,
            "a linha {i} ganhou carimbo inventado"
        );
        assert_eq!(relogio_de(&mut t, i), 0);
        // E o dado do usuario atravessou a reescrita intacto.
        assert_eq!(t.ler(i).unwrap().unwrap()[0], Value::Int(i as i64));
        assert_eq!(
            t.ler(i).unwrap().unwrap()[1],
            Value::Str(format!("linha {i}"))
        );
    }

    // A linha NOVA nasce carimbada, na mesma tabela.
    let r = t
        .inserir(&[Value::Int(11), Value::Str("depois da migracao".into())])
        .unwrap();
    assert!(carimbo_de(&mut t, r) > 0);

    // E a alteracao de uma linha velha NAO inventa carimbo para ela.
    t.atualizar(1, &[Value::Int(1), Value::Str("mexida".into())])
        .unwrap();
    assert_eq!(
        carimbo_de(&mut t, 1),
        0,
        "alterar a linha velha carimbou como se ela tivesse nascido hoje"
    );

    // Idempotente: rodar de novo nao toca no disco.
    assert_eq!(t.migrar_para_psch_v10().unwrap(), 0);
}

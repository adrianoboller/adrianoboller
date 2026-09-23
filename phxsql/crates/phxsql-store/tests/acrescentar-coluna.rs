//! `ALTER TABLE ADD COLUMN` numa tabela que ja tem dado.
//!
//! # O que estes testes protegem, em ordem de importancia
//!
//! 1. **o rowid nao muda.** Ele e o endereco, e o `.ndx` inteiro aponta para
//!    ele. Se ele mudasse, todo indice viraria mentira sem nenhum erro no
//!    caminho -- a busca acharia a linha vizinha.
//! 2. **a ordem de digitacao nao muda.** O i-esimo slot do arquivo novo e o
//!    i-esimo do velho, inclusive os excluidos, que continuam excluidos e
//!    continuam ocupando o lugar deles.
//! 3. **a coluna nova entra ANTES da `softdeleted` e do `rownum`**, e quem
//!    guardava POSICAO de coluna -- indice, chave estrangeira, coluna de
//!    particao -- e remapeado junto.
//! 4. **linha antiga nao recebe dado inventado**: ou o padrao que alguem
//!    declarou, ou nulo. Coluna obrigatoria sem padrao, numa tabela com linha,
//!    e recusada.
//! 5. **o espelho `.bkp` acompanha**, e continua sendo a segunda chance.

#[allow(
    dead_code,
    reason = "o modulo comum serve a varios testes; este usa so o DirTemp"
)]
mod comum;

use comum::DirTemp;

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{
    Column, IndexColumn, IndexDef, IndiceDeTexto, Schema, COLUNA_ROWNUM, COLUNA_ROWSTAMP,
    COLUNA_ROWTIME, COLUNA_SOFTDELETED,
};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Table, Visao};

const ID: usize = 0;
const NOME: usize = 1;
const CIDADE: usize = 2;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(30)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(ID)])
                .unico()
                .primaria(),
            IndexDef::new("porNome", vec![IndexColumn::asc(NOME)]),
        ],
    )
    .unwrap()
}

fn cliente(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("cliente {i:04}")),
        Value::Str(format!("cidade {}", i % 7)),
    ]
}

/// Uma coluna comum, do jeito que a tela a manda.
fn coluna_situacao() -> Column {
    Column::new("situacao", ColumnType::Str(12))
        .com_caption("Situação")
        .com_descricao("estado do cadastro")
}

/// O retrato de uma tabela: `(rowid, valores)` de tudo que esta no `.reg`,
/// marcado ou nao. E a foto que o teste compara antes e depois.
fn retrato(t: &mut Table) -> Vec<(u64, Vec<Value>)> {
    t.varrer_com(Visao::Todas).unwrap()
}

// ---------------------------------------------------------------------------
// 1. o rowid, a ordem e o conteudo
// ---------------------------------------------------------------------------

/// O que mais importa: cada linha continua no rowid dela, com os mesmos
/// valores, e a coluna nova aparece no fim das do usuario.
#[test]
fn a_coluna_entra_e_o_rowid_de_cada_linha_continua_o_mesmo() {
    let d = DirTemp::novo("rowid");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=200 {
        t.inserir(&cliente(i)).unwrap();
    }
    // Um buraco fisico e uma linha marcada: os dois casos que a reescrita
    // tem de atravessar sem mudar de estado.
    t.excluir_de_vez(50, "teste").unwrap();
    t.excluir_suave(51, "teste").unwrap();

    let antes = retrato(&mut t);
    let (slots, registros, marcadas) = (t.slots(), t.registros(), t.marcadas());

    let n = t
        .acrescentar_coluna(coluna_situacao(), Some(Value::Str("ativo".into())))
        .unwrap();
    assert_eq!(n, 200, "todos os slots passam pela reescrita");

    // A tabela e reaberta do disco: o que interessa e o que ficou GRAVADO,
    // e nao o que sobrou na memoria de quem alterou.
    drop(t);
    let mut t = Table::abrir(&d.0, "clientes").unwrap();

    assert_eq!(t.slots(), slots, "o `.reg` nao pode ganhar nem perder slot");
    assert_eq!(t.registros(), registros);
    assert_eq!(t.marcadas(), marcadas);

    let depois = retrato(&mut t);
    assert_eq!(antes.len(), depois.len());
    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    for ((r1, v1), (r2, v2)) in antes.iter().zip(depois.iter()) {
        assert_eq!(r1, r2, "o rowid mudou de lugar");
        assert_eq!(v1[ID], v2[ID]);
        assert_eq!(v1[NOME], v2[NOME]);
        assert_eq!(v1[CIDADE], v2[CIDADE]);
        assert_eq!(v2[situacao], Value::Str("ativo".into()));
    }

    // O slot excluido de vez continua excluido, e o marcado continua marcado.
    assert!(t.ler(50).unwrap().is_none(), "o buraco fisico sumiu");
    let linha51 = t.ler(51).unwrap().unwrap();
    assert!(t.esta_excluida(&linha51), "a linha marcada desmarcou");
    assert_eq!(linha51[situacao], Value::Str("ativo".into()));
}

/// O `.ndx` NAO e tocado -- e continua respondendo certo.
///
/// A chave do indice e o valor da coluna mais o rowid. Nenhum dos dois muda,
/// entao o arquivo tem de ficar byte a byte o mesmo. Se um dia alguem
/// reconstruir o indice aqui, este teste falha e obriga a explicar por que.
#[test]
fn o_indice_nao_e_tocado_e_continua_achando_a_linha() {
    let d = DirTemp::novo("ndx");
    {
        let mut t = Table::criar(&d.0, esquema()).unwrap();
        for i in 1..=120 {
            t.inserir(&cliente(i)).unwrap();
        }
        assert_eq!(
            t.buscar("porNome", &[Value::Str("cliente 0077".into())])
                .unwrap(),
            vec![77]
        );
    }
    // Lido com a tabela FECHADA: o `.ndx` escreve pagina suja no fechamento,
    // e comparar com o arquivo meio gravado mediria o cache, nao a alteracao.
    let antes = std::fs::read(d.0.join("clientes.ndx")).unwrap();

    {
        let mut t = Table::abrir(&d.0, "clientes").unwrap();
        t.acrescentar_coluna(coluna_situacao(), None).unwrap();
    }

    let depois = std::fs::read(d.0.join("clientes.ndx")).unwrap();
    assert_eq!(
        antes, depois,
        "o `.ndx` aponta para rowid, e o rowid nao mudou: ele nao pode ter sido reescrito"
    );

    let mut t = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(
        t.buscar("porNome", &[Value::Str("cliente 0077".into())])
            .unwrap(),
        vec![77]
    );
    // E a conferencia completa da tabela, que percorre indice por indice.
    let r = t.verificar().unwrap();
    assert_eq!(r.registros, 120);
    assert_eq!(
        r.indices.iter().find(|(n, _)| n == "porNome").unwrap().1,
        120
    );
}

/// Depois de alterar, a tabela continua sendo uma tabela: le, insere,
/// atualiza, marca e restaura.
#[test]
fn depois_de_alterar_a_tabela_continua_viva() {
    let d = DirTemp::novo("viva");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=20 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.acrescentar_coluna(coluna_situacao(), Some(Value::Str("ativo".into())))
        .unwrap();

    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    let mut nova = cliente(21);
    nova.push(Value::Str("novo".into()));
    let rowid = t.inserir(&nova).unwrap();
    assert_eq!(rowid, 21, "a ordem de digitacao continua de onde parou");
    assert_eq!(
        t.ler(21).unwrap().unwrap()[situacao],
        Value::Str("novo".into())
    );

    let mut alterada = t.ler(3).unwrap().unwrap();
    alterada[situacao] = Value::Str("inativo".into());
    t.atualizar(3, &alterada).unwrap();
    assert_eq!(
        t.ler(3).unwrap().unwrap()[situacao],
        Value::Str("inativo".into())
    );

    assert!(t.excluir_suave(4, "teste").unwrap());
    assert!(t.restaurar(4, "teste").unwrap());
    assert!(t.excluir_de_vez(5, "teste").unwrap());
    assert!(t.ler(5).unwrap().is_none());
    assert_eq!(t.registros(), 20);
}

// ---------------------------------------------------------------------------
// 2. onde a coluna entra, e quem se desloca com ela
// ---------------------------------------------------------------------------

/// A coluna nova entra ANTES da `softdeleted` e do `rownum`.
///
/// E a armadilha nomeada do sprint: a coluna de sistema anda de posicao, e
/// quem guarda posicao tem de andar junto.
#[test]
fn a_coluna_nova_entra_antes_das_de_sistema() {
    let d = DirTemp::novo("posicao");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();
    let sd_antes = t.esquema().coluna_softdeleted().unwrap();

    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    let nomes: Vec<&str> = t
        .esquema()
        .colunas()
        .iter()
        .map(|c| c.nome.as_str())
        .collect();
    assert_eq!(
        nomes,
        vec![
            "id",
            "nome",
            "cidade",
            "situacao",
            COLUNA_SOFTDELETED,
            COLUNA_ROWNUM,
            COLUNA_ROWSTAMP,
            COLUNA_ROWTIME
        ]
    );
    assert_eq!(
        t.esquema().coluna_softdeleted().unwrap(),
        sd_antes + 1,
        "a coluna de sistema tem de ter andado uma casa"
    );
    // E o motor continua sabendo qual e ela: marcar e desmarcar usa a POSICAO.
    assert!(t.excluir_suave(1, "teste").unwrap());
    let linha = t.ler(1).unwrap().unwrap();
    assert!(t.esta_excluida(&linha));
}

/// A coluna do usuario NAO se move -- nem quando a `softdeleted` foi declarada
/// a mao no meio da lista, que e permitido para quem recria uma tabela.
///
/// A regra e "depois da ultima coluna do usuario", e nao "antes da primeira de
/// sistema": as duas dao o mesmo lugar na tabela comum e discordam aqui, e a
/// segunda empurraria `nome` e `cidade` de lugar.
#[test]
fn com_softdeleted_no_meio_a_coluna_do_usuario_nao_se_move() {
    let d = DirTemp::novo("meio");
    let esq = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new(COLUNA_SOFTDELETED, ColumnType::Bool).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(30)),
        ],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(2)])],
    )
    .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    t.inserir(&[
        Value::Int(1),
        Value::Bool(false),
        Value::Str("Alves".into()),
        Value::Str("Blumenau".into()),
    ])
    .unwrap();

    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    let nomes: Vec<&str> = t
        .esquema()
        .colunas()
        .iter()
        .map(|c| c.nome.as_str())
        .collect();
    assert_eq!(
        nomes,
        vec![
            "id",
            COLUNA_SOFTDELETED,
            "nome",
            "cidade",
            "situacao",
            COLUNA_ROWNUM,
            COLUNA_ROWSTAMP,
            COLUNA_ROWTIME
        ]
    );
    let idx = &t.esquema().indices()[0];
    assert_eq!(
        t.esquema().colunas()[idx.colunas[0].coluna].nome,
        "nome",
        "o indice passou a apontar outra coluna"
    );
    assert_eq!(t.ler(1).unwrap().unwrap()[2], Value::Str("Alves".into()));
}

/// Um indice sobre coluna de SISTEMA e onde o remapeamento aparece.
///
/// A coluna nova entra antes da `softdeleted` e do `rownum`, entao as duas
/// andam uma casa. Um indice que guardou a posicao 4 (`rownum`) e nao for
/// remapeado passa a apontar a posicao 4 do esquema novo -- que e a coluna
/// nova -- e o `.ndx` continua respondendo, com a chave errada.
#[test]
fn indice_sobre_coluna_de_sistema_e_remapeado() {
    let d = DirTemp::novo("indice-sistema");
    let mut colunas = esquema().colunas().to_vec();
    let rownum = colunas
        .iter()
        .position(|c| c.nome == COLUNA_ROWNUM)
        .unwrap();
    let softdeleted = colunas
        .iter()
        .position(|c| c.nome == COLUNA_SOFTDELETED)
        .unwrap();
    colunas.truncate(colunas.len());
    let esq = Schema::do_disco(
        "clientes",
        colunas,
        vec![
            IndexDef::new("porOrdem", vec![IndexColumn::asc(rownum)]),
            IndexDef::new("porMarca", vec![IndexColumn::asc(softdeleted)]),
        ],
    )
    .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    for i in 1..=10 {
        t.inserir(&cliente(i)).unwrap();
    }

    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    for (nome_indice, esperada) in [
        ("porOrdem", COLUNA_ROWNUM),
        ("porMarca", COLUNA_SOFTDELETED),
    ] {
        let i = t.esquema().indice_por_nome(nome_indice).unwrap();
        let pos = t.esquema().indices()[i].colunas[0].coluna;
        assert_eq!(
            t.esquema().colunas()[pos].nome,
            esperada,
            "o indice {nome_indice} passou a apontar {}",
            t.esquema().colunas()[pos].nome
        );
    }
    // E a arvore continua respondendo, com a chave certa.
    drop(t);
    let mut t = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(t.buscar("porOrdem", &[Value::UInt(7)]).unwrap(), vec![7]);
}

/// A chave estrangeira tambem guarda posicao, e o remapeamento e o mesmo.
#[test]
fn a_chave_estrangeira_e_remapeada() {
    let d = DirTemp::novo("fk");
    let mut colunas = esquema().colunas().to_vec();
    let rownum = colunas
        .iter()
        .position(|c| c.nome == COLUNA_ROWNUM)
        .unwrap();
    colunas.truncate(colunas.len());
    let esq = Schema::do_disco("clientes", colunas, vec![])
        .unwrap()
        .com_chaves_estrangeiras(vec![
            // `.conferindo(false)` de proposito: desde que a chave declarada
            // nasce conferida, uma chave para `cidades` -- tabela que este
            // teste nunca cria -- pararia toda gravacao daqui. E o assunto
            // deste teste e o REMAPEAMENTO da posicao da coluna quando entra
            // uma nova, e nao a imposicao. Desligar aqui e escolha escrita.
            phxsql_core::schema::ForeignKey::new(
                "fk_cidade",
                vec![CIDADE],
                "cidades",
                vec!["nome".into()],
            )
            .conferindo(false),
            // Nao e uma chave que alguem declararia na vida real -- e a que
            // prova o remapeamento, porque a posicao dela e a que anda.
            phxsql_core::schema::ForeignKey::new(
                "fk_ordem",
                vec![rownum],
                "ordens",
                vec!["n".into()],
            )
            .conferindo(false),
        ])
        .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    t.inserir(&cliente(1)).unwrap();

    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    let fks = t.esquema().chaves_estrangeiras();
    let em = |nome: &str| {
        let fk = fks.iter().find(|f| f.nome == nome).unwrap();
        t.esquema().colunas()[fk.colunas[0]].nome.clone()
    };
    assert_eq!(
        em("fk_cidade"),
        "cidade",
        "a FK antes da nova nao pode andar"
    );
    assert_eq!(
        em("fk_ordem"),
        COLUNA_ROWNUM,
        "a FK depois da nova tem de andar"
    );
}

/// A particao aponta uma coluna por POSICAO -- e ela vem antes da coluna nova,
/// entao a particao NAO pode andar. E o outro lado do remapeamento.
#[test]
fn a_coluna_de_particao_continua_apontando_a_mesma_coluna() {
    let d = DirTemp::novo("particao");
    let esq = esquema()
        .com_paginacao(Paginacao::por_letra(1000, NOME as u16).unwrap())
        .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    for i in 1..=10 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    let modo = t.esquema().paginacao().modo;
    assert_eq!(modo.coluna(), Some(NOME));
    let nomes: Vec<&str> = t
        .esquema()
        .colunas()
        .iter()
        .map(|c| c.nome.as_str())
        .collect();
    assert_eq!(nomes[modo.coluna().unwrap()], "nome");
    // E as linhas continuam nos baldes delas.
    let mut t = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(t.varrer_com(Visao::Todas).unwrap().len(), 10);
}

/// **O indice de TEXTO tambem guarda posicao -- e era o que a remontagem
/// deixava para tras.**
///
/// `Schema::com_coluna` carregava indice comum, chave estrangeira e particao,
/// e nao carregava a lista dos indices de texto: a primeira coluna
/// acrescentada apagava a declaracao do disco, e o `.fts` ficava orfao sem um
/// erro no caminho. E a petrea ao contrario -- o conserto do `desloca` entrou
/// nos caminhos que existiam, e este nao existia.
///
/// Reponha o defeito tirando o `.com_indices_de_texto(textos)?` de
/// `Schema::com_coluna`: o `len()` daqui cai em 0.
#[test]
fn o_indice_de_texto_sobrevive_a_coluna_nova() {
    let d = DirTemp::novo("fts-sobrevive");
    let esq = esquema()
        .com_indices_de_texto(vec![
            IndiceDeTexto::new("porNomeTexto", NOME),
            IndiceDeTexto::new("porCidadeTexto", CIDADE).sem_dobrar(),
        ])
        .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    for i in 1..=5 {
        t.inserir(&cliente(i)).unwrap();
    }
    let antes = retrato(&mut t);

    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    // REABRE do disco: e o esquema gravado no `.reg` que manda, e e ele que
    // decide se o `.fts` volta a existir na proxima abertura.
    let mut t = Table::abrir(&d.0, "clientes").unwrap();

    // **O bloco do `PSCH` agora CRESCE onde antes encolhia**, porque leva a
    // lista de texto de volta -- e o `data_offset` sai do tamanho real do
    // bloco serializado (`reg.rs`, `alinhar(cab_len + bytes.len())`). Se um
    // dia ele passar a ser presumido, e aqui que o desalinhamento aparece:
    // rowid e valores das linhas velhas, lidos depois da remontagem.
    let depois = retrato(&mut t);
    let posicao = 3; // depois de `cidade`, antes das duas de sistema
    let esperado: Vec<(u64, Vec<Value>)> = antes
        .iter()
        .map(|(r, v)| {
            let mut v = v.clone();
            v.insert(posicao, Value::Null);
            (*r, v)
        })
        .collect();
    assert_eq!(
        depois, esperado,
        "o bloco de esquema maior deslocou rowid ou valor"
    );

    let textos = t.esquema().indices_de_texto();
    assert_eq!(
        textos.len(),
        2,
        "a coluna nova apagou os indices de texto do esquema gravado"
    );
    assert_eq!(textos[0].nome, "porNomeTexto");
    assert_eq!(textos[1].nome, "porCidadeTexto");
    // E cada um continua apontando a coluna DELE -- pelo nome, porque um
    // numero certo por coincidencia nao prova remapeamento.
    let nomes: Vec<&str> = t
        .esquema()
        .colunas()
        .iter()
        .map(|c| c.nome.as_str())
        .collect();
    assert_eq!(nomes[textos[0].coluna], "nome");
    assert_eq!(nomes[textos[1].coluna], "cidade");
    // A dobra e escolha escrita e atravessa a remontagem.
    assert!(textos[0].dobrar);
    assert!(!textos[1].dobrar);
}

// ---------------------------------------------------------------------------
// 3. o que a linha antiga recebe
// ---------------------------------------------------------------------------

/// Sem padrao, a linha antiga recebe NULO -- que e a verdade sobre ela.
#[test]
fn sem_padrao_a_linha_antiga_recebe_nulo() {
    let d = DirTemp::novo("nulo");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=5 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.acrescentar_coluna(coluna_situacao(), None).unwrap();
    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    for i in 1..=5 {
        assert_eq!(t.ler(i).unwrap().unwrap()[situacao], Value::Null);
    }
}

/// Coluna obrigatoria sem padrao, numa tabela COM linha, e recusada.
///
/// O que ela pediria era o motor escolher um valor por quem nunca digitou.
#[test]
fn obrigatoria_sem_padrao_com_linha_e_recusada() {
    let d = DirTemp::novo("obrigatoria");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();
    let e = t
        .acrescentar_coluna(coluna_situacao().obrigatoria(), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("obrigatoria"), "{e}");
    assert!(e.contains("inventar dado"), "{e}");
    // E a tabela nao foi tocada.
    assert_eq!(t.esquema().colunas().len(), 7);
    assert_eq!(
        t.ler(1).unwrap().unwrap()[NOME],
        Value::Str("cliente 0001".into())
    );
}

/// A mesma coluna obrigatoria passa COM padrao -- e passa numa tabela vazia,
/// onde nao ha linha sobre a qual mentir.
#[test]
fn obrigatoria_passa_com_padrao_e_passa_na_tabela_vazia() {
    let d = DirTemp::novo("obrigatoria-ok");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();
    t.acrescentar_coluna(
        coluna_situacao().obrigatoria(),
        Some(Value::Str("ativo".into())),
    )
    .unwrap();
    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    assert_eq!(
        t.ler(1).unwrap().unwrap()[situacao],
        Value::Str("ativo".into())
    );

    let d2 = DirTemp::novo("vazia");
    let mut v = Table::criar(&d2.0, esquema()).unwrap();
    v.acrescentar_coluna(coluna_situacao().obrigatoria(), None)
        .unwrap();
    let mut nova = cliente(1);
    nova.push(Value::Str("ativo".into()));
    assert_eq!(v.inserir(&nova).unwrap(), 1);
}

/// `Sequence` nao entra: o contador do `.reg` e unico, e numerar linha antiga
/// inventaria a ordem que ela teve.
#[test]
fn sequence_e_nome_de_sistema_sao_recusados() {
    let d = DirTemp::novo("recusas");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();

    let e = t
        .acrescentar_coluna(Column::new("seq", ColumnType::Sequence), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("Sequence"), "{e}");

    let e = t
        .acrescentar_coluna(Column::new(COLUNA_ROWNUM, ColumnType::UInt8), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("do motor"), "{e}");

    let e = t
        .acrescentar_coluna(Column::new("nome", ColumnType::Str(10)), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("ja tem uma coluna"), "{e}");
}

// ---------------------------------------------------------------------------
// 4. os arquivos irmaos
// ---------------------------------------------------------------------------

/// Uma tabela paginada em varios volumes: cada um vira mais largo, e a ordem
/// de digitacao atravessa a fronteira igual.
#[test]
fn paginada_reescreve_cada_volume_e_preserva_a_ordem() {
    let d = DirTemp::novo("paginada");
    let esq = esquema()
        .com_paginacao(Paginacao::nova(30, 9).unwrap())
        .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    for i in 1..=95 {
        t.inserir(&cliente(i)).unwrap();
    }
    let antes = retrato(&mut t);
    assert!(t.reindexar().is_ok());

    let n = t
        .acrescentar_coluna(coluna_situacao(), Some(Value::Str("ok".into())))
        .unwrap();
    assert_eq!(n, 95);

    drop(t);
    let mut t = Table::abrir(&d.0, "clientes").unwrap();
    let depois = retrato(&mut t);
    assert_eq!(antes.len(), depois.len());
    for ((r1, v1), (r2, v2)) in antes.iter().zip(depois.iter()) {
        assert_eq!(r1, r2);
        assert_eq!(v1[ID], v2[ID]);
    }
    // Os quatro volumes existem e todos declaram a MESMA largura de slot.
    let mut larguras = std::collections::BTreeSet::new();
    for v in 1..=4 {
        let b = std::fs::read(d.0.join(format!("clientes_{v:03}.reg"))).unwrap();
        larguras.insert(u32::from_le_bytes([b[16], b[17], b[18], b[19]]));
    }
    assert_eq!(larguras.len(), 1, "volume ficou com a largura velha");
}

/// O espelho `.bkp` acompanha, e continua sendo a segunda chance.
///
/// Sem isto, o espelho ficaria com a largura velha e a primeira leitura que
/// precisasse dele leria o slot errado -- que e pior do que nao ter espelho.
#[test]
fn o_espelho_acompanha_e_continua_salvando() {
    let d = DirTemp::novo("espelho");
    let mut t = Table::criar_espelhada(&d.0, esquema()).unwrap();
    for i in 1..=40 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.acrescentar_coluna(coluna_situacao(), Some(Value::Str("ativo".into())))
        .unwrap();
    drop(t);

    let reg = std::fs::read(d.0.join("clientes.reg")).unwrap();
    let bkp = std::fs::read(d.0.join("clientes.bkp")).unwrap();
    assert_eq!(reg.len(), bkp.len(), "o espelho ficou com o tamanho velho");

    // Estraga o slot 7 no principal; o espelho tem de salvar a leitura.
    let (slot, off) = {
        let s = u32::from_le_bytes([reg[16], reg[17], reg[18], reg[19]]) as usize;
        let o = u64::from_le_bytes(reg[44..52].try_into().unwrap()) as usize;
        (s, o)
    };
    let mut estragado = reg.clone();
    let alvo = off + 6 * slot + 30;
    estragado[alvo] ^= 0xFF;
    std::fs::write(d.0.join("clientes.reg"), &estragado).unwrap();

    let mut t = Table::abrir_espelhada(&d.0, "clientes").unwrap();
    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    let linha = t.ler(7).unwrap().unwrap();
    assert_eq!(linha[ID], Value::Int(7));
    assert_eq!(linha[situacao], Value::Str("ativo".into()));
    assert_eq!(
        t.recuperados(),
        1,
        "a leitura tinha de ter vindo do espelho"
    );
}

/// Uma imagem gravada ANTES da alteracao nao vira lixo: ela vira mensagem.
///
/// E o caso da replica que ainda nao alterou, e o da lixeira de antes. Sem a
/// conferencia de largura, o payload curto sairia por indice fora da faixa --
/// ou, pior, seria lido com os offsets deslocados.
#[test]
fn linha_guardada_antes_da_alteracao_da_mensagem_e_nao_lixo() {
    let d = DirTemp::novo("lixeira");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=3 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.excluir_de_vez(2, "antes de alterar").unwrap();
    t.acrescentar_coluna(coluna_situacao(), None).unwrap();

    let descartadas = t.lixeira(0, 10, false).unwrap();
    assert_eq!(descartadas.len(), 1);
    let e = t.linha_da_lixeira(&descartadas[0]).unwrap_err().to_string();
    assert!(e.contains("mudou depois"), "{e}");
}

// ---------------------------------------------------------------------------
// 5. a morte no meio da reescrita
// ---------------------------------------------------------------------------

/// Os arquivos de volume de uma tabela paginada, na ordem, com os bytes.
fn volumes(d: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("reg"))
        .collect();
    v.sort();
    v.into_iter()
        .map(|p| {
            let b = std::fs::read(&p).unwrap();
            (p, b)
        })
        .collect()
}

fn tabela_paginada(d: &DirTemp) -> Table {
    let esq = esquema()
        .com_paginacao(Paginacao::nova(30, 9).unwrap())
        .unwrap();
    let mut t = Table::criar(&d.0, esq).unwrap();
    for i in 1..=95 {
        t.inserir(&cliente(i)).unwrap();
    }
    t
}

/// A queda ENTRE as trocas: o volume 1 ja e novo, o volume 3 ficou para tras,
/// e o `*.novo` dele esta la, inteiro. Abrir TERMINA a troca.
///
/// A alteracao escreve todos os `*.novo` antes de trocar qualquer um, e troca
/// o volume 1 primeiro. E por isso que o volume 1 responde sozinho em que
/// estado o conjunto esta: novo, a alteracao esta decidida e o que falta e so
/// terminar.
#[test]
fn a_queda_entre_as_trocas_e_terminada_na_abertura() {
    let d = DirTemp::novo("queda-termina");
    let mut t = tabela_paginada(&d);
    let antes = retrato(&mut t);
    let velhos = volumes(&d.0);
    t.acrescentar_coluna(coluna_situacao(), Some(Value::Str("ok".into())))
        .unwrap();
    drop(t);

    // Repoe o estado que a queda deixaria: o volume 3 velho de volta no lugar,
    // e o novo dele esperando ao lado.
    let (caminho, velho) = &velhos[2];
    let pendente = caminho.with_file_name(format!(
        "{}.novo",
        caminho.file_name().unwrap().to_string_lossy()
    ));
    let novo = std::fs::read(caminho).unwrap();
    std::fs::write(&pendente, &novo).unwrap();
    std::fs::write(caminho, velho).unwrap();

    let mut t = Table::abrir(&d.0, "clientes").unwrap();
    let depois = retrato(&mut t);
    assert_eq!(antes.len(), depois.len());
    for ((r1, v1), (r2, v2)) in antes.iter().zip(depois.iter()) {
        assert_eq!(r1, r2);
        assert_eq!(v1[NOME], v2[NOME]);
    }
    assert!(
        !pendente.exists(),
        "o `.novo` continua la: a troca nao foi terminada"
    );
}

/// A mesma queda SEM o `*.novo`: o conjunto fica misturado, e abrir RECUSA
/// em vez de ler o volume 3 com a largura do volume 1.
///
/// Sem esta guarda, cada linha do volume 3 sairia deslocada da anterior -- e
/// nao ha CRC que reclame, porque os bytes lidos sao bytes de outra linha.
#[test]
fn a_queda_sem_o_novo_recusa_em_vez_de_ler_deslocado() {
    let d = DirTemp::novo("queda-recusa");
    let mut t = tabela_paginada(&d);
    let velhos = volumes(&d.0);
    t.acrescentar_coluna(coluna_situacao(), Some(Value::Str("ok".into())))
        .unwrap();
    drop(t);

    let (caminho, velho) = &velhos[2];
    std::fs::write(caminho, velho).unwrap();

    let e = match Table::abrir(&d.0, "clientes") {
        Ok(_) => panic!("o conjunto misturado abriu: o volume 3 sera lido com a largura do 1"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("pela metade"), "{e}");
    assert!(
        e.contains("_003"),
        "a mensagem tem de dizer QUAL volume: {e}"
    );
}

/// **A declaracao que se contradiz recusa NA DECLARACAO** -- pedido 245, O2.
///
/// Medido em 16/09/2026: `situacao Str(12) check "situacao <> ''"` com
/// `padrao = ""` era ACEITO numa tabela com linha, a linha velha ficava com o
/// valor que viola, e o `atualizar` dela passava a recusar para sempre. A
/// contradicao estava dentro de um comando so -- nao dependia de dado nenhum
/// que a funcao nao tivesse na mao.
///
/// Reponha o defeito tirando o bloco `if tem_linha && self.julga_integridade()`
/// de `Table::acrescentar_coluna`: o `unwrap_err` daqui vira `unwrap` de um
/// `Ok`, e o `atualizar` do fim passa a falhar.
#[test]
fn padrao_que_viola_o_proprio_check_recusa_antes_de_tocar_no_reg() {
    let d = DirTemp::novo("check-contradiz");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();
    let coluna = coluna_situacao().com_check("situacao <> 'nao'").unwrap();
    let e = t
        .acrescentar_coluna(coluna, Some(Value::Str("nao".into())))
        .unwrap_err()
        .to_string();
    assert!(e.contains("CHECK"), "{e}");
    assert!(e.contains("atualizar"), "{e}");
    // E a tabela NAO foi tocada: a recusa e antes do `.reg`.
    assert_eq!(t.esquema().colunas().len(), 7);
    assert_eq!(
        t.ler(1).unwrap().unwrap()[NOME],
        Value::Str("cliente 0001".into())
    );
}

/// **O crivo e PRECISO, e sao tres controles que provam isso.**
///
/// Ele nao pode virar «CHECK numa tabela com linha recusa»: isso tiraria uma
/// ordem de modelagem legitima -- «daqui para a frente vale esta regra» -- que
/// o PostgreSQL(R) atende com `NOT VALID`. O crivo avalia com NULO em todas as
/// outras colunas, e `NULL` passa no CHECK (e o SQL): entao so cai o que se
/// contradiz sozinho.
#[test]
fn o_crivo_do_check_nao_pega_quem_depende_da_linha_velha() {
    let d = DirTemp::novo("check-crivo");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&cliente(1)).unwrap();

    // (a) CHECK que fala de coluna VELHA passa -- o dado nao esta aqui, e
    //     inventar uma recusa seria pior que a falta dela.
    let c = coluna_situacao().com_check("nome <> ''").unwrap();
    t.acrescentar_coluna(c, None).unwrap();

    // (b) CHECK coerente com o padrao passa.
    let d2 = DirTemp::novo("check-crivo-b");
    let mut u = Table::criar(&d2.0, esquema()).unwrap();
    u.inserir(&cliente(1)).unwrap();
    let c = coluna_situacao().com_check("situacao <> 'nao'").unwrap();
    u.acrescentar_coluna(c, Some(Value::Str("sim".into())))
        .unwrap();

    // (c) A MESMA contradicao numa tabela VAZIA passa: nao ha linha velha
    //     sobre a qual a contradicao exista.
    let d3 = DirTemp::novo("check-crivo-c");
    let mut v = Table::criar(&d3.0, esquema()).unwrap();
    let c = coluna_situacao().com_check("situacao <> 'nao'").unwrap();
    v.acrescentar_coluna(c, Some(Value::Str("nao".into())))
        .unwrap();
}

// ---------------------------------------------------------------------------
// 6. as duas fases, e o cinto de seguranca entre elas  -- pedido 421
// ---------------------------------------------------------------------------

/// **O defeito, reposto e DETERMINISTICO**: sem revalidar o retrato, a FASE B
/// renomeia por cima de escrita confirmada.
///
/// Este teste nao prova um conserto -- ele prova que ha o que conferir. A
/// sequencia e' exatamente a do conserto ingenuo (soltar a trava na FASE A e
/// trocar na FASE B sem olhar nada), e o resultado esta escrito no `assert`:
/// a linha que o vizinho gravou SOME.
///
/// Sem ele, o teste de baixo poderia continuar passando no dia em que a FASE B
/// deixasse de perder a linha por outro motivo -- e ninguem saberia que a
/// revalidacao virou cerimonia.
#[test]
fn sem_revalidar_o_retrato_a_fase_b_perde_a_linha_do_vizinho() {
    let d = DirTemp::novo("fase-b-perde");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=50 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.sincronizar().unwrap();

    // FASE A: o `*.novo` e' um RETRATO das 50 linhas.
    let pendente = t
        .acrescentar_coluna_fase_a(coluna_situacao(), None)
        .unwrap();

    // O VIZINHO que escapou do congelamento, e ele grava no volume VIVO.
    {
        let mut outro = Table::abrir(&d.0, "clientes").unwrap();
        let rowid = outro.inserir(&cliente(51)).unwrap();
        assert_eq!(rowid, 51);
        outro.sincronizar().unwrap();
    }

    // FASE B sem conferir nada -- o conserto ingenuo.
    t.acrescentar_coluna_fase_b(pendente).unwrap();

    let depois = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(
        depois.slots(),
        50,
        "o defeito nao se reproduziu: a linha 51 sobreviveu a troca, e entao \
         a revalidacao da prova de baixo deixou de ter o que conferir"
    );
}

/// **O cinto**: com a revalidacao, a FASE B ABORTA e a tabela fica inteira.
///
/// # O defeito que ela repoe
///
/// Tire o `conferir_retrato` de `op_migrar_esquema` e de
/// `op_acrescentar_coluna` -- ou faca `TrocaPendente::conferir_retrato`
/// devolver `Ok(())` sempre -- e a linha 51 some, exatamente como no teste de
/// cima. A diferenca entre os dois e' UMA chamada.
#[test]
fn a_fase_b_aborta_quando_o_volume_mudou_no_meio() {
    let d = DirTemp::novo("fase-b-aborta");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=50 {
        t.inserir(&cliente(i)).unwrap();
    }
    t.sincronizar().unwrap();

    let pendente = t
        .acrescentar_coluna_fase_a(coluna_situacao(), None)
        .unwrap();
    {
        let mut outro = Table::abrir(&d.0, "clientes").unwrap();
        outro.inserir(&cliente(51)).unwrap();
        outro.sincronizar().unwrap();
    }

    let e = pendente
        .conferir_retrato()
        .expect_err("o retrato nao acusou a escrita do vizinho");
    let texto = e.to_string();
    assert!(
        texto.contains("ABORTADA") && texto.contains("clientes"),
        "a recusa nao diz o que houve nem em qual arquivo: {texto}"
    );
    // E o palco sai do disco: `*.novo` orfao ocuparia o tamanho da tabela.
    assert!(
        pendente.descartar() > 0,
        "o descarte nao apagou nenhum `*.novo`"
    );

    // A tabela continua INTEIRA e como estava -- com a linha do vizinho e sem
    // a coluna nova.
    let mut depois = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(depois.slots(), 51, "a linha do vizinho sumiu");
    assert!(
        depois.esquema().coluna_por_nome("situacao").is_none(),
        "a coluna entrou mesmo com a troca abortada"
    );
    assert_eq!(
        depois.varrer_com(Visao::Todas).unwrap().len(),
        51,
        "a tabela nao le mais as 51 linhas"
    );
}

/// As duas fases somadas dao **exatamente** o que a porta de sempre dava.
///
/// E' o teste do caminho VELHO da divisao: `acrescentar_coluna` continua
/// sendo fase A mais fase B, e nenhum chamador de fora sente a mudanca.
///
/// O `rowstamp` e o `rowtime` ficam de fora da comparacao, e nao por
/// conveniencia: os dois saem de contadores do PROCESSO (`no::proximo_carimbo`
/// e o relogio), entao duas tabelas criadas na mesma corrida nunca os teriam
/// iguais. Compara-los mediria a ordem em que o teste rodou.
#[test]
fn as_duas_fases_dao_o_mesmo_que_a_porta_de_sempre() {
    fn sem_carimbo(r: Vec<(u64, Vec<Value>)>) -> Vec<(u64, Vec<Value>)> {
        r.into_iter()
            .map(|(id, mut v)| {
                v.truncate(v.len() - 2);
                (id, v)
            })
            .collect()
    }
    let inteira = {
        let d = DirTemp::novo("fases-inteira");
        let mut t = Table::criar(&d.0, esquema()).unwrap();
        for i in 1..=30 {
            t.inserir(&cliente(i)).unwrap();
        }
        t.excluir_suave(7, "teste").unwrap();
        t.acrescentar_coluna(coluna_situacao(), Some(Value::Str("ativo".into())))
            .unwrap();
        sem_carimbo(retrato(&mut t))
    };
    let partida = {
        let d = DirTemp::novo("fases-partida");
        let mut t = Table::criar(&d.0, esquema()).unwrap();
        for i in 1..=30 {
            t.inserir(&cliente(i)).unwrap();
        }
        t.excluir_suave(7, "teste").unwrap();
        let p = t
            .acrescentar_coluna_fase_a(coluna_situacao(), Some(Value::Str("ativo".into())))
            .unwrap();
        p.conferir_retrato().unwrap();
        t.acrescentar_coluna_fase_b(p).unwrap();
        sem_carimbo(retrato(&mut t))
    };
    assert_eq!(
        inteira, partida,
        "a divisao em duas fases mudou o resultado"
    );
}

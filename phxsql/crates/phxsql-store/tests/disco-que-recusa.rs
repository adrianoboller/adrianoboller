//! O disco que RECUSA, e o que o motor faz depois -- pedidos 509 e 512.
//!
//! Os dois sairam do catalogo de catastrofes do papel C
//! (`docs/propostas/parecer-dba-496-catastrofes-2026-09-24.md`, C1 e C2b), e
//! os dois sao a mesma doenca em lugares diferentes: **o que falhou de ir ao
//! disco saia da lista do que falta ir ao disco.**
//!
//! * **509** -- no nucleo: depois de um `fsync` recusado, o proximo responde
//!   Ok sem o dado estar la, e o motor contava com repetir;
//! * **512** -- no nosso cache: a pagina do `.ndx` que o disco cheio recusou
//!   perdia a flag de suja antes de ser gravada, e o segundo fecho baixava o
//!   byte 52 sobre ela.
//!
//! # As recusas aqui sao forjadas, e a prova contra o SO mora ao lado
//!
//! `phxsql_store::sincronia::falha_de_teste` devolve o mesmo retorno que o
//! nucleo devolveria -- EIO no `fsync`, ENOSPC no `write` da pagina --, porque
//! montar o disco que recusa exige `CAP_SYS_ADMIN`. O que estes testes provam
//! e a CONDUTA de depois, que e decisao do motor. A recusa de verdade (tmpfs
//! cheio, ext4 sobre loop com provisionamento fino) e a
//! `bancada/catastrofes/`.
//!
//! So com `debug_assertions`: em `release` o gancho nao existe, e sem ele
//! nao ha recusa a provocar.
#![cfg(debug_assertions)]

mod comum;
use comum::DirTemp;

use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::sincronia::falha_de_teste::{self, Onde};
use phxsql_store::table::Table;
use std::path::Path;

/// `pedidos(id Int8 unico, nome, cidade)`: o esquema das provas do papel C.
fn esquema() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(id: i64) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(format!("cliente numero {id:08}")),
        Value::Str("Blumenau".into()),
    ]
}

/// A marca de sujo do `.ndx`, lida do ARQUIVO -- e o que a proxima abertura
/// vai ler, e nao o que a RAM acha.
fn byte_52(d: &Path) -> u8 {
    std::fs::read(d.join("pedidos.ndx")).unwrap()[52]
}

/// A tabela de `n` linhas, com o `.ndx` cheio de paginas sujas em RAM.
fn tabela_com(d: &Path, n: i64) -> Table {
    let mut t = Table::criar(d, esquema()).unwrap();
    t.sincronizar().unwrap();
    for id in 1..=n {
        t.inserir(&linha(id)).unwrap();
    }
    t
}

/// Cada id de `ids` e achado pela chave, e o rowid dele e o proprio id (as
/// linhas entram em ordem numa tabela nova).
fn todos_achados(t: &mut Table, ids: impl Iterator<Item = i64>) {
    for id in ids {
        let r = t
            .buscar("porId", &[Value::Int(id)])
            .map_err(|e| e.to_string());
        assert_eq!(r, Ok(vec![id as u64]), "o id {id} pela chave");
    }
}

/// **509: o `fsync` recusado nao se repete como sucesso.**
///
/// Os passos do servidor, no molde do `p3b.sh` do papel C: o fecho 1 recusa;
/// o operador libera espaco (a arma ja disparou); o fecho 2 no mesmo punho e o
/// fecho 3 numa tabela REABERTA -- o `descarregar_sujas_com` reabre para
/// sincronizar. Com o defeito, o 2 e o 3 respondiam Ok, e o servidor drenava
/// as marcas dos commits. E quem recusou foi o `.log`, e nao o `.ndx`: o
/// `Drop` da tabela gravava o cabecalho do `.ndx` limpo por cima das paginas
/// que o nucleo pode ter perdido -- a outra metade do que o papel C mediu.
#[test]
fn fsync_recusado_nao_se_repete_como_sucesso() {
    let d = DirTemp::novo("509-repetido");
    let mut t = tabela_com(&d, 500);
    falha_de_teste::armar(&d, Onde::Fsync, 1);
    assert!(
        t.sincronizar().is_err(),
        "premissa: o fecho 1 tinha de encontrar a recusa forjada"
    );

    let fecho2 = t.sincronizar().map_err(|e| e.to_string());
    assert!(
        fecho2.is_err(),
        "o fsync recusado foi repetido no mesmo punho e respondeu Ok: e a \
         resposta que o nucleo da sem o dado estar no disco"
    );
    drop(t);
    if let Ok(mut t) = Table::abrir(&d, "pedidos") {
        // Desde o pedido 522 o `Drop` nao grava mais o 0 -- o byte abaixo
        // fica em 1 de qualquer jeito --, e a mesma garantia passou a morar
        // no atestado do processo: a reabertura AQUI nao pode confiar no
        // `.ndx` que o nucleo pode ter perdido.
        assert!(
            t.indice_precisa_reconstruir(),
            "a reabertura neste processo confiou no .ndx depois de um fsync \
             recusado no diretorio: o Drop o atestou por cima da recusa"
        );
        assert!(
            t.sincronizar().is_err(),
            "a tabela REABERTA sincronizou Ok depois de um fsync recusado no \
             mesmo diretorio -- e o caminho do fecho da janela no servidor"
        );
    }
    assert_eq!(
        byte_52(&d),
        1,
        "o Drop baixou a marca de sujo do .ndx depois de um fsync recusado: a \
         proxima abertura nao saberia que tem de reconstruir"
    );
    assert!(
        fecho2.unwrap_err().contains("pedido 509"),
        "a recusa tinha de dizer por que nao repete"
    );
}

/// **509 com o 522: o atestado de ANTES da recusa nao vale depois dela.**
///
/// A tabela fechou limpa -- o `fechar` atestou o 1 dela para este processo
/// -- e so DEPOIS um `fsync` de outro arquivo do diretorio foi recusado. O
/// nucleo pode ter descartado as paginas que aquele fecho entregou, e a
/// reabertura aqui nao pode mais tomar o nucleo por testemunha. Quem segura
/// isso e a abertura, e nao o `Drop`: quando a recusa chegou, o `Drop` ja
/// tinha passado.
#[test]
fn o_atestado_de_antes_da_recusa_nao_vale_depois() {
    let d = DirTemp::novo("522-atestado-e-recusa");
    drop(tabela_com(&d, 500));
    let t = Table::abrir(&d, "pedidos").unwrap();
    assert!(!t.indice_precisa_reconstruir(), "premissa: atestada");
    drop(t);

    // A recusa, noutro arquivo do mesmo diretorio.
    let mut vizinha = Table::criar(
        &d,
        Schema::new(
            "vizinha",
            vec![Column::new("id", ColumnType::Int8)],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap(),
    )
    .unwrap();
    falha_de_teste::armar(&d.join("vizinha."), Onde::Fsync, 1);
    assert!(vizinha.sincronizar().is_err(), "premissa: a recusa forjada");
    drop(vizinha);

    let t = Table::abrir(&d, "pedidos").unwrap();
    assert!(
        t.indice_precisa_reconstruir(),
        "o atestado de antes da recusa valeu depois dela: a reabertura tomou \
         o nucleo por testemunha de paginas que ele pode ter descartado"
    );
}

/// **512: a pagina que o disco recusou continua suja -- e o segundo fecho a
/// grava quando o espaco volta.**
///
/// E o caminho do `gravar_de_verdade`: o segundo fecho no MESMO punho. Com o
/// defeito, o `tirar_sujas` ja tinha baixado a flag de todas antes de gravar
/// a primeira: o segundo fecho achava nada sujo, gravava o byte 52 em 0, e a
/// pagina ficava de zeros no arquivo -- `CRC invalido` na reabertura.
#[test]
fn pagina_que_o_disco_recusou_continua_suja() {
    let d = DirTemp::novo("512-liberado");
    let mut t = tabela_com(&d, 2_000);
    falha_de_teste::armar(&d, Onde::PaginaDoIndice, 1);
    assert!(
        t.sincronizar().is_err(),
        "premissa: o fecho 1 tinha de encontrar o disco cheio"
    );
    assert_eq!(
        byte_52(&d),
        1,
        "a marca de sujo baixou com pagina fora do disco"
    );

    t.sincronizar()
        .expect("com o espaco de volta, o segundo fecho grava a pagina que ficou");
    drop(t);
    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(!t.indice_precisa_reconstruir());
    t.verificar().unwrap_or_else(|e| {
        panic!(
            "o segundo fecho respondeu Ok e o .ndx no disco nao presta ({e}): a \
             pagina que o disco recusou saiu da lista de sujas sem ter sido gravada"
        )
    });
    todos_achados(&mut t, 1..=2_000);
}

/// **512, o outro lado: com o disco AINDA cheio, nada baixa a marca.**
///
/// O segundo fecho recusa de novo (a pagina continua suja e continua sem
/// lugar), o `Drop` tambem, e a abertura seguinte le o byte 52 em 1 e da o
/// recado certo -- «reparar indice» --, e nao «CRC invalido», que manda
/// procurar disco ruim.
#[test]
fn disco_ainda_cheio_deixa_a_marca_e_o_recado_certo() {
    let d = DirTemp::novo("512-cheio");
    let mut t = tabela_com(&d, 2_000);
    falha_de_teste::armar(&d, Onde::PaginaDoIndice, 1_000_000);
    assert!(t.sincronizar().is_err(), "premissa: o disco cheio");
    assert!(
        t.sincronizar().is_err(),
        "o segundo fecho respondeu Ok com a pagina fora do disco"
    );
    drop(t);
    falha_de_teste::desarmar(&d);
    assert_eq!(
        byte_52(&d),
        1,
        "o .ndx foi ao disco marcado LIMPO sobre uma pagina que nao se gravou"
    );

    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(t.indice_precisa_reconstruir());
    let recado = t
        .buscar("porId", &[Value::Int(1)])
        .map_err(|e| e.to_string())
        .unwrap_err();
    assert!(
        recado.contains("reparar indice"),
        "o recado tinha de mandar reconstruir, e disse: {recado}"
    );
    t.reindexar().unwrap();
    todos_achados(&mut t, 1..=2_000);
}

/// **512, o IRMAO no despejo: a pagina suja que sai do cache para abrir lugar
/// e nao se grava volta, suja.**
///
/// Irmao pela ordem, e nao pelo nome: tirar da lista do que falta gravar,
/// gravar, e o erro deixar a pagina sem lugar nenhum. Aqui quem despeja e uma
/// LEITURA -- o `buscar` que traz do arquivo uma pagina que nao esta no cache
/// --, e por isso nenhuma escrita fica interrompida para mandar reconstruir:
/// com o defeito, o fecho seguinte gravava o resto, baixava o byte 52, e a
/// pagina despejada ficava de fora sem ninguem saber.
///
/// O cache vai a 4 paginas para o despejo acontecer com poucas linhas. O teto
/// e do PROCESSO (`definir_cache_paginas`), entao os vizinhos deste binario
/// correm com o cache pequeno tambem -- o que so muda o custo deles, nunca a
/// resposta, que e justamente o que um cache tem de garantir.
#[test]
fn pagina_despejada_que_o_disco_recusou_volta_suja() {
    phxsql_store::ndx::definir_cache_paginas(4);
    let d = DirTemp::novo("512-despejo");
    let mut t = tabela_com(&d, 3_000);
    falha_de_teste::armar(&d, Onde::PaginaDoIndice, 1);
    let recusou = (1..=3_000)
        .step_by(37)
        .any(|id| t.buscar("porId", &[Value::Int(id)]).is_err());
    assert!(
        recusou,
        "premissa: nenhuma leitura despejou pagina suja com o disco cheio"
    );
    falha_de_teste::desarmar(&d);

    t.sincronizar()
        .expect("com o espaco de volta, o fecho grava tudo o que ficou sujo");
    drop(t);
    let mut t = Table::abrir(&d, "pedidos").unwrap();
    t.verificar().unwrap_or_else(|e| {
        panic!(
            "a pagina despejada que o disco recusou sumiu: o fecho gravou o resto, \
             baixou a marca, e o .ndx no disco nao presta ({e})"
        )
    });
    todos_achados(&mut t, 1..=3_000);
}

/// **509, o `.ndx` sozinho: a porta da arvore que nao presta nao responde Ok
/// por cima da recusa.**
///
/// Numa tabela, o `.reg` e sincronizado DEPOIS do `.ndx` e fica devendo quando
/// o `.ndx` recusa -- entao o fecho repetido da tabela ja recusa pelo `.reg`.
/// O `.ndx` sozinho (o `.fts` e um; o `NdxFile` e publico) nao tem esse
/// vizinho: as paginas ja foram entregues ao nucleo e estao limpas em RAM, e
/// a porta «arvore que nao presta nao sincroniza» -- que o `fsync` recusado
/// fecha, porque ele a faz nao prestar -- respondia Ok sem tocar no disco. E
/// a conferencia no topo do `NdxFile::sincronizar` que segura isso.
#[test]
fn indice_sozinho_que_o_fsync_recusou_nao_repete() {
    use phxsql_core::keyenc::{escrever_componente, largura_componente};
    use phxsql_store::ndx::NdxFile;

    let d = DirTemp::novo("509-indice");
    let caminho = d.join("t.ndx");
    let esquema = Schema::new(
        "t",
        vec![Column::new("k", ColumnType::Int8)],
        vec![IndexDef::new("porChave", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut n = NdxFile::criar(&caminho, &esquema).unwrap();
    n.sincronizar().unwrap();
    let mut chave = vec![0u8; largura_componente(&ColumnType::Int8).unwrap()];
    for v in 1..=2_000i64 {
        escrever_componente(&Value::Int(v), &ColumnType::Int8, false, false, &mut chave).unwrap();
        n.inserir(0, &chave, v as u64).unwrap();
    }
    falha_de_teste::armar(&d, Onde::Fsync, 1);
    assert!(n.sincronizar().is_err(), "premissa: a recusa forjada");
    assert!(
        n.sincronizar().is_err(),
        "o fecho repetido do .ndx respondeu Ok: a porta da arvore que nao \
         presta respondeu sem conferir a recusa"
    );
    drop(n);
    assert_eq!(
        std::fs::read(&caminho).unwrap()[52],
        1,
        "o Drop baixou a marca do .ndx cujo fsync foi recusado"
    );
}

/// **509, sem o `.ndx` para segurar: o diario sozinho tambem nao repete.**
///
/// Numa tabela, o `NdxFile::sincronizar` confere a recusa antes da porta que
/// responde Ok sem disco -- e isso bastaria para o `Table::sincronizar`
/// recusar mesmo que o `sync_all` do motor repetisse. Quem sincroniza um
/// arquivo de `Volumes` SEM `.ndx` ao lado (o diario aberto sozinho, a
/// lixeira, a copia do `.reg` na migracao) so tem a conferencia do proprio
/// `sync_all`, e e ela que este teste segura: sem ela, o segundo `fsync` do
/// mesmo `.log` responde Ok.
#[test]
fn o_diario_sozinho_tambem_nao_repete() {
    use phxsql_core::paginacao::Paginacao;
    use phxsql_store::log::{LogFile, Operacao};

    let d = DirTemp::novo("509-diario");
    let mut l = LogFile::criar(&*d, "t", Paginacao::DESLIGADA).unwrap();
    l.sincronizar().unwrap();
    for rowid in 1..=50 {
        l.registrar(Operacao::Inclusao, rowid, 1).unwrap();
    }
    falha_de_teste::armar(&d, Onde::Fsync, 1);
    assert!(l.sincronizar().is_err(), "premissa: a recusa forjada");
    assert!(
        l.sincronizar().is_err(),
        "o fsync recusado do diario foi repetido e respondeu Ok"
    );
    drop(l);
    let mut l = LogFile::abrir(&*d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar(Operacao::Inclusao, 51, 1).unwrap();
    assert!(
        l.sincronizar().is_err(),
        "o diario REABERTO sincronizou Ok depois de um fsync recusado no mesmo \
         diretorio"
    );
}

/// O tamanho do arquivo no disco, e zero quando ele ainda nao existe.
fn tamanho(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// **533: a SUBIDA do byte 52 vai ao disco antes da primeira escrita.**
///
/// Sem o `fdatasync` da subida, o 1 ficava no cache do nucleo e o `.reg`
/// seguia gravando: numa queda de energia o disco guardava as paginas novas
/// (ou so o `.reg` novo) sob o 0 do ultimo fecho, e a proxima abertura
/// confiava na arvore -- o pai com filhas se apagava calado (parecer do DBA
/// do 522, C4; a prova contra o SO e o cenario 533 da
/// `bancada/catastrofes/`). Aqui a recusa e forjada no `fdatasync` do `.ndx`
/// SO (a arma e por prefixo de texto): com o conserto, o `inserir` recusa e o
/// `.reg` nao anda; com o defeito, a arma nem dispara, porque nada sincroniza
/// na subida, e a linha entra com o 1 so no cache.
#[test]
fn a_subida_recusada_recusa_antes_do_reg() {
    let d = DirTemp::novo("533-subida");
    let mut t = Table::criar(&*d, esquema()).unwrap();
    t.inserir(&linha(1)).unwrap();
    t.sincronizar().unwrap();
    assert_eq!(byte_52(&d), 0, "premissa: o fecho da janela baixou a marca");
    let reg = d.join("pedidos.reg");
    let (reg_antes, vivas_antes) = (tamanho(&reg), t.registros());
    let ndx = d.join("pedidos.ndx");
    falha_de_teste::armar(&ndx, Onde::Fsync, 1);
    let r = t.inserir(&linha(2)).map_err(|e| e.to_string());
    falha_de_teste::desarmar(&ndx);
    assert!(
        r.is_err(),
        "a linha entrou sem a subida do byte 52 ter ido ao disco: nenhum \
         fdatasync passou pelo motor na primeira escrita da janela (pedido 533)"
    );
    assert_eq!(
        tamanho(&reg),
        reg_antes,
        "o .reg cresceu com a subida recusada: a recusa veio DEPOIS do slot"
    );
    assert_eq!(t.registros(), vivas_antes, "a linha contou como viva");
}

/// **533: e so UMA vez por janela -- o segundo `fdatasync` nao entra calado.**
///
/// A subida paga um `fdatasync` na passagem de 0 para 1, e so ali: a arma
/// posta DEPOIS da primeira escrita da janela nao pode disparar em nenhuma
/// das 300 seguintes (que sujam e despejam paginas), e so o fecho a
/// encontra. Com um `fdatasync` por pagina suja -- o conserto ingenuo --, a
/// segunda linha ja recusa. A contagem pelo nucleo e a catraca
/// `TETO_FSYNC_DA_SUBIDA` (`--example fsync-da-subida`).
#[test]
fn a_subida_sincroniza_uma_vez_por_janela() {
    let d = DirTemp::novo("533-uma-vez");
    let mut t = Table::criar(&*d, esquema()).unwrap();
    t.sincronizar().unwrap();
    t.inserir(&linha(1)).unwrap();
    let ndx = d.join("pedidos.ndx");
    falha_de_teste::armar(&ndx, Onde::Fsync, 1);
    for id in 2..=300 {
        if let Err(e) = t.inserir(&linha(id)) {
            falha_de_teste::desarmar(&ndx);
            panic!(
                "a linha {id} achou um fdatasync no .ndx dentro da MESMA janela: \
                 a subida sincroniza mais de uma vez ({e})"
            );
        }
    }
    let fecho = t.sincronizar();
    falha_de_teste::desarmar(&ndx);
    assert!(
        fecho.is_err(),
        "premissa: a arma continuava la para o fecho da janela encontrar"
    );
}

/// **533, o irmao: o `.fts` tem a mesma marca, e a subida dele tambem vai ao
/// disco.**
///
/// O `.fts` e um `.ndx` por dentro (`FtsFile` embrulha um `NdxFile`), entao o
/// conserto e o MESMO `levantar_marca` -- este teste prova que o embrulho nao
/// o contorna.
///
/// Prova so o lado da PAGINA: a subida do `.fts` vai ao disco antes da
/// primeira pagina dele. O lado do `.reg` nao vale para o `.fts` -- o
/// `indexar_texto` roda DEPOIS do slot (`Table::inserir`), entao uma queda
/// entre o slot e a subida deixa o indice de texto atras com o 0, calado. Esse
/// lado e o pedido 472 (parecer do DBA sobre o 533, C1).
#[test]
fn a_subida_do_fts_tambem_vai_ao_disco() {
    use phxsql_store::fts::FtsFile;
    let d = DirTemp::novo("533-fts");
    let caminho = d.join("pedidos.fts");
    let mut f = FtsFile::criar(&caminho, vec![true], false).unwrap();
    f.indexar(0, 1, "a fenix guardada").unwrap();
    f.sincronizar().unwrap();
    assert_eq!(
        std::fs::read(&caminho).unwrap()[52],
        0,
        "premissa: o fecho baixou a marca do .fts"
    );
    falha_de_teste::armar(&caminho, Onde::Fsync, 1);
    let r = f.indexar(0, 2, "outra fenix");
    falha_de_teste::desarmar(&caminho);
    assert!(
        r.is_err(),
        "o .fts indexou sem a subida do byte 52 ter ido ao disco (pedido 533)"
    );
}

/// Dois indices, para o diretorio vazio e o diretorio pela metade serem
/// diferentes do inteiro.
fn esquema_de_dois_indices() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

/// **533, P1 do parecer do DBA: o `.ndx` refeito leva o diretorio INTEIRO na
/// primeira subida.**
///
/// A subida roda na primeira `gravar_pagina` de `criar_com`, e o `fdatasync`
/// leva ao disco o cabecalho daquele instante. Com as raizes empurradas uma a
/// uma depois de cada folha, esse cabecalho tinha o byte 52 em 1 e ZERO
/// indices: uma queda de energia no meio do `reindexar` -- o do arranque do
/// 522 inclusive -- deixava a tabela travada em «o .ndx tem 0 indices, o
/// esquema do .reg declara 2», e nao ha porta que reconstrua sem abrir.
///
/// A queda se emula pela recusa forjada no `fdatasync` da subida: o cabecalho
/// ja foi ao arquivo quando ele recusa, e depois da recusa nada mais o regrava
/// (o `fechar` para em `pode_baixar_a_marca`). O que fica e o que o disco
/// guardaria. Com o conserto a reabertura diz «marcado, reconstrua»; com o
/// defeito, recusa abrir.
#[test]
fn o_ndx_refeito_leva_o_diretorio_inteiro_na_primeira_subida() {
    let d = DirTemp::novo("533-p1-diretorio");
    let mut t = Table::criar(&*d, esquema_de_dois_indices()).unwrap();
    for id in 1..=200 {
        t.inserir(&linha(id)).unwrap();
    }
    t.sincronizar().unwrap();
    let ndx = d.join("pedidos.ndx");
    falha_de_teste::armar(&ndx, Onde::Fsync, 1);
    let refeito = t.reindexar().map(|_| ());
    falha_de_teste::desarmar(&ndx);
    assert!(
        refeito.is_err(),
        "premissa: a subida do .ndx refeito tinha de encontrar a recusa forjada"
    );
    drop(t);
    assert_eq!(byte_52(&d), 1, "o cabecalho da subida tem o byte em 1");
    match Table::abrir(&*d, "pedidos") {
        Ok(t) => assert!(
            t.indice_precisa_reconstruir(),
            "a tabela abriu confiando num .ndx que a queda deixou pela metade"
        ),
        Err(e) => panic!(
            "o cabecalho que a subida levou ao disco travou a tabela -- e nao ha \
             porta que reconstrua sem abrir: {e}"
        ),
    }
}

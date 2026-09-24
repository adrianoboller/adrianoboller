//! O `fechar` NAO baixa o byte 52 -- pedido 522.
//!
//! # O defeito, provado contra o sistema operacional
//!
//! O `fechar` levava as paginas do `.ndx` ao nucleo e gravava o cabecalho com
//! o byte 52 em 0, sem `fsync`. O nucleo escreve as paginas na ordem dele, e o
//! cabecalho e a pagina 0: com a escrita de fundo recusada (ou a maquina
//! caindo), o disco ficou com o cabecalho LIMPO sobre paginas que nao
//! chegaram. Medido pelo papel C (ext4 sobre loop com provisionamento fino,
//! 3/3): byte 52 = 0, `CRC invalido na pagina 3`, `precisa_reconstruir`
//! falso. A prova contra o SO mora em `bancada/catastrofes/prova.sh`, cenario
//! `522`; estes testes provam a CONDUTA de depois, sem privilegio.
//!
//! # O que estes testes travam
//!
//! 1. so o `sincronizar`, depois dos dois `fsync`, grava o 0 no arquivo;
//! 2. o 1 que o `fechar` deixou, reaberto no MESMO processo, nao manda
//!    reconstruir -- o servidor abre e fecha a tabela a cada pedido;
//! 3. o mesmo 1, visto por um processo NOVO, manda -- e o preco;
//! 4. o atestado do processo vale para o arquivo, e nao para o caminho;
//! 5. a escrita que nao terminou tira o atestado antes de mexer;
//! 6. o irmao: o `.fts`, que nenhum `fsync` alcancava.

mod comum;
use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::esquecer_atestados_para_teste;
use phxsql_store::table::Table;
use std::path::Path;

fn esquema() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(id: i64) -> Vec<Value> {
    vec![Value::Int(id), Value::Str(format!("cliente {id:08}"))]
}

/// O byte 52, lido do ARQUIVO: e o que um processo novo vai ler.
fn byte_52(d: &Path, arquivo: &str) -> u8 {
    std::fs::read(d.join(arquivo)).unwrap()[52]
}

/// Tabela sincronizada com `n` linhas: o arquivo comeca dizendo 0.
fn semeada(d: &Path, n: i64) {
    let mut t = Table::criar(d, esquema()).unwrap();
    for id in 1..=n {
        t.inserir(&linha(id)).unwrap();
    }
    t.sincronizar().unwrap();
}

fn todos_achados(t: &mut Table, ids: impl Iterator<Item = i64>) {
    for id in ids {
        let r = t
            .buscar("porId", &[Value::Int(id)])
            .map_err(|e| e.to_string());
        assert_eq!(r, Ok(vec![id as u64]), "o id {id} pela chave");
    }
}

/// **1 e 2: o `fechar` deixa o 1, a reabertura aqui confia, e so o
/// `sincronizar` grava o 0.**
///
/// Com o defeito reposto (o `fechar` baixando a marca) o byte sai 0 do
/// `Drop` -- o mesmo 0 que o nucleo guardou sobre paginas perdidas.
#[test]
fn o_fechar_deixa_o_1_e_so_o_sincronizar_grava_o_0() {
    let d = DirTemp::novo("522-fechar");
    semeada(&d, 10);
    assert_eq!(byte_52(&d, "pedidos.ndx"), 0, "premissa: sincronizada");

    {
        let mut t = Table::abrir(&d, "pedidos").unwrap();
        for id in 11..=3_000 {
            t.inserir(&linha(id)).unwrap();
        }
    }
    assert_eq!(
        byte_52(&d, "pedidos.ndx"),
        1,
        "o fechar gravou o byte 52 em 0 sem fsync: numa queda da maquina o \
         nucleo pode guardar esse cabecalho e perder as paginas"
    );

    // O mesmo processo reabre como o servidor reabre a cada pedido: o 1 que
    // ele mesmo deixou nao e queda.
    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(
        !t.indice_precisa_reconstruir(),
        "o 1 que este processo deixou num fecho limpo mandou reconstruir: \
         toda tabela escrita ficaria inutil ate o fecho da janela"
    );
    todos_achados(&mut t, 1..=3_000);
    t.sincronizar().unwrap();
    drop(t);
    assert_eq!(byte_52(&d, "pedidos.ndx"), 0, "o sincronizar baixa o 0");
}

/// **3: o processo novo le o 1 do `fechar` e manda reconstruir.**
///
/// E o preco do conserto, e o teste existe para ele nao sumir calado: sem
/// atestado, o 1 quer dizer «pode estar para tras», porque o processo novo nao
/// sabe se a maquina caiu no meio. Com o defeito reposto a abertura confia
/// num arquivo que so foi ao nucleo.
#[test]
fn o_processo_novo_manda_reconstruir_o_que_so_foi_fechado() {
    let d = DirTemp::novo("522-processo-novo");
    semeada(&d, 10);
    {
        let mut t = Table::abrir(&d, "pedidos").unwrap();
        for id in 11..=500 {
            t.inserir(&linha(id)).unwrap();
        }
    }
    esquecer_atestados_para_teste(&d);

    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(
        t.indice_precisa_reconstruir(),
        "um processo novo confiou num .ndx que so foi ao nucleo, sem fsync"
    );
    let recado = t
        .buscar("porId", &[Value::Int(1)])
        .map_err(|e| e.to_string())
        .unwrap_err();
    assert!(recado.contains("reparar indice"), "recado: {recado}");
    t.reindexar().unwrap();
    todos_achados(&mut t, 1..=500);
}

/// **4: o atestado e do ARQUIVO, e nao do caminho.**
///
/// Uma restauracao por cima ou uma copia poe outro `.ndx` no mesmo caminho.
/// Aqui ele e uma copia de um instante anterior, marcada -- e a reabertura
/// neste processo nao pode confiar nela so porque o caminho foi atestado.
#[test]
fn o_atestado_nao_vale_para_outro_arquivo_no_mesmo_caminho() {
    let d = DirTemp::novo("522-outro-arquivo");
    semeada(&d, 10);
    {
        let mut t = Table::abrir(&d, "pedidos").unwrap();
        for id in 11..=100 {
            t.inserir(&linha(id)).unwrap();
        }
    }
    let velho = std::fs::read(d.join("pedidos.ndx")).unwrap();
    assert_eq!(velho[52], 1, "premissa: a copia e de um arquivo marcado");
    {
        let mut t = Table::abrir(&d, "pedidos").unwrap();
        for id in 101..=2_000 {
            t.inserir(&linha(id)).unwrap();
        }
    }
    std::fs::write(d.join("pedidos.ndx"), &velho).unwrap();

    let t = Table::abrir(&d, "pedidos").unwrap();
    assert!(
        t.indice_precisa_reconstruir(),
        "o atestado do caminho valeu para outro arquivo: a copia antiga, sem \
         as chaves 101..2000, foi aceita como coerente"
    );
}

/// **5: a escrita que para no meio tira o atestado ANTES de mexer.**
///
/// O punho abriu atestado, escreveu e caiu sem `Drop` (`forget`, a queda do
/// processo vista de dentro dele). A reabertura aqui tem de mandar
/// reconstruir -- o que ela faria num processo novo. Com o atestado saindo so
/// no `fechar`, ela confiaria na arvore de antes da escrita.
///
/// Poucas linhas, e de proposito: a insercao que divide pagina regrava o
/// cabecalho na hora («a ESTRUTURA vai na hora»), o CRC muda, e o atestado
/// velho deixa de casar SOZINHO -- o teste passaria com o defeito. Sem
/// divisao so o contador muda, e ele fica em RAM: o cabecalho do arquivo e o
/// mesmo que o `fechar` atestou, e o `.reg` esta nove linhas a frente dele.
#[test]
fn a_escrita_que_nao_terminou_tira_o_atestado() {
    let d = DirTemp::novo("522-no-meio");
    semeada(&d, 10);
    {
        let mut t = Table::abrir(&d, "pedidos").unwrap();
        t.inserir(&linha(11)).unwrap();
    }
    let crc_atestado = std::fs::read(d.join("pedidos.ndx")).unwrap()[124..128].to_vec();
    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(!t.indice_precisa_reconstruir(), "premissa: atestada");
    for id in 12..=20 {
        t.inserir(&linha(id)).unwrap();
    }
    std::mem::forget(t);
    assert_eq!(
        std::fs::read(d.join("pedidos.ndx")).unwrap()[124..128].to_vec(),
        crc_atestado,
        "premissa: o cabecalho no arquivo e o mesmo que o fechar atestou"
    );

    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert!(
        t.indice_precisa_reconstruir(),
        "o atestado sobreviveu a uma escrita que nao fechou: a reabertura \
         confiou numa arvore sem as chaves que so estavam na RAM do punho perdido"
    );
    t.reindexar().unwrap();
    todos_achados(&mut t, 1..=20);
}

/// **B1 do papel C: renomear, duplicar e colar uma tabela recem-escrita -- o
/// comportamento VELHO, e o destino abre sem recusa.**
///
/// O atestado mora no caminho, e as tres operacoes poem o `.ndx` num caminho
/// novo sem `fsync`. Sem o atestado ir junto, o destino abria com o 1 e sem
/// atestado e recusava TUDO, «arquivo corrompido», sem queda nenhuma (medido:
/// 9 em 9; antes do 522, 0 em 3). E o outro lado: o processo novo continua
/// mandando reconstruir o destino -- a copia so esta no nucleo.
#[test]
fn renomear_duplicar_e_colar_tabela_recem_escrita_abrem_sem_recusa() {
    use phxsql_store::catalogo::Instancia;

    for operacao in ["renomear", "duplicar", "colar"] {
        let d = DirTemp::novo(&format!("522-b1-{operacao}"));
        let inst = Instancia::nova(&d).unwrap();
        let z = inst.criar_database("Z").unwrap();
        let y = inst.criar_database("Y").unwrap();
        {
            let mut t = z.criar_tabela(None, esquema()).unwrap();
            for id in 1..=300 {
                t.inserir(&linha(id)).unwrap();
            }
            // Sem sincronizar: a janela ainda nao fechou.
        }
        let (destino, nome) = match operacao {
            "renomear" => {
                z.renomear_tabela("pedidos", "vendas").unwrap();
                (&z, "vendas")
            }
            "duplicar" => {
                z.duplicar_tabela("pedidos", "copia").unwrap();
                (&z, "copia")
            }
            _ => {
                z.copiar_tabela_para("pedidos", &y, "pedidos").unwrap();
                (&y, "pedidos")
            }
        };
        assert_eq!(
            byte_52(destino.caminho(), &format!("{nome}.ndx")),
            1,
            "{operacao}: premissa -- o destino chega marcado"
        );
        let mut t = destino.abrir_qualificada(nome).unwrap();
        assert!(
            !t.indice_precisa_reconstruir(),
            "{operacao}: o destino recusou sem queda nenhuma -- o atestado ficou \
             no caminho de origem"
        );
        todos_achados(&mut t, [1, 150, 300].into_iter());
        t.inserir(&linha(301))
            .unwrap_or_else(|e| panic!("{operacao}: o destino tem de aceitar escrita: {e}"));
        drop(t);

        // O processo novo nao tem o atestado: a copia so esta no nucleo.
        esquecer_atestados_para_teste(&d);
        let t = destino.abrir_qualificada(nome).unwrap();
        assert!(
            t.indice_precisa_reconstruir(),
            "{operacao}: um processo novo confiou num destino que so esta no nucleo"
        );
    }
}

/// **6, o irmao: o `.fts` vai ao fecho da janela.**
///
/// O `.fts` e um `.ndx` por dentro, e o `Table::sincronizar` nao o alcancava:
/// o byte 52 dele so descia pelo `fechar`, sem `fsync` nenhum. A prova de que
/// agora ele vai ao disco e a recusa forjada no `fsync` DELE derrubar o fecho;
/// e a de que a marca desce e o processo novo abrir sem mandar reconstruir.
#[cfg(debug_assertions)]
#[test]
fn o_indice_de_texto_vai_ao_fecho_da_janela() {
    use phxsql_store::sincronia::falha_de_teste::{self, Onde};

    let esquema_texto = || {
        Schema::new(
            "docs",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("titulo", ColumnType::Str(80)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
        .com_indices_de_texto(vec![IndiceDeTexto::new("porTitulo", 1)])
        .unwrap()
    };
    let doc = |i: i64| vec![Value::Int(i), Value::Str(format!("pedido numero {i}"))];

    // A marca desce pelo fecho, e o processo novo confia.
    let d = DirTemp::novo("522-fts");
    {
        let mut t = Table::criar(&d, esquema_texto()).unwrap();
        for i in 1..=200 {
            t.inserir(&doc(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    assert_eq!(
        byte_52(&d, "docs.fts"),
        0,
        "o fecho da janela nao baixou a marca do .fts: nenhum fsync o alcanca"
    );
    esquecer_atestados_para_teste(&d);
    let t = Table::abrir(&d, "docs").unwrap();
    assert!(!t.indice_precisa_reconstruir());
    drop(t);

    // E o fecho passa pelo `fsync` do `.fts`: recusado ali, ele recusa.
    let d = DirTemp::novo("522-fts-recusa");
    let mut t = Table::criar(&d, esquema_texto()).unwrap();
    t.inserir(&doc(1)).unwrap();
    falha_de_teste::armar(&d.join("docs.fts"), Onde::Fsync, 1);
    let r = t.sincronizar();
    falha_de_teste::desarmar(&d.join("docs.fts"));
    assert!(
        r.is_err(),
        "o fecho da janela respondeu Ok sem mandar o .fts ao disco"
    );
}

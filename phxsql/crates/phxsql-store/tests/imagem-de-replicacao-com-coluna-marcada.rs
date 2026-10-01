//! A imagem de replicacao de uma linha com coluna marcada -- a ASSIMETRIA,
//! medida e travada (pedido 342).
//!
//! # Por que um teste que prova um vazamento
//!
//! Porque a decisao do dono foi **exigir a cifra do fio**, e nao selar a
//! imagem. Entao a imagem continua carregando o valor da coluna INLINE
//! marcada em claro, e quem protege e o CANAL (`op_replicar` recusa sem o
//! tunel da §7 -- `servidor.rs`, `testes_da_cifra_exigida_na_replicacao`).
//!
//! Escrever isso num documento e deixar sem teste seria a mesma armadilha do
//! `.ndx`: a lista do `SEGURANCA.md` §11.3 e usada como inventario, e
//! inventario sem prova envelhece calado. **Se um dia a imagem passar a levar
//! a faixa marcada selada, este teste CAI -- e cair aqui e o aviso para
//! apagar a linha do documento e reabrir o pedido 344.**
//!
//! # A outra metade, que estava certa e sozinha
//!
//! A coluna EXTERNA marcada (`Memo`/`Bin`) ja viaja SELADA na mesma imagem, e
//! isso foi deliberado -- o comentario em `Table::conteudo_externo` diz por
//! que. O achado do pedido 342 nao e nenhuma das duas metades: e que elas
//! erram para lados opostos, pelo mesmo cano e no mesmo evento, e nunca foram
//! desenhadas juntas.
//!
//! # O que o pedido 344 mudou: o DIARIO sela, o FIO abre
//!
//! Selada no fio, a metade externa nao replicava: o sal e por arquivo, e nem a
//! mesma senha abre o selado de outro `.reg`. Hoje a imagem do DIARIO continua
//! levando o externo selado -- e diz que selou, pelo bit `EXTERNO_SELADO` --,
//! e o `replicar` a abre so na resposta (`Table::imagem_para_o_fio`), pelo fio
//! que o 342 ja exige cifrado. Quem recebe imagem selada recusa NOMEANDO o
//! motivo, em vez de gravar o texto cifrado como o anexo.

mod comum;
use std::sync::Mutex;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA: &str = "a chave do cofre de teste";
/// O valor da coluna INLINE marcada.
const NOME: &str = "Fulano de Tal da Silva";
/// O valor da coluna EXTERNA marcada.
const FICHA: &str = "anotacao confidencial sobre o cliente, com detalhe";

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("ficha", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty() && palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// As duas metades da imagem, na mesma linha e no mesmo evento.
///
/// **Medido em 23/09/2026, com o cofre ligado e a MESMA senha:**
/// `rowid=1 imagem=192 B | nome EM CLARO na imagem: SIM | memo em claro: nao`,
/// repartido em `payload=90 B` e `externos=98 B`.
///
/// # O TOTAL nao e guarda, e os dois vereditos sao
///
/// Os 192 andam com coisas que nada tem a ver com o pedido 342: a largura do
/// payload sobe a cada coluna de sistema nova -- as sete de hoje somam 89
/// bytes mais 1 de mapa de nulos --, e o pedaco dos externos sobe com o
/// acrescimo da cifra do `.memo`. Uma versao anterior deste comentario dizia
/// **146 B**, e o numero nao reproduz mais: envelheceu calado em seis dias,
/// que e exatamente a lei desta casa sobre numero digitado a mao.
///
/// Por isso as duas asercoes abaixo sao sobre os VEREDITOS -- inline em
/// claro, externo selado --, e nao sobre o tamanho. Veredito nao envelhece
/// com a largura do esquema; total envelhece.
#[test]
fn a_imagem_leva_o_inline_em_claro_e_o_externo_selado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = comum::DirTemp::novo("imagem-marcada");
    cofre::definir(SENHA, RAPIDO).unwrap();

    let mut t = Table::criar(&d, esquema()).unwrap();
    let rowid = t
        .inserir(&[
            Value::Int(1),
            Value::Str(NOME.into()),
            Value::Memo(FICHA.into()),
        ])
        .unwrap();
    t.sincronizar().unwrap();

    let imagem = t.imagem_da_linha_do_rowid(rowid).unwrap();

    // Controle positivo: o arquivo esta mesmo cifrado. Sem ele, "o memo nao
    // aparece" poderia ser so um memo que nao foi gravado.
    assert!(t.cifrada(), "a tabela tinha de estar cifrada");

    assert!(
        contem(&imagem, NOME.as_bytes()),
        "a coluna INLINE marcada deixou de viajar em claro na imagem -- se isso \
         foi de proposito, apague a assimetria do SEGURANCA.md 11.8 e reabra o \
         pedido 344, porque a replica nao tem chave compativel"
    );
    assert!(
        !contem(&imagem, FICHA.as_bytes()),
        "a coluna EXTERNA marcada passou a viajar em CLARO -- a metade que \
         estava certa quebrou (Table::conteudo_externo)"
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Sem coluna marcada, nada disto acontece: a imagem e a de sempre.
///
/// E o controle do ALCANCE. O portao do `op_replicar` olha
/// `Table::tem_dado_pessoal`, e uma tabela sem marca nenhuma nao e tocada por
/// linha nenhuma dele -- continua replicando em claro como sempre replicou.
#[test]
fn sem_coluna_marcada_a_imagem_e_a_de_sempre() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = comum::DirTemp::novo("imagem-sem-marca");
    cofre::definir(SENHA, RAPIDO).unwrap();

    let esquema = Schema::new(
        "publicos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = Table::criar(&d, esquema).unwrap();
    let rowid = t
        .inserir(&[
            Value::Int(1),
            Value::Str(NOME.into()),
            Value::Memo(FICHA.into()),
        ])
        .unwrap();
    t.sincronizar().unwrap();

    assert!(
        !t.tem_dado_pessoal(),
        "a tabela de controle nao pode ter coluna marcada"
    );
    assert!(
        !t.cifrada(),
        "tabela sem coluna marcada nasce em claro mesmo com o cofre ligado"
    );
    let imagem = t.imagem_da_linha_do_rowid(rowid).unwrap();
    assert!(contem(&imagem, NOME.as_bytes()));
    assert!(contem(&imagem, FICHA.as_bytes()));

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// O esquema do caso 344: o externo marcado e um `Bin`, onde o texto cifrado
/// cabe calado (num `Memo` ele ao menos quebraria o UTF-8).
fn esquema_bin() -> Schema {
    Schema::new(
        "laudos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("anexo", ColumnType::Bin).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// A imagem do DIARIO de uma origem com cofre, e a mesma imagem ABERTA para
/// o fio. O diretorio da origem sai junto, para a limpeza.
fn imagens_da_origem(rotulo: &str) -> (Vec<u8>, Vec<u8>, comum::DirTemp) {
    let d = comum::DirTemp::novo(rotulo);
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut t = Table::criar(&d, esquema_bin()).unwrap();
    let rowid = t
        .inserir(&[Value::Int(1), Value::Bin(FICHA.as_bytes().to_vec())])
        .unwrap();
    let do_diario = t.imagem_da_linha_do_rowid(rowid).unwrap();
    let do_fio = t.imagem_para_o_fio(&do_diario).unwrap();
    (do_diario, do_fio, d)
}

/// **344, a recusa que NOMEIA.** A imagem do diario -- selada com a chave do
/// arquivo da origem -- aplicada direto numa replica: sem cofre recusa
/// dizendo que falta o cofre; com a MESMA senha recusa dizendo que a chave e
/// outra por construcao, e nunca «o dado foi alterado».
///
/// # O vermelho
///
/// Antes do selo na imagem, a replica sem cofre gravava os 68 bytes selados
/// como o anexo e respondia `ok`; a com cofre devolvia o erro do cofre,
/// «ou o dado foi alterado».
#[test]
fn imagem_selada_recusa_nomeando_e_nunca_grava_o_selado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let (do_diario, _, d_o) = imagens_da_origem("344-recusa-origem");
    assert!(
        !contem(&do_diario, FICHA.as_bytes()),
        "controle: o diario tinha de levar o anexo selado"
    );

    // Replica SEM cofre.
    cofre::desligar();
    let d_r = comum::DirTemp::novo("344-recusa-sem-cofre");
    let mut r = Table::criar(&d_r, esquema_bin()).unwrap();
    let e = match r.aplicar_evento(Operacao::Inclusao, 1, &do_diario) {
        Ok(_) => panic!(
            "a replica sem cofre aceitou o anexo selado e guardou {:?}",
            r.ler(1).unwrap()
        ),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("anexo") && e.contains("cofre"), "{e}");
    // Fora da faixa ou ausente: as duas querem dizer que nada se gravou.
    assert!(
        r.ler(1).map(|l| l.is_none()).unwrap_or(true),
        "gravou a linha mesmo recusando"
    );
    drop(r);

    // Replica com a MESMA senha.
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d_m = comum::DirTemp::novo("344-recusa-mesma-senha");
    let mut m = Table::criar(&d_m, esquema_bin()).unwrap();
    let e = match m.aplicar_evento(Operacao::Inclusao, 1, &do_diario) {
        Ok(_) => panic!("a chave da replica abriu o selado de outro arquivo"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("sal e por arquivo"), "{e}");
    assert!(
        !e.contains("o dado foi alterado"),
        "acusou adulteracao: {e}"
    );
    drop(m);

    cofre::desligar();
    for d in [&d_o, &d_r, &d_m] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// **344, o caminho que replica -- e o 613.** A imagem aberta para o fio
/// entra na replica com a mesma senha com o anexo IGUAL -- o conteudo, nao o
/// veredito. Na replica SEM cofre ela e RECUSADA (decisao do dono,
/// 01/10/2026): o anexo marcado chegaria aberto e ficaria em claro no disco
/// dela. A prova ali e a do disco: nenhuma linha, e o texto em arquivo
/// nenhum.
#[test]
fn imagem_aberta_para_o_fio_replica_com_cofre_e_sem_cofre_recusa() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let (_, do_fio, d_o) = imagens_da_origem("344-fio-origem");
    let mut dirs = vec![d_o];
    for com_cofre in [false, true] {
        cofre::desligar();
        if com_cofre {
            cofre::definir(SENHA, RAPIDO).unwrap();
        }
        let d = comum::DirTemp::novo(if com_cofre {
            "344-fio-com-cofre"
        } else {
            "344-fio-sem-cofre"
        });
        let mut r = Table::criar(&d, esquema_bin()).unwrap();
        let aplicado = r.aplicar_evento(Operacao::Inclusao, 1, &do_fio);
        if !com_cofre {
            let e = match aplicado {
                Ok(_) => panic!("a replica sem cofre gravou o anexo marcado"),
                Err(e) => e.to_string(),
            };
            assert!(e.contains("Falta o cofre") && e.contains("anexo"), "{e}");
            assert!(
                r.ler(1).map(|l| l.is_none()).unwrap_or(true),
                "gravou a linha mesmo recusando"
            );
            drop(r);
            for arquivo in std::fs::read_dir(&d).unwrap().flatten() {
                let bytes = std::fs::read(arquivo.path()).unwrap_or_default();
                assert!(
                    !contem(&bytes, FICHA.as_bytes()),
                    "o anexo ficou em claro em {}",
                    arquivo.path().display()
                );
            }
            dirs.push(d);
            continue;
        }
        aplicado.unwrap();
        let linha = r.ler(1).unwrap().unwrap();
        assert_eq!(
            linha[1],
            Value::Bin(FICHA.as_bytes().to_vec()),
            "com_cofre={com_cofre}"
        );
        assert_eq!(r.cifrada(), com_cofre);
        drop(r);
        dirs.push(d);
    }
    cofre::desligar();
    for d in &dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A abertura para o fio NAO perde o rabo do «antes» de uma troca de chave.
///
/// `imagem_para_o_fio` desmonta e remonta a imagem para abrir os externos
/// selados -- e remontar so ate os externos jogava fora o que vem depois.
/// Seria a troca de chave voltando a virar linha nova do outro lado SO na
/// tabela cifrada, que e onde ninguem olharia.
///
/// **Defeito reposto**: tirar o `out.extend_from_slice(&imagem[fim..])` de
/// `imagem_para_o_fio` -- a asercao do fio cai com `None`.
#[test]
fn o_fio_cifrado_leva_o_antes_da_troca_de_chave() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = comum::DirTemp::novo("rabo-no-fio-cifrado");
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut t = Table::criar(&d, esquema_bin())
        .unwrap()
        .com_imagem_no_diario(true);
    let rowid = t
        .inserir(&[Value::Int(1), Value::Bin(FICHA.as_bytes().to_vec())])
        .unwrap();
    t.atualizar(
        rowid,
        &[Value::Int(2), Value::Bin(FICHA.as_bytes().to_vec())],
    )
    .unwrap();
    assert!(t.cifrada(), "controle: a tabela tinha de estar cifrada");
    let eventos = t.diario_com_imagem(0, 0).unwrap();
    let (e, do_diario) = &eventos[1];
    assert_eq!(e.operacao, Operacao::Alteracao);
    assert!(
        Table::payload_antes_da_imagem(do_diario).unwrap().is_some(),
        "controle: o diario tinha de levar o antes"
    );
    let do_fio = t.imagem_para_o_fio(do_diario).unwrap();
    let antes = t.valores_antes_da_imagem(&do_fio).unwrap();
    assert_eq!(
        antes.map(|v| v[0].clone()),
        Some(Value::Int(1)),
        "o fio cifrado perdeu o antes da troca de chave"
    );
    drop(t);
    cofre::desligar();
}

/// O esquema do caso 616: a marcada e INLINE (`Str`), e nao ha externa
/// nenhuma -- a recusa do 613 nao tinha por onde pega-la.
fn esquema_inline() -> Schema {
    Schema::new(
        "pacientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// O primeiro arquivo debaixo de `raiz` que contem `agulha`. Desce as
/// subpastas: o que conta e o disco inteiro de quem recebeu, nao so o `.reg`.
fn arquivo_que_contem(raiz: &std::path::Path, agulha: &[u8]) -> Option<std::path::PathBuf> {
    let mut pilha = vec![raiz.to_path_buf()];
    // Teto de voltas: uma tabela de teste tem dezenas de arquivos, nao mil.
    for _ in 0..1_000 {
        let p = pilha.pop()?;
        if p.is_dir() {
            pilha.extend(std::fs::read_dir(&p).unwrap().flatten().map(|i| i.path()));
        } else if contem(&std::fs::read(&p).unwrap_or_default(), agulha) {
            return Some(p);
        }
    }
    panic!("mais de mil entradas debaixo de {}", raiz.display());
}

/// **616, a metade INLINE.** A imagem aberta para o fio de uma origem com
/// cofre leva a coluna inline marcada em CLARO (o `imagem_da_linha` decifra a
/// faixa -- e o que o primeiro teste deste arquivo mede). O 613 so recusava a
/// tabela com coluna EXTERNA marcada; a inline passava e pousava em claro no
/// slot do `.reg` da replica sem cofre. Com cofre, replica e fica selada.
///
/// # O vermelho
///
/// Com o defeito reposto (a pergunta de volta a «externa marcada»), o
/// `aplicar_evento` sem cofre responde `Ok` e o nome esta no `.reg` da
/// replica: cai no `panic!` do `Ok`.
#[test]
fn replica_sem_cofre_recusa_a_coluna_inline_marcada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d_o = comum::DirTemp::novo("616-inline-origem");
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut o = Table::criar(&d_o, esquema_inline()).unwrap();
    let rowid = o
        .inserir(&[Value::Int(1), Value::Str(NOME.into())])
        .unwrap();
    let do_diario = o.imagem_da_linha_do_rowid(rowid).unwrap();
    let do_fio = o.imagem_para_o_fio(&do_diario).unwrap();
    drop(o);
    assert!(
        contem(&do_fio, NOME.as_bytes()),
        "controle: a imagem do fio tinha de levar o inline aberto"
    );

    // Replica SEM cofre: recusa, nenhuma linha, o nome em arquivo nenhum.
    cofre::desligar();
    let d_r = comum::DirTemp::novo("616-inline-sem-cofre");
    let mut r = Table::criar(&d_r, esquema_inline()).unwrap();
    let e = match r.aplicar_evento(Operacao::Inclusao, 1, &do_fio) {
        Ok(_) => panic!(
            "a replica sem cofre gravou a coluna inline marcada: {:?}",
            r.ler(1).unwrap()
        ),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("Falta o cofre") && e.contains("nome"), "{e}");
    assert!(
        r.ler(1).map(|l| l.is_none()).unwrap_or(true),
        "gravou a linha mesmo recusando"
    );
    drop(r);
    if let Some(p) = arquivo_que_contem(&d_r, NOME.as_bytes()) {
        panic!("o nome marcado ficou em claro em {}", p.display());
    }

    // O controle: COM cofre a mesma imagem replica, e o nome fica selado.
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d_c = comum::DirTemp::novo("616-inline-com-cofre");
    let mut c = Table::criar(&d_c, esquema_inline()).unwrap();
    c.aplicar_evento(Operacao::Inclusao, 1, &do_fio).unwrap();
    assert_eq!(c.ler(1).unwrap().unwrap()[1], Value::Str(NOME.into()));
    drop(c);
    if let Some(p) = arquivo_que_contem(&d_c, NOME.as_bytes()) {
        panic!("com cofre o nome ficou em claro em {}", p.display());
    }

    cofre::desligar();
    for d in [&d_o, &d_r, &d_c] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// **616, o caminho BIDIRECIONAL.** O bidirecional casa por chave e grava
/// pelos tres `*_replicado`, e nao pelo `aplicar_evento`, onde o 613 pos a
/// recusa: o mesmo dado de outro servidor pousava em claro pelo caminho
/// irmao. Os tres recusam pela MESMA conferencia (a tabela inteira, como no
/// 613), e a escrita LOCAL -- este servidor como origem -- segue.
///
/// # O vermelho
///
/// Com a recusa tirada de um dos tres, aquele responde `Ok` e cai no
/// `panic!` dele: o `inserir` e o `atualizar` deixariam o nome no `.reg`
/// daqui, o `excluir` apagaria a linha.
#[test]
fn bidirecional_sem_cofre_recusa_a_coluna_marcada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = comum::DirTemp::novo("616-bidi-sem-cofre");
    let mut t = Table::criar(&d, esquema_inline()).unwrap();
    // A escrita local continua: aqui este servidor e a ORIGEM do dado.
    let local = t
        .inserir(&[Value::Int(7), Value::Str("escrito aqui".into())])
        .unwrap();

    let e = match t.inserir_replicado(&[Value::Int(1), Value::Str(NOME.into())]) {
        Ok(_) => panic!("o inserir_replicado sem cofre gravou a coluna marcada"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("Falta o cofre") && e.contains("nome"), "{e}");

    let e = match t.atualizar_replicado(local, &[Value::Int(7), Value::Str(NOME.into())]) {
        Ok(_) => panic!("o atualizar_replicado sem cofre gravou a coluna marcada"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("Falta o cofre"), "{e}");

    let e = match t.excluir_de_vez_replicado(local, "616") {
        Ok(_) => panic!("o excluir_de_vez_replicado sem cofre passou pela recusa"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("Falta o cofre"), "{e}");
    assert_eq!(
        t.ler(local).unwrap().unwrap()[1],
        Value::Str("escrito aqui".into()),
        "a linha local mudou mesmo recusando"
    );
    drop(t);
    if let Some(p) = arquivo_que_contem(&d, NOME.as_bytes()) {
        panic!("o nome marcado ficou em claro em {}", p.display());
    }

    // O controle: COM cofre o bidirecional grava, e o nome fica selado.
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d_c = comum::DirTemp::novo("616-bidi-com-cofre");
    let mut c = Table::criar(&d_c, esquema_inline()).unwrap();
    let r = c
        .inserir_replicado(&[Value::Int(1), Value::Str(NOME.into())])
        .unwrap();
    assert_eq!(c.ler(r).unwrap().unwrap()[1], Value::Str(NOME.into()));
    drop(c);
    if let Some(p) = arquivo_que_contem(&d_c, NOME.as_bytes()) {
        panic!("com cofre o nome ficou em claro em {}", p.display());
    }

    cofre::desligar();
    for d in [&d, &d_c] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Apaga o bit `EXTERNO_SELADO` do primeiro externo da imagem -- o evento
/// como o diario o gravava ANTES do pedido 344: selado, e sem dizer.
fn sem_o_bit(imagem: &[u8]) -> Vec<u8> {
    let mut v = imagem.to_vec();
    let plen = u32::from_le_bytes(v[0..4].try_into().unwrap()) as usize;
    let i = 4 + plen + 2;
    let c = u16::from_le_bytes([v[i], v[i + 1]]);
    assert_ne!(c & 0x8000, 0, "controle: o diario de hoje acende o bit");
    v[i..i + 2].copy_from_slice(&(c & 0x7FFF).to_le_bytes());
    v
}

/// **Pedido 603 (NAO 344-a do DBA): o evento de ANTES do 344.** O diario ja
/// guardava o externo marcado selado, sem o bit. A origem -- o arquivo que
/// selou -- abre pela etiqueta da cifra e manda o anexo; o que nao abre e
/// recusado nomeando, nunca mandado.
///
/// **Defeito reposto**: tirar o braco `externo_selado` do
/// `imagem_para_o_fio` -- o evento pre-344 sai com o cifrado no lugar do
/// anexo (a primeira asercao cai), e o lixo sai `Ok` (a segunda cai).
#[test]
fn o_evento_de_antes_do_344_abre_na_origem_ou_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = comum::DirTemp::novo("603-pre-344");
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut t = Table::criar(&d, esquema_bin()).unwrap();
    let rowid = t
        .inserir(&[Value::Int(1), Value::Bin(FICHA.as_bytes().to_vec())])
        .unwrap();
    let do_diario = t.imagem_da_linha_do_rowid(rowid).unwrap();
    let do_fio = t.imagem_para_o_fio(&do_diario).unwrap();

    let antigo = sem_o_bit(&do_diario);
    assert!(
        !contem(&antigo, FICHA.as_bytes()),
        "controle: o antigo e selado"
    );
    let saiu = t.imagem_para_o_fio(&antigo).unwrap();
    assert!(
        contem(&saiu, FICHA.as_bytes()),
        "o evento pre-344 saiu para o fio com o CIFRADO no lugar do anexo"
    );
    assert_eq!(saiu, do_fio);

    // O que nao abre: conteudo do mesmo tamanho, sem etiqueta que confira.
    let mut lixo = antigo.clone();
    let n = lixo.len();
    for b in &mut lixo[n - 30..] {
        *b ^= 0x5A;
    }
    let e = match t.imagem_para_o_fio(&lixo) {
        Ok(_) => panic!("o externo que nao abre foi mandado ao fio"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("344") && e.contains("bit"), "{e}");
    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

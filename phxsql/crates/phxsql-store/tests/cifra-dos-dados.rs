//! A cifra dos arquivos de DADOS -- `.reg`, `.ndx`, `.memo` e `.bin` --,
//! provada em disco.
//!
//! # Por que isto e um teste de INTEGRACAO
//!
//! Pela mesma razao de `cifra-dos-diarios.rs`: a chave e do PROCESSO, e
//! `cargo test` roda os testes de um mesmo binario em paralelo. Ligar a cifra
//! dentro da biblioteca faria a tabela de outro teste nascer cifrada no meio
//! da corrida. Aqui o processo e so deste arquivo -- e mesmo assim os testes
//! passam pela trava, porque tambem dividem o processo entre si.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};

use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// Iteracoes no piso: o que se prova aqui e a amarracao, nao o custo do
/// PBKDF2 -- que ja tem vetor proprio em `phxsql_core::hash`.
const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA: &str = "a chave do cofre de teste";

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "phxsql-cifra-dados-{}-{rotulo}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Um segredo bem reconhecivel: se ele aparecer nos bytes, a cifra falhou.
const SEGREDO: &str = "Fulano de Tal da Silva";
const MEMO_SECRETO: &str = "anotacao confidencial sobre o cliente, com detalhe";

fn esquema(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("{SEGREDO} {i:04}")),
        Value::Memo(format!("{MEMO_SECRETO} numero {i}")),
    ]
}

/// Todos os bytes de todos os arquivos da tabela, por extensao.
fn bytes_com_extensao(d: &Path, ext: &str) -> Vec<u8> {
    let mut tudo = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some(ext) {
            tudo.extend_from_slice(&std::fs::read(&p).unwrap());
        }
    }
    tudo
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty() && palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// A versao declarada no byte 8 do primeiro arquivo com esta extensao.
fn versao(d: &Path, nome: &str, ext: &str) -> u16 {
    let b = std::fs::read(d.join(format!("{nome}.{ext}"))).unwrap();
    u16::from_le_bytes([b[8], b[9]])
}

/// `slot_size` e `data_offset`, lidos do cabecalho do `.reg` como qualquer um
/// que so tenha o arquivo leria. Sao os dois numeros de que um teste de
/// formato precisa para achar um slot na mao.
fn geometria(d: &Path, nome: &str) -> (usize, usize) {
    let b = std::fs::read(d.join(format!("{nome}.reg"))).unwrap();
    let slot = u32::from_le_bytes([b[16], b[17], b[18], b[19]]) as usize;
    let off = u64::from_le_bytes(b[44..52].try_into().unwrap()) as usize;
    (slot, off)
}

// ---------------------------------------------------------------------------
// O teste que mais importa: o comportamento VELHO
// ---------------------------------------------------------------------------

/// Tabela gravada ANTES da cifra continua abrindo, lendo e gravando depois.
///
/// E a regra da casa -- guarda nova entra pedida, nao imposta. Quem liga a
/// cifra na terca nao pode perder o que gravou na segunda, e nem receber um
/// erro de versao ao abrir.
#[test]
fn tabela_escrita_antes_da_cifra_continua_abrindo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("velha");

    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=30 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    assert_eq!(
        versao(&d, "clientes", "reg"),
        4,
        "nasceu na versao de sempre"
    );

    // Agora liga a cifra e abre a MESMA tabela.
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        let l = t.ler(7).unwrap().unwrap();
        assert_eq!(l[1], Value::Str(format!("{SEGREDO} 0007")));
        // E continua aceitando gravacao, no formato em que nasceu.
        t.inserir(&linha(31)).unwrap();
        t.atualizar(3, &linha(300)).unwrap();
        assert_eq!(
            t.ler(3).unwrap().unwrap()[1],
            Value::Str(format!("{SEGREDO} 0300"))
        );
        t.sincronizar().unwrap();
    }
    assert_eq!(
        versao(&d, "clientes", "reg"),
        4,
        "ligar a cifra nao pode reescrever a versao de uma tabela que ja existe"
    );
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Sem a secao `cifra`, o disco fica byte por byte como sempre foi.
///
/// O teste do comportamento velho pelo outro lado: nao basta abrir, tem de
/// nao ter mudado nada. Se um dia alguem trocar o padrao para "cifrado", este
/// e o teste que cai.
#[test]
fn sem_cofre_nada_muda_no_disco() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("nada-muda");

    let mut t = Table::criar(&d, esquema("clientes")).unwrap();
    for i in 1..=20 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);

    assert_eq!(versao(&d, "clientes", "reg"), 4);
    let reg = bytes_com_extensao(&d, "reg");
    assert!(
        contem(&reg, SEGREDO.as_bytes()),
        "sem cifra o nome TEM de estar legivel no .reg -- se nao estiver, \
         este teste esta medindo outra coisa"
    );
    assert!(
        contem(&bytes_com_extensao(&d, "memo"), MEMO_SECRETO.as_bytes()),
        "sem cifra o memo TEM de estar legivel"
    );
    assert!(
        contem(&bytes_com_extensao(&d, "ndx"), SEGREDO.as_bytes()),
        "sem cifra a chave TEM de estar legivel no .ndx"
    );
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------
// O que a cifra faz
// ---------------------------------------------------------------------------

/// O valor da coluna marcada some do `.reg`, do `.memo` e do espelho `.bkp`.
#[test]
fn o_dado_da_coluna_marcada_some_do_disco() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("some");
    cofre::definir(SENHA, RAPIDO).unwrap();

    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=200 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    {
        // O espelho `.bkp` e uma copia byte a byte do `.reg`, entao ele
        // tambem entra na conferencia: uma cifra que esquecesse a copia
        // deixaria o dado em claro do lado.
        let mut t = Table::abrir_espelhada(&d, "clientes").unwrap();
        t.inserir(&linha(201)).unwrap();
        t.sincronizar().unwrap();
    }

    for (ext, agulha) in [("reg", SEGREDO), ("memo", MEMO_SECRETO), ("bkp", SEGREDO)] {
        let bytes = bytes_com_extensao(&d, ext);
        assert!(
            !bytes.is_empty(),
            "o teste nao achou nenhum .{ext} -- estaria provando nada"
        );
        assert!(
            !contem(&bytes, agulha.as_bytes()),
            "o texto claro apareceu dentro do .{ext}"
        );
    }

    // E o cabecalho declara a versao cifrada.
    assert_eq!(versao(&d, "clientes", "reg"), 5);

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// O `.ndx` sobre a coluna marcada CONTINUA EM CLARO -- e este teste existe
/// para essa verdade nao poder ser esquecida.
///
/// # Por que um teste que prova um vazamento
///
/// Porque a escolha e por COLUNA, e um indice guarda a chave da coluna para
/// poder compara-la. Cifrar a chave destruiria a ordem, e sem ordem nao ha
/// B+tree -- seria trocar o indice por uma varredura. A alternativa honesta e
/// dizer: **indice sobre coluna marcada vaza o valor e a ordem**. Esta em
/// `docs/SEGURANCA.md` §10, e este teste e o que impede alguem escrever no
/// painel que a tabela esta cifrada sem essa frase do lado.
///
/// Se um dia o `.ndx` passar a ser cifrado, este teste cai -- e cair aqui e o
/// aviso para apagar a ressalva do documento.
#[test]
fn o_indice_sobre_a_coluna_marcada_continua_em_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("indice-vaza");
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=50 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    assert!(
        contem(&bytes_com_extensao(&d, "ndx"), SEGREDO.as_bytes()),
        "o .ndx deixou de guardar a chave em claro -- se isso foi de proposito, \
         apague a ressalva do SEGURANCA.md §10 junto com este teste"
    );
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Cifrada, a tabela continua sendo uma tabela: le, atualiza, exclui, busca
/// pelo indice e varre.
#[test]
fn cifrada_a_tabela_funciona_igual() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("funciona");
    cofre::definir(SENHA, RAPIDO).unwrap();

    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=500 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    // Fecha e reabre: a chave sai do cabecalho, e nao da memoria.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.ler(1).unwrap().unwrap()[1],
        Value::Str(format!("{SEGREDO} 0001"))
    );
    assert_eq!(
        t.ler(500).unwrap().unwrap()[1],
        Value::Str(format!("{SEGREDO} 0500"))
    );

    // Busca pelo indice, que e o caminho que passa pelo `.ndx`.
    let achados = t
        .buscar("porNome", &[Value::Str(format!("{SEGREDO} 0123"))])
        .unwrap();
    assert_eq!(
        achados,
        vec![123],
        "a busca pelo indice cifrado achou a linha"
    );

    // Atualizar regrava o mesmo slot com outra versao -- e outro nonce.
    t.atualizar(123, &linha(9999)).unwrap();
    assert_eq!(
        t.ler(123).unwrap().unwrap()[1],
        Value::Str(format!("{SEGREDO} 9999"))
    );

    // Excluir e varrer.
    t.excluir(2).unwrap();
    assert!(t.ler(2).unwrap().is_none());
    assert_eq!(t.varrer().unwrap().len(), 499);

    // O memo tambem volta inteiro.
    assert_eq!(
        t.ler(400).unwrap().unwrap()[2],
        Value::Memo(format!("{MEMO_SECRETO} numero 400"))
    );

    t.sincronizar().unwrap();
    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// A senha errada e a falta de senha param na ABERTURA, com texto que diz o
/// que fazer -- e nao na primeira leitura de linha.
#[test]
fn senha_errada_e_falta_de_senha_param_na_abertura() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("senha-errada");
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
    }

    cofre::definir("outra senha qualquer", RAPIDO).unwrap();
    let e = match Table::abrir(&d, "clientes") {
        Ok(_) => panic!("a tabela cifrada abriu sem a chave certa"),
        Err(e) => e.to_string(),
    };
    assert!(
        e.contains("senha") && e.contains("cifra"),
        "o erro de senha errada precisa dizer o que esta errado: {e}"
    );

    cofre::desligar();
    let e = match Table::abrir(&d, "clientes") {
        Ok(_) => panic!("a tabela cifrada abriu sem a chave certa"),
        Err(e) => e.to_string(),
    };
    assert!(
        e.contains("cifrado") && e.contains("config.json"),
        "o erro de falta de chave precisa dizer onde preencher: {e}"
    );

    let _ = std::fs::remove_dir_all(&d);
}

/// Trocar o corpo cifrado de uma linha pelo de outra nao passa.
///
/// # O defeito que este teste repoe
///
/// Sem o dado associado do slot (`aad_do_slot`), quem tem o arquivo mas nao a
/// chave ainda poderia EMBARALHAR as linhas: copiar os bytes do slot 5 por
/// cima do slot 9, consertar o CRC-32 -- que e publico -- e a linha 9 passaria
/// a devolver o conteudo da 5 sem erro nenhum. Cifra sem essa amarracao
/// protege o conteudo e nao protege a tabela.
///
/// # Quem amarra sao DUAS fechaduras, e nao uma
///
/// Esta ficha dizia que tirar o `aad` do `montar_slot` e do `abrir_slot` fazia
/// este teste ler a linha trocada. Medido pelo `bancada/guardas/`, com o
/// defeito reposto de verdade: **nao faz** -- o teste continua verde.
///
/// O endereco esta amarrado duas vezes. O `aad_do_slot` leva (volume, rowid,
/// versao) e o `cofre::nonce_de_pedaco(rowid, volume, versao, tempero)` leva
/// os mesmos tres, e nonce diferente ja da texto cifrado e etiqueta
/// diferentes. Cada uma segura sozinha; o teste so cai quando as DUAS somem.
///
/// A garantia que este teste nomeia continua de pe -- o que estava errado era
/// a atribuicao dela a uma unica peca. E o corolario do CLAUDE.md em miniatura:
/// diagnostico plausivel nao e diagnostico medido, e o errado sobrevive melhor
/// quando o conserto funcionou por outro motivo.
///
/// As tres entradas do catalogo que travam isto: `aad-fora-do-slot` e
/// `nonce-sem-endereco` afirmam a redundancia (tirar uma so nao muda nada) e
/// `endereco-fora-da-amarracao` prova a guarda (tirar as duas derruba este
/// teste). No dia em que o nonce deixar de carregar o endereco, as duas
/// primeiras deixam de ser redundantes e o relatorio avisa.
#[test]
fn trocar_o_corpo_de_uma_linha_pela_outra_nao_passa() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("embaralha");
    cofre::definir(SENHA, RAPIDO).unwrap();

    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=10 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let (slot_size, data_offset) = geometria(&d, "clientes");

    // Copia o slot 5 por cima do slot 9, INCLUSIVE o CRC -- que e o que um
    // atacante sem chave conseguiria fazer.
    let caminho = d.join("clientes.reg");
    let mut bytes = std::fs::read(&caminho).unwrap();
    let de = data_offset + 4 * slot_size;
    let para = data_offset + 8 * slot_size;
    let copia = bytes[de..de + slot_size].to_vec();
    bytes[para..para + slot_size].copy_from_slice(&copia);
    std::fs::write(&caminho, &bytes).unwrap();
    // O espelho, se existir, tem de estragar junto: senao a segunda chance
    // conserta e o teste passa medindo o espelho em vez da etiqueta.
    if d.join("clientes.bkp").exists() {
        std::fs::write(d.join("clientes.bkp"), &bytes).unwrap();
    }

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let erro = t.ler(9);
    assert!(
        erro.is_err(),
        "a linha 9 abriu com o corpo da linha 5: a etiqueta nao amarrou o endereco"
    );

    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Duas gravacoes da mesma linha nunca usam o mesmo nonce.
///
/// # O defeito que este teste repoe
///
/// Se o nonce saisse so do endereco -- volume e rowid --, atualizar a linha
/// reusaria o par (chave, nonce), e o XOR dos dois textos claros vazaria: quem
/// visse as duas versoes do arquivo subtrairia uma da outra. Tirando a
/// `versao` e o `tempero` do `nonce_de_pedaco`, este teste acha texto cifrado
/// repetido e cai.
///
/// A prova nao olha nonce nenhum (ele nao vai ao disco inteiro): olha o texto
/// cifrado do MESMO conteudo gravado muitas vezes. Com nonce repetido, o
/// texto cifrado do mesmo claro tambem se repete.
#[test]
fn regravar_a_mesma_linha_nunca_repete_o_texto_cifrado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("nonce");
    cofre::definir(SENHA, RAPIDO).unwrap();

    let mut vistos = std::collections::HashSet::new();
    let caminho = d.join("clientes.reg");
    let mut t = Table::criar(&d, esquema("clientes")).unwrap();
    t.inserir(&linha(1)).unwrap();
    t.sincronizar().unwrap();
    let (slot_size, data_offset) = geometria(&d, "clientes");

    // Grava SEMPRE o mesmo conteudo, 200 vezes.
    for _ in 0..200 {
        t.atualizar(1, &linha(1)).unwrap();
        t.sincronizar().unwrap();
        let bytes = std::fs::read(&caminho).unwrap();
        let corpo = bytes[data_offset + 24..data_offset + slot_size].to_vec();
        assert!(
            vistos.insert(corpo),
            "o mesmo texto claro deu o mesmo texto cifrado duas vezes: nonce repetido"
        );
    }

    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

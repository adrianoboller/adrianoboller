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

mod comum;
use std::path::Path;
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

fn dir(rotulo: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("cifra-dados-{rotulo}"))
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

/// O tamanho do cabecalho declarado nos bytes 10..12: 128 em claro, 192 com
/// material. E o segundo discriminador do formato, e o unico que diz se o
/// sal e a prova da chave estao no arquivo.
fn cab_len(d: &Path, nome: &str) -> u16 {
    let b = std::fs::read(d.join(format!("{nome}.reg"))).unwrap();
    u16::from_le_bytes([b[10], b[11]])
}

/// O esquema do pedido 210: a UNICA coluna marcada e EXTERNA. `nome` existe e
/// nao esta marcada -- e o que deixa provar, mais abaixo, que marca-la depois
/// e recusado.
fn esquema_so_externas(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn ficha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("ficha {i:04}")),
        Value::Memo(format!("{MEMO_SECRETO} numero {i}")),
    ]
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

// ---------------------------------------------------------------------------
// Acrescentar coluna numa tabela CIFRADA
// ---------------------------------------------------------------------------

/// A reescrita do `acrescentar_coluna` decifra com o esquema velho e sela com
/// o novo -- e a linha continua legivel, com o segredo ainda fora do disco.
///
/// # Por que a coluna a mais mexe com a cifra
///
/// O texto cifrado mora NO LUGAR do claro, no offset da coluna marcada. Uma
/// coluna nova antes das de sistema move o offset de tudo que vem depois dela,
/// e a etiqueta cobre as faixas marcadas juntas. Copiar o slot byte a byte
/// deixaria a etiqueta cobrindo bytes que sairam do lugar, e a linha nao
/// abriria mais -- perda total, sem erro na hora.
#[test]
fn acrescentar_coluna_em_tabela_cifrada_mantem_a_linha_legivel() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("alter-cifrado");
    cofre::definir(SENHA, RAPIDO).unwrap();

    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=40 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    // A tabela nasceu na versao 5: ha coluna marcada e o cofre esta ligado.
    assert_eq!(versao(&d, "clientes", "reg"), 5);
    let (slot_antes, _) = geometria(&d, "clientes");

    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        let n = t
            .acrescentar_coluna(
                Column::new("situacao", ColumnType::Str(12)),
                Some(Value::Str("ativo".into())),
            )
            .unwrap();
        assert_eq!(n, 40);
    }

    // Continua na 5, e o slot cresceu exatamente a largura da coluna nova.
    assert_eq!(versao(&d, "clientes", "reg"), 5);
    let (slot_depois, _) = geometria(&d, "clientes");
    assert_eq!(slot_depois, slot_antes + 12);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let situacao = t.esquema().coluna_por_nome("situacao").unwrap();
    for i in 1..=40i64 {
        let l = t.ler(i as u64).unwrap().unwrap();
        assert_eq!(l[0], Value::Int(i));
        assert_eq!(l[1], Value::Str(format!("{SEGREDO} {i:04}")));
        assert_eq!(l[2], Value::Memo(format!("{MEMO_SECRETO} numero {i}")));
        assert_eq!(l[situacao], Value::Str("ativo".into()));
    }
    drop(t);

    // E o segredo continua fora do disco: nem o nome marcado nem o memo.
    let reg = bytes_com_extensao(&d, "reg");
    assert!(
        !contem(&reg, SEGREDO.as_bytes()),
        "o nome marcado voltou a aparecer em claro no .reg depois da alteracao"
    );
    let memo = bytes_com_extensao(&d, "memo");
    assert!(!contem(&memo, MEMO_SECRETO.as_bytes()));

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// **Guarda entregue VERMELHA em 05/09/2026 e consertada em 09/09/2026**
/// (pedido 210).
///
/// Tabela cujas UNICAS colunas marcadas sao EXTERNAS (`Memo`/`Bin`). Ate o
/// conserto ela nascia em claro com o cofre ligado, o texto sigiloso ia para
/// o `.memo` legivel -- sem erro, sem aviso, e com `{"op":"config"}`
/// respondendo `cifra.ligada: true`.
///
/// # A cadeia, medida no fonte
///
/// 1. `faixas_pessoais` faz `continue` em `col.ty.externo()`, entao coluna
///    externa marcada NAO gera faixa;
/// 2. sem faixa, `faixas.is_empty()` e o volume nascia `Material::EM_CLARO`
///    -- e o comentario ao lado dizia «uma tabela SEM coluna marcada nasce em
///    claro», que nao era este caso: havia coluna marcada;
/// 3. `selar_externo` devolvia o dado intacto porque `!self.material.cifrado()`.
///
/// O caminho de selar o externo estava escrito e estava CERTO. O que nao
/// alcancava este caso era a condicao que o LIGA, derivada so das colunas
/// inline. Hoje ela le `Schema::tem_dado_pessoal`, que enxerga as externas.
///
/// # Por que nenhum teste pegava
///
/// O `esquema()` deste arquivo marca `nome` (Str, INLINE) alem de `obs`
/// (Memo). Com uma inline marcada, `faixas` nao fica vazio, o material nasce
/// cifrado, e o `.memo` e selado -- entao toda a bateria de cifra provava o
/// caminho que FUNCIONA, e o irmao nunca foi exercitado. E a mesma forma dos
/// pedidos 172, 173 e 176: o conserto entrou no caminho que o motivou e o
/// irmao ficou.
///
/// Medido com dez linhas pelo soquete, nos dois sentidos, em 05/09:
///
/// ```text
/// so externas (Memo+Bin)         .memo 6264 B   texto em claro? SIM
/// externas + UMA inline (nome)   .memo 6664 B   texto em claro? nao
/// ```
///
/// Os 400 bytes de diferenca sao os 40 por valor (nonce de 24 + etiqueta de
/// 16) das dez linhas.
///
/// # Por que a prova REABRE a tabela
///
/// Ate 09/09 este teste so olhava o `.memo`. Medido com a condicao do
/// `criar` trocada e o resto intacto: o `.memo` saia selado, o teste
/// passava -- e a tabela NAO reabria, recusada pela conferencia do `abrir`
/// contra «desmarcar nao decifra», que lia a mesma premissa («cifrado, logo
/// ha faixa inline»). Guarda que nao percorre o ciclo inteiro -- gravar,
/// fechar, reabrir, ler, alterar -- prova meio conserto.
#[test]
fn coluna_externa_marcada_sozinha_nao_pode_ir_em_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("so-externa");
    // A gemea em claro diz quanto e o slot SEM cifra: mesmo esquema, sem
    // cofre. E contra ela que se prova que o slot nao ganhou etiqueta.
    let gemea = dir("so-externa-gemea");
    {
        let mut t = Table::criar(&gemea, esquema_so_externas("fichas")).unwrap();
        t.inserir(&ficha(1)).unwrap();
        t.sincronizar().unwrap();
    }
    cofre::definir(SENHA, RAPIDO).unwrap();

    {
        let mut t = Table::criar(&d, esquema_so_externas("fichas")).unwrap();
        assert!(
            t.cifrada(),
            "a tabela cuja unica coluna marcada e externa nasceu sem material"
        );
        for i in 1..=10 {
            t.inserir(&ficha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    let memo = bytes_com_extensao(&d, "memo");
    assert!(!memo.is_empty(), "o teste nao achou nenhum .memo");
    assert!(
        !contem(&memo, MEMO_SECRETO.as_bytes()),
        "o `.memo` de uma tabela com coluna EXTERNA marcada como sensivel \
         guardou o texto em claro, com o cofre ligado"
    );

    // O formato: versao 5 e cabecalho de 192 (o sal e a prova da chave estao
    // la), e o slot do MESMO tamanho da gemea em claro -- nao ha faixa inline
    // para selar, entao nao ha etiqueta. E o rabo zero do `Material::rabo`.
    assert_eq!(versao(&d, "fichas", "reg"), 5);
    assert_eq!(cab_len(&d, "fichas"), 192);
    assert_eq!(
        geometria(&d, "fichas").0,
        geometria(&gemea, "fichas").0,
        "o slot ganhou etiqueta sem ter faixa inline para selar"
    );

    // O ciclo inteiro: reabrir, ler tudo, alterar, reabrir, reler.
    {
        let mut t = Table::abrir(&d, "fichas").unwrap();
        assert!(t.cifrada());
        for i in 1..=10i64 {
            let l = t.ler(i as u64).unwrap().unwrap();
            assert_eq!(l[1], Value::Str(format!("ficha {i:04}")));
            assert_eq!(l[2], Value::Memo(format!("{MEMO_SECRETO} numero {i}")));
        }
        let mut alterada = ficha(3);
        alterada[2] = Value::Memo("outro segredo, alterado depois".into());
        t.atualizar(3, &alterada).unwrap();
        t.sincronizar().unwrap();
    }
    {
        let mut t = Table::abrir(&d, "fichas").unwrap();
        assert_eq!(
            t.ler(3).unwrap().unwrap()[2],
            Value::Memo("outro segredo, alterado depois".into())
        );
    }
    assert!(!contem(
        &bytes_com_extensao(&d, "memo"),
        b"outro segredo, alterado depois"
    ));
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// O comportamento VELHO ao redor do conserto do pedido 210: o que NAO mudou
// ---------------------------------------------------------------------------

/// Tabela SEM coluna marcada continua nascendo em claro com o cofre ligado:
/// versao 4, cabecalho de 128, e o mesmo slot da gemea criada sem cofre.
///
/// E o outro lado da condicao trocada: «ha dado pessoal declarado» tem de
/// continuar dizendo NAO quando nao ha marca nenhuma -- senao toda tabela
/// nasceria com material e 64 bytes de cabecalho para nao proteger nada.
#[test]
fn tabela_sem_coluna_marcada_continua_em_claro_com_o_cofre_ligado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("sem-marca");
    let gemea = dir("sem-marca-gemea");
    let sem_marca = |nome: &str| {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)).obrigatoria(),
                Column::new("obs", ColumnType::Memo),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    };
    {
        let mut t = Table::criar(&gemea, sem_marca("simples")).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
    }
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, sem_marca("simples")).unwrap();
        assert!(!t.cifrada(), "tabela sem marca nasceu com material");
        for i in 1..=5 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    assert_eq!(versao(&d, "simples", "reg"), 4);
    assert_eq!(cab_len(&d, "simples"), 128);
    assert_eq!(geometria(&d, "simples"), geometria(&gemea, "simples"));
    assert!(
        contem(&bytes_com_extensao(&d, "reg"), SEGREDO.as_bytes()),
        "sem marca o nome TEM de estar legivel no .reg"
    );
    assert!(
        contem(&bytes_com_extensao(&d, "memo"), MEMO_SECRETO.as_bytes()),
        "sem marca o memo TEM de estar legivel"
    );
    cofre::desligar();
}

/// Coluna INLINE marcada continua exatamente como era: versao 5, cabecalho de
/// 192, e o slot 16 bytes maior que o da gemea em claro -- a etiqueta. A
/// coluna `Memo` NAO marcada continua em claro, porque a escolha e por coluna.
#[test]
fn coluna_inline_marcada_continua_com_a_etiqueta_de_16_bytes() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("so-inline");
    let gemea = dir("so-inline-gemea");
    let so_inline = |nome: &str| {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40))
                    .obrigatoria()
                    .com_dado_pessoal(DadoPessoal::Pessoal),
                Column::new("obs", ColumnType::Memo),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    };
    {
        let mut t = Table::criar(&gemea, so_inline("clientes")).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
    }
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, so_inline("clientes")).unwrap();
        assert!(t.cifrada());
        for i in 1..=5 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    assert_eq!(versao(&d, "clientes", "reg"), 5);
    assert_eq!(cab_len(&d, "clientes"), 192);
    assert_eq!(
        geometria(&d, "clientes").0,
        geometria(&gemea, "clientes").0 + 16,
        "a etiqueta da linha e de 16 bytes, uma so para as faixas marcadas"
    );
    assert!(!contem(&bytes_com_extensao(&d, "reg"), SEGREDO.as_bytes()));
    assert!(
        contem(&bytes_com_extensao(&d, "memo"), MEMO_SECRETO.as_bytes()),
        "o memo NAO marcado tem de continuar em claro: a escolha e por coluna"
    );
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        assert_eq!(
            t.ler(5).unwrap().unwrap()[1],
            Value::Str(format!("{SEGREDO} 0005"))
        );
    }
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// O encontro: o que o conserto do 210 abriria se viesse sozinho
// ---------------------------------------------------------------------------

/// Numa tabela CIFRADA, marcar ou desmarcar uma coluna depois e recusado --
/// e o grau (pessoal/sensivel) pode mudar.
///
/// # O defeito que este teste impede, medido antes de escrever o conserto
///
/// O `remarcar_dado_pessoal` nao recalcula as faixas nem o `slot_size`. Numa
/// tabela cifrada com faixa inline isso nunca doeu, porque a conta do
/// `slot_size` (sem contar o rabo) recusava toda remarcacao por acidente. A
/// tabela so de externas tem rabo ZERO: a conta passava, `nome` ficava
/// marcada no esquema com as faixas velhas na memoria, as linhas seguintes
/// iam com `nome` em claro sob um esquema que diz «cifrado», e a reabertura
/// seguinte recusava com «slot_size nao bate com o esquema». Desmarcar a
/// unica marcada passava tambem, e a reabertura caia em «desmarcar nao
/// decifra».
#[test]
fn marcar_coluna_depois_numa_tabela_cifrada_e_recusado_e_o_grau_pode_mudar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("remarcar");
    cofre::definir(SENHA, RAPIDO).unwrap();

    // So de externas: rabo zero, o caso que passava pela conta antiga.
    {
        let mut t = Table::criar(&d, esquema_so_externas("fichas")).unwrap();
        for i in 1..=5 {
            t.inserir(&ficha(i)).unwrap();
        }
        let e = t
            .marcar_dado_pessoal(&[("nome".into(), DadoPessoal::Pessoal)])
            .unwrap_err();
        assert!(
            e.to_string().contains("reselar"),
            "a recusa tem de dizer o motivo, e nao falar de estrutura: {e}"
        );
        let e = t
            .marcar_dado_pessoal(&[("obs".into(), DadoPessoal::Nao)])
            .unwrap_err();
        assert!(e.to_string().contains("reselar"), "{e}");
        // O grau nao muda o que se sela: passa.
        t.marcar_dado_pessoal(&[("obs".into(), DadoPessoal::Pessoal)])
            .unwrap();
        t.sincronizar().unwrap();
    }
    {
        let mut t = Table::abrir(&d, "fichas").unwrap();
        assert_eq!(
            t.esquema()
                .coluna_por_nome("obs")
                .map(|i| t.esquema().colunas()[i].dado_pessoal),
            Some(DadoPessoal::Pessoal)
        );
        assert_eq!(
            t.esquema()
                .coluna_por_nome("nome")
                .map(|i| t.esquema().colunas()[i].dado_pessoal),
            Some(DadoPessoal::Nao),
            "a marca recusada nao pode ter ficado gravada"
        );
        for i in 1..=5i64 {
            assert_eq!(
                t.ler(i as u64).unwrap().unwrap()[2],
                Value::Memo(format!("{MEMO_SECRETO} numero {i}"))
            );
        }
    }

    // Com faixa inline (rabo de 16): a conta nova conta o rabo, entao mudar
    // o grau passa -- antes era recusado com a mensagem errada -- e marcar
    // uma coluna a mais continua recusado, agora pelo motivo certo.
    let d2 = dir("remarcar-inline");
    {
        let mut t = Table::criar(&d2, esquema("clientes")).unwrap();
        for i in 1..=5 {
            t.inserir(&linha(i)).unwrap();
        }
        t.marcar_dado_pessoal(&[("nome".into(), DadoPessoal::Sensivel)])
            .unwrap();
        let e = t
            .marcar_dado_pessoal(&[("id".into(), DadoPessoal::Pessoal)])
            .unwrap_err();
        assert!(e.to_string().contains("reselar"), "{e}");
        t.sincronizar().unwrap();
    }
    {
        let mut t = Table::abrir(&d2, "clientes").unwrap();
        assert_eq!(
            t.ler(2).unwrap().unwrap()[1],
            Value::Str(format!("{SEGREDO} 0002"))
        );
    }
    cofre::desligar();
}

/// Acrescentar uma coluna INLINE marcada a uma tabela cifrada so de externas
/// sela a coluna nova: o slot ganha a largura dela E a etiqueta, e nem o valor
/// padrao das linhas velhas nem o das novas aparece no `.reg`.
///
/// E o caminho de `acrescentar_coluna` com as faixas indo de vazias para uma
/// -- o unico jeito legitimo de uma tabela so de externas ganhar faixa inline.
#[test]
fn acrescentar_coluna_inline_marcada_a_tabela_cifrada_so_de_externas_sela_a_nova() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("add-inline");
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, esquema_so_externas("fichas")).unwrap();
        for i in 1..=5 {
            t.inserir(&ficha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let (slot_antes, _) = geometria(&d, "fichas");
    const PADRAO: &str = "98765432100";
    const NOVO: &str = "12345678909";
    {
        let mut t = Table::abrir(&d, "fichas").unwrap();
        let n = t
            .acrescentar_coluna(
                Column::new("cpf", ColumnType::Str(11)).com_dado_pessoal(DadoPessoal::Pessoal),
                Some(Value::Str(PADRAO.into())),
            )
            .unwrap();
        assert_eq!(n, 5);
        let mut nova = ficha(6);
        nova.push(Value::Str(NOVO.into()));
        t.inserir(&nova).unwrap();
        t.sincronizar().unwrap();
    }
    let (slot_depois, _) = geometria(&d, "fichas");
    assert_eq!(
        slot_depois,
        slot_antes + 11 + 16,
        "a faixa nova de 11 bytes e a etiqueta que passou a existir"
    );
    let reg = bytes_com_extensao(&d, "reg");
    assert!(
        !contem(&reg, PADRAO.as_bytes()),
        "o padrao das linhas velhas foi em claro"
    );
    assert!(
        !contem(&reg, NOVO.as_bytes()),
        "o valor da linha nova foi em claro"
    );
    {
        let mut t = Table::abrir(&d, "fichas").unwrap();
        let cpf = t.esquema().coluna_por_nome("cpf").unwrap();
        for i in 1..=5i64 {
            let l = t.ler(i as u64).unwrap().unwrap();
            assert_eq!(l[cpf], Value::Str(PADRAO.into()));
            assert_eq!(l[2], Value::Memo(format!("{MEMO_SECRETO} numero {i}")));
        }
        assert_eq!(t.ler(6).unwrap().unwrap()[cpf], Value::Str(NOVO.into()));
    }
    cofre::desligar();
}

/// **Nomeado, e decidido pelo comportamento velho:** coluna marcada
/// acrescentada a uma tabela que nasceu EM CLARO continua em claro -- externa
/// ou inline. «Ligar a cifra nao cifra o que ja existe» vale para a coluna
/// nova: o material e decidido na criacao, e virar um volume da versao 4 para
/// a 5 e uma operacao com nome, que nao existe. O `esquema` responde
/// `material` para isso nao ficar invisivel.
///
/// Se um dia essa decisao mudar, este e o teste que cai -- e cair aqui e o
/// aviso para reescrever o paragrafo do FORMATO.md §1.1 junto.
#[test]
fn acrescentar_coluna_marcada_a_tabela_em_claro_continua_em_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("add-em-claro");
    // A tabela de ontem: nasceu antes de o cofre ligar.
    {
        let e = Schema::new(
            "antiga",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = Table::criar(&d, e).unwrap();
        for i in 1..=3 {
            t.inserir(&[Value::Int(i), Value::Str(format!("cliente {i}"))])
                .unwrap();
        }
        t.sincronizar().unwrap();
    }
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::abrir(&d, "antiga").unwrap();
        assert!(!t.cifrada());
        t.acrescentar_coluna(
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
            None,
        )
        .unwrap();
        t.acrescentar_coluna(
            Column::new("apelido", ColumnType::Str(40)).com_dado_pessoal(DadoPessoal::Pessoal),
            Some(Value::Str("sem apelido".into())),
        )
        .unwrap();
        assert!(!t.cifrada(), "acrescentar coluna nao pode virar o material");
        t.inserir(&[
            Value::Int(4),
            Value::Str("cliente 4".into()),
            Value::Memo(MEMO_SECRETO.into()),
            Value::Str(SEGREDO.into()),
        ])
        .unwrap();
        t.sincronizar().unwrap();
    }
    assert_eq!(versao(&d, "antiga", "reg"), 4);
    assert_eq!(cab_len(&d, "antiga"), 128);
    assert!(
        contem(&bytes_com_extensao(&d, "memo"), MEMO_SECRETO.as_bytes()),
        "a decisao e que continua em claro; se isto caiu, a decisao mudou"
    );
    assert!(contem(&bytes_com_extensao(&d, "reg"), SEGREDO.as_bytes()));
    cofre::desligar();
}

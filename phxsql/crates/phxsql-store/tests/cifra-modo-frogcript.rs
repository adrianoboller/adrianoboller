//! O modo FrogCript da cifra de coluna, provado em disco.
//!
//! Arquivo separado do `cifra-dos-dados.rs` pela mesma razao que aquele e
//! separado da biblioteca: o MODO tambem e do processo, e um teste que o troca
//! no meio da corrida trocaria o modo de outro teste.
//!
//! O que se prova aqui e o que o modo entrega **e o que ele custa**. A parte
//! criptografica -- que ele nao acrescenta forca -- nao se prova em teste; esta
//! escrita no cabecalho de `phxsql_core::frogcript` e em `SEGURANCA.md` §10.4.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phxsql_core::frogcript::{self, Ajuste};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA: &str = "a chave do cofre de teste";
const SEGREDO: &str = "Fulano de Tal da Silva";

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-frog-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A coluna `nome` e `Str(40)` e vai marcada; e ela que define o custo.
fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("{SEGREDO} {i:04}"))]
}

fn bytes_do_reg(d: &Path) -> Vec<u8> {
    std::fs::read(d.join("clientes.reg")).unwrap()
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

fn slot_size(d: &Path) -> usize {
    let b = bytes_do_reg(d);
    u32::from_le_bytes([b[16], b[17], b[18], b[19]]) as usize
}

/// No modo FrogCript a tabela funciona igual -- e custa o que o documento diz.
///
/// O pacote nao cabe no lugar do texto claro, entao a faixa marcada do payload
/// vai a ZEROS e o pacote inteiro mora no rabo do slot. O teste confere as
/// duas coisas: que abre, e quanto custa.
#[test]
fn o_modo_frogcript_guarda_devolve_e_custa_o_que_esta_escrito() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());

    // Primeiro a mesma tabela em claro, para o custo sair de uma SUBTRACAO e
    // nao de uma conta escrita a mao.
    cofre::desligar();
    let claro = dir("claro");
    let em_claro = {
        let mut t = Table::criar(&claro, esquema()).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
        drop(t);
        slot_size(&claro)
    };
    let _ = std::fs::remove_dir_all(&claro);

    cofre::definir_com(SENHA, RAPIDO, cofre::Modo::FrogCript, Ajuste::default()).unwrap();
    let d = dir("frog");
    {
        let mut t = Table::criar(&d, esquema()).unwrap();
        for i in 1..=100 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        assert_eq!(
            t.ler(42).unwrap().unwrap()[1],
            Value::Str(format!("{SEGREDO} 0042"))
        );
        t.atualizar(42, &linha(4242)).unwrap();
        assert_eq!(
            t.ler(42).unwrap().unwrap()[1],
            Value::Str(format!("{SEGREDO} 4242"))
        );
        t.sincronizar().unwrap();
    }
    assert!(
        !contem(&bytes_do_reg(&d), SEGREDO.as_bytes()),
        "o texto claro apareceu no .reg no modo FrogCript"
    );

    assert_eq!(
        slot_size(&d) - em_claro,
        40 + frogcript::ACRESCIMO + 4 * phxsql_core::cifra::XNONCE_LEN,
        "o custo do FrogCript por linha nao e o que o documento diz"
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Trocar o modo no `config.json` nao muda como um arquivo JA GRAVADO se le.
///
/// # O defeito que este teste repoe
///
/// Se o modo saisse da configuracao em vez do arquivo, ligar `frogcript` numa
/// terca faria toda tabela cifrada como AEAD parar de abrir -- com a mensagem
/// errada, mandando procurar corrupcao onde so ha um interruptor trocado.
/// Tirando a `FLAG_FROGCRIPT` da leitura do material em `cofre.rs`, este teste
/// cai.
#[test]
fn o_modo_sai_do_arquivo_e_nao_da_configuracao() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("modo-do-arquivo");

    cofre::definir(SENHA, RAPIDO).unwrap(); // nasce AEAD
    {
        let mut t = Table::criar(&d, esquema()).unwrap();
        t.inserir(&linha(7)).unwrap();
        t.sincronizar().unwrap();
    }
    let como_nasceu = slot_size(&d);

    // Agora o processo passa a preferir FrogCript. A tabela de antes continua
    // sendo AEAD, e continua abrindo.
    cofre::definir_com(SENHA, RAPIDO, cofre::Modo::FrogCript, Ajuste::default()).unwrap();
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.ler(1).unwrap().unwrap()[1],
        Value::Str(format!("{SEGREDO} 0007"))
    );
    assert_eq!(
        slot_size(&d),
        como_nasceu,
        "o slot mudou de tamanho sozinho"
    );

    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Salto e separador personalizados sao parte do segredo: sem eles, nao abre.
///
/// E a secao 10 do documento do autor, provada. O que ela promete e que os
/// dois lados precisam concordar -- e o que este teste mostra e o preco disso:
/// quem perde o salto perde o dado, do mesmo jeito que quem perde a senha.
#[test]
fn salto_e_separador_personalizados_viram_parte_do_segredo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("a-gosto");

    let meu = Ajuste::novo(7, b'#').unwrap();
    cofre::definir_com(SENHA, RAPIDO, cofre::Modo::FrogCript, meu).unwrap();
    {
        let mut t = Table::criar(&d, esquema()).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
    }

    // Com o mesmo ajuste, abre.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.ler(1).unwrap().unwrap()[1],
        Value::Str(format!("{SEGREDO} 0001"))
    );
    drop(t);

    // Com o ajuste de fabrica, nao abre -- e o erro nomeia o separador.
    cofre::definir_com(SENHA, RAPIDO, cofre::Modo::FrogCript, Ajuste::default()).unwrap();
    let mut t = Table::abrir(&d, "clientes").unwrap();
    let e = match t.ler(1) {
        Ok(_) => panic!("abriu com o separador errado"),
        Err(e) => e.to_string(),
    };
    assert!(
        e.contains("FrogCript") && e.contains("separador"),
        "o erro precisa dizer o que conferir: {e}"
    );

    drop(t);
    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

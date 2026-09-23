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

mod comum;
use std::sync::Mutex;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
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

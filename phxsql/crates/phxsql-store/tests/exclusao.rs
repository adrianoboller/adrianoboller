//! Exclusao suave, exclusao fisica com lixeira, e o motivo de cada uma.
//!
//! O que estes testes protegem, em ordem de importancia:
//!
//! 1. a linha esta no `.trash` **antes** de sair do `.reg`;
//! 2. ela volta de la INTEIRA, com os anexos;
//! 3. marcar como excluida tira a linha da varredura comum, e so dela;
//! 4. um `atualizar` de rotina nao ressuscita linha marcada.

#[allow(
    dead_code,
    reason = "o modulo comum serve a varios testes; este usa so o DirTemp"
)]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema, COLUNA_SOFTDELETED};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::motivo::Tipo;
use phxsql_store::table::{Table, Visao};

const ID: usize = 0;
const NOME: usize = 1;
const FOTO: usize = 2;
const FICHA: usize = 3;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("foto", ColumnType::Bin),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(ID)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// Uma linha com anexos, que e o caso que separa lixeira certa de errada.
fn cliente(id: i64, nome: &str) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(nome.into()),
        Value::Bin(vec![id as u8; 300]),
        Value::Memo(format!("ficha de {nome}")),
        Value::Bool(false),
        // Zero = "numere para mim". O motor troca pelo proximo da tabela.
        Value::UInt(0),
    ]
}

fn com_dados(dir: &DirTemp) -> Table {
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    for (i, nome) in ["Adriano", "Maria", "Joao"].iter().enumerate() {
        t.inserir(&cliente(i as i64 + 1, nome)).unwrap();
    }
    t
}

#[test]
fn toda_tabela_nova_tem_a_coluna_de_sistema() {
    let dir = DirTemp::novo("coluna");
    let t = Table::criar(&dir.0, esquema()).unwrap();
    let e = t.esquema();
    let i = e.coluna_softdeleted().expect("sem a coluna de sistema");
    assert_eq!(e.colunas()[i].nome, COLUNA_SOFTDELETED);
    assert_eq!(e.colunas()[i].ty, ColumnType::Bool);
    assert!(!e.colunas()[i].nullable);
}

/// Inserir sem a coluna de sistema tem de funcionar: quem monta a linha
/// declarou quatro colunas e nao sabe da quinta.
#[test]
fn inserir_sem_a_coluna_de_sistema() {
    let dir = DirTemp::novo("aridade");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    let curta = vec![
        Value::Int(1),
        Value::Str("Sem a quinta".into()),
        Value::Null,
        Value::Null,
    ];
    let rowid = t.inserir(&curta).unwrap();
    let linha = t.ler(rowid).unwrap().unwrap();
    // As DUAS colunas de sistema entraram sozinhas: a marca falsa e o numero
    // de ordem, que o motor preencheu.
    assert_eq!(linha.len(), 6);
    assert_eq!(linha[4], Value::Bool(false));
    assert_eq!(linha[5], Value::UInt(1));
    assert!(!t.esta_excluida(&linha));
}

#[test]
fn suave_some_da_varredura_mas_continua_no_reg() {
    let dir = DirTemp::novo("suave");
    let mut t = com_dados(&dir);

    assert!(t.excluir_suave(2, "pedido do titular").unwrap());

    // Sumiu de quem le normalmente...
    let ativas = t.varrer().unwrap();
    assert_eq!(ativas.len(), 2);
    assert!(!ativas.iter().any(|(r, _)| *r == 2));

    // ... mas o registro esta la, e o slot tambem.
    assert_eq!(t.registros(), 3, "a exclusao suave nao apaga registro");
    assert_eq!(t.varrer_com(Visao::Todas).unwrap().len(), 3);
    let so_excluidas = t.varrer_com(Visao::Excluidas).unwrap();
    assert_eq!(so_excluidas.len(), 1);
    assert_eq!(so_excluidas[0].0, 2);

    // E o conteudo continua inteiro, anexos incluidos.
    let linha = t.ler(2).unwrap().unwrap();
    assert_eq!(linha[NOME], Value::Str("Maria".into()));
    assert_eq!(linha[FICHA], Value::Memo("ficha de Maria".into()));
    assert!(t.esta_excluida(&linha));
}

#[test]
fn restaurar_traz_de_volta() {
    let dir = DirTemp::novo("restaurar");
    let mut t = com_dados(&dir);
    t.excluir_suave(2, "engano").unwrap();
    assert_eq!(t.varrer().unwrap().len(), 2);

    assert!(t.restaurar(2, "era engano mesmo").unwrap());
    assert_eq!(t.varrer().unwrap().len(), 3);
    let viva = t.ler(2).unwrap().unwrap();
    assert!(!t.esta_excluida(&viva));

    // Restaurar de novo nao e erro, mas tambem nao gera segundo registro.
    assert!(!t.restaurar(2, "").unwrap());
}

/// Marcar duas vezes nao duplica o motivo.
#[test]
fn marcar_duas_vezes_registra_uma() {
    let dir = DirTemp::novo("duas-vezes");
    let mut t = com_dados(&dir);
    assert!(t.excluir_suave(1, "primeira").unwrap());
    assert!(!t.excluir_suave(1, "segunda").unwrap());
    let motivos = t.motivos(0, 0).unwrap();
    assert_eq!(motivos.len(), 1);
    assert_eq!(motivos[0].motivo, "primeira");
}

/// O teste central: a linha esta na lixeira, e volta de la inteira.
#[test]
fn fisica_guarda_a_linha_inteira_antes_de_apagar() {
    let dir = DirTemp::novo("fisica");
    let mut t = com_dados(&dir);
    let antes = t.ler(2).unwrap().unwrap();

    assert!(t.excluir_de_vez(2, "duplicidade").unwrap());

    // Saiu do `.reg` de verdade.
    assert!(t.ler(2).unwrap().is_none());
    assert_eq!(t.registros(), 2);
    assert_eq!(t.varrer_com(Visao::Todas).unwrap().len(), 2);

    // E esta na lixeira, com anexo e tudo.
    let lixo = t.lixeira(0, 0, true).unwrap();
    assert_eq!(lixo.len(), 1);
    assert_eq!(lixo[0].rowid, 2);
    let volta = t.linha_da_lixeira(&lixo[0]).unwrap();
    assert_eq!(volta[ID], antes[ID]);
    assert_eq!(volta[NOME], antes[NOME]);
    assert_eq!(volta[FOTO], antes[FOTO], "a foto nao voltou");
    assert_eq!(volta[FICHA], antes[FICHA], "a ficha nao voltou");
}

/// A razao de o `.trash` guardar o CONTEUDO dos anexos e nao os ponteiros:
/// os blocos do `.bin` sao liberados na exclusao e podem ser reaproveitados.
/// Depois de vinte insercoes por cima, a foto da linha excluida tem de
/// continuar sendo a dela.
#[test]
fn o_anexo_sobrevive_ao_bloco_ser_reaproveitado() {
    let dir = DirTemp::novo("reaproveita");
    let mut t = com_dados(&dir);
    let foto_original = t.ler(1).unwrap().unwrap()[FOTO].clone();

    t.excluir_de_vez(1, "").unwrap();
    for i in 10..30 {
        t.inserir(&cliente(i, &format!("Depois {i}"))).unwrap();
    }

    let lixo = t.lixeira(0, 0, true).unwrap();
    let volta = t.linha_da_lixeira(&lixo[0]).unwrap();
    assert_eq!(volta[FOTO], foto_original, "a foto virou a de outra linha");
}

/// Sincronizar a lixeira antes de liberar o slot e a garantia inteira desta
/// funcionalidade. Este teste prova que ela nao depende de um `sincronizar`
/// posterior: mata o processo simulado (fecha sem sincronizar a tabela) e
/// reabre.
#[test]
fn a_lixeira_esta_no_disco_antes_de_o_slot_sair() {
    let dir = DirTemp::novo("ordem");
    {
        let mut t = com_dados(&dir);
        t.sincronizar().unwrap();
        t.excluir_de_vez(2, "sem sincronizar depois").unwrap();
        // De proposito: NAO chama `t.sincronizar()`. O que a lixeira promete
        // e que ela ja esta no disco quando `excluir_de_vez` retorna.
    }
    let mut t = Table::abrir(&dir.0, "clientes").unwrap();
    let lixo = t.lixeira(0, 0, true).unwrap();
    assert_eq!(lixo.len(), 1, "a linha nao chegou ao disco");
    assert_eq!(lixo[0].rowid, 2);
    assert_eq!(
        t.linha_da_lixeira(&lixo[0]).unwrap()[NOME],
        Value::Str("Maria".into())
    );
}

/// **O teste do comportamento VELHO, e e ele o que mais importa aqui.**
///
/// A janela de durabilidade da exclusao entrou nesta rodada
/// (`recursos.exclusao_na_janela`), e ela entra PEDIDA. Quem nao pede continua
/// com a garantia de sempre: um `excluir` que retorna ja esta no disco.
///
/// O teste vizinho (`a_lixeira_esta_no_disco_antes_de_o_slot_sair`) reabre a
/// tabela e acha a linha -- mas isso passaria com o `fsync` ligado OU
/// desligado, porque o `write` ja esta no sistema operacional nos dois casos e
/// quem reabre le a mesma pagina. Ele prova a ORDEM, e nao a espera. A espera
/// so aparece numa queda de energia, que teste nenhum provoca, entao o que se
/// confere e o unico rastro que ela deixa: a chamada.
///
/// Reponha o defeito tirando o `if !na_janela()` de `LixeiraFile::guardar`, ou
/// virando o padrao do global para `true`, e este teste cai.
#[test]
fn sem_pedir_a_janela_cada_exclusao_espera_o_disco() {
    assert!(
        !phxsql_store::lixeira::na_janela(),
        "a exclusao nasceu na janela: o padrao afrouxou sozinho"
    );
    let dir = DirTemp::novo("espera-o-disco");
    let mut t = com_dados(&dir);
    t.sincronizar().unwrap();

    let antes = t.lixeira_sincronizacoes();
    t.excluir_de_vez(1, "uma").unwrap();
    assert_eq!(
        t.lixeira_sincronizacoes(),
        antes + 1,
        "a exclusao voltou sem esperar o .trash chegar ao disco"
    );
    t.excluir_de_vez(2, "duas").unwrap();
    t.excluir_de_vez(3, "tres").unwrap();
    assert_eq!(
        t.lixeira_sincronizacoes(),
        antes + 3,
        "uma exclusao por fsync: e essa a garantia de quem nao pediu nada"
    );

    // Excluir o que nao existe nao promete nada, entao nao paga nada.
    let de_novo = t.lixeira_sincronizacoes();
    assert!(!t.excluir_de_vez(1, "ja saiu").unwrap());
    assert_eq!(t.lixeira_sincronizacoes(), de_novo);
}

/// O `.trash` fecha ANTES do `.reg`, e a ordem esta no codigo, nao no
/// comentario.
///
/// Enquanto o `fsync` acontecia por exclusao a ordem daqui era indiferente --
/// o `.trash` ja estava no disco muito antes. Com a exclusao na janela os dois
/// passam a fechar aqui, e sincronizar o `.reg` primeiro seria escolher a
/// unica ordem em que uma queda no meio do fechamento deixa a linha liberada
/// sem a copia. Reponha o defeito trocando a ordem em `Table::sincronizar`.
#[test]
fn o_trash_fecha_antes_do_reg() {
    let dir = DirTemp::novo("ordem-do-fsync");
    let mut t = com_dados(&dir);
    t.sincronizar().unwrap();
    let (trash, reg) = t.selos_de_sincronizacao();
    assert!(trash > 0 && reg > 0, "algum dos dois nao sincronizou");
    assert!(
        trash < reg,
        "o .reg fechou antes do .trash ({trash} contra {reg}): \
         a copia de recuperacao tem de ir ao disco antes da liberacao"
    );
}

#[test]
fn o_motivo_registra_quem_quando_e_por_que() {
    let dir = DirTemp::novo("motivo");
    let mut t = com_dados(&dir);
    t.definir_usuario(42);

    t.excluir_suave(1, "pedido de remocao").unwrap();
    t.excluir_de_vez(3, "duplicidade com o contrato 9").unwrap();
    t.restaurar(1, "revertido a pedido").unwrap();

    let m = t.motivos(0, 0).unwrap();
    assert_eq!(m.len(), 3);

    assert_eq!(m[0].tipo, Tipo::Suave);
    assert_eq!(m[0].rowid, 1);
    assert_eq!(m[0].motivo, "pedido de remocao");
    assert_eq!(m[0].usuario, 42);
    // A identidade sai da chave primaria: e o que sobrevive a linha.
    assert_eq!(m[0].identidade, "id=1");
    assert!(m[0].instante_iso().starts_with("20"));

    assert_eq!(m[1].tipo, Tipo::Fisica);
    assert_eq!(m[1].identidade, "id=3");
    assert_eq!(m[2].tipo, Tipo::Restauracao);

    // O uuid do evento e v7: ordenar por ele e ordenar por tempo.
    assert!(m[0].uuid.bytes() < m[1].uuid.bytes());
    assert_eq!(t.motivos_de(1).unwrap().len(), 2);
}

#[test]
fn motivo_obrigatorio_recusa_exclusao_calada() {
    let dir = DirTemp::novo("obrigatorio");
    let mut t = Table::criar(&dir.0, esquema().com_motivo_obrigatorio(true)).unwrap();
    t.inserir(&cliente(1, "Adriano")).unwrap();

    let e = t.excluir_suave(1, "   ").unwrap_err();
    assert!(format!("{e}").contains("motivo"), "{e}");
    assert!(t.excluir_de_vez(1, "").is_err());
    // Continua viva: a recusa aconteceu antes de qualquer gravacao.
    assert_eq!(t.varrer().unwrap().len(), 1);

    // Com motivo, passa.
    assert!(t.excluir_suave(1, "agora sim").unwrap());
}

/// A marca sobrevive a um `atualizar` que nao fala dela. Se nao sobrevivesse,
/// qualquer edicao de rotina ressuscitaria a linha em silencio.
#[test]
fn atualizar_sem_a_coluna_nao_ressuscita() {
    let dir = DirTemp::novo("ressuscita");
    let mut t = com_dados(&dir);
    t.excluir_suave(2, "excluida").unwrap();

    let curta = vec![
        Value::Int(2),
        Value::Str("Maria com outro nome".into()),
        Value::Null,
        Value::Null,
    ];
    t.atualizar(2, &curta).unwrap();

    let linha = t.ler(2).unwrap().unwrap();
    assert_eq!(linha[NOME], Value::Str("Maria com outro nome".into()));
    assert!(t.esta_excluida(&linha), "a alteracao ressuscitou a linha");
    assert_eq!(t.varrer().unwrap().len(), 2);
}

#[test]
fn esvaziar_registra_o_expurgo_antes_de_apagar() {
    let dir = DirTemp::novo("expurgo");
    let mut t = com_dados(&dir);
    t.excluir_de_vez(1, "a").unwrap();
    t.excluir_de_vez(2, "b").unwrap();
    assert_eq!(t.lixeira_tamanho().unwrap().0, 2);

    assert_eq!(t.esvaziar_lixeira("limpeza anual").unwrap(), 2);
    assert_eq!(t.lixeira_tamanho().unwrap().0, 0);

    // O dado foi; o registro de que foi, nao.
    let m = t.motivos(0, 0).unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(m[2].tipo, Tipo::Expurgo);
    assert_eq!(m[2].motivo, "limpeza anual");
}

/// A busca por indice devolve rowid, e a marca esta no registro: filtrar
/// exige ler. O que nao pode e a linha marcada aparecer numa lista comum.
#[test]
fn filtrar_ativos_tira_os_marcados() {
    let dir = DirTemp::novo("filtrar");
    let mut t = com_dados(&dir);
    t.excluir_suave(2, "").unwrap();

    let todos = t.varrer_indice("porId").unwrap();
    assert_eq!(todos.len(), 3);
    let vivos = t.filtrar_ativos(&todos).unwrap();
    assert_eq!(vivos, vec![1, 3]);
}

#[test]
fn verificar_confere_os_dois_arquivos_novos() {
    let dir = DirTemp::novo("verificar");
    let mut t = com_dados(&dir);
    t.excluir_de_vez(1, "um").unwrap();
    t.excluir_suave(2, "dois").unwrap();
    t.sincronizar().unwrap();

    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, 2, "a suave continua contando como registro");
    assert_eq!(rel.descartadas, 1);
    assert_eq!(rel.motivos, 2);
}

/// Excluir de vez uma linha ja marcada: a lixeira guarda a linha COM a marca,
/// que e como ela estava.
#[test]
fn suave_e_depois_fisica() {
    let dir = DirTemp::novo("suave-fisica");
    let mut t = com_dados(&dir);
    t.excluir_suave(2, "primeiro marca").unwrap();
    t.excluir_de_vez(2, "depois some").unwrap();

    assert!(t.ler(2).unwrap().is_none());
    let lixo = t.lixeira(0, 0, true).unwrap();
    let volta = t.linha_da_lixeira(&lixo[0]).unwrap();
    assert!(t.esta_excluida(&volta), "a marca nao veio junto");
    assert_eq!(t.motivos_de(2).unwrap().len(), 2);
}

/// A visao por indice: a ordem vem do `.ndx`, e o filtro le cada registro.
#[test]
fn filtrar_por_visao_pelo_indice() {
    let dir = DirTemp::novo("visao-indice");
    let mut t = com_dados(&dir);
    t.excluir_suave(2, "").unwrap();

    let todos = t.varrer_indice("porId").unwrap();
    assert_eq!(t.filtrar(&todos, Visao::Todas).unwrap(), vec![1, 2, 3]);
    assert_eq!(t.filtrar(&todos, Visao::Ativas).unwrap(), vec![1, 3]);
    assert_eq!(t.filtrar(&todos, Visao::Excluidas).unwrap(), vec![2]);
}

/// Numa tabela sem a coluna de sistema, `Excluidas` volta VAZIA e nao a lista
/// inteira: nao ha nenhuma linha marcada porque nao ha onde marcar.
#[test]
fn sem_a_coluna_a_visao_de_excluidas_e_vazia() {
    use phxsql_core::schema::Column;
    let dir = DirTemp::novo("sem-coluna");
    let antiga = Schema::do_disco(
        "velha",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![],
    )
    .unwrap();
    let mut t = Table::criar(&dir.0, antiga).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    t.inserir(&[Value::Int(2)]).unwrap();

    let todos = vec![1, 2];
    assert_eq!(t.filtrar(&todos, Visao::Ativas).unwrap(), vec![1, 2]);
    assert!(t.filtrar(&todos, Visao::Excluidas).unwrap().is_empty());
    assert_eq!(t.varrer().unwrap().len(), 2);

    // E a exclusao suave nela recusa com a explicacao, em vez de fingir.
    let e = t.excluir_suave(1, "").unwrap_err();
    assert!(format!("{e}").contains("recrie"), "{e}");

    // Mas a fisica funciona, e passa pela lixeira como qualquer outra.
    assert!(t.excluir_de_vez(1, "some").unwrap());
    assert_eq!(t.lixeira(0, 0, true).unwrap().len(), 1);
}

//! O indice de texto pela TABELA: declarar, gravar, achar, alterar e excluir.
//!
//! Estes testes provam o que o `docs/FTS.md` §7 exige, e o que mais importa
//! neles nao e "achar": e **nunca achar a mais**. Achar a menos e atraso e o
//! indice o declara; achar a mais e mentira, e nao ha como o cliente perceber.

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("titulo", ColumnType::Str(80)),
            Column::new("corpo", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![
        IndiceDeTexto::new("porTitulo", 1),
        IndiceDeTexto::new("porCorpo", 2),
    ])
    .expect("indices de texto")
}

fn nova(nome: &str) -> (Table, comum::DirTemp) {
    let dir = comum::DirTemp::novo(&format!("fts-tab-{nome}"));
    (Table::criar(&dir, esquema()).expect("criar"), dir)
}

fn linha(i: i64, titulo: &str, corpo: &str) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(titulo.into()),
        Value::Memo(corpo.into()),
    ]
}

/// Achar pelo `Str` inline e pelo `Memo` externo -- e o `Memo` e o caso que a
/// lacuna do HFSQL(R) nomeia: "procurar uma palavra dentro de um `.memo`".
#[test]
fn acha_no_titulo_e_no_corpo() {
    let (mut t, _d) = nova("basico");
    let a = t
        .inserir(&linha(1, "pedido urgente", "o cliente fenix pediu"))
        .unwrap();
    let b = t
        .inserir(&linha(2, "nota fiscal", "entrega comum"))
        .unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "nota").unwrap().rowids,
        vec![b]
    );
    // O termo do corpo NAO pode aparecer no indice do titulo.
    assert!(t
        .procurar_texto("porTitulo", "fenix")
        .unwrap()
        .rowids
        .is_empty());
}

/// A dobra de acento, que e a razao de o indice existir em portugues.
#[test]
fn acha_sem_acento_o_que_foi_gravado_com() {
    let (mut t, _d) = nova("dobra");
    let a = t
        .inserir(&linha(1, "a Fênix", "renasceu em Blumenau"))
        .unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "FENIX").unwrap().rowids,
        vec![a]
    );
}

/// **O teste que mais importa.** Alterar tem de tirar as palavras VELHAS.
///
/// Sem o `desindexar_texto` no `atualizar`, o indice fica apontando a palavra
/// que a linha nao tem mais -- e a busca por ela devolve uma linha que nao
/// casa. Achar a mais e mentira, e o cliente nao tem como perceber.
#[test]
fn alterar_tira_a_palavra_velha_do_indice() {
    let (mut t, _d) = nova("alterar");
    let a = t
        .inserir(&linha(1, "pedido antigo", "corpo original"))
        .unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "antigo").unwrap().rowids,
        vec![a]
    );

    t.atualizar(a, &linha(1, "pedido novo", "corpo trocado"))
        .unwrap();

    assert!(
        t.procurar_texto("porTitulo", "antigo")
            .unwrap()
            .rowids
            .is_empty(),
        "a palavra velha ficou no indice: ele passou a achar a MAIS"
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "novo").unwrap().rowids,
        vec![a]
    );
    assert!(
        t.procurar_texto("porCorpo", "original")
            .unwrap()
            .rowids
            .is_empty(),
        "a palavra velha do MEMO ficou no indice -- e o memo e o caso em que \
         o texto antigo so existe no `.memo`, que o `atualizar` decodifica \
         SEM externos"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "trocado").unwrap().rowids,
        vec![a]
    );
}

/// Excluir de vez tem de tirar a linha do indice, e o memo tambem.
#[test]
fn excluir_de_vez_tira_do_indice() {
    let (mut t, _d) = nova("excluir");
    let a = t
        .inserir(&linha(1, "pedido fenix", "corpo com raridade"))
        .unwrap();
    let b = t.inserir(&linha(2, "pedido comum", "outro corpo")).unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a, b]
    );

    t.excluir_de_vez(a, "teste").unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![b]
    );
    assert!(t
        .procurar_texto("porTitulo", "fenix")
        .unwrap()
        .rowids
        .is_empty());
    assert!(
        t.procurar_texto("porCorpo", "raridade")
            .unwrap()
            .rowids
            .is_empty(),
        "o termo do memo sobreviveu a exclusao"
    );
}

/// O indice acha EXATAMENTE o que a varredura acha -- conjuntos, nao contagens.
///
/// E a prova da §7.1 do `FTS.md`: indice que acha a mais e pior que indice que
/// acha a menos, e comparar contagem esconderia uma troca.
#[test]
fn o_indice_acha_o_mesmo_que_a_varredura() {
    let (mut t, _d) = nova("igual");
    let mut esperados = Vec::new();
    for i in 0..60i64 {
        let titulo = if i % 7 == 0 {
            format!("pedido fenix {i}")
        } else {
            format!("pedido comum {i}")
        };
        let r = t.inserir(&linha(i, &titulo, "corpo")).unwrap();
        if i % 7 == 0 {
            esperados.push(r);
        }
    }
    let pelo_indice = t.procurar_texto("porTitulo", "fenix").unwrap().rowids;
    assert_eq!(pelo_indice, esperados, "o indice divergiu da verdade");
    assert!(!esperados.is_empty(), "o teste tem de achar alguma coisa");
}

/// Tabela SEM indice de texto nao ganha o arquivo, e a busca recusa dizendo.
#[test]
fn tabela_sem_indice_de_texto_nao_paga_nada() {
    let dir = comum::DirTemp::novo("fts-sem");
    let e = Schema::new(
        "simples",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(&dir, e).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    assert!(
        !dir.join("simples.fts").exists(),
        "nasceu um `.fts` numa tabela que nao o declarou"
    );
    assert!(t.procurar_texto("qualquer", "x").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// O `.fts` e DERIVADO: apaga-lo custa tempo, nunca dado.
///
/// Reabrir a tabela sem o arquivo tem de reconstrui-lo -- porque a alternativa
/// e uma busca que devolve VAZIO em silencio, e resposta vazia errada e pior
/// que espera.
#[test]
fn apagar_o_fts_e_reabrir_reconstroi_em_vez_de_achar_nada() {
    let (mut t, dir) = nova("derivado");
    let a = t.inserir(&linha(1, "pedido fenix", "corpo raro")).unwrap();
    t.sincronizar().unwrap();
    drop(t);

    std::fs::remove_file(dir.join("docs.fts")).expect("apagar o .fts");

    let mut t = Table::abrir(&dir, "docs").unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a],
        "o `.fts` nao foi reconstruido, e a busca devolveu vazio em silencio"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "raro").unwrap().rowids,
        vec![a]
    );
}

// ------------------------------------------- a coluna nova e o .fts orfao
//
// `ALTER TABLE ADD COLUMN` remonta o esquema, e a remontagem nao carregava a
// lista dos indices de TEXTO: a declaracao sumia do `.reg`, o `.fts` do disco
// virava orfao, e `procurar_texto` passava a RECUSAR por nome inexistente.
// Nem a busca nem o `reconstruir_fts` diziam «o indice foi apagado»: um
// recusava por nome, o outro devolvia `Ok(0)` -- e `Ok(0)` anuncia sucesso.

/// A ponta a ponta: declara, grava, acrescenta coluna, REABRE e procura.
///
/// Reponha o defeito tirando o `.com_indices_de_texto(textos)?` de
/// `Schema::com_coluna`: o `procurar_texto` daqui devolve
/// `Err("a tabela docs nao tem indice de texto chamado porTitulo")` e o
/// `reconstruir_fts` devolve `Ok(0)`.
#[test]
fn depois_da_coluna_nova_a_busca_por_texto_continua_achando() {
    let (mut t, dir) = nova("coluna-nova");
    let a = t.inserir(&linha(1, "pedido fenix", "corpo raro")).unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a],
        "o controle: sem achar ANTES, a prova de baixo nao prova nada"
    );

    t.acrescentar_coluna(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    t.sincronizar().unwrap();
    drop(t);

    let mut t = Table::abrir(&dir, "docs").unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix")
            .expect("a coluna nova apagou a declaracao do indice de texto")
            .rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "raro").unwrap().rowids,
        vec![a]
    );

    // A linha gravada DEPOIS da alteracao tambem entra no indice, e ela tem
    // uma coluna a mais.
    let b = t
        .inserir(&[
            Value::Int(2),
            Value::Str("nota rara".into()),
            Value::Memo("entrega urgente".into()),
            Value::Null,
        ])
        .unwrap();
    assert_eq!(
        t.procurar_texto("porCorpo", "urgente").unwrap().rowids,
        vec![b]
    );

    // E o `.fts` nao esta orfao: o `reconstruir_fts` VE o indice e refaz as
    // duas linhas. Com o defeito no lugar ele devolvia `Ok(0)` -- pelo portao
    // `self.fts.is_none()` --, que e o pior jeito de perder um indice:
    // anunciando sucesso.
    assert_eq!(
        t.reconstruir_fts().unwrap(),
        2,
        "o `.fts` ficou orfao: o esquema nao declara indice de texto nenhum"
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a]
    );
}

// ------------------------------------------------------- a queda e o irmao
//
// O `.fts` e um `.ndx` por dentro, e por isso ele herda a marca de «ficou
// para tras numa queda». Herda a marca e nao herdava o conserto: quem
// reconstroi o `.ndx` e o `reindexar`, e ele nao olhava o `.fts`. E o
// caminho IRMAO da petrea, com o agravante de a mensagem do proprio `.fts`
// mandar «reconstrua o indice de texto com `reindexar`» -- uma ordem que o
// codigo nao sabia cumprir.

/// Simula a queda: esquece a tabela sem fechar, como um `SIGKILL` faz.
///
/// `mem::forget` e a simulacao honesta -- o `Drop` nao roda, e o cabecalho no
/// disco fica com a marca de sujo levantada, que e exatamente o que a queda
/// do processo deixa.
fn derruba(t: Table) {
    std::mem::forget(t);
}

/// **A prova.** Depois da queda, a tabela reabre com o `.fts` marcado, e
/// **nenhuma gravacao passa** ate alguem reconstruir.
///
/// Com o defeito reposto (o `reindexar` sem o `.fts`) o `inserir` de baixo
/// falha: a marca continua la depois da reconstrucao.
#[test]
fn a_queda_marca_o_fts_e_o_reindexar_tem_de_baixar_a_marca() {
    let (mut t, d) = nova("queda");
    t.inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();
    derruba(t);

    let mut t = Table::abrir(&d, "docs").expect("a tabela tem de reabrir");
    assert!(
        t.indice_precisa_reconstruir(),
        "a marca do .fts tem de ser VISTA por quem pergunta -- \
         perguntar so ao .ndx e nao ver a queda"
    );

    let indices = t.reindexar().expect("reindexar");
    assert!(!indices.is_empty());
    assert!(
        !t.indice_precisa_reconstruir(),
        "o reindexar tem de baixar a marca dos DOIS arquivos"
    );

    // E o que mais importa: a tabela volta a gravar e a busca volta a achar.
    let b = t
        .inserir(&linha(2, "nota fiscal", "corpo comum"))
        .expect("a tabela tem de voltar a gravar depois do reindexar");
    assert_eq!(
        t.procurar_texto("porTitulo", "nota").unwrap().rowids,
        vec![b]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids.len(),
        1,
        "a linha de antes da queda tem de continuar no indice"
    );
}

/// Reconstruir NAO pode somar: chamar duas vezes tem de dar o mesmo resultado.
///
/// **Eu previ o defeito errado, e o teste mediu.** Achei que a segunda passada
/// acrescentaria a mesma chave e a busca devolveria o rowid duas vezes -- achar
/// a mais, que e mentira. O que acontece e melhor e pior ao mesmo tempo: a
/// arvore RECUSA (`chave completa ja existe no indice`), entao nao ha mentira,
/// mas o `reconstruir_fts` deixa de ser idempotente -- e era o `reindexar` que
/// ia bater nessa recusa em toda tabela que ja tivesse uma linha.
#[test]
fn reconstruir_duas_vezes_nao_duplica() {
    let (mut t, _d) = nova("duas-vezes");
    let a = t
        .inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();

    t.reconstruir_fts().unwrap();
    t.reconstruir_fts().unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a],
        "reconstruir duas vezes duplicou a ocorrencia"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}

/// `.fts` que nao abre nao pode derrubar a tabela -- ele e DERIVADO.
///
/// Dois casos caem aqui: arquivo corrompido, e contagem de indices divergente
/// (alguem declarou um indice de texto numa tabela que ja tem dados, que e uma
/// das tres coisas para as quais o `reindexar` existe). Com o defeito reposto
/// a tabela nem abria, e a mensagem mandava rodar o `reindexar` -- numa tabela
/// que nao abre. Refazer custa uma varredura e nunca custa dado.
#[test]
fn fts_corrompido_reconstroi_na_abertura_em_vez_de_derrubar_a_tabela() {
    let (mut t, d) = nova("corrompido");
    let a = t
        .inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();
    drop(t);

    // Corrompe o `.fts` de proposito: nem magica, nem cabecalho.
    std::fs::write(d.join("docs.fts"), b"isto nao e um indice").unwrap();

    let mut t = Table::abrir(&d, "docs").expect("a tabela tem de abrir mesmo assim");
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a],
        "o indice tem de nascer cheio, varrendo a tabela"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}

/// **A pista de LEITURA não pode escrever, nem para fazer nascer o `.fts`.**
///
/// Sob a ficha compartilhada há N leitores nos mesmos arquivos, e a única
/// coisa que torna isso seguro é nenhum deles escrever. Os outros quatro
/// componentes que escrevem na abertura já recusavam com o motivo; o `.fts`
/// nasceu sem essa conferência — e refazê-lo escreve.
///
/// Com o defeito reposto este teste acha `Aberta` no lugar de
/// `PrecisaEscrever`, e a mensagem diz o que isso significa.
#[test]
fn a_pista_de_leitura_recusa_criar_o_fts() {
    use phxsql_store::table::SemEscrever;

    let (mut t, d) = nova("pista-de-leitura");
    t.inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();
    drop(t);
    // Abre e fecha pela ficha EXCLUSIVA para curar o que a gravacao deixou
    // pendente (o `.log`); sem isto a recusa que o teste mede seria a dele.
    drop(Table::abrir(&d, "docs").unwrap());

    // **O controle**, e ele e o que da sentido ao resto: nesta tabela, com o
    // `.fts` no lugar, a pista de leitura ABRE. Sem ele, uma recusa por
    // qualquer outro motivo passaria por prova.
    assert!(
        matches!(
            Table::abrir_para_ler(&d, "docs").unwrap(),
            SemEscrever::Aberta(_)
        ),
        "o controle falhou: a pista de leitura ja recusava esta tabela por \
         outro motivo, e entao a prova de baixo nao prova nada"
    );

    std::fs::remove_file(d.join("docs.fts")).unwrap();

    match Table::abrir_para_ler(&d, "docs").unwrap() {
        SemEscrever::PrecisaEscrever(motivo) => {
            assert!(motivo.contains(".fts"), "{motivo}");
        }
        SemEscrever::Aberta(_) => panic!(
            "a pista de leitura ABRIU e criou o .fts -- e escrever sob a ficha \
             compartilhada e o unico erro que ela existe para impedir"
        ),
    }

    // E a ficha exclusiva continua fazendo o trabalho, que é o outro lado.
    let mut t = Table::abrir(&d, "docs").unwrap();
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids.len(),
        1
    );
}

/// **Palavra que não cabe na chave: o índice dá candidatas, a tabela confere.**
///
/// A chave guarda 26 bytes do termo mais o comprimento saturado. Duas palavras
/// de 32 letras que compartilham os 26 primeiros bytes caem na MESMA chave, e o
/// índice sozinho devolve as duas — que é achar a mais, e achar a mais é
/// mentira que o cliente não tem como perceber.
///
/// A conferência mora na `Table`, e não em quem chama: quem esquecesse viraria
/// a porta dos fundos, e nenhum teste do caminho normal acusaria.
#[test]
fn palavra_longa_confere_a_linha_em_vez_de_achar_a_mais() {
    let (mut t, _d) = nova("longa");
    // Os 26 primeiros bytes são iguais, e o comprimento também: 32.
    let alfa = "desenvolvimentossustentaveisalfa";
    let beta = "desenvolvimentossustentaveisbeta";
    assert_eq!(alfa.len(), beta.len());
    assert_eq!(alfa[..26], beta[..26]);

    let a = t.inserir(&linha(1, "titulo a", alfa)).unwrap();
    let b = t.inserir(&linha(2, "titulo b", beta)).unwrap();

    let achado = t.procurar_texto("porCorpo", alfa).unwrap();
    assert!(achado.conferir, "esta palavra passa da largura da chave");
    assert_eq!(
        achado.rowids,
        vec![a],
        "sem a conferencia da linha, o indice devolve as DUAS"
    );
    assert_eq!(t.procurar_texto("porCorpo", beta).unwrap().rowids, vec![b]);
}

/// Pedido 364: a lista nova substitui a velha, e o `.fts` se refaz do `.reg`
/// para ela -- inclusive trocando de coluna com o MESMO numero de indices,
/// que e o caso em que um `.fts` velho abriria sem reclamar.
#[test]
fn redeclarar_troca_a_coluna_e_o_fts_acompanha() {
    let (mut t, d) = nova("redeclarar");
    let a = t
        .inserir(&linha(1, "pedido urgente", "o cliente fenix pediu"))
        .unwrap();
    let linhas = t
        .redeclarar_indices_de_texto(vec![
            IndiceDeTexto::new("porTitulo", 2),
            IndiceDeTexto::new("porCorpo", 1),
        ])
        .unwrap();
    assert_eq!(linhas, 1);
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a]
    );
    drop(t);
    let mut t = Table::abrir(&d, "docs").unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a]
    );
    assert!(t
        .procurar_texto("porTitulo", "pedido")
        .unwrap()
        .rowids
        .is_empty());
    assert!(
        !d.join("docs.fts.novo").exists(),
        "sobrou o montado ao lado"
    );
}

/// A FASE A nao toca em nada vivo: descartada, a tabela e a mesma de antes.
#[test]
fn a_troca_descartada_deixa_a_tabela_como_estava() {
    let (mut t, d) = nova("descartar");
    let a = t.inserir(&linha(1, "pedido", "fenix")).unwrap();
    let troca = t
        .preparar_indices_de_texto(vec![IndiceDeTexto::new("porOutro", 2)])
        .unwrap();
    assert!(d.join("docs.fts.novo").exists());
    troca.descartar();
    assert!(!d.join("docs.fts.novo").exists());
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a]
    );
    drop(t);
    let mut t = Table::abrir(&d, "docs").unwrap();
    assert_eq!(t.esquema().indices_de_texto().len(), 2);
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}

/// A queda logo depois do primeiro passo da FASE B -- o `.fts` velho ja
/// saiu, o `.reg` ainda nao trocou. O esquema no disco e o velho, sem `.fts`,
/// e a abertura o refaz do `.reg` pela declaracao velha: nenhum `.fts`
/// montado para uma declaracao serve a outra.
#[test]
fn queda_entre_apagar_o_fts_e_trocar_o_reg_reabre_pelo_esquema_velho() {
    let (mut t, d) = nova("queda");
    let a = t.inserir(&linha(1, "pedido", "fenix")).unwrap();
    t.sincronizar().unwrap();
    let troca = t
        .preparar_indices_de_texto(vec![IndiceDeTexto::new("porTitulo", 2)])
        .unwrap();
    // O passo 1 da FASE B, e a queda: nada mais acontece.
    std::fs::remove_file(d.join("docs.fts")).unwrap();
    drop(troca);
    drop(t);
    let mut t = Table::abrir(&d, "docs").unwrap();
    assert_eq!(
        t.esquema().indices_de_texto().len(),
        2,
        "o esquema velho ficou"
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}

// ------------------------------------------------ pedido 618: o orfao ao lado

/// Uma base com a tabela `clientes`, uma linha, e o `.fts` velho ja no disco.
fn base_de_clientes(rotulo: &str) -> (phxsql_store::catalogo::Database, comum::DirTemp) {
    let base = comum::DirTemp::novo(&format!("fts-618-{rotulo}"));
    let cat = phxsql_store::catalogo::Instancia::nova(&base).unwrap();
    let db = cat.criar_database("loja").unwrap();
    let mut esq = esquema();
    esq.renomear("clientes");
    let mut t = db.criar_tabela(None, esq).unwrap();
    t.inserir(&linha(1, "ana souza", "mora na rua fenix"))
        .unwrap();
    t.sincronizar().unwrap();
    drop(t);
    (db, base)
}

/// Os arquivos que ainda levam o nome da tabela, `.novo` inclusive.
fn sobras(dir: &std::path::Path, tabela: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.starts_with(&format!("{tabela}.")) || f.starts_with(&format!("{tabela}#")))
        .collect();
    v.sort();
    v
}

/// Mata a redeclaracao no ponto do pedido 618: o `.fts` novo montado e
/// sincronizado ao lado, e nada mais.
fn morrer_depois_de_montar_ao_lado(t: &mut Table) {
    use phxsql_store::ndx::panico_de_teste::{armar, desarmar, Ponto};
    armar(Ponto::FtsAoLadoDepoisDoSincronizar);
    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = t.preparar_indices_de_texto(vec![IndiceDeTexto::new("porTitulo", 1)]);
    }));
    desarmar();
    assert!(
        morreu.is_err(),
        "o panico armado depois do sincronizar nao aconteceu"
    );
}

/// **Pedido 618 (LGPD):** a redeclaracao que morre depois de montar o `.fts`
/// ao lado deixa o `clientes.fts.novo` -- o VOCABULARIO da coluna indexada --,
/// e o `excluir_tabela` o deixava para tras sob um nome que nao existe mais.
///
/// # Prova real
///
/// Com o `excluir_tabela` de volta ao `pertence` seco (sem o `.novo`), a
/// sobra e `["clientes.fts.novo"]` e este teste reprova nomeando-a.
#[test]
fn excluir_a_tabela_leva_o_fts_ao_lado_de_uma_redeclaracao_morta() {
    let (db, base) = base_de_clientes("excluir");
    let dir = base.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    morrer_depois_de_montar_ao_lado(&mut t);
    drop(t);
    assert!(
        dir.join("clientes.fts.novo").exists(),
        "o ponto do panico nao deixou o orfao que a prova diz"
    );
    db.excluir_tabela("clientes").unwrap();
    assert_eq!(
        sobras(&dir, "clientes"),
        Vec::<String>::new(),
        "a tabela foi excluida e o vocabulario dela ficou no disco"
    );
}

/// O caminho IRMAO do de cima: a FASE A inteira (o `.fts` ao lado E os
/// `*.novo` do `.reg`, que sao copia do `.reg` inteiro) e a queda antes da
/// FASE B. O excluir leva os dois.
///
/// # Prova real
///
/// Sem o `.novo` no `excluir_tabela`, sobram `clientes.fts.novo` e
/// `clientes.reg.novo`.
#[test]
fn excluir_a_tabela_leva_os_novos_do_reg_de_uma_fase_a_sem_fase_b() {
    let (db, base) = base_de_clientes("fase-a");
    let dir = base.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    let troca = t
        .preparar_indices_de_texto(vec![IndiceDeTexto::new("porTitulo", 1)])
        .unwrap();
    // A queda: nem FASE B, nem descartar.
    drop(troca);
    drop(t);
    assert!(dir.join("clientes.reg.novo").exists());
    assert!(dir.join("clientes.fts.novo").exists());
    db.excluir_tabela("clientes").unwrap();
    assert_eq!(sobras(&dir, "clientes"), Vec::<String>::new());
}

/// O renomear leva a tabela INTEIRA, e o `.novo` e dela: deixa-lo no nome
/// velho e o mesmo orfao do excluir, sob um nome que nao existe mais.
///
/// # Prova real
///
/// Sem o `.novo` no `renomear_tabela`, sobram os dois `.novo` em `clientes.`.
#[test]
fn renomear_a_tabela_nao_deixa_o_novo_no_nome_velho() {
    let (db, base) = base_de_clientes("renomear");
    let dir = base.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    let troca = t
        .preparar_indices_de_texto(vec![IndiceDeTexto::new("porTitulo", 1)])
        .unwrap();
    drop(troca);
    drop(t);
    db.renomear_tabela("clientes", "pessoas").unwrap();
    assert_eq!(sobras(&dir, "clientes"), Vec::<String>::new());
    let mut t = Table::abrir(&dir, "pessoas").unwrap();
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids.len(),
        1,
        "a tabela renomeada nao abriu com o esquema que tinha"
    );
}

/// A abertura com a ficha exclusiva recolhe o `.fts.novo` orfao -- sem
/// esperar alguem excluir a tabela -- e a tabela continua com a declaracao
/// e o `.fts` de antes da redeclaracao morta.
///
/// # Prova real
///
/// Sem o `recolher_fts_ao_lado` no `abrir_com`, o `clientes.fts.novo`
/// continua no disco depois da abertura.
#[test]
fn a_abertura_recolhe_o_fts_ao_lado_de_uma_redeclaracao_morta() {
    let (_db, base) = base_de_clientes("abrir");
    let dir = base.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    morrer_depois_de_montar_ao_lado(&mut t);
    drop(t);
    assert!(dir.join("clientes.fts.novo").exists());
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    assert!(
        !dir.join("clientes.fts.novo").exists(),
        "a abertura nao recolheu o vocabulario orfao"
    );
    assert_eq!(
        t.esquema().indices_de_texto().len(),
        2,
        "o esquema velho ficou"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids.len(),
        1
    );
}

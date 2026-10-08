//! O terceiro bit da trilha no JSON da op `trilha` -- pedido 375.
//!
//! A TELA ja lia `antes_indisponivel` do `Evento`; quem consome o JSON da op
//! nao o recebia. Sem ele, a unica saida de um cliente seria comparar o texto
//! do `antes` com a FRASE de `phxsql_store::trilha::INDISPONIVEL` -- e decidir
//! por frase quebra calado no dia em que alguem melhorar a redacao, que e a
//! mesma lei do `docs/MENSAGENS.md` e o motivo de o bit existir.
//!
//! # Por que este teste nao e o do 367
//!
//! As provas do 367 sao da LOJA (`phxsql-store/tests/trilha-lgpd.rs:459-551` e
//! `trilha.rs:1004`): elas travam o bit no `Evento` e na ida-e-volta do
//! arquivo. Esta e outra camada e outro defeito -- o bit existia, era lido, e
//! **nao saia pela porta**. Nenhum teste da loja pode acusar isso.
//!
//! # O preparo, e por que ele e o que e
//!
//! O evento indisponivel nasce de um bloco externo velho que nao abre. O
//! servidor nao tem operacao que estrague um `.memo` -- e nao deve ter --,
//! entao o arquivo e adulterado aqui, com a tabela fechada (o servidor abre e
//! fecha a tabela DENTRO de cada operacao, em `abrir_travada`), exatamente
//! como o irmao da loja faz. O que atravessa o servidor e o que importa: o
//! `atualizar` que le o bloco estragado, e o `trilha` que serializa o evento.
//!
//! E o preparo e **conferido antes do veredito**, pela licao de
//! `docs/cognicao/cognicao_prova-real-que-morre-no-preparo-nao-prova-o-caminho_20260918_1311.md`:
//! se o `.memo` estragado ainda se lesse, o vermelho sairia na linha do
//! preparo e nao na do campo sob prova, e as duas cores sao iguais.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor com a tabela `p` no database `b`, com `laudo` (`Memo`)
/// marcada como dado sensivel.
///
/// A marca entra com a tabela VAZIA de proposito: `marcar_lgpd` pode
/// reescrever os volumes, e o `.memo` que este teste vai estragar tem de
/// ser so' o que a insercao gravou.
fn servidor(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("bit-indisponivel-{nome}"));
    let config = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(config).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"p",
                    "colunas":[{"nome":"id","tipo":"Int8"},
                               {"nome":"paciente","tipo":"Str(40)"},
                               {"nome":"laudo","tipo":"Memo"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"p","colunas":{"laudo":"sensivel"}}"#),
        &dono,
    )
    .unwrap();
    (s, dir)
}

/// O bit como o CLIENTE o ve: presente ou ausente, nunca "false por
/// omissao". Com o defeito reposto o campo some, e `None` e o que denuncia
/// -- um `booleano_ou(..., false)` engoliria a ausencia e pintaria de
/// verde a metade do teste que importa.
fn bit(e: &Json) -> Option<bool> {
    e.campo("antes_indisponivel").and_then(Json::booleano)
}

fn alteracoes(s: &Arc<Servidor>) -> Vec<Json> {
    s.executar(
        "trilha",
        &pedido(r#"{"database":"b","tabela":"p","tipo":"alteracao"}"#),
        &Sessao::default(),
    )
    .unwrap()
    .campo("registros")
    .and_then(Json::lista)
    .map(|l| l.to_vec())
    .unwrap_or_default()
}

fn do_rowid(regs: &[Json], rowid: i64) -> &Json {
    regs.iter()
        .find(|e| e.inteiro_ou("rowid", -1) == rowid)
        .unwrap_or_else(|| {
            panic!(
                "o preparo nao gerou registro de alteracao para o rowid {rowid}: {:?}",
                regs.iter().map(|e| e.escrever()).collect::<Vec<_>>()
            )
        })
}

/// **Defeito reposto**: tirar a linha
/// `("antes_indisponivel", Json::Bool(e.antes_indisponivel()))` do
/// `op_trilha` faz este teste falhar nas DUAS asercoes do bit, porque o
/// campo deixa de existir no JSON.
///
/// # Os dois sentidos, e por que o segundo e o que decide
///
/// O primeiro (`true` no evento cujo `antes` nao pode ser lido) sozinho
/// passaria ate numa implementacao que publicasse `Json::Bool(true)`
/// cravado. O segundo mata essa -- e mata tambem a confusao vizinha, que e
/// a que o bit existe para desfazer: um valor velho **NULO** tem `antes`
/// vazio, e vazio nao e indisponivel. Um campo em branco e um fato sobre o
/// dado; "nao pude ler" e um fato sobre a trilha.
#[test]
fn o_json_da_trilha_distingue_antes_ilegivel_de_antes_vazio() {
    let (s, dir) = servidor("dois-sentidos");
    let dono = Sessao::default();

    // A linha 1 tem laudo no `.memo`; a 2 nasce com o laudo NULO, e por
    // isso nao gasta bloco nenhum -- o unico bloco do arquivo e o da 1.
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"b","tabela":"p",
                    "linha":{"id":1,"paciente":"Ana Prado",
                             "laudo":"LAUDO_VELHO: benigno"}}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"b","tabela":"p",
                    "linha":{"id":2,"paciente":"Bia Rocha","laudo":null}}"#,
        ),
        &dono,
    )
    .unwrap();

    // Vira o ultimo byte do `.memo`: ele esta dentro do conteudo do unico
    // bloco gravado, entao o CRC-32 do bloco deixa de bater (ver
    // `blob.rs`). A tabela esta fechada -- o servidor a abre e a fecha
    // dentro de cada operacao.
    let memo = dir.join("b/p.memo");
    let mut cru = std::fs::read(&memo).expect("o .memo da linha 1 tinha de existir");
    let ultimo = cru.len() - 1;
    cru[ultimo] ^= 0xFF;
    std::fs::write(&memo, &cru).unwrap();

    // O `atualizar` de cada linha e o que gera os dois eventos. O da 1 le
    // o bloco estragado (e nao derruba a gravacao: dentro do `atualizar`
    // da loja o erro do externo velho vira `None`, que e o bit); o da 2 le
    // um NULO, que e um valor, e nao uma falha.
    //
    // O `"softdeleted"` era obrigatorio aqui ate o pedido 381: sem ele,
    // `op_atualizar` relia a linha inteira para herdar a marca, e o
    // `.memo` estragado subia como `Corrompido` antes do campo sob prova.
    // Hoje essa leitura nao carrega as externas
    // (`testes_do_memo_estragado_no_atualizar`); ele fica porque e o
    // pedido que a interface web manda.
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"p","rowid":1,
                    "valores":{"id":1,"paciente":"Ana Prado",
                               "laudo":"LAUDO_NOVO: carcinoma",
                               "softdeleted":false}}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"p","rowid":2,
                    "valores":{"id":2,"paciente":"Bia Rocha",
                               "laudo":"LAUDO_DA_BIA: normal",
                               "softdeleted":false}}"#,
        ),
        &dono,
    )
    .unwrap();

    let regs = alteracoes(&s);
    let ilegivel = do_rowid(&regs, 1);
    let vazio = do_rowid(&regs, 2);

    // ---- preparo conferido, ANTES do veredito ----------------------
    // A frase se compara AQUI, e so' aqui: e a conferencia de que o
    // `.memo` estragado realmente nao abriu, nao a decisao que o defeito
    // obrigava o cliente a tomar. Sem ela, um `.memo` que voltasse a ser
    // legivel deixaria este teste vermelho na linha do bit -- a cor certa
    // pelo motivo errado.
    assert_eq!(
        ilegivel.texto_ou("antes", ""),
        phxsql_store::trilha::INDISPONIVEL,
        "o preparo falhou: o bloco velho do .memo ainda se le, entao este \
             teste nao chegou a exercitar o evento indisponivel"
    );

    // ---- sentido 1: o evento ilegivel sai com o bit ligado ---------
    assert_eq!(
        bit(ilegivel),
        Some(true),
        "o JSON da op `trilha` nao publicou `antes_indisponivel` para o \
             evento cujo valor velho nao pode ser lido; quem consome o JSON \
             so' poderia decidir comparando a FRASE do `antes`: {}",
        ilegivel.escrever()
    );

    // ---- sentido 2: o evento de valor velho NULO sai com ele desligado
    assert_eq!(
        vazio.texto_ou("antes", ""),
        "",
        "o preparo falhou: o laudo velho da linha 2 era nulo e tinha de \
             chegar a trilha como vazio"
    );
    assert_eq!(
        bit(vazio),
        Some(false),
        "vazio virou indisponivel (ou o campo nao existe): um campo em \
             branco e um fato sobre o DADO, e `indisponivel` e um fato sobre a \
             TRILHA -- confundir os dois e o que o bit existe para impedir: {}",
        vazio.escrever()
    );

    // E os dois bits vizinhos continuam apagados nos dois eventos: um
    // `Json::Bool` ligado no campo errado passaria despercebido sem isto.
    for e in [ilegivel, vazio] {
        assert!(
            !e.booleano_ou("antes_redigido", true) && !e.booleano_ou("depois_redigido", true),
            "a redacao nao entra nesta historia: {}",
            e.escrever()
        );
    }
}

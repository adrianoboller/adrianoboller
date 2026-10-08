//! A cifra do fio EXIGIDA para replicar tabela com coluna marcada (pedido
//! 342, decisao do dono de 23/09/2026).
//!
//! # O que estes testes provam, e o que NAO provam
//!
//! Provam o PORTAO: quem chega em claro nao leva a imagem de uma tabela com
//! coluna marcada, e quem chega pelo tunel leva. **Nao** provam que a imagem
//! deixou de carregar o valor -- ela carrega, e isso esta escrito e tem teste
//! proprio no store (`imagem_de_coluna_inline_marcada_vai_em_claro`). O que
//! muda e o CANAL, porque selar a imagem esta bloqueado com numero: a replica
//! nao tem chave compativel (pedido 344, sal por arquivo).
use super::*;

/// Um source com a imagem no diario. `exigir` diz se `cifra_fio.exigir`
/// esta ligado -- e so ele muda o veredito da porta HTTP.
fn source(dir: &std::path::Path, exigir: bool) -> Arc<Servidor> {
    let txt = r#"{"token":"t","replicacao":{"papel":"source","id_servidor":"src-01",
                        "imagem_da_linha":true}}"#;
    let mut c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
    c.base = dir.to_path_buf();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cifra_fio.exigir = exigir;
    Servidor::novo(c).unwrap()
}

/// A tabela do caso: `nome` MARCADA quando `marcada`, e nao marcada
/// quando nao -- tudo o mais igual. E a unica diferenca entre os dois
/// lados da prova.
fn tabela(dir: &std::path::Path, marcada: bool) -> Table {
    std::fs::create_dir_all(dir.join("loja")).unwrap();
    let mut nome = Column::new("nome", ColumnType::Str(40)).obrigatoria();
    if marcada {
        nome = nome.com_dado_pessoal(phxsql_core::types::DadoPessoal::Pessoal);
    }
    let esquema = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int8).obrigatoria(), nome],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    Table::criar(dir.join("loja"), esquema)
        .unwrap()
        .com_imagem_no_diario(true)
}

fn encher(dir: &std::path::Path, marcada: bool) {
    let mut t = tabela(dir, marcada);
    for i in 1..=5 {
        t.inserir(&[Value::Int(i), Value::Str(format!("fulano {i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
}

/// Uma conexao pela porta de DADOS, com ou sem o tunel da §7.
fn pela_porta_de_dados(com_tunel: bool) -> Sessao {
    Sessao {
        entrada: Entrada::Dados,
        transcricao_do_fio: com_tunel.then_some([7u8; 32]),
        ..Sessao::default()
    }
}

fn replicar(s: &Arc<Servidor>, sessao: &Sessao) -> Result<Json> {
    let p = Json::analisar(r#"{"database":"loja","tabela":"clientes","desde":0}"#).unwrap();
    s.op_replicar(&p, sessao)
}

fn quantos(r: &Json) -> usize {
    r.campo("eventos").and_then(Json::lista).unwrap().len()
}

/// **A prova do pedido 342.** Em claro, a tabela marcada NAO viaja.
///
/// # O vermelho
///
/// Medido em 23/09/2026 com o portao removido: `replicou 5 eventos em
/// claro` -- os cinco com `fulano 1..5` dentro da imagem, pela conexao em
/// texto puro.
///
/// A assimetria que o pedido nomeia tem guarda propria no store
/// (`tests/imagem-de-replicacao-com-coluna-marcada.rs`), e la ela e
/// afirmada por VEREDITO e nao por tamanho: o total da imagem anda com a
/// largura do esquema, e o numero que este comentario citava -- 146 B --
/// ja nao reproduzia seis dias depois de escrito.
#[test]
fn tabela_com_coluna_marcada_nao_replica_em_claro() {
    let dir = DirTemp::novo("replicar-marcada-claro");
    encher(&dir, true);
    let s = source(&dir, false);
    let e = match replicar(&s, &pela_porta_de_dados(false)) {
        Err(e) => e.to_string(),
        Ok(r) => panic!("replicou {} eventos em claro", quantos(&r)),
    };
    // A recusa tem de NOMEAR a tabela e as duas saidas. Recusa seca
    // mandaria o operador procurar permissao, que e o lugar errado.
    assert!(e.contains("clientes"), "{e}");
    assert!(e.contains("cifrar") || e.contains("cifra"), "{e}");
}

/// E pelo tunel ela viaja -- que e a metade sem a qual o portao seria so
/// uma parede.
#[test]
fn com_o_tunel_a_tabela_marcada_replica() {
    let dir = DirTemp::novo("replicar-marcada-tunel");
    encher(&dir, true);
    let s = source(&dir, false);
    let r = replicar(&s, &pela_porta_de_dados(true)).unwrap();
    assert_eq!(quantos(&r), 5);
}

/// **O comportamento VELHO, onde ele ainda vale.** Tabela SEM coluna
/// marcada continua replicando em claro, exatamente como antes.
///
/// E o teste que mais importa numa guarda imposta: o alcance dela. Sem
/// isto, um portao de seguranca certo derrubaria todo laco de replicacao
/// que existe hoje -- e proteccao que quebra todo cliente antigo nao e
/// protecao, e estrago.
#[test]
fn sem_coluna_marcada_replicar_em_claro_continua() {
    let dir = DirTemp::novo("replicar-sem-marca-claro");
    encher(&dir, false);
    let s = source(&dir, false);
    let r = replicar(&s, &pela_porta_de_dados(false)).unwrap();
    assert_eq!(quantos(&r), 5, "quem nao declarou dado pessoal nao muda");
}

/// A porta HTTP nunca tem tunel: o veredito dela sai de `cifra_fio.exigir`
/// -- e a inferencia esta escrita em `fio_cifrado`, com o portao de rede
/// HTTP como prova indireta do proxy declarado.
#[test]
fn pela_porta_http_o_veredito_sai_do_exigir() {
    let dir = DirTemp::novo("replicar-marcada-http");
    encher(&dir, true);
    let http = Sessao {
        entrada: Entrada::Http,
        ..Sessao::default()
    };
    assert!(
        replicar(&source(&dir, false), &http).is_err(),
        "sem `exigir`, a porta HTTP nao prova proxy nenhum"
    );
    assert_eq!(
        quantos(&replicar(&source(&dir, true), &http).unwrap()),
        5,
        "com `exigir`, chegar ate aqui ja e a prova do `atras_de_proxy`"
    );
}

/// A rotina INTERNA nao tem fio, e por isso nao e recusada.
///
/// Sem esta linha o portao recusaria a si mesmo: job agendado,
/// reconciliacao e ponte MCP entram por `Entrada::SemFio`, e nenhum deles
/// tem soquete por onde vazar.
#[test]
fn sem_fio_nao_ha_o_que_cifrar() {
    let dir = DirTemp::novo("replicar-marcada-semfio");
    encher(&dir, true);
    let s = source(&dir, false);
    assert_eq!(quantos(&replicar(&s, &Sessao::default()).unwrap()), 5);
}

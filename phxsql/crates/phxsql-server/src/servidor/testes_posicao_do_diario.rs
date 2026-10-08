//! A posicao do diario, e o que ela faz quando uma tabela nao abre.
//!
//! # Por que este modulo existe
//!
//! Achado em 05/09/2026 conferindo, um a um, os oito sitios que abrem tabela
//! sem sessao. `posicao_do_diario` nao tinha teste NENHUM -- so um sitio de
//! chamada, no pulso.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("pos-{nome}"))
}

fn json(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Duas tabelas com linhas, para a soma ter duas parcelas.
fn com_duas(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let ses = Sessao::default();
    s.executar("criar_database", &json(r#"{"database":"b"}"#), &ses)
        .unwrap();
    for t in ["uma", "outra"] {
        s.executar(
            "criar_tabela",
            &json(&format!(
                r#"{{"database":"b","tabela":"{t}",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]}}"#
            )),
            &ses,
        )
        .unwrap();
        for i in 1..=5 {
            s.executar(
                "inserir",
                &json(&format!(
                    r#"{{"database":"b","tabela":"{t}","linha":{{"id":{i}}}}}"#
                )),
                &ses,
            )
            .unwrap();
        }
    }
    s
}

/// **GUARDA do pedido 211 -- nasceu VERMELHA em 05/09/2026, virou VERDE em
/// 10/09/2026 com a decisao (b) do dono.**
///
/// Uma tabela que NAO ABRE some da soma da posicao do diario. O que mudou
/// nao foi a soma -- uma posicao incompleta continua MENOR que a real --,
/// foi o SILENCIO: agora a posicao volta marcada `incompleta`, e a eleicao
/// (`cluster::vencedor`) prefere quem esta completo.
///
/// # Por que isso e grave, e nao um detalhe de contagem
///
/// O comentario da propria `posicao_do_diario` diz o que esse numero e:
/// *«a soma dos eventos das tabelas replicadas -- a posicao que o pulso
/// carrega e que a ELEICAO compara»*. E o `cluster.rs` fecha a conta:
/// *«uma posicao COMPLETA ganha de uma incompleta antes de qualquer
/// numero»*.
///
/// Antes do conserto, um no que nao conseguia abrir uma tabela **se
/// declarava mais atrasado do que era** e perdia uma eleicao que deveria
/// vencer, EM SILENCIO. Agora ele ainda conta menos -- nao ha como contar
/// o que nao abre --, mas a bandeira `incompleta` conta a verdade, e a
/// eleicao a le.
///
/// # Por que a causa aqui e um `.reg` estragado, e nao a cifra
///
/// Porque o que se prova e o marcar da falha, e nao uma causa. Cifrada sem
/// chave, corrompida, sem permissao: para esta funcao sao todas «nao
/// abriu», e uma so basta. Estragar o arquivo e a causa mais
/// deterministica das tres, e nao depende do estado do cofre do processo.
///
/// # A prova pega nos dois sentidos
///
/// Antes de estragar, a posicao e COMPLETA (a bandeira e `false`). Depois,
/// a bandeira vira `true` E o numero encolhe -- as duas coisas. Se o
/// conserto for revertido (a funcao volta a devolver so `u64`, ou o
/// `incompleta` fica sempre `false`), a asserção da bandeira falha.
#[test]
fn tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio() {
    let d = dir_temp("engole");
    let s = com_duas(&d);

    let (inteira, incompleta_antes, vetor) = s.posicao_do_diario(&[]);
    // Pedido 294: o vetor sai da MESMA passada, entao soma igual.
    assert_eq!(vetor.iter().map(|(_, n)| n).sum::<u64>(), inteira);
    assert_eq!(vetor.len(), 2, "{vetor:?}");
    assert!(
        inteira > 0,
        "as duas tabelas somadas tem de dar posicao maior que zero"
    );
    assert!(
        !incompleta_antes,
        "com as duas tabelas abrindo, a posicao ({inteira}) e COMPLETA"
    );

    // Estraga o `.reg` de UMA delas: a partir daqui ela existe e nao abre.
    let alvo = d.join("b").join("uma.reg");
    assert!(
        alvo.exists(),
        "o arquivo da tabela tem de existir: {alvo:?}"
    );
    std::fs::write(&alvo, b"nao sou um PHXREG").unwrap();

    let (depois, incompleta_depois, _) = s.posicao_do_diario(&[]);
    assert!(
        incompleta_depois,
        "a tabela que nao abre NAO some em silencio: a posicao ({depois}) \
             volta marcada INCOMPLETA, e a eleicao prefere posicao completa."
    );
    assert!(
        depois < inteira,
        "a posicao incompleta ({depois}) e MENOR que a inteira ({inteira}), \
             como manda a verdade -- nao ha como contar a tabela que nao abre. \
             O que mudou e que agora ela vem MARCADA, nao engolida."
    );
}

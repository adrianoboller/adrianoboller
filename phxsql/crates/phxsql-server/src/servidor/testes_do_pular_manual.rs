//! A saida manual do par parado -- `replicacao_pular`, pedido 292 parte (1).
//!
//! E o `ALTER SUBSCRIPTION ... SKIP (lsn)` somado ao
//! `pg_replication_origin_advance()` do PostgreSQL: o par que um conflito de
//! unicidade parou so volta a andar por aqui, pulando AQUELE evento pela
//! posicao. A prova do ciclo inteiro -- parar, ver, pular, voltar a replicar
//! -- e pelo soquete, em `tests/laco-do-unico-secundario.rs`; aqui ficam as
//! recusas e a aritmetica da posicao, que nao precisam de dois servidores.
use super::*;
use crate::usuarios::Cadastro;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

/// Poe o par de pe com uma parada pronta, como o laco a deixaria.
fn com_parada(s: &Arc<Servidor>, posicao: u64) {
    s.parar_o_par(
        "parceiro",
        "loja/clientes",
        "parceiro|loja/clientes",
        posicao,
        "conflito_de_unicidade",
        "indice \"porEmail\", valor \"a@x\"; a linha daqui e (id=1, email=a@x), \
             a de la e (id=2, email=a@x)"
            .into(),
    );
}

fn pular(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.op_replicacao_pular(&pedido(corpo))
}

/// **Prova real.** A parada na posicao 7 vira posicao consumida 8, a marca
/// sai, e o arquivo de posicoes grava -- senao um reinicio desfaria o pulo
/// e o par pararia de novo no mesmo evento.
///
/// **Defeito reposto**: trocar `parada.posicao + 1` por `parada.posicao`.
/// A posicao volta a 7, o laco reapresenta o evento que parou tudo e a
/// asercao do 8 cai.
#[test]
fn pular_anda_para_depois_do_evento_e_solta_a_parada() {
    let dir = DirTemp::novo("pular-anda");
    let s = servidor(&dir, Cadastro::default());
    com_parada(&s, 7);

    let r = pular(
        &s,
        r#"{"origem":"parceiro","database":"loja","tabela":"clientes"}"#,
    )
    .expect("o pulo tinha de ser aceito");
    assert_eq!(r.inteiro_ou("pulou", -1), 7, "nao disse o que pulou");
    assert_eq!(r.inteiro_ou("posicao", -1), 8, "a posicao nao andou");
    assert_eq!(r.texto_ou("motivo", ""), "conflito_de_unicidade");

    assert_eq!(
        s.posicoes_bidi.lock().unwrap()["parceiro|loja/clientes"],
        8,
        "a posicao consumida nao andou em memoria"
    );
    assert_eq!(
        bidirecional::ler_posicoes(&dir.join("replicacao-posicoes.json"))
            .get("parceiro|loja/clientes"),
        Some(&8),
        "a posicao nao foi ao disco: um reinicio desfaria o pulo"
    );

    // A marca some do `replicacao_estado`, senao o painel diria parado
    // para sempre -- recado que sobrevive ao conserto vira configuracao
    // que mente.
    let e = s.op_replicacao_estado().unwrap();
    assert!(
        e.campo("origens")
            .and_then(|o| o.campo("parceiro"))
            .and_then(|o| o.campo("paradas"))
            .and_then(|p| p.campo("loja/clientes"))
            .is_none(),
        "a parada sobreviveu ao pulo: {}",
        e.escrever()
    );
}

/// **Pedido 597: a posicao que nao foi ao disco nao vira «pulou».**
///
/// A falha de gravar e forjada sem `fsync` nenhum -- um DIRETORIO no nome
/// do temporario do `gravar_privado`, que o `remove_file` dele recusa --,
/// porque uma recusa de `fsync` armada derrubaria este binario pelo gancho
/// do 509 que o `Servidor::novo` registra.
///
/// **Defeito reposto** (o `let _ =` de antes, com a parada saindo antes da
/// gravacao): a operacao responde Ok, e as tres asercoes caem.
#[test]
fn pular_que_nao_grava_a_posicao_diz_que_nao_pulou() {
    let dir = DirTemp::novo("pular-sem-disco");
    let s = servidor(&dir, Cadastro::default());
    com_parada(&s, 7);
    let caminho = dir.join("replicacao-posicoes.json");
    std::fs::create_dir_all(crate::config::temporario_de(&caminho)).unwrap();

    let erro = pular(
        &s,
        r#"{"origem":"parceiro","database":"loja","tabela":"clientes"}"#,
    )
    .expect_err("a posicao nao foi ao disco, e a resposta disse que pulou");
    assert!(
        erro.to_string().contains("NAO foi pulado"),
        "a recusa nao diz o que nao aconteceu: {erro}"
    );
    // A parada fica, para o operador pular de novo depois do disco.
    assert!(
        s.esta_parada("parceiro", "loja/clientes"),
        "a parada saiu por um pulo que nao foi ao disco"
    );
    // E o mapa volta: ele e gravado INTEIRO pelo proximo alcance, que
    // levaria ao disco o pulo que a resposta negou.
    assert!(
        s.posicoes_bidi.lock().unwrap().is_empty(),
        "o mapa ficou com a posicao que nao foi ao disco"
    );
    assert!(
        !caminho.exists(),
        "o arquivo de posicoes nasceu assim mesmo"
    );
}

/// **O teste do comportamento VELHO.** Par SAO nao se anda pela mao: a
/// operacao recusa nomeando o que esta parado, em vez de descartar um
/// evento que ninguem leu. E a diferenca deliberada para o
/// `pg_replication_origin_advance()`, que aceita qualquer LSN -- aqui
/// reaplicar e inofensivo, entao andar a posicao nunca conserta nada e so
/// pode perder dado.
#[test]
fn pular_num_par_sao_recusa_nomeando() {
    let dir = DirTemp::novo("pular-sao");
    let s = servidor(&dir, Cadastro::default());
    // A origem existe e esta sadia -- ela so ficou conhecida por ter
    // rodado, como o laco a registra.
    s.anotar_estado("parceiro", |e| e.modo = "streaming".into());

    let erro = pular(
        &s,
        r#"{"origem":"parceiro","database":"loja","tabela":"clientes"}"#,
    )
    .expect_err("par sao nao se anda pela mao");
    assert_eq!(erro.nome(), "NAO_ENCONTRADO", "{erro}");
    assert!(
        erro.to_string().contains("nao esta parada"),
        "a recusa nao diz por que: {erro}"
    );
    assert!(
        s.posicoes_bidi.lock().unwrap().is_empty(),
        "a recusa mexeu na posicao"
    );
}

/// Origem que nem existe recusa listando as que existem -- e o mesmo
/// recado do `replicacao_ligar`, porque errar o nome da origem e o erro
/// de digitacao mais comum que ha.
#[test]
fn origem_desconhecida_recusa_listando_as_conhecidas() {
    let dir = DirTemp::novo("pular-origem");
    let s = servidor(&dir, Cadastro::default());
    s.anotar_estado("matriz", |e| e.modo = "streaming".into());
    let erro = pular(
        &s,
        r#"{"origem":"ninguem","database":"loja","tabela":"clientes"}"#,
    )
    .unwrap_err();
    assert!(erro.to_string().contains("matriz"), "{erro}");
}

/// Source velho demais para dizer ONDE o evento mora: recusa em vez de
/// adivinhar. Adivinhar a posicao descarta evento que ninguem olhou.
#[test]
fn sem_a_posicao_do_source_o_pulo_recusa_em_vez_de_adivinhar() {
    let dir = DirTemp::novo("pular-sem-posicao");
    let s = servidor(&dir, Cadastro::default());
    com_parada(&s, crate::replica::POSICAO_DESCONHECIDA);
    let erro = pular(
        &s,
        r#"{"origem":"parceiro","database":"loja","tabela":"clientes"}"#,
    )
    .unwrap_err();
    assert!(erro.to_string().contains("posicao"), "{erro}");
    assert!(
        s.posicoes_bidi.lock().unwrap().is_empty(),
        "a recusa mexeu na posicao"
    );
    // E a parada FICA: recusar nao pode soltar o par calado.
    assert!(s.esta_parada("parceiro", "loja/clientes"));
}

/// Sem os tres campos a operacao nao adivinha nenhum deles -- e `tabela`
/// esta entre eles de proposito: e o campo que o portao unico do
/// `despachar` le, e uma operacao que o deixasse vazio cairia na regra da
/// base e furaria o direito por tabela.
#[test]
fn os_tres_campos_sao_obrigatorios() {
    let dir = DirTemp::novo("pular-campos");
    let s = servidor(&dir, Cadastro::default());
    com_parada(&s, 1);
    for corpo in [
        r#"{"database":"loja","tabela":"clientes"}"#,
        r#"{"origem":"parceiro","tabela":"clientes"}"#,
        r#"{"origem":"parceiro","database":"loja"}"#,
    ] {
        let erro = pular(&s, corpo).unwrap_err();
        assert!(erro.to_string().contains("tabela"), "{corpo}: {erro}");
    }
}

/// **O portao.** A operacao exige `administrar` NA TABELA: quem tem o
/// poder na base e nao nela para aqui. E o portao unico do `despachar`
/// fazendo o trabalho, e nao uma conferencia propria -- por isso o pedido
/// obriga o campo `"tabela"`.
#[test]
fn o_pular_passa_pelo_portao_da_tabela() {
    assert_eq!(
        Atividade::da_operacao("replicacao_pular"),
        Some(Atividade::Administrar),
        "a operacao mais perigosa da replicacao nao declarou poder"
    );
    let dir = DirTemp::novo("pular-portao");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,"nivel":"operador",
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"loja":{"administrar":true,"tabelas":{"clientes":{}}}}}]}"#,
    ))
    .unwrap();
    let s = servidor(&dir, cadastro.clone());
    com_parada(&s, 3);
    let mut ses = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"replicacao_pular","origem":"parceiro","database":"loja","tabela":"clientes"}"#,
        &mut ses,
        "127.0.0.1",
    );
    let erro = r.expect_err("quem nao administra a tabela nao solta o laco dela");
    assert_eq!(erro.nome(), "ACESSO_NEGADO", "{erro}");
    // E o par continua parado: o portao recusou ANTES do trabalho.
    assert!(s.esta_parada("parceiro", "loja/clientes"));

    // O outro sentido do laco: com o poder na tabela, passa.
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,"nivel":"operador",
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"loja":{"tabelas":{"clientes":{"administrar":true}}}}}]}"#,
    ))
    .unwrap();
    let dir2 = DirTemp::novo("pular-portao-ok");
    let s2 = servidor(&dir2, cadastro.clone());
    com_parada(&s2, 3);
    let mut ses2 = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r2) = s2.despachar(
        r#"{"token":"t","op":"replicacao_pular","origem":"parceiro","database":"loja","tabela":"clientes"}"#,
        &mut ses2,
        "127.0.0.1",
    );
    assert_eq!(
        r2.expect("quem administra a tabela solta o laco dela")
            .inteiro_ou("posicao", -1),
        4
    );
}

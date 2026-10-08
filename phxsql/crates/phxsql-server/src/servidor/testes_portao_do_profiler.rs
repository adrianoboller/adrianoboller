//! Quem pode LIGAR o profiler e LER o que ele viu.
//!
//! # O furo, e por que a leitura do codigo nao o mostrava
//!
//! A ficha do `op_profiler_ligar` dizia «**So administrador**» desde sempre, e
//! o `da_operacao` de fato pede `Atividade::Administrar` para as quatro
//! operacoes. So que nenhum pedido do profiler tem campo `"database"`, entao
//! o portao 3 do `despachar` pergunta «pode administrar a base VAZIA?» -- e
//! `bases: {"*": {administrar: true}}` responde sim para quem e leitor.
//!
//! E o mesmo furo do `juntar`/`unir` com o sinal trocado: la o portao olhava
//! um campo que a operacao nao tinha e ela ESCAPAVA; aqui ele olha um campo
//! vazio e a regra curinga a DEIXA PASSAR. A telemetria ja tinha aprendido
//! isso e ganhou `portao_da_telemetria`; o profiler ficou para tras.
//!
//! O que estes testes travam:
//!
//! 1. **leitor com o curinga nao entra** -- nas quatro operacoes;
//! 2. **administrador de verdade continua entrando**;
//! 3. **sem cadastro nada muda**, que e o teste do comportamento VELHO: quem
//!    sobe com token de servico e sem usuarios nao pode perder o profiler de
//!    um dia para o outro.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("pp-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// O cadastro sai do JSON para o teste exercitar tambem a leitura do
/// `config.json`, que e onde o nivel e o curinga sao escritos de verdade.
fn cadastro(nivel: &str, bases: &str) -> Cadastro {
    Cadastro::de_json(&pedido(&format!(
        r#"{{"usuarios":[{{"login":"ana","id":9,"nivel":"{nivel}",
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
    )))
    .unwrap()
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

fn sessao_de(c: &Cadastro) -> Sessao {
    Sessao {
        usuario: c.por_login("ana").cloned(),
        ..Sessao::default()
    }
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade.
fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: sessao.usuario.clone(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

const AS_QUATRO: [&str; 4] = [
    r#""op":"profiler_ligar""#,
    r#""op":"profiler""#,
    r#""op":"profiler_limpar""#,
    r#""op":"profiler_desligar""#,
];

/// O caso do enunciado: leitor com `administrar` na regra `"*"`. Ele
/// passava pelo portao geral -- e o profiler lhe entregava o texto dos
/// pedidos de todo mundo, inclusive das tabelas que ele nao pode ler.
///
/// O recado tem de dizer POR QUE, e nao so «negado»: sem conferir o texto,
/// o teste passaria com o pedido falhando por qualquer outro motivo -- e
/// teste que passa por engano e pior que teste que falta.
#[test]
fn leitor_com_administrar_no_curinga_nao_liga_o_profiler() {
    let dir = dir_temp("curioso");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"administrar":true}}"#);
    let s = servidor(&dir, c.clone());
    let ses = sessao_de(&c);
    for corpo in AS_QUATRO {
        let e = pede(&s, &ses, corpo).expect_err(corpo);
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
        assert!(
            e.to_string().contains("administrador deste servidor"),
            "{corpo}: {e}"
        );
    }
}

/// Administrador de verdade continua ligando -- senao o conserto teria
/// tirado a funcionalidade em vez de fechar a porta.
#[test]
fn administrador_continua_ligando() {
    let dir = dir_temp("admin");
    let c = cadastro("admin", r#"{"*":{"ler":true,"administrar":true}}"#);
    let s = servidor(&dir, c.clone());
    let ses = sessao_de(&c);
    for corpo in AS_QUATRO {
        pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
    }
}

/// **O teste do comportamento VELHO.** Servidor sem cadastro de usuarios:
/// quem entra pelo token de servico nao tem `usuario` na sessao, e sempre
/// pode tudo. Uma guarda nova que mudasse isso tiraria o profiler de todo
/// servidor que ainda nao cadastrou ninguem -- e ninguem pediu isso.
#[test]
fn sem_cadastro_nada_muda() {
    let dir = dir_temp("sem-cadastro");
    let s = servidor(&dir, Cadastro::default());
    let ses = Sessao::default();
    for corpo in AS_QUATRO {
        pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
    }
}

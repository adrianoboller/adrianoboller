//! Observacao que nao esta ligada nao pode custar nada.
//!
//! Ate a 0.17.0 custava, e escondido: todo pedido pagava dois `Json::analisar`
//! do corpo inteiro, tres `String` e um mutex ANTES de `chegou` olhar `ligado`
//! e devolver `None`. Num `inserir_lote` de cinco mil linhas era analisar meio
//! megabyte de JSON duas vezes, para nada -- medido em 7% da carga pela rede
//! (`bancada/carga/medir.py`).
//!
//! O portao barato e um `AtomicBool`, e o que pode dar errado nele e DIVERGIR
//! do estado real: preso em `true` faz o servidor pagar o parse para sempre;
//! preso em `false` faz o profiler nao ver nada, ligado. E isso que estes
//! testes travam.
//!
//! A captura em si mora no laco da conexao, e nao no `despachar` -- entao ela
//! nao se exercita daqui. Quem a exercita e a bancada, e o numero dela e o que
//! denuncia se o portao sumir.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("prof-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

/// O espelho e o estado de verdade, lado a lado.
fn conferir(s: &Arc<Servidor>, esperado: bool) {
    let real = s.profiler.lock().unwrap().ligado();
    let espelho = s.profiler_ligado.load(Ordering::Relaxed);
    assert_eq!(real, esperado, "o profiler de verdade");
    assert_eq!(
        espelho, real,
        "o espelho divergiu: espelho={espelho}, real={real}"
    );
}

/// **A lista de `cifra.tabelas` chega mesmo ao Profiler.**
///
/// Nao e conferencia de enfeite: o conserto da parte (3) mora no
/// `profiler.rs`, e um `definir_sigilosas` que ninguem chamasse deixaria
/// os sete testes de la passando com o arquivo gravando em claro. O fio
/// que liga a configuracao ao instrumento e o que este teste segura.
#[test]
fn a_lista_de_tabelas_declaradas_chega_ao_profiler_no_ligar() {
    let dir = dir_temp("lista-no-ligar");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    c.cifra.tabelas = vec!["loja.clientes".into(), "rh.folha".into()];
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();
    let r = s
        .executar("profiler_ligar", &pedido("{}"), &sessao)
        .unwrap();
    assert_eq!(
        s.profiler.lock().unwrap().sigilosas(),
        ["loja.clientes".to_string(), "rh.folha".to_string()],
        "o Profiler ligou sem a lista: o perfil.txt vai gravar em claro"
    );
    // E ela SAI na resposta: quem escolhe o caminho do arquivo precisa
    // saber, ali, quais tabelas nao vao ter o texto gravado.
    let lista = r.campo("tabelas_sem_texto").and_then(Json::lista).unwrap();
    assert_eq!(lista.len(), 2, "a resposta de ligar escondeu a lista");
}

/// **A raiz de dados tambem chega ao Profiler, e pelo mesmo motivo.**
///
/// O conserto do pedido 356 mora no `profiler.rs` e depende de haver disco
/// a quem perguntar: sem a raiz, os seis testes de `testes_reg_cifrado`
/// continuam passando -- eles mesmos definem a raiz -- e o `perfil.txt` de
/// um servidor de verdade volta a gravar em claro o payload da tabela
/// cifrada. E o fio que este teste segura, irmao do de cima.
#[test]
fn a_raiz_dos_dados_chega_ao_profiler_no_ligar() {
    let dir = dir_temp("raiz-no-ligar");
    let s = servidor(&dir);
    let sessao = Sessao::default();
    s.executar("profiler_ligar", &pedido("{}"), &sessao)
        .unwrap();
    assert_eq!(
        s.profiler.lock().unwrap().raiz_dos_dados(),
        dir.as_ref() as &std::path::Path,
        "o Profiler ligou sem a raiz de dados: o perfil.txt vai gravar em \
             claro o pedido da tabela cifrada que ninguem declarou"
    );
}

/// Nasce desligado, senao o caminho quente pagaria desde o arranque por uma
/// observacao que ninguem pediu.
#[test]
fn nasce_desligado() {
    let dir = dir_temp("nasce");
    conferir(&servidor(&dir), false);
}

/// Ligar e desligar, varias voltas: o espelho acompanha em todas.
#[test]
fn o_espelho_nunca_diverge() {
    let dir = dir_temp("espelho");
    let s = servidor(&dir);
    let sessao = Sessao::default();
    for _ in 0..3 {
        s.executar("profiler_ligar", &pedido("{}"), &sessao)
            .unwrap();
        conferir(&s, true);
        s.executar("profiler_desligar", &pedido("{}"), &sessao)
            .unwrap();
        conferir(&s, false);
    }
}

/// Ligar duas vezes seguidas nao pode deixar o espelho para tras -- nem
/// desligar duas vezes.
#[test]
fn ligar_ou_desligar_repetido_nao_confunde_o_espelho() {
    let dir = dir_temp("repetido");
    let s = servidor(&dir);
    let sessao = Sessao::default();

    s.executar("profiler_ligar", &pedido("{}"), &sessao)
        .unwrap();
    s.executar("profiler_ligar", &pedido("{}"), &sessao)
        .unwrap();
    conferir(&s, true);

    s.executar("profiler_desligar", &pedido("{}"), &sessao)
        .unwrap();
    s.executar("profiler_desligar", &pedido("{}"), &sessao)
        .unwrap();
    conferir(&s, false);
}

/// Ligar com filtro tambem liga o espelho: o filtro decide o que ENTRA no
/// anel, e nao se a observacao existe.
#[test]
fn ligar_com_filtro_tambem_liga_o_espelho() {
    let dir = dir_temp("filtro");
    let s = servidor(&dir);
    s.executar(
        "profiler_ligar",
        &pedido(r#"{"database":"b","so_escrita":true}"#),
        &Sessao::default(),
    )
    .unwrap();
    conferir(&s, true);
}

/// Ligar que FALHA nao pode ligar o espelho -- senao o servidor pagaria o
/// parse por uma observacao que nunca existiu.
#[test]
fn ligar_que_falha_nao_liga_o_espelho() {
    let dir = dir_temp("falha");
    let s = servidor(&dir);
    let r = s.executar(
        "profiler_ligar",
        &pedido(r#"{"arquivo":"/diretorio/que/nao/existe/prof.txt"}"#),
        &Sessao::default(),
    );
    assert!(r.is_err(), "aceitou um caminho que nao existe");
    conferir(&s, false);
}

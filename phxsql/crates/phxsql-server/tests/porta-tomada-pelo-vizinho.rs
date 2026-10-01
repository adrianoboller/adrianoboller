//! Prova real dos pedidos 352 e 401: o teste que sorteia a porta, solta, e
//! sobe o servidor com `let _ = escutar()` CONVERSA COM O VIZINHO quando
//! outro servidor do mesmo binario pegou o numero -- e o apoio novo
//! (`comum::tentar_no_ar` / `comum::no_ar_no_ouvinte`) torna isso um erro que
//! nomeia a causa, ou impossivel por construcao.
//!
//! A montagem e DETERMINISTICA: em vez de esperar o sorteio colidir (medido
//! raro, uma queda em dezenas de corridas cheias), o vizinho JA esta na
//! porta que o segundo servidor pede. E exatamente o estado que o sorteio
//! produz quando colide, sem depender de sorte.

mod comum;
use comum::DirTemp;

use std::sync::Arc;

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

/// O MESMO token nos dois, de proposito: e o que os testes de um mesmo
/// arquivo tinham (uma `const TOKEN` por arquivo), e e o que faz o vizinho
/// ATENDER a conversa cruzada em vez de recusa-la.
const TOKEN: &str = "porta-tomada-pelo-vizinho";

fn config(base: &std::path::Path, bind: &str) -> Config {
    let mut c = Config {
        bind: bind.into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // Em claro: o que se mede aqui e QUEM atende, nao a cifra do fio.
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let linha = comum::pedir(porta, &format!("{{\"token\":\"{TOKEN}\",{corpo}}}"));
    Json::analisar(&linha).unwrap_or_else(|e| panic!("resposta ilegivel ({e}): {linha}"))
}

/// Para a porta de dados no fim: servidor de teste nao fica aceitando
/// conexao depois que o teste acabou.
fn parar(porta: u16) {
    let _ = pedir(porta, r#""op":"servico_parar""#);
}

/// O vizinho no ar, numa porta pedida em 0 e lida de volta, com `loja` ja
/// criada -- o estado do servidor de OUTRO teste.
fn vizinho(dir: &DirTemp) -> (Arc<Servidor>, u16) {
    let v = Servidor::novo(config(dir, "127.0.0.1:0")).unwrap();
    let copia = Arc::clone(&v);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| v.porta_dos_dados());
    let r = pedir(porta, r#""op":"criar_database","database":"loja""#);
    assert!(
        r.booleano_ou("ok", false),
        "o vizinho nao criou loja: {}",
        r.escrever()
    );
    (v, porta)
}

/// **A causa do 352, reproduzida.** O padrao velho (`let _ = escutar()` e
/// depois esperar a porta atender) sobe «com sucesso» um servidor que nunca
/// ligou porta nenhuma, e o primeiro pedido do teste cai no vizinho: a
/// mensagem e a do pedido, «database loja ja existe», num servidor que acabou
/// de nascer vazio.
#[test]
fn o_padrao_velho_conversa_com_o_vizinho_e_ouve_loja_ja_existe() {
    let dv = DirTemp::novo("porta-tomada-vizinho");
    let (_v, porta) = vizinho(&dv);

    let dt = DirTemp::novo("porta-tomada-teste");
    let t = Servidor::novo(config(&dt, &format!("127.0.0.1:{porta}"))).unwrap();
    let copia = Arc::clone(&t);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    // O `esperar_porta` velho: passa, porque a porta ATENDE -- e do vizinho.
    let alvo: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    assert!(
        std::net::TcpStream::connect_timeout(&alvo, std::time::Duration::from_secs(2)).is_ok(),
        "a porta do vizinho nao atendeu"
    );
    let r = pedir(porta, r#""op":"criar_database","database":"loja""#);
    assert!(
        r.escrever().contains("ja existe"),
        "o pedido do teste devia ter caido no vizinho: {}",
        r.escrever()
    );
    assert_eq!(
        t.porta_dos_dados(),
        None,
        "o servidor do teste nao devia ter porta nenhuma: o `bind` dele falhou"
    );
    parar(porta);
}

/// **O conserto, primeira metade.** O apoio novo confere a porta pelo
/// PROPRIO servidor e traz a falha do `escutar`: o mesmo arranque vira erro
/// que nomeia a porta, em vez de conversa cruzada calada.
///
/// Defeito reposto (guarda `apoio-engole-a-falha-do-bind`): `tentar_no_ar`
/// volta a so esperar a porta atender, e devolve `Ok` -- este teste cai.
#[test]
fn o_apoio_recusa_a_porta_que_nao_e_do_servidor() {
    let dv = DirTemp::novo("porta-tomada-vizinho-2");
    let (_v, porta) = vizinho(&dv);

    let dt = DirTemp::novo("porta-tomada-teste-2");
    let t = Servidor::novo(config(&dt, &format!("127.0.0.1:{porta}"))).unwrap();
    let e = comum::tentar_no_ar(&t, None, porta)
        .expect_err("o apoio aceitou uma porta que o servidor nao ligou");
    assert!(
        e.contains(&porta.to_string()) && e.contains("nao consegui escutar"),
        "o erro nao nomeia a porta nem a falha do bind: {e}"
    );
    parar(porta);
}

/// **O conserto, segunda metade: por construcao.** O ouvinte reservado fica
/// PRESO ate o servidor recebe-lo, entao o numero nunca volta ao sistema e
/// dois servidores reservados nunca dividem porta -- e cada um ATENDE na sua
/// (o pedido que um recebe e dele: o segundo cria `loja` sem ouvir «ja
/// existe»).
#[test]
fn o_ouvinte_reservado_e_o_que_o_servidor_escuta() {
    let da = DirTemp::novo("porta-tomada-reservado-a");
    let db = DirTemp::novo("porta-tomada-reservado-b");
    let (oa, pa) = comum::ouvinte_reservado();
    let (ob, pb) = comum::ouvinte_reservado();
    assert_ne!(pa, pb, "dois ouvintes vivos receberam o mesmo numero");
    let a = Servidor::novo(config(&da, &format!("127.0.0.1:{pa}"))).unwrap();
    let b = Servidor::novo(config(&db, &format!("127.0.0.1:{pb}"))).unwrap();
    assert_eq!(comum::no_ar_no_ouvinte(&a, oa), pa);
    assert_eq!(comum::no_ar_no_ouvinte(&b, ob), pb);
    for p in [pa, pb] {
        let r = pedir(p, r#""op":"criar_database","database":"loja""#);
        assert!(
            r.booleano_ou("ok", false),
            "o servidor da porta {p} nao era o do teste: {}",
            r.escrever()
        );
        parar(p);
    }
}

//! A reserva do `BULKINSERT` nao se contorna pelo lado B de uma junção --
//! provado PELO SOQUETE, com o servidor de pe e duas ligacoes (pedido 322).
//!
//! A promessa escrita do `BULKINSERT` (`carga.rs`) e «ninguem mais mexendo
//! naquela tabela enquanto ela entra». O portao da carga lia um campo so, o
//! `"tabela"`, e o `juntar` guarda as duas tabelas em `a.tabela` e `b.tabela`:
//! bastava pedir a tabela em carga como lado B para le-la no meio da carga.
//!
//! Por que pelo soquete, e nao so pela unidade: a reserva e amarrada a
//! LIGACAO, e quem e a «outra ligacao» quem decide e o laco de conexao do
//! servidor, nao o teste. A unidade (`testes_bulkinsert` no `servidor.rs`)
//! inventa o numero da ligacao; aqui ele nasce do `accept`, e a queda que
//! solta a reserva e um soquete fechado de verdade.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "carga-pelo-lado-b";

/// Porta 0 e a real lida de volta pelo proprio servidor (pedido 401).
fn subir(base: &Path) -> (Arc<Servidor>, u16) {
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "cifra_fio": {{ "exigir": false }},
              "recursos": {{ "durabilidade": "sistema" }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

/// Uma ligacao que FICA aberta: a reserva vive enquanto ela vive.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let r = self.pedir(corpo);
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
        r
    }

    /// Fecha os DOIS descritores: o `try_clone` e outro fd, e fechar so um
    /// deixaria o servidor sem ver o fim da conexao -- o engano que o teste do
    /// `BULKINSERT` em Python ja pagou.
    fn fechar(self) {
        let _ = self.escrita.shutdown(std::net::Shutdown::Both);
        drop(self.escrita);
        drop(self.leitor);
    }
}

/// `estoque` (a que entra em carga) e `produtos` (a vizinha), as duas com
/// uma linha de `id` 1: a junção, quando passa, devolve uma linha.
fn terreno(porta: u16) {
    let mut l = Ligacao::nova(porta);
    l.exigir(r#""op":"criar_database","database":"loja""#);
    for t in ["estoque", "produtos"] {
        l.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{t}",
               "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
               "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
        l.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"{t}","linha":{{"id":1}}"#
        ));
    }
    l.fechar();
}

const RESERVA: &str = r#""op":"bulkinsert","database":"loja","tabela":"estoque","ligado":true"#;
const JUNTA_PELO_B: &str = r#""op":"juntar","database":"loja",
    "a":{"tabela":"produtos","chave":"id"},"b":{"tabela":"estoque","chave":"id"}"#;

fn exigir_em_carga(r: &Json, corpo: &str) {
    assert_eq!(
        r.texto_ou("nome", ""),
        "EM_CARGA",
        "a tabela em carga foi alcancada por {corpo} -> {}",
        r.escrever()
    );
    assert!(
        r.texto_ou("erro", "").contains("loja.estoque"),
        "o recado nao diz qual tabela: {}",
        r.escrever()
    );
}

/// O caso do pedido: reservar `estoque` numa ligacao e, de outra, pedir
/// `juntar` com `b.tabela = estoque`.
#[test]
fn o_lado_b_do_juntar_nao_le_a_tabela_em_carga() {
    let dir = DirTemp::novo("carga-lado-b");
    let (_s, porta) = subir(&dir);
    terreno(porta);

    let mut carregador = Ligacao::nova(porta);
    carregador.exigir(RESERVA);

    let mut outro = Ligacao::nova(porta);
    let r = outro.pedir(JUNTA_PELO_B);
    exigir_em_carga(&r, JUNTA_PELO_B);
}

/// A familia inteira de quem esconde a tabela do campo que o portao lia.
#[test]
fn diferencas_unir_pivotar_e_a_copia_tambem_param() {
    let dir = DirTemp::novo("carga-familia");
    let (_s, porta) = subir(&dir);
    terreno(porta);

    let mut carregador = Ligacao::nova(porta);
    carregador.exigir(RESERVA);

    // Junta TODAS as que passaram antes de reprovar: parar na primeira
    // esconderia as outras, e cada operacao e uma porta dos fundos propria.
    let mut passaram = Vec::new();
    let mut outro = Ligacao::nova(porta);
    for corpo in [
        r#""op":"diferencas","database":"loja","a":"produtos","b":"estoque","indice":"porId""#,
        r#""op":"unir","database":"loja","tabelas":["produtos","estoque"]"#,
        r#""op":"pivotar","database":"loja","tabela":"produtos",
           "juntar":[{"tabela":"estoque","coluna":"id","prefixo":"e","chave":"id"}],
           "linhas":[{"campo":"e.id"}],"colunas":[{"campo":"id"}],"agregador":"contagem""#,
        r#""op":"duplicar_tabela","database":"loja","tabela":"produtos","destino":"estoque""#,
    ] {
        let r = outro.pedir(corpo);
        let barrou =
            r.texto_ou("nome", "") == "EM_CARGA" && r.texto_ou("erro", "").contains("loja.estoque");
        if !barrou {
            passaram.push(format!("{} -> {}", r.texto_ou("op", "?"), r.escrever()));
        }
    }
    assert!(
        passaram.is_empty(),
        "a tabela em carga foi alcancada por {} operacao(oes):\n{}",
        passaram.len(),
        passaram.join("\n")
    );
}

/// O comportamento VELHO, pelo mesmo soquete: sem reserva a junção passa;
/// quem reservou continua juntando; e quando a ligacao do carregador CAI, o
/// outro volta a juntar. Sem este teste, um portao que recusasse toda junção
/// passaria com louvor nos dois de cima.
#[test]
fn sem_reserva_e_depois_da_queda_o_juntar_continua() {
    let dir = DirTemp::novo("carga-velho");
    let (_s, porta) = subir(&dir);
    terreno(porta);

    let mut outro = Ligacao::nova(porta);
    let r = outro.exigir(JUNTA_PELO_B);
    let corpo = r.campo("resultado").cloned().unwrap_or(Json::Nulo);
    assert_eq!(
        corpo.inteiro_ou("quantas", -1),
        1,
        "a junção sem reserva nao devolveu a linha: {}",
        r.escrever()
    );

    let mut carregador = Ligacao::nova(porta);
    carregador.exigir(RESERVA);
    carregador.exigir(JUNTA_PELO_B);
    carregador.fechar();

    // A soltura roda na saida do laco da conexao, que e de outra thread:
    // espera-se por ela com teto, e nao por um `sleep` que adivinha.
    let ate = Instant::now() + Duration::from_secs(10);
    loop {
        let r = outro.pedir(JUNTA_PELO_B);
        if r.booleano_ou("ok", false) {
            break;
        }
        assert!(
            Instant::now() < ate,
            "a queda da ligacao nao soltou a reserva em 10 s: {}",
            r.escrever()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

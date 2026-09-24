//! A cifra do fio, provada PELO SOQUETE.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! Os testes de `phxsql_core::fio` provam a matematica e a maquina de estados;
//! eles nao provam nada sobre o LACO DE CONEXAO -- se o servidor troca mesmo de
//! canal, se um cliente que nunca ouviu falar do aperto continua sendo
//! atendido, se `exigir` recusa e se a conexao cai quando o aperto falha. Isso
//! depende do sistema operacional e do laco, e a licao do `BULKINSERT` foi
//! exatamente essa: teste de unidade nao prova queda de conexao, soquete prova.
//!
//! E a armadilha ja paga aqui tem irma: nada neste arquivo segura o descritor
//! por fora do soquete. Quando um teste precisa que o servidor VEJA o fim da
//! conexao, ele solta o `TcpStream` inteiro.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido, Tipo};
use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";

/// Uma pasta so deste teste.
///
/// O nome carrega um numero de serie, e nao um relogio: dois testes que rodam
/// no mesmo instante pegariam o mesmo carimbo, e um apagaria a base do outro
/// -- que foi exatamente o que aconteceu na primeira versao deste arquivo.
/// A serie hoje vem do contador do proprio `DirTemp`, que tambem apaga a
/// pasta no `Drop`.
fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("fio-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

/// Sobe a partir de um `config.json` DE VERDADE, com a secao `cifra_fio`
/// exatamente como o texto pedir -- vazio = sem a secao, que e o arquivo de
/// ontem.
///
/// Existe porque montar o `Config` na mao ESCREVE os campos, e um teste que
/// escreve o campo nao pode provar o padrao dele.
fn subir_do_arquivo(base: &std::path::Path, secao: &str) -> (Arc<Servidor>, u16) {
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "web": {{ "ligado": false }}{secao}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    no_ar(Servidor::novo(Config::ler(&caminho).unwrap()).unwrap())
}

fn subir(base: &std::path::Path, exigir: bool, ligada: bool) -> (Arc<Servidor>, u16) {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        // O caminho existe para a chave do fio nascer AO LADO dele, e nao no
        // diretorio de onde a bateria por acaso rodou.
        caminho: Some(base.join("config.json")),
        ..Default::default()
    };
    c.web.ligado = false;
    c.cifra_fio.exigir = exigir;
    c.cifra_fio.ligada = ligada;
    no_ar(Servidor::novo(c).unwrap())
}

/// Poe o servidor no ar e espera a porta atender -- por CONDICAO, e nao por
/// tempo fixo: dormir um tempo passa nesta maquina e falha na proxima.
///
/// Le a porta REAL do proprio servidor -- pedido 401: o `bind` do `config`
/// pede porta 0, e so depois do `escutar()` chegar la existe um numero para
/// ler. Nao ha janela de "escolher e torcer" porque nao se escolhe nada.
fn no_ar(s: Arc<Servidor>) -> (Arc<Servidor>, u16) {
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

/// Uma conexao aberta, com PRAZO -- teste que pendura nao reprova, trava.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
    canal: Canal,
}

impl Conexao {
    fn abrir(porta: u16) -> Conexao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        fluxo
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
            canal: Canal::Claro,
        }
    }

    /// O aperto, do jeito que um cliente o faz: uma linha em claro, uma
    /// resposta em claro, e o tunel da linha seguinte em diante.
    fn cifrar(&mut self, pino: Option<[u8; 32]>) -> Result<[u8; 32], String> {
        let (iniciador, m1) = Iniciador::comecar(pino);
        writeln!(
            self.escrita,
            r#"{{"op":"cifrar","e":"{}"}}"#,
            base64::codificar(&m1)
        )
        .map_err(|e| e.to_string())?;
        self.escrita.flush().map_err(|e| e.to_string())?;
        let mut resposta = String::new();
        self.leitor
            .read_line(&mut resposta)
            .map_err(|e| e.to_string())?;
        let j = Json::analisar(&resposta).map_err(|e| e.to_string())?;
        if !j.booleano_ou("ok", false) {
            return Err(resposta);
        }
        let m2 = base64::decodificar(
            j.campo("resultado")
                .map(|r| r.texto_ou("m2", ""))
                .unwrap_or(""),
        )
        .map_err(|e| e.to_string())?;
        let (t, apresentada) = iniciador.terminar(&m2).map_err(|e| e.to_string())?;
        self.canal = Canal::Cifrado(Box::new(t));
        Ok(apresentada)
    }

    fn pedir(&mut self, linha: &str) -> String {
        self.canal.escrever(&mut self.escrita, linha).unwrap();
        match self.canal.ler(&mut self.leitor) {
            Ok(Recebido::Linha(l)) => l,
            Ok(Recebido::Fim) => panic!("o servidor encerrou no meio: {linha}"),
            Err(e) => panic!("erro lendo a resposta de {linha}: {e}"),
        }
    }

    /// A linha crua, sem passar pelo canal -- para os testes que MEXEM no fio.
    fn mandar_cru(&mut self, linha: &str) {
        writeln!(self.escrita, "{linha}").unwrap();
        self.escrita.flush().unwrap();
    }

    fn ler_cru(&mut self) -> String {
        let mut l = String::new();
        let _ = self.leitor.read_line(&mut l);
        l
    }
}

/// Um ciclo de vida inteiro de dado, para o teste medir TRABALHO e nao so
/// resposta: criar, inserir e ler de volta.
fn exercitar(c: &mut Conexao, marca: &str) {
    let base = format!("fio_{marca}");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"criar_database","database":"{base}"}}"#
    ));
    assert!(r.contains("\"ok\":true"), "criar_database: {r}");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"criar_tabela","database":"{base}","tabela":"cidades","colunas":[{{"nome":"n","tipo":"Int8"}},{{"nome":"nome","tipo":"Str(40)"}}]}}"#
    ));
    assert!(r.contains("\"ok\":true"), "criar_tabela: {r}");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"inserir","database":"{base}","tabela":"cidades","linha":{{"n":1,"nome":"Blumenau"}}}}"#
    ));
    assert!(r.contains("\"ok\":true"), "inserir: {r}");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"varrer","database":"{base}","tabela":"cidades"}}"#
    ));
    assert!(r.contains("Blumenau"), "varrer: {r}");
}

// ---------------------------------------------------------------------------
// O teste que mais importa
// ---------------------------------------------------------------------------

/// **O padrao, e este teste MUDOU DE LADO em 18/09/2026 (pedido 370).**
///
/// Ate ontem ele se chamava `cliente_sem_cifra_continua_como_antes` e travava
/// a regra da casa: guarda nova entra PEDIDA, entao um `config.json` sem a
/// secao `cifra_fio` deixava o cliente velho gravar e ler como sempre. **Quem
/// revogou isso foi o dono**, em palavra propria -- *«A comunicacao deve
/// obrigatoriamente ser cifrada»* --, e o teste nao foi apagado: ele mudou de
/// lado, e o lado novo esta escrito aqui. Teste que some leva a garantia
/// junto, e a garantia continua sendo a MESMA pergunta: «o que acontece com
/// quem so trocou o binario?»
///
/// A resposta nova, sem enfeite: **ele para de entrar, e o servidor lhe diz o
/// que fazer.** A recusa e uma linha JSON em claro com erro nomeado, que um
/// cliente velho sabe exibir -- e nao um silencio, que e a diferenca entre uma
/// mudanca dura e uma mudanca cruel.
///
/// # Por que ele sobe de um `config.json` sem a secao, e nao de um `Config`
///
/// Porque a primeira versao deste teste montava o `Config` na mao e escrevia
/// `cifra_fio.exigir = false` -- e ai ele parava de provar o padrao. O
/// executor das guardas mediu: com o padrao trocado, ele continuava VERDE,
/// porque ele mesmo desfazia a troca. Teste que passa por engano e pior que
/// teste que falta, e a razao vale igual agora que o padrao e o outro.
#[test]
fn o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo() {
    let base = pasta("velho");
    let (_s, porta) = subir_do_arquivo(&base, "");

    // A leitura do padrao DE VERDADE, e nao de um campo que este teste
    // escreveu: o arquivo nao fala de `cifra_fio` em lugar nenhum.
    let lido = Config::ler(base.join("config.json")).unwrap();
    assert!(
        lido.cifra_fio.exigir,
        "um config.json sem a secao cifra_fio NAO exige o tunel: a ordem do \
         dono de 18/09/2026 voltou atras sem ninguem ter pedido"
    );
    assert!(
        lido.estranhas.is_empty(),
        "o config de ontem virou aviso de campo estranho: {:?}",
        lido.estranhas
    );

    let mut c = Conexao::abrir(porta);
    c.mandar_cru(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    let r = c.ler_cru();
    // O DANO, e nao so o veredito: o ping nao pode ter sido atendido.
    assert!(
        !r.contains("\"ok\":true"),
        "o cliente velho foi atendido em claro: {r}"
    );
    // E a recusa tem de DIZER o que fazer -- um "acesso negado" seco mandaria
    // procurar a permissao errada.
    assert!(r.contains("cifrar"), "a recusa nao diz o que fazer: {r}");
    assert!(
        c.ler_cru().is_empty(),
        "a conexao continuou aberta em claro"
    );

    // E o servidor NAO escreveu a chave do fio: ninguem pediu o aperto.
    assert!(
        !base.join("chave-do-fio.hex").exists(),
        "o servidor criou a chave do fio sem ninguem ter pedido o aperto"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// **O ESCAPE ESCRITO.** Com `"exigir": false` no `config.json`, o cliente em
/// claro continua entrando -- e continua entrando no dia em que o padrao
/// virar.
///
/// # Por que este teste existe ao lado do de cima, e nao no lugar dele
///
/// Os dois provam coisas diferentes, e so um deles sobrevive a troca do
/// padrao. O `cliente_sem_cifra_continua_como_antes` prova a OMISSAO: arquivo
/// sem a secao, cliente velho entra. Este prova a ESCOLHA ESCRITA: alguem
/// digitou `false`, e o motor obedece. Hoje os dois passam; no dia em que
/// `exigir` nascer `true` (ordem do dono, 18/09/2026), o primeiro muda de
/// significado e este fica exatamente como esta -- e e por isso que ele entra
/// ANTES da troca, e nao depois: guarda escrita depois do fato nao prova que o
/// fato nao quebrou nada.
///
/// **Aconteceu em 18/09/2026, e a previsao se cumpriu letra por letra:** o
/// primeiro virou `o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo`
/// e este nao mudou uma linha. Ele e o que fica IGUAL dos dois lados da virada.
///
/// E o padrao e o mesmo da chave conferida: quem NAO quer a guarda escreve o
/// `false`, em vez de ganha-la por esquecimento.
#[test]
fn o_escape_escrito_deixa_o_cliente_em_claro_entrar() {
    let base = pasta("escape");
    let (_s, porta) = subir_do_arquivo(&base, r#","cifra_fio": { "exigir": false }"#);

    let lido = Config::ler(base.join("config.json")).unwrap();
    assert!(
        !lido.cifra_fio.exigir,
        "o `false` escrito no arquivo foi ignorado: o escape nao existe"
    );
    assert!(lido.estranhas.is_empty(), "{:?}", lido.estranhas);

    let mut c = Conexao::abrir(porta);
    let r = c.pedir(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    assert!(
        r.contains("\"ok\":true"),
        "com o escape escrito, o cliente em claro foi recusado: {r}"
    );
    exercitar(&mut c, "escape");
    let _ = std::fs::remove_dir_all(&base);
}

/// O outro sentido do escape: `"exigir": true` escrito RECUSA o texto claro,
/// e a recusa vem do arquivo -- nao de um campo que o teste escreveu na mao.
///
/// Irmao do `exigir_recusa_texto_claro_e_deixa_o_tunel_passar`, que monta o
/// `Config` em memoria. A diferenca importa: o caminho do arquivo passa pelo
/// `CifraFio::de_json`, e e ele que decide o que a omissao vale.
#[test]
fn exigir_escrito_no_arquivo_recusa_o_texto_claro() {
    let base = pasta("escrito-true");
    let (_s, porta) = subir_do_arquivo(&base, r#","cifra_fio": { "exigir": true }"#);

    let lido = Config::ler(base.join("config.json")).unwrap();
    assert!(lido.cifra_fio.exigir);

    let mut c = Conexao::abrir(porta);
    c.mandar_cru(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    let r = c.ler_cru();
    assert!(r.contains("\"ok\":false"), "o ping em claro passou: {r}");
    assert!(
        r.contains("cifrar"),
        "a recusa tem de dizer o que fazer: {r}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// O tunel
// ---------------------------------------------------------------------------

/// O aperto fecha, e o mesmo trabalho acontece por dentro dele.
#[test]
fn com_o_aperto_o_mesmo_trabalho_acontece_cifrado() {
    let base = pasta("tunel");
    let (_s, porta) = subir(&base, false, true);

    let mut c = Conexao::abrir(porta);
    let apresentada = c.cifrar(None).expect("o aperto tinha de fechar");
    exercitar(&mut c, "tunel");

    // A chave nasceu no arquivo, ao lado do config, e so agora.
    let arquivo = base.join("chave-do-fio.hex");
    assert!(arquivo.exists(), "a estatica nao foi gravada");
    let hex = std::fs::read_to_string(&arquivo).unwrap();
    let bytes = phxsql_core::hash::de_hex(hex.trim()).unwrap();
    let mut privada = [0u8; 32];
    privada.copy_from_slice(&bytes);
    assert_eq!(
        phxsql_core::x25519::chave_publica(&privada),
        apresentada,
        "o servidor apresentou uma chave que nao e a do arquivo"
    );

    // E o PINO certo passa numa conexao nova -- que e como um cliente
    // configurado se protege de quem esta no meio.
    let mut c2 = Conexao::abrir(porta);
    assert_eq!(c2.cifrar(Some(apresentada)).unwrap(), apresentada);

    // O pino ERRADO derruba, e derruba no cliente: ele nao chega a mandar
    // pedido nenhum por um fio que nao provou ser do servidor.
    let outra = phxsql_core::x25519::chave_publica(&phxsql_core::x25519::gerar_privada());
    let mut c3 = Conexao::abrir(porta);
    assert!(c3.cifrar(Some(outra)).is_err(), "o pino errado passou");
    let _ = std::fs::remove_dir_all(&base);
}

/// `cifra_fio.ligada: false` recusa o aperto -- e a conexao morre, em vez de
/// seguir em claro fingindo que deu certo.
#[test]
fn servidor_com_a_cifra_desligada_recusa_o_aperto() {
    let base = pasta("desligada");
    let (_s, porta) = subir(&base, false, false);

    let mut c = Conexao::abrir(porta);
    let erro = c
        .cifrar(None)
        .expect_err("o aperto passou com ela desligada");
    assert!(erro.contains("\"ok\":false"), "{erro}");
    assert!(
        !base.join("chave-do-fio.hex").exists(),
        "recusou o aperto e mesmo assim criou a chave"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// O rebaixamento
// ---------------------------------------------------------------------------

/// Com `exigir` ligado, texto claro e RECUSADO -- e a recusa e uma linha JSON
/// que um cliente velho sabe exibir, e nao um silencio.
#[test]
fn exigir_recusa_texto_claro_e_deixa_o_tunel_passar() {
    let base = pasta("exigir");
    let (_s, porta) = subir(&base, true, true);

    let mut c = Conexao::abrir(porta);
    c.mandar_cru(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    let r = c.ler_cru();
    assert!(r.contains("\"ok\":false"), "o ping em claro passou: {r}");
    assert!(
        r.contains("cifrar"),
        "a recusa tem de dizer o que fazer: {r}"
    );
    // E a conexao FECHA: a proxima leitura nao traz outra resposta.
    assert!(
        c.ler_cru().is_empty(),
        "a conexao continuou aberta em claro"
    );

    // O mesmo servidor, pelo tunel, trabalha normalmente.
    let mut c2 = Conexao::abrir(porta);
    c2.cifrar(None).expect("o aperto tinha de fechar");
    exercitar(&mut c2, "exigir");
    let _ = std::fs::remove_dir_all(&base);
}

/// **A diretiva da conexao de DADOS diz a verdade dela** -- a outra metade da
/// prova do pedido 370.
///
/// `encryption_exigida` mora em `diretivas_da_conexao`, cuja documentacao diz
/// «o que e verdade DESTA conexao», e ate 18/09/2026 ele publicava o campo do
/// `config.json` para quem quer que perguntasse: a conexao HTTP em claro
/// recebia `true` (a metade que `cifra-das-portas-http.rs` prova). Aqui se
/// prova o sentido CONTRARIO, que e o que impede o conserto de virar um `false`
/// constante: dentro do tunel, com a exigencia ligada, a resposta e `true` --
/// e ela e verdade, porque o texto claro foi recusado nesta porta.
///
/// Sem esta metade, trocar o campo por `false` seco passaria verde.
#[test]
fn dentro_do_tunel_a_diretiva_confirma_a_cifra_desta_conexao() {
    let base = pasta("diretiva-do-tunel");
    let (_s, porta) = subir(&base, true, true);

    let mut c = Conexao::abrir(porta);
    c.cifrar(None).expect("o aperto tinha de fechar");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"diretivas","escopo":"conexao"}}"#
    ));
    let j = Json::analisar(&r).unwrap();
    let d = j.campo("resultado").unwrap_or(&j);
    assert_eq!(d.texto_ou("via", ""), "dados", "{r}");
    assert!(
        d.booleano_ou("encryption_exigida", false),
        "a conexao de dados sob exigencia tinha de confirmar: {r}"
    );
    assert!(
        d.booleano_ou("encryption_neste_canal", false),
        "esta conexao ESTA dentro do tunel e a diretiva nao diz: {r}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// Truncamento, repeticao e despedida
// ---------------------------------------------------------------------------

/// Registro repetido nao passa: o servidor fecha a conexao.
///
/// E a prova do contador pelo SOQUETE -- o teste de unidade prova que o
/// `abrir` recusa; este prova que o laco do servidor age sobre a recusa em vez
/// de engolir e seguir.
#[test]
fn registro_repetido_derruba_a_conexao() {
    let base = pasta("repetido");
    let (_s, porta) = subir(&base, false, true);

    let mut c = Conexao::abrir(porta);
    c.cifrar(None).unwrap();

    // Sela o mesmo pedido duas vezes com o mesmo contador: a segunda copia e
    // literalmente a primeira, gravada e reenviada.
    let pedido = format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#);
    let registro = match &mut c.canal {
        Canal::Cifrado(t) => t.selar(Tipo::Pedido, pedido.as_bytes()).unwrap(),
        Canal::Claro => unreachable!(),
    };
    c.mandar_cru(&registro);
    assert!(
        !c.ler_cru().is_empty(),
        "o primeiro registro tinha de valer"
    );
    c.mandar_cru(&registro);
    assert!(
        c.ler_cru().is_empty(),
        "o registro repetido foi atendido: o contador nao esta valendo no laco"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Registro mexido nao passa: o servidor fecha em vez de responder.
#[test]
fn registro_mexido_derruba_a_conexao() {
    let base = pasta("mexido");
    let (_s, porta) = subir(&base, false, true);

    let mut c = Conexao::abrir(porta);
    c.cifrar(None).unwrap();
    let pedido = format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#);
    let registro = match &mut c.canal {
        Canal::Cifrado(t) => t.selar(Tipo::Pedido, pedido.as_bytes()).unwrap(),
        Canal::Claro => unreachable!(),
    };
    let mut bytes = base64::decodificar(&registro).unwrap();
    bytes[0] ^= 1;
    c.mandar_cru(&base64::codificar(&bytes));
    assert!(
        c.ler_cru().is_empty(),
        "um registro adulterado foi respondido"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// **Fim de conversa e fio cortado sao vereditos diferentes** -- e a diferenca
/// aparece no `acessos.log`, que e onde alguem a procuraria.
///
/// O corte e feito soltando o `TcpStream` INTEIRO. Se um descritor irmao
/// ficasse aberto (a armadilha do `socket.makefile()` do Python, ja paga aqui),
/// o servidor nunca veria o fim da conexao e este teste passaria por engano --
/// que e pior que teste que falta.
#[test]
fn fio_cortado_vira_erro_e_despedida_nao() {
    let base = pasta("corte");
    let (_s, porta) = subir(&base, false, true);

    // (a) o corte: um pedido, e o soquete morre sem despedida.
    {
        let mut c = Conexao::abrir(porta);
        c.cifrar(None).unwrap();
        let r = c.pedir(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
        assert!(r.contains("\"ok\":true"), "{r}");
        // Solta os DOIS descritores desta conexao. Nada segura o fd.
        drop(c);
    }

    // (b) a despedida: o mesmo pedido, e o `FIM` antes de fechar.
    {
        let mut c = Conexao::abrir(porta);
        c.cifrar(None).unwrap();
        let r = c.pedir(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
        assert!(r.contains("\"ok\":true"), "{r}");
        c.canal.despedir(&mut c.escrita).unwrap();
        drop(c);
    }

    // O log e escrito na thread da conexao; esperar por CONDICAO, e nao por
    // tempo fixo, senao o teste passa nesta maquina e falha na proxima.
    let log = base.join("acessos.log");
    let ate = Instant::now() + Duration::from_secs(5);
    let mut texto = String::new();
    while Instant::now() < ate {
        texto = std::fs::read_to_string(&log).unwrap_or_default();
        if texto.contains("\"op\":\"fio\"") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let cortes: Vec<&str> = texto
        .lines()
        .filter(|l| l.contains("\"op\":\"fio\""))
        .collect();
    assert_eq!(
        cortes.len(),
        1,
        "o log tinha de ter UM corte -- o (a) -- e nao {}: {texto}",
        cortes.len()
    );
    assert!(
        cortes[0].contains("cortado"),
        "o corte tem de dizer o que foi: {}",
        cortes[0]
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// A prova de que o tunel esconde: o token de servico nao aparece nos bytes
/// que passam pelo fio.
///
/// Ele e lido do SOQUETE, e nao de uma estrutura em memoria -- o que se quer
/// saber e o que um `tcpdump` veria.
#[test]
fn o_token_nao_aparece_nos_bytes_do_fio() {
    let base = pasta("escuta");
    let (_s, porta) = subir(&base, false, true);

    let mut c = Conexao::abrir(porta);
    c.cifrar(None).unwrap();
    let pedido = format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#);
    let registro = match &mut c.canal {
        Canal::Cifrado(t) => t.selar(Tipo::Pedido, pedido.as_bytes()).unwrap(),
        Canal::Claro => unreachable!(),
    };
    assert!(
        !registro.contains("token") && !registro.contains("ping"),
        "o registro no fio mostra o pedido: {registro}"
    );
    // E a resposta que volta tambem nao e legivel.
    c.mandar_cru(&registro);
    let resposta = c.ler_cru();
    assert!(!resposta.is_empty(), "o servidor nao respondeu");
    assert!(
        !resposta.contains("\"ok\":true"),
        "a resposta voltou em claro dentro do tunel: {resposta}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

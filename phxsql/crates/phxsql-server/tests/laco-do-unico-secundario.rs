//! O conflito de unicidade PARA o par, marcado -- e o `replicacao_pular` o
//! solta. Provado PELO SOQUETE, com dois servidores no ar (pedido 292).
//!
//! O casamento entre servidores usa UMA chave (`bidirecional::chave_unica`), e
//! a unicidade dos OUTROS indices continua sendo conferida na gravacao -- e
//! esta certo que continue, porque violacao de indice unico nao se cura quando
//! o proximo lote chega, ao contrario da chave estrangeira. Com primaria
//! `porId` e um secundario `porEmail`, o evento do outro lado com um e-mail
//! que ja existe aqui e recusado -- e a recusa subia pelo `?` do laco: a
//! posicao consumida nunca andava, e o MESMO lote voltava para sempre. Nao e
//! uma linha perdida: e o par de servidores parado, sem ninguem saber.
//!
//! A forma que ficou e a dos motores maduros, medida em
//! `docs/propostas/regua-dos-motores-decisoes-289-294-2026-09-17.md` §292:
//! **nenhum dos tres recusa a tabela**, e os tres fazem o conflito APARECER.
//! O PostgreSQL para a assinatura e grita com o indice, o valor da chave e as
//! duas linhas, e oferece `disable_on_error` e `ALTER SUBSCRIPTION ... SKIP`.
//! Entao aqui: a tabela continua nascendo e replicando, no conflito o par
//! PARA naquela tabela -- marcado, contado e gritado --, e a saida e humana.
//!
//! Teste unitario nao prova isto, e o motivo e o de sempre nesta casa: o que
//! se quer saber nao e se `aplicar_por_chave` devolve `Ok` ou `Err`, e se a
//! PROXIMA linha -- a que nada tem a ver com o conflito -- chega do outro
//! lado. Isso so os dois lacos rodando de verdade mostram. A unidade esta em
//! `servidor.rs`, nos modulos `testes_da_recusa_por_unicidade` e
//! `testes_do_pular_manual`.
//!
//! **Por que so um lado puxa.** O defeito mora em quem PUXA, e os dois lados
//! de um multi puxam -- mas montar o conflito com os dois puxando e uma
//! corrida: o parceiro receberia a linha daqui antes de gravar a dele, e o
//! `inserir` do teste e que seria recusado. Entao o parceiro fica sem origem
//! configurada, e o cenario nasce igual toda vez.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::cofre;

/// O cofre e global ao PROCESSO, e o cenario marcado precisa dele ligado
/// (pedido 616: o bidirecional sem cofre recusa a tabela com coluna marcada,
/// e o conflito que se quer medir nem chegaria a acontecer). Os outros
/// cenarios leem com a trava compartilhada, e o marcado a toma exclusiva:
/// ligar e desligar o cofre no meio de um vizinho mudaria a marca de
/// transacao e a trilha dele de claro para cifrado.
static COFRE: RwLock<()> = RwLock::new(());

/// Desliga o cofre ao sair, inclusive no `panic!` -- o vizinho que entra
/// depois nao pode herdar o cofre ligado.
struct CofreLigado;
impl Drop for CofreLigado {
    fn drop(&mut self) {
        cofre::desligar();
    }
}

const TOKEN: &str = "unico-secundario";
/// Quanto esperar por um laco que roda a cada segundo. O mesmo teto dos
/// outros testes de replicacao desta pasta.
const ESPERA: Duration = Duration::from_secs(20);

/// A porta de cada servidor nasce de `comum::ouvinte_reservado()` e fica
/// PRESA ate o servidor recebe-la (pedidos 352 e 401). `terreno_com` precisa
/// escrever a porta do parceiro no `Origem` de `alfa` ANTES de `beta` existir
/// -- e o cenario que este arquivo mede, o dado local nascendo com o parceiro
/// fechado --, e por isso nao da para pedir porta 0 e ler de volta. O padrao
/// de antes sorteava e SOLTAVA o numero; quando um vizinho do mesmo binario o
/// pegava, o `bind` de `beta` falhava calado e o teste conversava com o
/// servidor do vizinho («database loja ja existe», medido). Com o ouvinte
/// preso, o parceiro «fechado» continua fechado para o laco de `alfa` no que
/// importa: ninguem ACEITA a conexao ate `beta` subir.
fn config_base(base: &std::path::Path, porta: u16, id: &str) -> Config {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e o conflito de unicidade no papel multi. Sem esta
    // linha a recusa lida aqui seria a da cifra, e a prova passaria a medir
    // o portao errado.
    c.cifra_fio.exigir = false;
    // A ESTATICA MORA DENTRO DO DIR DO TESTE, e nao no cwd. `exigir = false`
    // acima nao basta: o cenario `marcado` sobe a origem com `cifra: true`
    // (pedido 342), o parceiro RESPONDE o aperto, e `estatica_do_fio` cria o
    // arquivo. Como este `Config` nasce em memoria, `caminho` e `None`, e o
    // padrao relativo `chave-do-fio.hex` cai no diretorio de onde o binario
    // de teste rodou -- que no `cargo test` e a raiz do crate. Foi assim que
    // um `crates/phxsql-server/chave-do-fio.hex` de 65 bytes sobreviveu a
    // corrida e apareceu na lista de um commit (pedido 402). Dentro do
    // `DirTemp`, a chave morre com o teste.
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = id.into();
    // No multi a chave mora dentro da imagem, dos dois lados.
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config, ouvinte: TcpListener) -> Arc<Servidor> {
    let s = Servidor::novo(c).unwrap();
    comum::no_ar_no_ouvinte(&s, ouvinte);
    s
}

/// Um pedido pela porta de dados; devolve o JSON da resposta inteira.
fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

/// A tabela do cenario: primaria `porId` e um unico SECUNDARIO `porEmail` --
/// chave primaria + e-mail unico e a modelagem mais comum que existe.
///
/// `marcado` liga a marca de DADO PESSOAL no e-mail, que e o outro cenario
/// desta bateria: o grito do conflito nao pode publicar o valor da coluna
/// marcada, nem na linha nem na chave.
fn criar_clientes(porta: u16, marcado: bool) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    let pessoal = if marcado {
        r#","dado_pessoal":"pessoal""#
    } else {
        ""
    };
    exigir(
        porta,
        &format!(
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                      {{"nome":"email","tipo":"Str(30)"{pessoal}}}],
           "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}},
                      {{"nome":"porEmail","colunas":["email"],"unico":true}}]"#
        ),
    );
}

fn inserir(porta: u16, id: i64, email: &str) {
    exigir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{{"id":{id},"email":"{email}"}}"#
        ),
    );
}

fn linhas(porta: u16) -> Vec<(i64, String)> {
    exigir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .map(|l| (l.inteiro_ou("id", -1), l.texto_ou("email", "").to_string()))
    .collect()
}

fn estado(porta: u16) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
}

/// Quantas recusas por unicidade este servidor ja contou em `loja/clientes`.
fn recusas(porta: u16) -> i64 {
    estado(porta)
        .campo("recusas_por_unicidade")
        .and_then(|r| r.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(0)
}

/// Ate onde o laco ja consumiu o diario do parceiro. E o numero que ficava
/// parado: `desde = lote.ate` so roda depois do lote inteiro aplicado.
fn posicao(porta: u16) -> i64 {
    estado(porta)
        .campo("origens")
        .and_then(|o| o.campo("parceiro"))
        .and_then(|p| p.campo("posicoes"))
        .and_then(|p| p.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(-1)
}

fn esperar<F: Fn() -> bool>(o_que: &str, f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

/// Sobe os dois: o que PUXA (com o parceiro como origem) e o parceiro, sem
/// origem nenhuma. O que puxa sobe primeiro, de proposito -- a porta do
/// parceiro ainda esta fechada, o laco tenta, falha e volta a tentar, e e
/// nessa janela que o dado local deste lado nasce sem corrida.
fn terreno(nome: &str) -> (Arc<Servidor>, Arc<Servidor>, u16, u16, DirTemp, DirTemp) {
    terreno_com(nome, false)
}

fn terreno_com(
    nome: &str,
    marcado: bool,
) -> (Arc<Servidor>, Arc<Servidor>, u16, u16, DirTemp, DirTemp) {
    let dir_a = DirTemp::novo(&format!("unico-secundario-puxa-{nome}"));
    let dir_b = DirTemp::novo(&format!("unico-secundario-parceiro-{nome}"));
    let (ouvinte_a, porta_a) = comum::ouvinte_reservado();
    let (ouvinte_b, porta_b) = comum::ouvinte_reservado();

    let mut c = config_base(&dir_a, porta_a, "alfa");
    c.replicacao.origens = vec![Origem {
        nome: "parceiro".into(),
        host: "127.0.0.1".into(),
        porta: porta_b,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        // O TUNEL segue a MARCA, e nao o gosto do teste: desde o pedido 342 o
        // `op_replicar` recusa servir a imagem de tabela com coluna marcada
        // por conexao em claro. O cenario marcado desta bateria mede o GRITO
        // do conflito, nao o portao da cifra -- entao ele sobe pelo unico
        // caminho que a lei nova deixa de pe, e o cenario sem marca continua
        // em claro exatamente como sempre subiu. As duas linhas juntas sao o
        // ALCANCE do 342 visto pelo soquete: marcada exige, sem marca nao
        // muda.
        cifra: marcado,
        chave_do_fio: String::new(),
        espelho: false,
    }];
    let a = subir(c, ouvinte_a);
    criar_clientes(porta_a, marcado);
    inserir(porta_a, 1, "a@x");

    let b = subir(config_base(&dir_b, porta_b, "beta"), ouvinte_b);
    criar_clientes(porta_b, marcado);
    (a, b, porta_a, porta_b, dir_a, dir_b)
}

/// A parada desta tabela nesta origem, quando ha uma.
fn parada(porta: u16) -> Option<Json> {
    estado(porta)
        .campo("origens")
        .and_then(|o| o.campo("parceiro"))
        .and_then(|p| p.campo("paradas"))
        .and_then(|p| p.campo("loja/clientes"))
        .cloned()
}

/// **Prova real.** O parceiro cria a linha 2 com o e-mail que a linha 1 daqui
/// ja ocupa: o `porEmail` recusa o evento, a recusa e CONTADA, e a replicacao
/// DESTA tabela neste par PARA -- marcada, com o indice, o valor da chave e as
/// duas linhas em `replicacao_estado`. A posicao NAO anda, que e o que permite
/// pular exatamente aquele evento depois.
///
/// **Defeito reposto (1)**: trocar o bloco do conflito por `escrita?;` em
/// `aplicar_por_chave` -- a rodada vira `Err`, nada e contado e `paradas` fica
/// vazio; o `esperar` da parada estoura em 20 s.
///
/// **Defeito reposto (2)**: em `alcancar_tabela_bidi`, tirar o `break` do
/// `parou_em` e deixar `desde = lote.ate` correr -- a posicao vai a 1, a
/// parada nunca aparece, e a asercao da posicao zero cai.
#[test]
fn o_conflito_de_unicidade_para_o_par_marcado() {
    let _c = COFRE.read().unwrap_or_else(|e| e.into_inner());
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno("para");
    inserir(porta_b, 2, "a@x");

    esperar("a parada do par", || parada(porta_a).is_some());
    let p = parada(porta_a).unwrap();
    assert_eq!(
        p.texto_ou("motivo", ""),
        "conflito_de_unicidade",
        "o motivo tem de ser CHAVE, nunca frase: {}",
        p.escrever()
    );
    // O conteudo que o PostgreSQL carrega no `conflict=insert_exists`: o
    // indice, o valor da chave e as duas linhas. Sem ele quem opera sabe que
    // parou e nao sabe em que.
    let detalhe = p.texto_ou("detalhe", "").to_string();
    for pedaco in ["porEmail", "a@x", "id=1", "id=2"] {
        assert!(
            detalhe.contains(pedaco),
            "o grito nao carrega {pedaco:?}: {detalhe}"
        );
    }
    assert_eq!(recusas(porta_a), 1, "uma recusa, e uma so");
    assert_eq!(
        posicao(porta_a),
        -1,
        "a posicao andou: o evento que parou o par nao teria mais como ser \
         pulado pela posicao"
    );
    assert_eq!(p.inteiro_ou("posicao", -1), 0, "nao disse ONDE parou");

    // O dado deste lado ficou como estava: a recusa nao apaga nem sobrescreve
    // a linha que ocupa o e-mail.
    assert_eq!(linhas(porta_a), vec![(1, "a@x".to_string())]);

    // **A parada e parada mesmo**: a linha seguinte, que nada tem a ver com o
    // conflito, NAO chega enquanto ninguem soltar o par. E a metade que um
    // contador sozinho nunca provou.
    inserir(porta_b, 3, "c@x");
    let ate = Instant::now() + Duration::from_secs(4);
    while Instant::now() < ate {
        assert!(
            !linhas(porta_a).iter().any(|(id, _)| *id == 3),
            "a linha 3 passou por cima de um par declarado parado"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // E o grito nao se repete a cada rodada: uma parada, uma marca.
    assert_eq!(recusas(porta_a), 1, "o conflito foi reapresentado em laco");
}

/// **Prova real da saida manual.** Com o par parado, `replicacao_pular` anda a
/// posicao para DEPOIS do evento que parou tudo, a marca sai, e a linha
/// seguinte -- que nada tem a ver com o conflito -- finalmente chega.
///
/// **Defeito reposto**: em `op_replicacao_pular`, nao remover a parada
/// (`estado.paradas.remove`). O portao de `alcancar_tabela_bidi` continua
/// tirando a tabela da rodada, a linha 3 nunca chega, e o `esperar` estoura.
#[test]
fn o_pular_manual_solta_o_par_e_a_linha_seguinte_chega() {
    let _c = COFRE.read().unwrap_or_else(|e| e.into_inner());
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno("pula");
    inserir(porta_b, 2, "a@x");
    esperar("a parada do par", || parada(porta_a).is_some());
    inserir(porta_b, 3, "c@x");

    let r = exigir(
        porta_a,
        r#""op":"replicacao_pular","origem":"parceiro",
           "database":"loja","tabela":"clientes""#,
    );
    assert_eq!(r.inteiro_ou("pulou", -1), 0, "{}", r.escrever());
    assert_eq!(r.inteiro_ou("posicao", -1), 1, "{}", r.escrever());

    esperar("a linha 3 chegar depois do pulo", || {
        linhas(porta_a).iter().any(|(id, _)| *id == 3)
    });
    // A linha 2 NAO entra -- foi ela que o operador descartou --, e a 1 fica
    // com o e-mail dela. Os dois lados ficam diferentes, e isso esta dito na
    // resposta em vez de acontecer calado.
    assert_eq!(
        linhas(porta_a),
        vec![(1, "a@x".to_string()), (3, "c@x".to_string())]
    );
    assert!(parada(porta_a).is_none(), "a marca sobreviveu ao pulo");
    assert!(
        r.texto_ou("aviso", "").contains("NAO entra"),
        "a resposta nao avisa que o evento morreu: {}",
        r.escrever()
    );
}

/// O comportamento VELHO, que nao pode mudar: sem colisao nenhuma o laco
/// replica como sempre replicou, `recusas_por_unicidade` nem aparece na
/// resposta e `paradas` fica vazio -- campo que aparece cheio em toda
/// instalacao sa e campo que ninguem olha quando enche.
///
/// E a petrea que derrubou a forma antiga deste pedido -- «guarda nova entra
/// pedida, nao imposta»: o par que NAO colide continua replicando, e e ele que
/// a recusa na declaracao teria tirado do ar.
#[test]
fn sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado() {
    let _c = COFRE.read().unwrap_or_else(|e| e.into_inner());
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno("limpo");
    inserir(porta_b, 2, "b@x");
    inserir(porta_b, 3, "c@x");

    esperar("as duas linhas do parceiro", || linhas(porta_a).len() == 3);
    assert_eq!(
        linhas(porta_a),
        vec![
            (1, "a@x".to_string()),
            (2, "b@x".to_string()),
            (3, "c@x".to_string())
        ]
    );
    assert!(
        estado(porta_a)
            .campo("recusas_por_unicidade")
            .and_then(|r| r.campo("loja/clientes"))
            .is_none(),
        "tabela sem recusa apareceu no contador: {}",
        estado(porta_a).escrever()
    );
    assert!(
        parada(porta_a).is_none(),
        "par sadio apareceu como parado: {}",
        estado(porta_a).escrever()
    );
    assert_eq!(posicao(porta_a), 2, "a posicao do par sadio parou de andar");
}

/// **Prova real da petrea.** O grito do conflito carrega a linha daqui e a de
/// la -- e a coluna marcada como DADO PESSOAL nunca sai por extenso, nem na
/// linha nem no valor da chave.
///
/// E aqui esta a divergencia deliberada com o PostgreSQL, que imprime
/// `Key (c)=(1)` porque nao tem marca de dado pessoal no esquema: nos temos, e
/// uma chave de CPF sairia no `replicacao_estado` e no log do processo se
/// copiassemos o comportamento dele. Quem opera perde o valor e fica com o
/// tamanho, o nome da coluna e o indice -- da para achar a linha sem publicar
/// o dado.
///
/// **Defeito reposto**: tirar o ramo do `dado_pessoal` de
/// `bidirecional::valor_redigido`. O e-mail aparece inteiro no `detalhe` e a
/// primeira asercao cai.
#[test]
fn a_coluna_marcada_nao_vaza_no_grito_do_conflito() {
    let _c = COFRE.write().unwrap_or_else(|e| e.into_inner());
    // Os dois lados com cofre: sem ele, desde o pedido 616 o bidirecional
    // recusa a tabela marcada por «Falta o cofre», antes de qualquer
    // unicidade -- e o grito que esta prova mede nao existiria.
    cofre::definir("o cofre do grito marcado", cofre::ITERACOES_MINIMAS).unwrap();
    let _desliga = CofreLigado;
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno_com("marcado", true);
    inserir(porta_b, 2, "a@x");

    esperar("a parada do par", || parada(porta_a).is_some());
    let detalhe = parada(porta_a).unwrap().texto_ou("detalhe", "").to_string();
    assert!(
        !detalhe.contains("a@x"),
        "o dado pessoal vazou no grito do conflito: {detalhe}"
    );
    // O que sobra ainda serve para achar a linha: o indice, o tamanho e a
    // outra coluna.
    assert!(detalhe.contains("porEmail"), "{detalhe}");
    assert!(detalhe.contains("dado pessoal"), "{detalhe}");
    assert!(
        detalhe.contains("id=1") && detalhe.contains("id=2"),
        "as duas linhas sumiram junto com o e-mail: {detalhe}"
    );
}

/* ------------------------------------------- pedido 535: dado antes da posicao

A posicao consumida ia ao disco a cada lote, num `write` no lugar, e o dado
do lote so no `fsync` do fim do alcance. Nada ordenava os dois. O que se prova
aqui e a ORDEM contra o sistema operacional, pelo `strace`: na thread que
aplica, todo arquivo da tabela escrito antes de a posicao ir ao disco recebe
`fsync` DEPOIS da ultima escrita e ANTES da posicao; e a posicao entra pela
troca duravel (temporario com `fsync`, `rename`, `fsync` da pasta).

**Nao medido:** a queda em si. Um SIGKILL entre os dois nao a simula -- o
cache do nucleo sobrevive a morte do processo, e o dado sem `fsync` chega ao
disco igual, com ou sem o defeito. Distinguir pede derrubar a maquina (ou um
`dm-flakey` com `CAP_SYS_ADMIN`). */

/// O filho que o teste de baixo roda debaixo do `strace`: o par sobe, o
/// parceiro escreve duas linhas, e o filho espera a posicao 2 NO DISCO.
#[test]
#[ignore = "roda so dentro de a_posicao_do_bidirecional_vai_ao_disco_depois_do_dado"]
fn filho_da_posicao_depois_do_dado() {
    let saida = std::path::PathBuf::from(std::env::var("PHX_535_DIR").unwrap());
    let (_a, _b, porta_a, porta_b, da, _db) = terreno("535");
    // O caminho resolvido: e o que o `-y` do `strace` imprime.
    let real = std::fs::canonicalize(&da.0).unwrap();
    std::fs::write(saida.join("dir_a"), real.display().to_string()).unwrap();
    inserir(porta_b, 2, "b@x");
    inserir(porta_b, 3, "c@x");
    esperar("as duas linhas do parceiro", || linhas(porta_a).len() == 3);
    let arquivo = da.join("replicacao-posicoes.json");
    esperar("a posicao 2 no disco", || {
        std::fs::read_to_string(&arquivo)
            .ok()
            .and_then(|t| Json::analisar(&t).ok())
            .and_then(|j| j.campo("parceiro|loja/clientes").and_then(Json::inteiro))
            == Some(2)
    });
}

/// **Pedido 535, contra o sistema operacional.** Ver a nota acima.
///
/// **Defeito reposto (1):** a posicao gravada a cada lote, antes do
/// `sincronizar_replicada` -- o `write` da posicao aparece com o `.reg` da
/// tabela escrito e sem `fsync`. **(2):** a posicao pelo `write` no lugar
/// (`escrever_do_banco`), mesmo na ordem certa -- nao ha `rename` nem
/// `fsync` dela.
#[cfg(unix)]
#[test]
fn a_posicao_do_bidirecional_vai_ao_disco_depois_do_dado() {
    use std::process::Command;
    if Command::new("strace").arg("-V").output().is_err() {
        eprintln!("sem strace nesta maquina: a prova do 535 NAO MEDIDA");
        return;
    }
    let d = DirTemp::novo("535-strace");
    let prefixo = d.join("traco");
    let saida = Command::new("strace")
        .args([
            "-ff",
            "-qq",
            "-y",
            "-e",
            "trace=openat,write,pwrite64,writev,pwritev,fsync,fdatasync,rename,renameat,renameat2",
            "-o",
        ])
        .arg(&prefixo)
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "filho_da_posicao_depois_do_dado",
            "--test-threads=1",
        ])
        .env("PHX_535_DIR", &d.0)
        .output()
        .unwrap();
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let dir_a = std::fs::read_to_string(d.join("dir_a")).unwrap();
    let posicoes = format!("{dir_a}/replicacao-posicoes.json");
    let temporario = format!("{posicoes}.tmp");
    let dados = format!("{dir_a}/loja/");
    // O caminho do descritor que o `-y` imprime no primeiro argumento.
    let alvo = |l: &str| -> Option<String> {
        let a = l.find('<')?;
        let b = l[a..].find(">,").or_else(|| l[a..].find(">)"))?;
        Some(l[a + 1..a + b].to_string())
    };
    let chamada = |l: &str, nome: &str| l.starts_with(&format!("{nome}("));
    let mut threads_com_posicao = 0;
    let mut erros = Vec::new();
    for entrada in std::fs::read_dir(&d.0).unwrap() {
        let caminho = entrada.unwrap().path();
        let nome = caminho.file_name().unwrap().to_string_lossy().to_string();
        if !nome.starts_with("traco.") {
            continue;
        }
        let texto = std::fs::read_to_string(&caminho).unwrap();
        // So as chamadas que deram certo.
        let linhas: Vec<&str> = texto.lines().filter(|l| !l.contains("= -1")).collect();
        // A posicao vai ao disco: o `rename` para o nome final, ou -- o
        // defeito -- um `write` direto no descritor dele.
        let marcos: Vec<(usize, bool)> = linhas
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let renomeia = l.starts_with("rename") && l.contains(&format!("\"{posicoes}\""));
                let escreve = chamada(l, "write") && alvo(l).as_deref() == Some(&posicoes);
                (renomeia || escreve).then_some((i, renomeia))
            })
            .collect();
        if marcos.is_empty() {
            continue;
        }
        threads_com_posicao += 1;
        for (i, renomeia) in marcos {
            if !renomeia {
                erros.push(format!(
                    "{nome}:{i}: a posicao foi escrita no lugar, sem troca duravel"
                ));
                continue;
            }
            if !linhas[..i]
                .iter()
                .any(|l| chamada(l, "fsync") && alvo(l).as_deref() == Some(&temporario))
            {
                erros.push(format!("{nome}:{i}: rename sem fsync do temporario antes"));
            }
            if !linhas[i + 1..]
                .iter()
                .any(|l| chamada(l, "fsync") && alvo(l).as_deref() == Some(&dir_a))
            {
                erros.push(format!("{nome}:{i}: rename sem fsync da pasta depois"));
            }
            // Todo arquivo da tabela escrito antes da posicao: `fsync` depois
            // da ultima escrita, e antes da posicao.
            let mut sem_fsync: std::collections::HashMap<String, usize> = Default::default();
            for (j, l) in linhas[..i].iter().enumerate() {
                let Some(p) = alvo(l) else { continue };
                // O `.pag` e descritor gerado do `.reg`, e o motor nunca o le
                // para decidir nada (`pag.rs`): ele troca por `rename` sem
                // `fsync` de proposito, e nao e o dado que a posicao conta.
                if !p.starts_with(&dados) || p.ends_with(".pag") || p.ends_with(".pag.novo") {
                    continue;
                }
                if chamada(l, "write") || l.starts_with("pwrite") || l.starts_with("writev") {
                    sem_fsync.insert(p, j);
                } else if chamada(l, "fsync") || chamada(l, "fdatasync") {
                    sem_fsync.remove(&p);
                }
            }
            for (p, j) in sem_fsync {
                erros.push(format!(
                    "{nome}:{i}: a posicao foi ao disco com {p} escrito (linha {j}) e sem fsync"
                ));
            }
        }
    }
    assert!(
        threads_com_posicao > 0,
        "a premissa: alguma thread gravou a posicao em {posicoes}"
    );
    assert!(erros.is_empty(), "{}", erros.join("\n"));
}

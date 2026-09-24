//! Pedido 450, etapa 2 -- o `config.phz` provado pelo BINARIO e pelo sistema
//! operacional, nao so pela biblioteca.
//!
//! Os testes de `config_phz.rs` provam o motor. O que so aparece aqui: o
//! `main` chamando a troca de forma na ordem certa, o arranque dizendo pelo
//! erro padrao que o arquivo esta em claro, o soquete devolvendo a
//! configuracao lida do `.phz` -- e a senha fora de TUDO o que o processo
//! escreve e responde. E a prova contra o 7-Zip do sistema, que e o que o
//! administrador usa quando abre o `.phz` na mao.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use phxsql_server::config_phz::{self, SENHA_DO_PHZ};

const ASSINATURA_7Z: &[u8] = b"7z\xbc\xaf\x27\x1c";

/// A porta que o PROPRIO servidor abriu, lida do erro padrao dele.
///
/// O `config` pede a porta 0 e o sistema escolhe. Escolher uma porta «livre»
/// no teste e passa-la adiante deixa uma janela entre o teste solta-la e o
/// servidor pega-la. Na primeira corrida desta bateria, ainda com o esquema
/// antigo, um `config_gravar` voltou «connection reset»; a causa NAO foi
/// medida -- a colisao de porta e a hipotese, e o esquema antigo passou nas
/// quatro corridas seguintes. A porta 0 fecha a janela sem depender do
/// diagnostico.
fn porta_aberta(erro_padrao: &Path) -> u16 {
    const LINHA: &str = "porta de dados escutando em ";
    let ate = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ate {
        let texto = std::fs::read_to_string(erro_padrao).unwrap_or_default();
        if let Some(resto) = texto.lines().find_map(|l| l.strip_prefix(LINHA)) {
            let alvo: SocketAddr = resto.trim().parse().unwrap();
            return alvo.port();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "o phxsqld nao abriu a porta de dados em 10 s: {}",
        std::fs::read_to_string(erro_padrao).unwrap_or_default()
    );
}

/// Mata o processo no `Drop`, inclusive quando uma assercao falha no meio.
struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{linha}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

/// O `config.json` de um servidor que so fala pelo soquete de dados, em
/// claro -- o que se mede aqui e o arquivo de configuracao, nao a cifra do
/// fio nem a porta web.
const CONFIG_DE_PROVA: &str = "{\n  \"bind\": \"127.0.0.1:0\",\n  \"token\": \"t\",\n  \
     \"max_linhas\": 10,\n  \"cifra_fio\": { \"exigir\": false },\n  \
     \"web\": { \"ligado\": false }\n}\n";

/// Roda o `phxsqld` e devolve (sucesso, saida + erro) -- com PRAZO: sem
/// `--empacotar-config` o processo e um servidor, e o defeito que o deixasse
/// subir com os dois arquivos presentes faria `output()` esperar para
/// sempre. Passado o prazo, ele e morto e o teste falha dizendo que subiu.
fn rodar(args: &[&str], config: &Path) -> (bool, String) {
    rodar_como(Path::new(env!("CARGO_BIN_EXE_phxsqld")), None, args, config)
}

/// O `phxsqld` de `bin`, e com qual uid. `como: Some(uid)` roda o processo
/// com esse uid e esse gid -- e so o root consegue, entao quem nao e root
/// recebe um `spawn` que FALHA, nunca um teste verde que nao rodou. E o
/// servico como usuario comum, o caso realista do pedido 481: o teste roda
/// como root, e um servidor como root nunca exercitaria o uid de quem roda
/// (revisao SEC do 481, MEDIO 3).
fn comando(bin: &Path, como: Option<u32>, config: &Path) -> Command {
    // Fora do Unix nao ha uid para trocar, e o `mut` sobra.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut c = Command::new(bin);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        if let Some(uid) = como {
            // A pasta de trabalho de quem roda o teste pode nem ser
            // atravessavel pelo servico (`~/.cache/phx-guardas` e do root,
            // 0700): o servico roda de dentro da pasta do config.
            c.uid(uid).gid(uid).current_dir(config.parent().unwrap());
        }
    }
    #[cfg(not(unix))]
    let _ = (como, config);
    c
}

fn rodar_como(bin: &Path, como: Option<u32>, args: &[&str], config: &Path) -> (bool, String) {
    let pasta = config.parent().unwrap();
    let saida = pasta.join("rodar-stdout.txt");
    let erro = pasta.join("rodar-stderr.txt");
    let mut filho = Filho(
        comando(bin, como, config)
            .args(args)
            .arg("--config")
            .arg(config)
            .stdout(Stdio::from(std::fs::File::create(&saida).unwrap()))
            .stderr(Stdio::from(std::fs::File::create(&erro).unwrap()))
            .spawn()
            .expect("nao consegui rodar o phxsqld"),
    );
    let ate = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(s) = filho.0.try_wait().unwrap() {
            break s;
        }
        assert!(
            Instant::now() < ate,
            "phxsqld {args:?} ainda rodava depois de 20 s: subiu como servidor"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut texto = std::fs::read_to_string(&saida).unwrap();
    texto.push_str(&std::fs::read_to_string(&erro).unwrap());
    let _ = std::fs::remove_file(&saida);
    let _ = std::fs::remove_file(&erro);
    (status.success(), texto)
}

/// Sobe o servidor com o erro padrao num arquivo, para ler DEPOIS do fim.
fn subir(config: &Path, erro_padrao: &Path) -> Filho {
    subir_como(
        Path::new(env!("CARGO_BIN_EXE_phxsqld")),
        None,
        config,
        erro_padrao,
    )
}

fn subir_como(bin: &Path, como: Option<u32>, config: &Path, erro_padrao: &Path) -> Filho {
    let erro = std::fs::File::create(erro_padrao).unwrap();
    Filho(
        comando(bin, como, config)
            .arg("--config")
            .arg(config)
            .stdout(Stdio::null())
            .stderr(Stdio::from(erro))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    )
}

/// **(a) e (e) pelo binario.** A migracao pedida pela linha de comando, o
/// servidor subindo do `.phz` com o MESMO `--config config.json` de antes,
/// a tela gravando de volta no `.phz` -- e a senha fora de toda saida do
/// processo e de toda resposta do soquete.
///
/// Derruba: qualquer mensagem do `main` ou do motor que interpole a senha
/// (por exemplo, o aviso de arranque dizendo «abra com 7z x -p...»); e o
/// `main` nao chamando a troca (`--empacotar-config` subiria o servidor).
#[test]
fn a_migracao_pelo_binario_sobe_do_phz_e_a_senha_nao_sai_do_processo() {
    let d = DirTemp::novo("phz-binario");
    let claro = d.join("config.json");
    let phz = d.join("config.phz");
    let original = CONFIG_DE_PROVA;
    std::fs::write(&claro, original).unwrap();
    let mut tudo: Vec<String> = Vec::new();

    let (ok, saida) = rodar(&["--empacotar-config"], &claro);
    assert!(ok, "a migracao falhou: {saida}");
    assert!(saida.contains("config.phz"), "{saida}");
    assert!(saida.contains("config.json.migrado-para-phz"), "{saida}");
    tudo.push(saida);
    assert!(!claro.exists());
    assert_eq!(
        std::fs::read_to_string(d.join("config.json.migrado-para-phz")).unwrap(),
        original,
        "o original nao ficou guardado byte a byte"
    );
    assert!(std::fs::read(&phz).unwrap().starts_with(ASSINATURA_7Z));

    // O mesmo --config de antes: quem migrou nao edita a unidade do systemd.
    let erro_padrao = d.join("stderr.txt");
    let filho = subir(&claro, &erro_padrao);
    let porta = porta_aberta(&erro_padrao);
    // O valor que so o arquivo tem: o servidor subiu do que estava no .phz.
    let config = pedir(porta, r#"{"token":"t","op":"config"}"#);
    assert!(config.contains("\"max_linhas\":10"), "{config}");
    let gravar = pedir(
        porta,
        r#"{"token":"t","op":"config_gravar","campos":{"max_linhas":42}}"#,
    );
    assert!(gravar.contains("\"gravado\":true"), "{gravar}");
    let recusado = pedir(porta, r#"{"token":"errado","op":"config"}"#);
    tudo.extend([config, gravar, recusado]);
    drop(filho);
    let erro = std::fs::read_to_string(&erro_padrao).unwrap();
    assert!(
        erro.contains("a copia EM CLARO de antes da migracao"),
        "o arranque do .phz nao lembrou da copia em claro: {erro}"
    );
    tudo.push(erro);

    let relido = config_phz::ler_texto(&phz).unwrap();
    assert!(relido.contains("\"max_linhas\": 42"), "{relido}");
    assert!(std::fs::read(&phz).unwrap().starts_with(ASSINATURA_7Z));
    assert!(!claro.exists(), "a gravacao pela tela fez nascer o .json");

    // E a volta, tambem pela linha de comando.
    let (ok, saida) = rodar(&["--desempacotar-config"], &phz);
    assert!(ok, "a volta falhou: {saida}");
    assert!(saida.contains("config.phz.aberto-em-json"), "{saida}");
    tudo.push(saida);
    assert!(std::fs::read_to_string(&claro)
        .unwrap()
        .contains("\"max_linhas\": 42"));

    for t in &tudo {
        assert!(!t.contains(SENHA_DO_PHZ), "a senha saiu do processo: {t}");
    }
}

/// **Pedido 478.** O roteiro que o MANUAL (secao 7.3) e o README ensinam --
/// gerar um dos tres exemplos, editar, `--empacotar-config`, subir -- fecha
/// com um `.phz` de verdade, e o servidor sobe dele.
///
/// Os outros testes deste arquivo empacotam `CONFIG_DE_PROVA`, sintetico e
/// minusculo. Este usa os TRES `exemplos/Config_exemplo_0N.json` que o
/// `--exemplo` imprime -- o que quem segue o manual copia de verdade -- para
/// que um campo novo neles (uma lista grande, um caractere fora do comum)
/// que travasse o `--empacotar-config` ou a leitura de volta caia aqui, e nao
/// so no dia em que um administrador tentar.
///
/// Derruba: qualquer coisa que faca `--empacotar-config` falhar sobre um dos
/// tres exemplos reais, que devolva um `.phz` que nao seja 0600, ou do qual o
/// servidor nao suba.
#[test]
fn o_roteiro_documentado_empacota_o_exemplo_de_verdade_e_sobe_do_phz() {
    use phxsql_core::json::Json;

    for exemplo in ["1", "2", "3"] {
        let d = DirTemp::novo(&format!("phz-roteiro-{exemplo}"));
        let claro = d.join("config.json");
        let bruto = phxsql_server::config_exemplo(exemplo)
            .unwrap_or_else(|| panic!("exemplo {exemplo} sumiu de config_exemplo"));

        // O bind fixo dos exemplos (para copiar e colar) colidiria entre as
        // tres voltas deste laco e com outro teste rodando em paralelo; o
        // `base` fixo do exemplo 3 e um caminho absoluto do sistema (nao do
        // diretorio do teste). Os dois viram porta 0 e a pasta do teste --
        // o mesmo texto_trocar que a tela usa para editar o config, entao a
        // prova continua exercitando o caminho real, so com valores que nao
        // colidem.
        let texto = Json::texto_trocar(bruto, &["bind"], &Json::texto_de("127.0.0.1:0"))
            .unwrap_or_else(|| panic!("exemplo {exemplo} nao tem \"bind\" de primeiro nivel"));
        let texto = Json::texto_trocar(
            &texto,
            &["base"],
            &Json::texto_de(d.join("dados").display().to_string()),
        )
        .unwrap_or_else(|| panic!("exemplo {exemplo} nao tem \"base\" de primeiro nivel"));
        std::fs::write(&claro, &texto).unwrap();

        let (ok, saida) = rodar(&["--empacotar-config"], &claro);
        assert!(ok, "exemplo {exemplo}: --empacotar-config falhou: {saida}");
        let phz = d.join("config.phz");
        assert!(phz.exists(), "exemplo {exemplo}: nao nasceu config.phz");
        assert!(
            std::fs::read(&phz).unwrap().starts_with(ASSINATURA_7Z),
            "exemplo {exemplo}: config.phz nao e um 7z"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let modo = std::fs::metadata(&phz).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600, "exemplo {exemplo}: config.phz nasceu {modo:o}");
        }

        // O MESMO --config de antes do empacotar sobe do .phz -- e o que
        // o manual promete: a unidade do systemd nao muda.
        let erro_padrao = d.join("stderr.txt");
        let filho = subir(&claro, &erro_padrao);
        porta_aberta(&erro_padrao);
        drop(filho);
    }
}

/// **(c) pelo binario.** O `config.json` em claro de sempre sobe, o erro
/// padrao diz que ele esta em claro e qual comando o empacota, e a gravacao
/// pela tela o mantem em claro -- o comportamento que as bancadas usam.
///
/// Derruba: o `main` sem a linha do aviso; e uma gravacao que empacotasse
/// sozinha (o `read_to_string` do fim quebra).
#[test]
fn config_json_em_claro_sobe_e_o_arranque_diz_como_empacotar() {
    let d = DirTemp::novo("phz-claro");
    let claro = d.join("config.json");
    std::fs::write(&claro, CONFIG_DE_PROVA).unwrap();

    let erro_padrao = d.join("stderr.txt");
    let filho = subir(&claro, &erro_padrao);
    let porta = porta_aberta(&erro_padrao);
    let gravar = pedir(
        porta,
        r#"{"token":"t","op":"config_gravar","campos":{"max_linhas":42}}"#,
    );
    assert!(gravar.contains("\"gravado\":true"), "{gravar}");
    drop(filho);

    let erro = std::fs::read_to_string(&erro_padrao).unwrap();
    assert!(erro.contains("EM CLARO"), "o arranque ficou calado: {erro}");
    assert!(
        erro.contains("phxsqld --empacotar-config --config"),
        "o aviso nao diz como migrar: {erro}"
    );
    assert!(!erro.contains(SENHA_DO_PHZ));
    let texto = std::fs::read_to_string(&claro).expect("o config.json deixou de ser texto");
    assert!(texto.contains("\"max_linhas\": 42"), "{texto}");
    assert!(!d.join("config.phz").exists());
}

/// A dica que o `main` da quando o `Config::ler` falha. Seguida ao pe da
/// letra, ela TRUNCA o arquivo de configuracao -- e por isso so pode aparecer
/// quando nao ha arquivo nenhum.
const DICA_DO_MODELO: &str = "phxsqld --exemplo 1 >";

/// **(d) pelo binario.** Os dois presentes: o processo sai com erro em vez de
/// subir, nomeia os dois arquivos, nenhum deles muda -- e a saida NAO manda
/// gerar um modelo, que truncaria o `.json` que o administrador extraiu para
/// editar.
///
/// Derruba: o `resolver` escolhendo um dos dois -- o processo sobe. E o
/// `main` dando a dica do modelo para qualquer erro (parecer do DBA de
/// 24/09/2026): «a saida manda gerar o modelo».
#[test]
fn os_dois_presentes_o_binario_nao_sobe_e_nao_toca_em_nenhum() {
    let d = DirTemp::novo("phz-dois");
    let claro = d.join("config.json");
    let phz = d.join("config.phz");
    std::fs::write(&claro, CONFIG_DE_PROVA).unwrap();
    config_phz::gravar_texto(&phz, CONFIG_DE_PROVA).unwrap();
    let antes = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());

    for args in [
        &[][..],
        &["--empacotar-config"][..],
        &["--desempacotar-config"][..],
    ] {
        let (ok, saida) = rodar(args, &claro);
        assert!(!ok, "{args:?} seguiu com os dois presentes: {saida}");
        assert!(saida.contains("existem os dois"), "{saida}");
        assert!(saida.contains(&phz.display().to_string()), "{saida}");
        assert!(!saida.contains(SENHA_DO_PHZ));
        assert!(
            !saida.contains(DICA_DO_MODELO),
            "a saida manda gerar o modelo, que truncaria o .json: {saida}"
        );
    }
    let depois = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());
    assert!(antes == depois, "um dos dois mudou");
}

/// O usuario de servico (`User=phxsql` do MANUAL §7.4) e um terceiro comum.
#[cfg(unix)]
const SERVICO: u32 = 65534;
#[cfg(unix)]
const TERCEIRO: u32 = 65533;

/// O motivo dos dois testes que so o root roda: sem ele, `chown` e o
/// `setuid` do filho FALHAM, e o teste cai dizendo isso -- nunca um verde
/// que nao rodou (revisao SEC do 481, MEDIO 2).
#[cfg(unix)]
const SO_O_ROOT: &str = "exige root: chown, e o phxsqld rodando como o uid 65534";

/// Muda dono e modo de um caminho -- e cai, com o motivo, sem privilegio.
#[cfg(unix)]
fn dar_a(caminho: &Path, dono: u32, modo: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::os::unix::fs::chown(caminho, Some(dono), Some(dono))
        .unwrap_or_else(|e| panic!("{SO_O_ROOT} ({e})"));
    std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(modo)).unwrap();
}

/// Uma copia do `phxsqld` que o uid do servico consegue executar: o do
/// `target/` pode morar numa pasta que so o root atravessa (a arvore do
/// `provar-guardas.py` e `~/.cache/phx-guardas`).
#[cfg(unix)]
fn binario_para_o_servico() -> (DirTemp, PathBuf) {
    let d = DirTemp::novo("phz-481-bin");
    let bin = d.join("phxsqld");
    std::fs::copy(env!("CARGO_BIN_EXE_phxsqld"), &bin).unwrap();
    dar_a(&d, 0, 0o755);
    dar_a(&bin, 0, 0o755);
    (d, bin)
}

/// A configuracao VELHA, a que carrega o token vazado: o `max_linhas` 99 e o
/// que a denuncia na resposta do servidor. Sobe como servidor de verdade --
/// e isso que faz o teste do ALTO cair pelo motivo certo com o defeito
/// reposto, em vez de cair por um arranque que nem conseguiria subir.
#[cfg(unix)]
const CONFIG_VELHO: &str = "{\n  \"bind\": \"127.0.0.1:0\",\n  \"token\": \"t\",\n  \
     \"max_linhas\": 99,\n  \"cifra_fio\": { \"exigir\": false },\n  \
     \"web\": { \"ligado\": false }\n}\n";

/// **Pedido 481, o achado ALTO da revisao SEC, pelo sistema operacional.**
/// A instalacao do MANUAL §7.4 (`/opt/phxsql`, `User=phxsql`): a pasta e o
/// `.phz` sao do servico, que grava o `.phz` por troca atomica. O
/// administrador roda `sudo 7z x` e troca o token vazado -- e o `.json` nasce
/// do ROOT. No reinicio, o servico (uid 65534, e nao root) NAO SOBE: dos dois,
/// nenhum e plantado -- o root e quem administra, e a pasta nem tem sticky
/// bit --, e a recusa de sempre vale. A frente anterior subia do `.phz` VELHO
/// chamando o do root de terceiro, e o token revogado continuava valendo.
///
/// Derruba: a regra da frente anterior reposta -- o root como terceiro e a
/// pasta sem sticky bit aceita (guarda `config-phz-par-root-vira-terceiro`).
#[cfg(unix)]
#[test]
#[ignore = "exige root: chown, e o phxsqld rodando como o uid 65534 -- rode como root com `-- --include-ignored`"]
fn o_json_do_root_ao_lado_do_phz_do_servico_recusa_o_arranque() {
    let (_bin_d, bin) = binario_para_o_servico();
    let d = DirTemp::novo("phz-481-alto");
    let claro = d.join("config.json");
    let phz = d.join("config.phz");
    config_phz::gravar_texto(&phz, CONFIG_VELHO).unwrap();
    std::fs::write(&claro, CONFIG_DE_PROVA).unwrap();
    dar_a(&phz, SERVICO, 0o600);
    dar_a(&claro, 0, 0o644); // o `sudo 7z x`, com a umask de sempre
    dar_a(&d, SERVICO, 0o755); // a pasta do servico, sem sticky bit
    let antes = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());

    let (subiu, saida) = rodar_como(&bin, Some(SERVICO), &[], &claro);
    assert!(
        !subiu,
        "o servico subiu com o .json do root ao lado: {saida}"
    );
    assert!(
        saida.contains("configuracao ambigua: existem os dois"),
        "nao foi a recusa do par: {saida}"
    );
    assert!(saida.contains(&claro.display().to_string()), "{saida}");
    assert!(saida.contains(&phz.display().to_string()), "{saida}");
    let depois = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());
    assert!(antes == depois, "um dos dois mudou");
}

/// **Pedido 481, o que ele pede, pelo sistema operacional.** O `/tmp`
/// classico (root, 1777): o `.json` e do servico, e um TERCEIRO comum plantou
/// um `.phz` ao lado sem poder tocar no `.json`. O servico (uid 65534) sobe do
/// SEU `.json` e avisa pelo erro padrao qual nome ignorou e de quem ele e --
/// e a troca de forma pedida nesse estado recusa dizendo o que ha, e nao
/// «apareceu durante a troca», que mandava procurar uma corrida que nao
/// houve (revisao SEC do 481, BAIXO).
///
/// Derruba: o aviso fora dos `avisos` do `Config::ler` (guarda
/// `config-phz-terceiro-nao-avisa-no-arranque`) e a troca sem olhar o
/// ignorado (guarda `config-phz-troca-sobre-terceiro-diz-corrida`).
#[cfg(unix)]
#[test]
#[ignore = "exige root: chown, e o phxsqld rodando como o uid 65534 -- rode como root com `-- --include-ignored`"]
fn o_phz_de_um_terceiro_em_pasta_com_sticky_e_ignorado_e_o_servico_sobe_do_json() {
    let (_bin_d, bin) = binario_para_o_servico();
    let d = DirTemp::novo("phz-481-sticky");
    let claro = d.join("config.json");
    let phz = d.join("config.phz");
    std::fs::write(&claro, CONFIG_DE_PROVA).unwrap();
    config_phz::gravar_texto(&phz, CONFIG_VELHO).unwrap();
    dar_a(&claro, SERVICO, 0o600);
    dar_a(&phz, TERCEIRO, 0o644);
    dar_a(&d, 0, 0o1777);
    let phz_antes = std::fs::read(&phz).unwrap();

    let erro_padrao = d.join("stderr.txt");
    let filho = subir_como(&bin, Some(SERVICO), &claro, &erro_padrao);
    let porta = porta_aberta(&erro_padrao); // so abre porta se SUBIU como servidor
    let config = pedir(porta, r#"{"token":"t","op":"config"}"#);
    assert!(
        config.contains("\"max_linhas\":10"),
        "subiu do .phz do terceiro, nao do .json: {config}"
    );
    drop(filho);

    let erro = std::fs::read_to_string(&erro_padrao).unwrap();
    let aviso = erro
        .lines()
        .find(|l| l.starts_with("AVISO:") && l.contains("TERCEIRO"))
        .unwrap_or_else(|| panic!("o arranque calou o que ignorou: {erro}"));
    assert!(aviso.contains(&phz.display().to_string()), "{aviso}");
    assert!(aviso.contains(&format!("uid {TERCEIRO}")), "{aviso}");
    assert!(aviso.contains(&claro.display().to_string()), "{aviso}");

    // A troca pedida com o terceiro ocupando o nome de destino.
    let (trocou, saida) = rodar_como(&bin, Some(SERVICO), &["--empacotar-config"], &claro);
    assert!(
        !trocou,
        "a troca seguiu com o .phz do terceiro no caminho: {saida}"
    );
    assert!(
        !saida.contains("apareceu durante a troca"),
        "a troca culpou uma corrida que nao houve: {saida}"
    );
    assert!(saida.contains("configuracao ambigua"), "{saida}");
    assert!(saida.contains(&phz.display().to_string()), "{saida}");
    assert!(saida.contains(&format!("uid {TERCEIRO}")), "{saida}");

    // Nada se apaga: o .phz do terceiro continua la, para quem investiga.
    assert_eq!(
        std::fs::read(&phz).unwrap(),
        phz_antes,
        "o .phz do terceiro mudou"
    );
    assert!(claro.exists(), "o .json do servico sumiu");

    // O espelho, na volta: o `.phz` e do servico, o `.json` foi plantado, e
    // o `--desempacotar-config` gravaria no nome do terceiro.
    let d2 = DirTemp::novo("phz-481-sticky-volta");
    let claro2 = d2.join("config.json");
    let phz2 = d2.join("config.phz");
    config_phz::gravar_texto(&phz2, CONFIG_DE_PROVA).unwrap();
    std::fs::write(&claro2, CONFIG_VELHO).unwrap();
    dar_a(&phz2, SERVICO, 0o600);
    dar_a(&claro2, TERCEIRO, 0o644);
    dar_a(&d2, 0, 0o1777);
    let (trocou, saida) = rodar_como(&bin, Some(SERVICO), &["--desempacotar-config"], &phz2);
    assert!(
        !trocou,
        "a volta seguiu com o .json do terceiro no caminho: {saida}"
    );
    assert!(!saida.contains("apareceu durante a troca"), "{saida}");
    assert!(saida.contains(&claro2.display().to_string()), "{saida}");
    assert!(saida.contains(&format!("uid {TERCEIRO}")), "{saida}");
    assert_eq!(std::fs::read_to_string(&claro2).unwrap(), CONFIG_VELHO);
}

/// O irmao: UM arquivo presente e ilegivel -- um `.phz` cortado, ou um
/// `.json` com a sintaxe quebrada. O processo recusa com o nome do erro, nao
/// toca no arquivo, e tambem nao manda gerar o modelo por cima dele.
///
/// Derruba: o `main` dando a dica do modelo para qualquer erro.
#[test]
fn arquivo_ilegivel_o_binario_nao_sobe_e_nao_manda_gerar_o_modelo() {
    let d = DirTemp::novo("phz-ilegivel");
    let phz = d.join("config.phz");
    config_phz::gravar_texto(&phz, CONFIG_DE_PROVA).unwrap();
    let bytes = std::fs::read(&phz).unwrap();
    let cortado = &bytes[..bytes.len() / 2];
    std::fs::write(&phz, cortado).unwrap();

    let (ok, saida) = rodar(&[], &d.join("config.json"));
    assert!(!ok, "subiu de um .phz cortado: {saida}");
    assert!(saida.contains(".phz nao abriu"), "{saida}");
    assert!(
        !saida.contains(DICA_DO_MODELO),
        "a saida manda gerar o modelo, que truncaria o .phz: {saida}"
    );
    assert_eq!(std::fs::read(&phz).unwrap(), cortado, "o .phz mudou");

    std::fs::remove_file(&phz).unwrap();
    let claro = d.join("config.json");
    std::fs::write(&claro, "{ \"token\": \"t\", }").unwrap();
    let (ok, saida) = rodar(&[], &claro);
    assert!(!ok, "subiu de um .json torto: {saida}");
    assert!(
        !saida.contains(DICA_DO_MODELO),
        "a saida manda gerar o modelo, que truncaria o .json: {saida}"
    );
}

/// **Um `.phz` que abre e nao passa na validacao sai pela ferramenta.**
/// Medio 2 da revisao SEC de 24/09/2026: a troca de forma so rodava depois de
/// o `Config::ler` aceitar o arquivo, entao o `.phz` com um campo torto nao
/// tinha como ser aberto para conserto -- e o erro ainda sugeria gerar o
/// modelo por cima. E a recusa da troca nomeia o arquivo resolvido.
///
/// Derruba: a troca de forma voltar para depois do `Config::ler`.
#[test]
fn phz_que_nao_valida_sai_pelo_desempacotar() {
    let d = DirTemp::novo("phz-nao-valida");
    let phz = d.join("config.phz");
    // Sem "token": o `Config::ler` recusa subir.
    config_phz::gravar_texto(&phz, "{ \"max_linhas\": 10 }\n").unwrap();
    let (ok, saida) = rodar(&[], &d.join("config.json"));
    assert!(!ok, "subiu sem token: {saida}");

    let (ok, saida) = rodar(&["--desempacotar-config"], &d.join("config.json"));
    assert!(
        ok,
        "o .phz que nao valida nao saiu pela ferramenta: {saida}"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("config.json")).unwrap(),
        "{ \"max_linhas\": 10 }\n"
    );

    // Empacotar um texto que nao e JSON recusa, nomeando o arquivo.
    std::fs::write(d.join("config.json"), "isto nao e json").unwrap();
    std::fs::remove_file(d.join("config.phz.aberto-em-json")).unwrap();
    let (ok, saida) = rodar(&["--empacotar-config"], &d.join("config.json"));
    assert!(!ok, "{saida}");
    let esperado = format!(
        "a troca nao se completou em {}",
        d.join("config.json").display()
    );
    assert!(saida.contains(&esperado), "{saida}");
    assert!(!d.join("config.phz").exists());
}

/// O comportamento VELHO que a dica existe para servir: sem arquivo nenhum,
/// ela continua aparecendo -- e aponta para o `.json` do par pedido.
///
/// Derruba: tirar a dica de vez para passar nos dois testes de cima.
#[test]
fn sem_configuracao_nenhuma_a_dica_do_modelo_continua() {
    let d = DirTemp::novo("phz-nenhum");
    let (ok, saida) = rodar(&[], &d.join("config.phz"));
    assert!(!ok, "{saida}");
    let esperada = format!("{DICA_DO_MODELO} {}", d.join("config.json").display());
    assert!(saida.contains(&esperada), "sem a dica do modelo: {saida}");
}

// ------------------------------------------- contra o 7-Zip do sistema

fn sete_zip() -> Option<&'static str> {
    ["7z", "7za"].into_iter().find(|p| {
        Command::new(p)
            .arg("i")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// **(b)** O `.phz` que o servidor grava abre no `7z x -p<senha>` do
/// sistema, byte a byte; e o que o 7-Zip grava de volta (LZMA2, cabecalho
/// cifrado e as 2^19 rodadas dele) o servidor le com os limites apertados.
/// E o caminho que o dono descreveu: o administrador abre no 7-Zip, confere,
/// e o PhxSql le o que o 7-Zip gravar.
///
/// Precisa do `7z` instalado, e por isso e `#[ignore]`; sem ele o teste FALHA
/// dizendo «NAO MEDIDO», em vez de passar calado:
///
/// ```text
/// cargo test -p phxsql-server --test config-phz -- --ignored
/// ```
///
/// Derruba: o servidor gravando o texto cru com a extensao `.phz` (o `7z x`
/// recusa: «Can not open the file as archive»), ou gravando com outra senha
/// que nao a constante.
#[test]
#[ignore]
fn o_7zip_do_sistema_abre_o_phz_do_servidor_e_o_servidor_le_o_do_7zip() {
    let Some(sete) = sete_zip() else {
        panic!("NAO MEDIDO: nao ha 7z nem 7za nesta maquina");
    };
    let d = DirTemp::novo("phz-7z");
    let texto = CONFIG_DE_PROVA;
    let phz = d.join("config.phz");
    config_phz::gravar_texto(&phz, texto).unwrap();

    let senha = format!("-p{SENHA_DO_PHZ}");
    let extraido: PathBuf = d.join("extraido");
    let x = Command::new(sete)
        .arg("x")
        .arg(&senha)
        .arg(format!("-o{}", extraido.display()))
        .arg(&phz)
        .output()
        .unwrap();
    assert!(
        x.status.success(),
        "7z x recusou o .phz do servidor: {}{}",
        String::from_utf8_lossy(&x.stdout),
        String::from_utf8_lossy(&x.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(extraido.join("config.json")).unwrap(),
        texto,
        "o 7-Zip extraiu outra coisa"
    );
    let errada = Command::new(sete)
        .args(["t", "-psenha-errada"])
        .arg(&phz)
        .output()
        .unwrap();
    assert!(!errada.status.success(), "o 7z abriu com a senha errada");

    // O 7-Zip reempacota com os padroes dele, e o servidor le.
    let volta = d.join("volta");
    std::fs::create_dir_all(&volta).unwrap();
    let a = Command::new(sete)
        .args(["a", &senha, "-mhe=on"])
        .arg(volta.join("config.phz"))
        .arg(extraido.join("config.json"))
        .output()
        .unwrap();
    assert!(a.status.success(), "7z a falhou");
    assert_eq!(
        config_phz::ler_texto(&volta.join("config.phz")).unwrap(),
        texto,
        "o servidor nao le o que o 7-Zip grava"
    );
    let c = phxsql_server::Config::ler(volta.join("config.json")).unwrap();
    assert_eq!(c.max_linhas, 10);
}

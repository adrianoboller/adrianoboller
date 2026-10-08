//! Pedido 513, passo 1: a copia do backup nao para a LEITURA, e continua
//! parando a ESCRITA -- provado pelo soquete, na porta de dados de producao.
//!
//! A copia lenta e a pausa de teste no gancho da copia
//! (`PanicoDeTeste::Pausa`), com a ficha dela na mao: e o que um backup de
//! 100 GB faz durante 50 minutos, comprimido em 1,5 s.
use super::*;
use crate::apoio_teste::{DirTemp, Ligacao};

/// Quanto a copia fica parada com a ficha na mao.
const PAUSA: Duration = Duration::from_millis(1500);

fn falar(porta: u16, corpo: &str) -> Json {
    let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
    let r = Ligacao::nova(porta)
        .pedir(&format!("{{\"token\":\"t\",{corpo}}}"))
        .unwrap_or_else(|| panic!("a conexao caiu sem resposta: {corpo}"));
    let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
    assert!(j.booleano_ou("ok", false), "{corpo}: {r}");
    j.campo("resultado").cloned().unwrap_or(j)
}

/// O `.reg` da tabela, achado andando a arvore -- o lugar exato dele e do
/// formato, e esta prova nao depende disso.
fn achar_reg(raiz: &std::path::Path, nome: &str) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(raiz).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(a) = achar_reg(&p, nome) {
                return Some(a);
            }
        } else if p.file_name().is_some_and(|n| n == nome) {
            return Some(p);
        }
    }
    None
}

/// **A leitura feita durante o backup NAO espera a copia, mesmo com um
/// escritor esperando; a escrita espera, e o retrato nao a contem.**
///
/// Os dois defeitos que esta prova derruba, cada um com a sua guarda no
/// catalogo: a copia sob a ficha EXCLUSIVA (o de antes -- a leitura
/// espera a pausa inteira) e a ficha compartilhada SEM o portao do
/// retrato (a H2 ingenua -- o escritor na fila do `RwLock` faz a leitura
/// nova esperar junto). Por isso o escritor entra ANTES da leitura: lendo
/// antes de gravar, a H2 ingenua passaria por engano.
#[test]
fn a_leitura_nao_espera_o_backup_e_a_escrita_espera() {
    let dir = DirTemp::novo("retrato-513");
    let destino = dir.with_file_name(format!(
        "{}-backup",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&destino);
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    let s = Arc::new(Servidor::novo(c).unwrap());
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let fio = {
        let s = Arc::clone(&s);
        std::thread::spawn(move || s.aceitar_ate_mandarem_parar(&ouvinte))
    };

    falar(porta, r#""op":"criar_database","database":"loja""#);
    falar(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(20)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
    );
    for id in 1..=5 {
        falar(
            porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "valores":{{"id":{id},"nome":"C{id}"}}"#
            ),
        );
    }
    // A primeira leitura depois das gravacoes CURA o cabecalho do `.log`,
    // que so vai a disco no `sincronizar` (a janela `por_lote`): ate la a
    // ficha compartilhada recusa abrir a tabela e o `varrer` recua para a
    // exclusiva. Durante um backup esse recuo ESPERA a copia -- e o limite
    // medido do passo 1, escrito no MANUAL §11: so a tabela gravada na
    // ultima janela antes do backup paga. Aqui a cura vem antes, para a
    // prova medir o portao e nao a janela.
    falar(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":1"#,
    );
    let reg = achar_reg(&dir, "clientes.reg").expect("o .reg da tabela");
    let antes = std::fs::metadata(&reg).unwrap().len();

    // O gancho NAO tem o nome da operacao: o `despachar` arma pelo nome
    // da op ANTES de qualquer trava, e a pausa ali pararia so a conexao
    // -- a prova passaria por engano, ou cairia pelo motivo errado.
    *s.panico_de_teste_na_op.lock().unwrap() = Some((
        "backup_copia".into(),
        PanicoDeTeste::Pausa(PAUSA, "pausa de teste na copia do backup (pedido 513)"),
    ));
    let backup = {
        let d = destino.display().to_string();
        std::thread::spawn(move || {
            falar(porta, &format!(r#""op":"backup","destino":"{d}""#));
        })
    };
    // A arma sai do campo quando a copia a dispara: dali em diante a
    // ficha da copia esta na mao e a pausa correndo. Prazo de 10 s.
    let ate = Instant::now() + Duration::from_secs(10);
    while s.panico_de_teste_na_op.lock().unwrap().is_some() {
        assert!(Instant::now() < ate, "a copia do backup nunca comecou");
        std::thread::sleep(Duration::from_millis(2));
    }
    let pausou = Instant::now();

    let escrita = std::thread::spawn(move || {
        let t = Instant::now();
        falar(
            porta,
            r#""op":"inserir","database":"loja","tabela":"clientes",
                   "valores":{"id":6,"nome":"C6"}"#,
        );
        t.elapsed()
    });
    // O escritor chega primeiro e fica esperando -- e e com ele esperando
    // que a leitura tem de passar.
    std::thread::sleep(Duration::from_millis(150));
    let t = Instant::now();
    let pagina = falar(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    );
    let leitura = t.elapsed();
    let restava = PAUSA.saturating_sub(pausou.elapsed());

    let espera_da_escrita = escrita.join().unwrap();
    backup.join().unwrap();
    let depois = falar(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    );
    let conferido = falar(
        porta,
        &format!(
            r#""op":"conferir_backup","destino":"{}""#,
            destino.display()
        ),
    );
    let copiado = achar_reg(&destino, "clientes.reg").map(|p| std::fs::metadata(p).unwrap().len());

    s.parar_de_aceitar.store(true, Ordering::SeqCst);
    let _ = TcpStream::connect(("127.0.0.1", porta));
    let _ = fio.join();
    let _ = std::fs::remove_dir_all(&destino);

    eprintln!(
        "513: leitura durante o backup {} ms, escrita {} ms (pausa {} ms)",
        leitura.as_millis(),
        espera_da_escrita.as_millis(),
        PAUSA.as_millis()
    );
    assert!(
        leitura < PAUSA / 4,
        "a leitura ESPEROU o backup: {leitura:?} com a copia parada {PAUSA:?} \
             (restavam {restava:?} quando ela terminou)"
    );
    let linhas = |j: &Json| j.escrever().matches("\"C").count();
    assert_eq!(
        linhas(&pagina),
        5,
        "a leitura do meio do backup viu a escrita que devia estar \
             esperando: {}",
        pagina.escrever()
    );
    assert!(
        espera_da_escrita >= PAUSA / 2,
        "a escrita NAO esperou a copia: {espera_da_escrita:?} com a copia \
             parada {PAUSA:?}"
    );
    assert_eq!(
        linhas(&depois),
        6,
        "a escrita se perdeu: {}",
        depois.escrever()
    );
    assert!(
        conferido.booleano_ou("integro", false),
        "o backup nao confere: {}",
        conferido.escrever()
    );
    assert_eq!(
        copiado,
        Some(antes),
        "o retrato tem o .reg de outro instante: a escrita entrou no meio \
             da copia"
    );
}

// ------------------------------------------------ o passo 2 do 513

/// Um servidor com `loja.clientes` (5 linhas, cabecalho curado) no ar,
/// e o destino irmao do backup -- o cenario das provas do passo 2.
struct Loja {
    s: Arc<Servidor>,
    dir: DirTemp,
    destino: std::path::PathBuf,
    porta: u16,
    fio: Option<std::thread::JoinHandle<()>>,
}

impl Loja {
    fn subir(nome: &str) -> Loja {
        let dir = DirTemp::novo(nome);
        let destino = dir.with_file_name(format!(
            "{}-backup",
            dir.file_name().unwrap().to_string_lossy()
        ));
        let _ = std::fs::remove_dir_all(&destino);
        let mut c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            jobs: dir.join("jobs.json"),
            token: "t".into(),
            ..Config::default()
        };
        c.cifra_fio.exigir = false;
        let s = Servidor::novo(c).unwrap();
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let fio = {
            let s = Arc::clone(&s);
            std::thread::spawn(move || s.aceitar_ate_mandarem_parar(&ouvinte))
        };
        falar(porta, r#""op":"criar_database","database":"loja""#);
        falar(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"nome","tipo":"Str(20)"}],
                   "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
        );
        for id in 1..=5 {
            falar(
                porta,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                           "valores":{{"id":{id},"nome":"C{id}"}}"#
                ),
            );
        }
        // A cura do cabecalho do `.log` antes do backup -- ver a prova
        // do passo 1, acima.
        falar(
            porta,
            r#""op":"varrer","database":"loja","tabela":"clientes","max":1"#,
        );
        Loja {
            s,
            dir,
            destino,
            porta,
            fio: Some(fio),
        }
    }

    /// Arma a pausa no gancho `gancho` e dispara um backup em arvore
    /// noutra thread; volta quando a pausa COMECOU.
    fn backup_pausado(&self, gancho: &str) -> std::thread::JoinHandle<Json> {
        *self.s.panico_de_teste_na_op.lock().unwrap() = Some((
            gancho.into(),
            PanicoDeTeste::Pausa(PAUSA, "pausa de teste no backup (pedido 513, passo 2)"),
        ));
        let (porta, d) = (self.porta, self.destino.display().to_string());
        let backup =
            std::thread::spawn(move || falar(porta, &format!(r#""op":"backup","destino":"{d}""#)));
        let ate = Instant::now() + Duration::from_secs(10);
        while self.s.panico_de_teste_na_op.lock().unwrap().is_some() {
            assert!(
                Instant::now() < ate,
                "o backup nunca chegou ao gancho {gancho}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        backup
    }

    /// A resposta CRUA (com `ok` falso quando recusa).
    fn cru(&self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        let r = Ligacao::nova(self.porta)
            .pedir(&format!("{{\"token\":\"t\",{corpo}}}"))
            .unwrap_or_else(|| panic!("a conexao caiu sem resposta: {corpo}"));
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }

    fn reg_vivo(&self) -> u64 {
        let reg = achar_reg(&self.dir, "clientes.reg").expect("o .reg da tabela");
        std::fs::metadata(reg).unwrap().len()
    }

    fn reg_copiado(&self) -> Option<u64> {
        achar_reg(&self.destino, "clientes.reg").map(|p| std::fs::metadata(p).unwrap().len())
    }
}

impl Drop for Loja {
    fn drop(&mut self) {
        self.s.parar_de_aceitar.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.porta));
        if let Some(f) = self.fio.take() {
            let _ = f.join();
        }
        let _ = std::fs::remove_dir_all(&self.destino);
    }
}

/// **Passo 2: a escrita NAO espera a fase 1, e o retrato e o do FIM.**
///
/// A copia para no gancho `backup_fase_1` -- depois de copiar tudo, antes
/// da trava. Um `inserir` nessa janela tem de voltar na hora (com a trava
/// na fase 1, guarda `backup-fase-1-sob-a-trava`, ele espera a pausa
/// inteira). E a copia que sai tem de conter essa linha: a fase 2
/// recopia o `.reg` que mudou (sem a fase 2, guarda `backup-sem-fase-2`,
/// o `.reg` copiado e o de antes da escrita e o `conferir` aprova o
/// retrato ERRADO -- por isso a prova compara o tamanho do `.reg` vivo
/// com o copiado, e nao so o `conferir`).
#[test]
fn a_escrita_nao_espera_a_fase_1_e_o_retrato_e_o_do_fim() {
    let loja = Loja::subir("retrato-513p2");
    let antes = loja.reg_vivo();
    let backup = loja.backup_pausado("backup_fase_1");

    let t = Instant::now();
    falar(
        loja.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":6,"nome":"C6"}"#,
    );
    let escrita = t.elapsed();
    let vivo = loja.reg_vivo();
    assert!(vivo > antes, "a premissa: a escrita cresce o .reg");

    let resposta = backup.join().unwrap();
    let conferido = falar(
        loja.porta,
        &format!(
            r#""op":"conferir_backup","destino":"{}""#,
            loja.destino.display()
        ),
    );
    eprintln!(
        "513/2: escrita durante a fase 1 {} ms (pausa {} ms); resposta {}",
        escrita.as_millis(),
        PAUSA.as_millis(),
        resposta.escrever()
    );
    assert!(
        escrita < PAUSA / 4,
        "a escrita ESPEROU a fase 1 do backup: {escrita:?} com a copia parada {PAUSA:?}"
    );
    assert_eq!(resposta.texto_ou("modo", ""), "duas_passadas");
    let fase_2 = resposta
        .campo("fase_2")
        .expect("o bloco fase_2 na resposta");
    assert!(
        fase_2.inteiro_ou("arquivos", 0) >= 1,
        "a fase 2 nao recopiou nada: {}",
        resposta.escrever()
    );
    assert!(
        resposta.inteiro_ou("retrato_ms", 0) > 0,
        "sem retrato_ms: {}",
        resposta.escrever()
    );
    assert!(
        conferido.booleano_ou("integro", false),
        "o backup nao confere: {}",
        conferido.escrever()
    );
    assert_eq!(
        loja.reg_copiado(),
        Some(vivo),
        "o retrato NAO e o do fim: o .reg copiado e o de antes da escrita feita \
             durante a fase 1"
    );
    let manifesto = std::fs::read_to_string(loja.destino.join("backup.json")).unwrap();
    let manifesto = Json::analisar(&manifesto).unwrap();
    assert!(
        manifesto.inteiro_ou("retrato_ms", 0) >= manifesto.inteiro_ou("quando_ms", 0),
        "o manifesto nao carrega o retrato_ms: {}",
        manifesto.escrever()
    );
}

/// **Passo 2: a manutencao espera o backup; o dado nao.**
///
/// Com a copia parada na fase 1, `criar_tabela` (que nao congela) e
/// `acrescentar_coluna` (que congela) recusam com 4006 e `repetir:
/// true`; o `inserir` passa. Depois do backup, as duas passam. Guarda
/// `manutencao-durante-o-retrato`: sem a pergunta no `congelar`, a
/// reescrita entra no meio da copia.
#[test]
fn a_manutencao_espera_o_backup_e_o_dado_nao() {
    let loja = Loja::subir("retrato-513p2-ddl");
    let backup = loja.backup_pausado("backup_fase_1");

    let criar = loja.cru(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    );
    let coluna = loja.cru(
        r#""op":"acrescentar_coluna","database":"loja","tabela":"clientes",
               "coluna":{"nome":"email","tipo":"Str(40)"}"#,
    );
    let dado = loja.cru(
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":7,"nome":"C7"}"#,
    );
    backup.join().unwrap();
    let depois = loja.cru(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    );
    let coluna_depois = loja.cru(
        r#""op":"acrescentar_coluna","database":"loja","tabela":"clientes",
               "coluna":{"nome":"email","tipo":"Str(40)"}"#,
    );

    for (nome, r) in [("criar_tabela", &criar), ("acrescentar_coluna", &coluna)] {
        assert!(
            !r.booleano_ou("ok", true),
            "{nome} ENTROU durante a fase 1 do backup: {}",
            r.escrever()
        );
        assert_eq!(
            r.inteiro_ou("codigo", 0),
            4006,
            "{nome}: codigo errado: {}",
            r.escrever()
        );
        assert!(
            r.booleano_ou("repetir", false),
            "{nome}: a recusa nao diz repetir: {}",
            r.escrever()
        );
    }
    assert!(
        dado.booleano_ou("ok", false),
        "o inserir ESPEROU ou recusou durante a fase 1: {}",
        dado.escrever()
    );
    assert!(
        depois.booleano_ou("ok", false),
        "depois do backup o criar_tabela continua recusado: {}",
        depois.escrever()
    );
    assert!(
        coluna_depois.booleano_ou("ok", false),
        "depois do backup o acrescentar_coluna continua recusado: {}",
        coluna_depois.escrever()
    );
}

/// **Comportamento velho: sem backup em curso, a manutencao passa na
/// hora** -- e o `zip` em duas passadas sai integro e sem a arvore
/// temporaria no destino.
#[test]
fn sem_backup_a_manutencao_passa_e_o_zip_sai_em_duas_passadas() {
    let loja = Loja::subir("retrato-513p2-zip");
    let criar = loja.cru(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    );
    assert!(criar.booleano_ou("ok", false), "{}", criar.escrever());
    let r = falar(
        loja.porta,
        &format!(
            r#""op":"backup","destino":"{}","zip":true"#,
            loja.destino.display()
        ),
    );
    assert_eq!(r.texto_ou("modo", ""), "duas_passadas", "{}", r.escrever());
    let zip = std::path::PathBuf::from(r.texto_ou("arquivo", ""));
    assert!(zip.is_file(), "o zip nao existe: {}", r.escrever());
    let sobras: Vec<String> = std::fs::read_dir(&loja.destino)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".retrato.part"))
        .collect();
    assert!(
        sobras.is_empty(),
        "a arvore temporaria ficou no destino: {sobras:?}"
    );
}

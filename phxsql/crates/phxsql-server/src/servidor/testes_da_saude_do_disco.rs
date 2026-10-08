use super::*;
use crate::apoio_teste::{rele_falso, DirTemp};
use crate::usuarios::{Nivel, Permissoes};

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O ESCAPE ESCRITO, e ele esta aqui por assunto: desde 18/09/2026 a
    // cifra do fio nasce exigida (pedido 370), e estes testes conectam em
    // claro porque o que eles medem e OUTRA coisa. Sem esta linha, a
    // recusa que eles leriam seria a da cifra, e a prova mediria o portao
    // errado -- teste que passa (ou falha) por engano.
    c.cifra_fio.exigir = false;
    c
}

/// **Pedido 255, decisao do dono de 30/09/2026:** o arranque que
/// reconstroi indice marcado tambem AVISA pelo carteiro da saude do
/// disco, e nao so no `stderr`. E o arranque de sempre nao avisa nada.
///
/// # Prova real
///
/// Sem o `evento_do_arranque` no `Servidor::novo`, a fila sai vazia com
/// o indice reconstruido -- o vermelho medido.
#[test]
fn o_arranque_que_reconstroi_indice_avisa_pelo_carteiro() {
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;
    let dir = DirTemp::novo("255-arranque");
    {
        let inst = phxsql_store::catalogo::Instancia::nova(&*dir).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let e = Schema::new(
            "itens",
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = db.criar_tabela(None, e).unwrap();
        t.inserir(&[Value::Int(1)]).unwrap();
        t.sincronizar().unwrap();
    }
    // O arranque de sempre: nada a dizer.
    let limpo = Servidor::novo(config_base(&dir)).unwrap();
    assert_eq!(limpo.saude.na_fila(), 0, "o arranque limpo avisou");
    drop(limpo);

    // A queda que deixa o `.ndx` marcado: escrita sem `sincronizar` -- o
    // `fechar` nao baixa o byte 52 (pedido 522).
    {
        let inst = phxsql_store::catalogo::Instancia::nova(&*dir).unwrap();
        let mut t = inst
            .abrir_database("loja")
            .unwrap()
            .abrir_qualificada("itens")
            .unwrap();
        t.inserir(&[Value::Int(2)]).unwrap();
    }
    // O processo NOVO de depois da queda: o atestado deste nao vale la.
    phxsql_store::ndx::esquecer_atestados_para_teste(&dir);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let eventos = s.saude.esperar(std::time::Duration::ZERO);
    let ev = eventos
        .iter()
        .find(|e| e.tipo == crate::saude_do_disco::Tipo::Arranque)
        .unwrap_or_else(|| panic!("o arranque reconstruiu e nao avisou: {eventos:?}"));
    assert!(ev.texto.starts_with("1 indice(s)"), "{}", ev.texto);
    // Nao e erro do disco: o painel continua sem evento de disco.
    assert!(s.saude.ultimo_evento().is_none());
    drop(s);

    // **Pedido 575:** o cabecalho do `.ndx` RASGADO -- byte mexido sem o
    // CRC acompanhar, o que uma queda no meio da pagina 0 deixa. Antes a
    // tabela nem abria, e o arranque a deixava pendente («cabecalho com
    // CRC invalido», medido); agora ela abre, o arranque a reconstroi pelo
    // `.reg`, avisa, e o indice acha as duas linhas.
    let ndx = dir.join("loja").join("itens.ndx");
    let mut b = std::fs::read(&ndx).unwrap();
    b[52] ^= 1;
    std::fs::write(&ndx, b).unwrap();
    phxsql_store::ndx::esquecer_atestados_para_teste(&dir);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let eventos = s.saude.esperar(std::time::Duration::ZERO);
    let ev = eventos
        .iter()
        .find(|e| e.tipo == crate::saude_do_disco::Tipo::Arranque)
        .unwrap_or_else(|| panic!("o cabecalho rasgado nao avisou: {eventos:?}"));
    assert!(
        ev.texto.starts_with("1 indice(s)") && !ev.texto.contains("NAO se"),
        "o cabecalho rasgado ficou pendente: {}",
        ev.texto
    );
    drop(s);
    let inst = phxsql_store::catalogo::Instancia::nova(&*dir).unwrap();
    let mut t = inst
        .abrir_database("loja")
        .unwrap()
        .abrir_qualificada("itens")
        .unwrap();
    for id in [1, 2] {
        assert_eq!(
            t.buscar("porId", &[Value::Int(id)]).unwrap().len(),
            1,
            "id {id}"
        );
    }
}

/// Liga o rele falso e, se pedido, o SMS pelo gateway.
fn com_rele(c: &mut Config, porta: u16, sms: bool) {
    c.alertas.email.ligado = true;
    c.alertas.email.servidor = "127.0.0.1".into();
    c.alertas.email.porta = porta;
    c.alertas.email.de = "phxsql@exemplo.com".into();
    c.alertas.email.para = vec!["admin@exemplo.com".into()];
    c.alertas.email.timeout_s = 5;
    if sms {
        c.alertas.sms.ligado = true;
        c.alertas.sms.numeros = vec!["+5541999990000".into()];
        c.alertas.sms.gateway_email = "sms.exemplo".into();
    }
}

/// Um banco com uma tabela -- e depois o `.reg` dela trocado por um
/// DIRETORIO: e o sistema operacional recusando a escrita seguinte, e
/// nao um erro fabricado. O `inserir` que vem depois recebe um
/// `PhxError::Io` de verdade.
fn com_tabela_quebrada(s: &Arc<Servidor>, dir: &std::path::Path) {
    let sessao = Sessao::default();
    s.executar(
        "criar_database",
        &Json::analisar(r#"{"database":"b"}"#).unwrap(),
        &sessao,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &Json::analisar(r#"{"database":"b","tabela":"t","colunas":[{"nome":"n","tipo":"Int8"}]}"#)
            .unwrap(),
        &sessao,
    )
    .unwrap();
    s.executar(
        "inserir",
        &Json::analisar(r#"{"database":"b","tabela":"t","valores":{"n":1}}"#).unwrap(),
        &sessao,
    )
    .unwrap();
    let reg = dir.join("b").join("t.reg");
    assert!(reg.exists(), "o .reg tem de existir para ser trocado");
    std::fs::remove_file(&reg).unwrap();
    std::fs::create_dir(&reg).unwrap();
}

/// Sobe a porta de dados pelo laco DE PRODUCAO: e o `atender` quem chama
/// o `anotar`, e o gancho mora no `anotar`. Um `despachar` direto, como
/// nos outros testes, provaria a recusa e pularia o gancho.
fn porta_de_dados_de_verdade(s: &Arc<Servidor>) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let s = Arc::clone(s);
    std::thread::spawn(move || s.aceitar_ate_mandarem_parar(&ouvinte));
    porta
}

/// Um `inserir` pelo soquete, na tabela quebrada. Devolve o `codigo` da
/// resposta de erro.
fn inserir_quebrado(porta: u16) -> u64 {
    let linha = crate::apoio_teste::Ligacao::tentar_com_prazo(porta, Duration::from_secs(5))
        .expect("a porta de dados recusou a conexao")
        .pedir(r#"{"token":"t","op":"inserir","database":"b","tabela":"t","valores":{"n":2}}"#)
        .expect("o inserir na tabela quebrada caiu sem resposta");
    let r = Json::analisar(&linha).unwrap();
    assert!(
        !r.booleano_ou("ok", true),
        "a tabela quebrada aceitou: {linha}"
    );
    r.campo("codigo").and_then(Json::numero).unwrap_or(0.0) as u64
}

fn corpo_do_email(bruto: &str) -> (String, String) {
    let (cabecalho, corpo) = bruto.split_once("\r\n\r\n").unwrap();
    let texto = phxsql_core::base64::decodificar_texto(&corpo.replace("\r\n", "")).unwrap();
    (cabecalho.to_string(), texto)
}

// ------------------------------------- o gancho externo do operador (249)

/// Liga o gancho do operador na configuracao do teste.
#[cfg(unix)]
fn com_gancho(c: &mut Config, comando: Vec<String>, timeout_s: u64) {
    c.alertas.gancho = crate::config::Gancho {
        ligado: true,
        comando,
        timeout_s,
    };
}

/// Espera o arquivo ter `linhas` linhas, ate `ate`. Devolve o que houver.
#[cfg(unix)]
fn esperar_linhas(caminho: &std::path::Path, linhas: usize, ate: Duration) -> String {
    let fim = Instant::now() + ate;
    loop {
        let t = std::fs::read_to_string(caminho).unwrap_or_default();
        if t.lines().count() >= linhas || Instant::now() >= fim {
            return t;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// O pedido do papel J: disco com defeito de verdade (o `.reg` virou
/// diretorio -> `EISDIR`; o contêiner roda como root e `chmod` nao
/// prova EROFS) dispara o gancho PELO SOQUETE, que passa pelo `anotar`.
/// O e-mail esta DESLIGADO de proposito: o gancho e um meio proprio, e
/// nao carona do rele. O segundo erro do mesmo tipo, dentro da janela,
/// nao executa nada (o silencio por tipo e o do carteiro, o mesmo).
///
/// Reponha o defeito (tire a chamada `avisar_pelo_gancho` do carteiro) e
/// este teste cai na espera do arquivo.
#[cfg(unix)]
#[test]
fn erro_de_es_numa_gravacao_chama_o_gancho_uma_vez_so() {
    let dir = DirTemp::novo("gancho-aviso");
    let apoio = crate::gancho::apoio_de_teste::dir("aviso");
    let saida = apoio.join("saida.txt");
    let s_ = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!(
            "{{ echo \"$PHXSQL_TIPO|$PHXSQL_ORIGEM|$PHXSQL_QUANDO\"; cat; }} >> {}",
            saida.display()
        ),
    );
    let mut c = config_base(&dir);
    com_gancho(&mut c, vec![s_], 5);
    assert!(!c.alertas.email.ligado, "o teste e do gancho SEM e-mail");
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    s.ligar_sonda_de_disco();
    let porta = porta_de_dados_de_verdade(&s);

    let inicio = Instant::now();
    assert_eq!(inserir_quebrado(porta), CODIGO_DE_ES as u64);
    let t = esperar_linhas(&saida, 2, Duration::from_secs(10));
    assert!(
        inicio.elapsed() < Duration::from_secs(5),
        "o gancho nao foi imediato"
    );
    let linhas: Vec<&str> = t.lines().collect();
    assert_eq!(linhas.len(), 2, "esperava variaveis + a linha: {t:?}");
    assert!(linhas[0].starts_with("entrada_saida|inserir|20"), "{t}");
    assert!(linhas[1].starts_with("PhxSql "), "{t}");
    assert!(linhas[1].contains("erro de E/S"), "{t}");
    assert!(linhas[1].chars().count() <= 160, "{t}");
    // Nunca o caminho do disco e nunca o pedido.
    let base = dir.display().to_string();
    assert!(!t.contains(&base), "o gancho recebeu o caminho: {t}");
    assert!(!t.contains("valores"), "o gancho recebeu o pedido: {t}");

    // O segundo erro do MESMO tipo, dentro da janela: nada executa.
    inserir_quebrado(porta);
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(
        std::fs::read_to_string(&saida).unwrap().lines().count(),
        2,
        "o segundo erro dentro da janela executou o gancho de novo"
    );
    // O painel conta o gancho entregue.
    let fim = Instant::now() + Duration::from_secs(5);
    let mut n = 0.0;
    while n < 1.0 && Instant::now() < fim {
        n = s
            .op_saude_disco(&Sessao::default())
            .campo("avisos")
            .and_then(|a| a.campo("gancho"))
            .and_then(Json::numero)
            .unwrap_or(0.0);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(n, 1.0, "o painel nao contou o gancho");
}

/// Tres erros do mesmo tipo seguidos -> UMA execucao (o silencio por
/// tipo), pelo `evento_de_disco` que e o que o `anotar` e o fecho
/// chamam. Um tipo DIFERENTE passa: o silencio e por tipo.
#[cfg(unix)]
#[test]
fn tres_erros_do_mesmo_tipo_executam_o_gancho_uma_vez() {
    let dir = DirTemp::novo("gancho-silencio");
    let apoio = crate::gancho::apoio_de_teste::dir("silencio");
    let saida = apoio.join("saida.txt");
    let g = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!("echo \"$PHXSQL_TIPO\" >> {}", saida.display()),
    );
    let mut c = config_base(&dir);
    com_gancho(&mut c, vec![g], 5);
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    use crate::saude_do_disco::Tipo;
    for _ in 0..3 {
        s.evento_de_disco(Tipo::EntradaSaida, "inserir", "b", "t", "x");
    }
    esperar_linhas(&saida, 1, Duration::from_secs(10));
    s.evento_de_disco(Tipo::SemEspaco, "inserir", "b", "t", "y");
    let t = esperar_linhas(&saida, 2, Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(400));
    let t2 = std::fs::read_to_string(&saida).unwrap();
    assert_eq!(t, t2, "execucao a mais depois do silencio");
    assert_eq!(
        t.lines().collect::<Vec<_>>(),
        vec!["entrada_saida", "sem_espaco"],
        "{t}"
    );
}

/// Sem `gancho.ligado` NADA executa -- o comportamento velho. O programa
/// esta configurado e e valido; so o interruptor esta desligado. E o
/// e-mail continua saindo, intacto.
#[cfg(unix)]
#[test]
fn sem_gancho_ligado_nada_executa_e_o_email_segue() {
    let dir = DirTemp::novo("gancho-off");
    let apoio = crate::gancho::apoio_de_teste::dir("off");
    let marca = apoio.join("rodou");
    let g = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!("echo x >> {}", marca.display()),
    );
    let (porta_rele, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta_rele, false);
    com_gancho(&mut c, vec![g], 5);
    c.alertas.gancho.ligado = false;
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    s.ligar_sonda_de_disco();
    inserir_quebrado(porta_de_dados_de_verdade(&s));
    caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o e-mail velho parou de sair");
    std::thread::sleep(Duration::from_millis(500));
    assert!(!marca.exists(), "executou com gancho.ligado = false");
}

/// ADITIVO: com o e-mail E o gancho ligados, os dois saem pelo mesmo
/// evento -- o gancho nao substitui o e-mail.
#[cfg(unix)]
#[test]
fn o_gancho_e_aditivo_o_email_sai_junto() {
    let dir = DirTemp::novo("gancho-aditivo");
    let apoio = crate::gancho::apoio_de_teste::dir("aditivo");
    let marca = apoio.join("rodou");
    let g = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!("echo x >> {}", marca.display()),
    );
    let (porta_rele, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta_rele, false);
    com_gancho(&mut c, vec![g], 5);
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    s.ligar_sonda_de_disco();
    inserir_quebrado(porta_de_dados_de_verdade(&s));
    caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o e-mail nao saiu com o gancho ligado");
    esperar_linhas(&marca, 1, Duration::from_secs(10));
    assert!(marca.exists(), "o gancho nao executou ao lado do e-mail");
}

/// Prazo duro, no servidor: o script dorme muito mais que `timeout_s`, o
/// filho morre e e COLHIDO (nao sobra zumbi em `/proc`), o erro vira
/// `avisos.ultima_falha` -- e o carteiro SEGUE: o evento seguinte, de
/// outro tipo, executa o gancho de novo.
#[cfg(all(unix, target_os = "linux"))]
#[test]
fn gancho_que_estoura_o_prazo_e_morto_e_o_carteiro_segue() {
    let dir = DirTemp::novo("gancho-prazo");
    let apoio = crate::gancho::apoio_de_teste::dir("prazo");
    let log = apoio.join("log.txt");
    let g = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!(
            "echo \"$PHXSQL_TIPO $$\" >> {}; exec sleep 60",
            log.display()
        ),
    );
    let mut c = config_base(&dir);
    com_gancho(&mut c, vec![g], 1);
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    use crate::saude_do_disco::Tipo;
    s.evento_de_disco(Tipo::EntradaSaida, "inserir", "b", "t", "x");
    s.evento_de_disco(Tipo::SemEspaco, "inserir", "b", "t", "y");
    // Dois prazos de 1 s, em fila: o carteiro nao ficou preso no primeiro.
    let t = esperar_linhas(&log, 2, Duration::from_secs(15));
    assert_eq!(t.lines().count(), 2, "o carteiro nao seguiu: {t:?}");
    let pids: Vec<u32> = t
        .lines()
        .map(|l| l.split(' ').nth(1).unwrap().parse().unwrap())
        .collect();
    // Espera o segundo kill acontecer (o prazo dele corre depois da linha).
    let fim = Instant::now() + Duration::from_secs(10);
    while pids
        .iter()
        .any(|p| std::path::Path::new(&format!("/proc/{p}")).exists())
        && Instant::now() < fim
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    for p in &pids {
        assert!(
            !std::path::Path::new(&format!("/proc/{p}")).exists(),
            "o filho {p} do gancho continua vivo ou zumbi"
        );
    }
    let falha = s
        .op_saude_disco(&Sessao::default())
        .campo("avisos")
        .and_then(|a| a.campo("ultima_falha"))
        .and_then(Json::texto)
        .map(str::to_string)
        .unwrap_or_default();
    assert!(
        falha.starts_with("gancho: ") && falha.contains("foi morto"),
        "ultima_falha = {falha:?}"
    );
}

/// A sentinela de segredo que o script imprime em stdout e stderr NAO
/// chega ao `acessos.log`, ao painel, a `op_saude_disco` NEM ao stderr do
/// servidor. O stderr so se prova em outro processo: o teste reexecuta
/// este binario (`filho_do_gancho_com_sentinela`) com `--nocapture` e
/// le o que o servidor de verdade escreveu.
#[cfg(unix)]
#[test]
fn a_sentinela_que_o_gancho_imprime_nao_vaza_para_lugar_nenhum() {
    const FILHO: &str = "servidor::testes_da_saude_do_disco::filho_do_gancho_com_sentinela";
    let apoio = crate::gancho::apoio_de_teste::dir("sentinela");
    let saida = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", FILHO, "--nocapture", "--test-threads=1"])
        .env("PHX_TESTE_FILHO_DO_GANCHO", apoio.as_os_str())
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&saida.stdout).to_string();
    let err = String::from_utf8_lossy(&saida.stderr).to_string();
    assert!(saida.status.success(), "o filho falhou:\n{out}\n{err}");
    assert!(
        out.contains("1 passed") || out.contains("test result: ok"),
        "{out}"
    );
    // O servidor DE VERDADE falou do gancho no stderr: sem isto, a
    // ausencia da sentinela nao provaria nada.
    assert!(
        err.contains("gancho do operador NAO EXECUTADO: o gancho saiu com codigo 3"),
        "o carteiro nao escreveu o erro do gancho:\n{err}"
    );
    for (onde, texto) in [("stdout", &out), ("stderr", &err)] {
        assert!(
            !texto.contains("SENTINELA-DE-SEGREDO"),
            "a sentinela vazou para o {onde} do servidor:\n{texto}"
        );
    }
}

/// O corpo do teste acima, rodado no processo filho. Sem a variavel de
/// ambiente ele nao faz nada.
#[cfg(unix)]
#[test]
fn filho_do_gancho_com_sentinela() {
    let Ok(apoio) = std::env::var("PHX_TESTE_FILHO_DO_GANCHO") else {
        return;
    };
    let apoio = std::path::PathBuf::from(apoio);
    let dir = DirTemp::novo("gancho-sentinela");
    let marca = apoio.join("rodou");
    let g = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!(
            "echo SENTINELA-DE-SEGREDO-STDOUT; echo SENTINELA-DE-SEGREDO-STDERR >&2; \
                 echo x >> {}; exit 3",
            marca.display()
        ),
    );
    let mut c = config_base(&dir);
    com_gancho(&mut c, vec![g], 5);
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    s.ligar_sonda_de_disco();
    inserir_quebrado(porta_de_dados_de_verdade(&s));
    esperar_linhas(&marca, 1, Duration::from_secs(10));
    // A falha do gancho vira `ultima_falha`: espera por ela.
    let fim = Instant::now() + Duration::from_secs(10);
    let mut saude = s.op_saude_disco(&Sessao::default());
    while saude
        .campo("avisos")
        .and_then(|a| a.campo("ultima_falha"))
        .and_then(Json::texto)
        .is_none()
        && Instant::now() < fim
    {
        std::thread::sleep(Duration::from_millis(25));
        saude = s.op_saude_disco(&Sessao::default());
    }
    let falha = saude
        .campo("avisos")
        .and_then(|a| a.campo("ultima_falha"))
        .and_then(Json::texto)
        .unwrap_or_default()
        .to_string();
    assert_eq!(falha, "gancho: o gancho saiu com codigo 3");
    // Da carga: o eprintln do carteiro tem de sair ANTES de o processo
    // acabar; o log e o painel sao lidos agora.
    std::thread::sleep(Duration::from_millis(200));
    let log = std::fs::read_to_string(dir.join("acessos.log")).unwrap_or_default();
    let painel = s.op_painel(&Sessao::default()).unwrap().escrever();
    for (onde, texto) in [
        ("acessos.log", log),
        ("painel", painel),
        ("saude", saude.escrever()),
    ] {
        assert!(
            !texto.contains("SENTINELA-DE-SEGREDO"),
            "a sentinela vazou para {onde}"
        );
    }
}

/// **Pedido 641, medido antes de consertar.** A hipotese era que um
/// `PhxError::Io` causado pelo CAMINHO que o usuario digitou (e nao pelo
/// disco do banco) disparava o aviso e o gancho. Medido pelo soquete no
/// codigo de antes: `profiler_ligar` com `arquivo` apontando para um
/// diretorio (EISDIR) e `backup` com `destino` inexistente (ENOENT)
/// voltaram `codigo 5001` e o gancho EXECUTOU (`entrada_saida|
/// profiler_ligar`). Aqui os dois voltam como erro do pedido e o gancho
/// nao roda; o controle no fim prova que o portao continua ABERTO para o
/// disco de verdade (o `.reg` trocado por diretorio).
///
/// Reponha o defeito (tire o `do_caminho_pedido` do `op_backup` ou volte
/// o `Io` do `profiler::ligar`) e o gancho executa antes do controle.
#[cfg(unix)]
#[test]
fn io_do_caminho_do_usuario_nao_avisa_o_disco() {
    let dir = DirTemp::novo("caminho641");
    let apoio = crate::gancho::apoio_de_teste::dir("caminho641");
    let saida = apoio.join("saida.txt");
    let s_ = crate::gancho::apoio_de_teste::script(
        &apoio,
        "g.sh",
        &format!(
            "echo \"$PHXSQL_TIPO|$PHXSQL_ORIGEM\" >> {}",
            saida.display()
        ),
    );
    let mut c = config_base(&dir);
    com_gancho(&mut c, vec![s_], 5);
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    let porta = porta_de_dados_de_verdade(&s);
    let alvo_dir = dir.join("umdir");
    std::fs::create_dir(&alvo_dir).unwrap();
    let falso_zip = dir.join("falso.zip");
    std::fs::create_dir(&falso_zip).unwrap();
    for corpo in [
        format!(
            r#"{{"token":"t","op":"profiler_ligar","arquivo":"{}"}}"#,
            alvo_dir.display()
        ),
        format!(
            r#"{{"token":"t","op":"restaurar_backup","origem":"{}","simular":true}}"#,
            falso_zip.display()
        ),
        format!(
            r#"{{"token":"t","op":"backup","destino":"{}"}}"#,
            "/proc/nao-existe/x"
        ),
        format!(
            r#"{{"token":"t","op":"conferir_backup","destino":"{}"}}"#,
            falso_zip.display()
        ),
    ] {
        let linha = crate::apoio_teste::Ligacao::tentar_com_prazo(porta, Duration::from_secs(5))
            .unwrap()
            .pedir(&corpo)
            .unwrap();
        let r = Json::analisar(&linha).unwrap();
        assert!(!r.booleano_ou("ok", true), "{corpo} -> {linha}");
        assert_ne!(
            r.campo("codigo").and_then(Json::numero).unwrap_or(0.0) as u64,
            CODIGO_DE_ES as u64,
            "o erro do caminho pedido virou erro de E/S: {corpo} -> {linha}"
        );
    }
    // O carteiro acorda na hora; um segundo e folga de sobra.
    std::thread::sleep(Duration::from_millis(1000));
    assert!(
        !saida.exists(),
        "o gancho executou por erro de caminho digitado: {:?}",
        std::fs::read_to_string(&saida)
    );

    // Controle: o disco de verdade (o `.reg` virou diretorio) continua
    // chamando o gancho, na mesma configuracao.
    com_tabela_quebrada(&s, &dir);
    assert_eq!(inserir_quebrado(porta), CODIGO_DE_ES as u64);
    let t = esperar_linhas(&saida, 1, Duration::from_secs(10));
    assert!(t.starts_with("entrada_saida|inserir"), "{t:?}");
}

/// O `Io` de um caminho pedido sai do alerta; o do disco fica. Os dois
/// lados da regra, na funcao que decide.
#[test]
fn do_caminho_pedido_separa_os_dois_io() {
    use std::io::{Error, ErrorKind};
    let dir = DirTemp::novo("caminho_pedido");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let io = |k: ErrorKind| -> Result<()> { Err(PhxError::Io(Error::from(k))) };
    // Operacao que le o banco: so as formas de caminho/permissao saem.
    for k in [
        ErrorKind::NotFound,
        ErrorKind::PermissionDenied,
        ErrorKind::NotADirectory,
    ] {
        let e = s.do_caminho_pedido(io(k), "/x", false).unwrap_err();
        assert_eq!(e.codigo(), 2001, "{k:?} deveria ser erro do pedido");
    }
    for k in [
        ErrorKind::ReadOnlyFilesystem,
        ErrorKind::StorageFull,
        ErrorKind::Other,
        ErrorKind::TimedOut,
    ] {
        let e = s.do_caminho_pedido(io(k), "/x", false).unwrap_err();
        assert_eq!(e.codigo(), CODIGO_DE_ES, "{k:?} e do disco e tem de avisar");
    }
    // Operacao que so le o caminho do pedido: todo `Io` e dele.
    let e = s
        .do_caminho_pedido(io(ErrorKind::Other), "/x", true)
        .unwrap_err();
    assert_eq!(e.codigo(), 2001);
    // E o que nao e `Io` passa intacto.
    let e = s
        .do_caminho_pedido::<()>(Err(PhxError::NaoEncontrado("n".into())), "/x", true)
        .unwrap_err();
    assert_eq!(e.codigo(), 3001);
}

/// **Pedido 643.** `database`/`tabela` chegam do pedido de um usuario e
/// entram na linha do SMS e do stdin do gancho. So CR/LF eram trocados:
/// um `ESC[2J` (ou NUL, DEL, C1) atravessava para o terminal do operador
/// e para um `eval` descuidado do script dele. A linha sai SEM controle
/// nenhum e no maximo com 160 caracteres.
///
/// Reponha o defeito (o `map` antigo, so `\r` e `\n`) e o ESC aparece.
#[test]
fn a_linha_do_sms_e_do_gancho_nao_leva_controle_do_usuario() {
    use crate::saude_do_disco::{Evento, Tipo};
    let e = Evento {
        quando_ms: 1_790_000_000_000,
        tipo: Tipo::EntradaSaida,
        origem: "inserir".into(),
        database: "b\x1b[2J\x07\r\n".into(),
        tabela: "t\0\x7f\u{85}`id`$(id);".into(),
        texto: String::new(),
    };
    let linha = Servidor::texto_do_sms_de_saude(&e);
    assert!(
        !linha.chars().any(char::is_control),
        "controle na linha: {linha:?}"
    );
    assert!(linha.chars().count() <= 160, "{linha:?}");
    // O texto comum do nome continua la: higiene, nao apagamento.
    assert!(linha.contains("b [2J"), "{linha:?}");
    // E o corte vale mesmo para um nome gigante.
    let e2 = Evento {
        tabela: "t".repeat(1000),
        ..e
    };
    assert_eq!(Servidor::texto_do_sms_de_saude(&e2).chars().count(), 160);
}

/// O portao do gancho compara com 5001: se o codigo do `Io` mudar, este
/// teste cai antes de o gancho virar letra morta.
#[test]
fn o_codigo_do_gancho_e_o_do_erro_de_es() {
    assert_eq!(
        PhxError::Io(std::io::Error::other("x")).codigo(),
        CODIGO_DE_ES
    );
}

/// A PROVA DO PEDIDO DO DONO: erro de E/S numa gravacao avisa NA HORA
/// (sem esperar o relogio da sonda, que aqui e de 60 s), UMA vez -- o
/// segundo erro dentro da janela nao manda segundo e-mail --, e o painel
/// conta os dois.
///
/// Reponha o defeito tirando o `if acesso.codigo == CODIGO_DE_ES` do
/// `anotar` e este teste cai na espera do rele.
#[test]
fn erro_de_es_numa_gravacao_avisa_na_hora_e_uma_vez_so() {
    let dir = DirTemp::novo("saude-aviso");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta, false);
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    // O carteiro sobe no `servir`; aqui, sem porta, sobe a mao. E a
    // mesma thread da sonda, e e ela quem fala com o rele.
    s.ligar_sonda_de_disco();
    let porta = porta_de_dados_de_verdade(&s);

    let inicio = Instant::now();
    let codigo = inserir_quebrado(porta);
    assert_eq!(
        codigo, CODIGO_DE_ES as u64,
        "o defeito plantado nao e de E/S"
    );

    let bruto = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("nenhum e-mail chegou ao rele depois do erro de E/S");
    let levou = inicio.elapsed();
    assert!(
        levou < Duration::from_secs(5),
        "o aviso nao foi imediato: {levou:?}"
    );
    let (cabecalho, texto) = corpo_do_email(&bruto);
    assert!(cabecalho.contains("To: admin@exemplo.com"), "{cabecalho}");
    assert!(cabecalho.contains("saude do disco"), "{cabecalho}");
    assert!(texto.contains("origem     inserir (b/t)"), "{texto}");
    assert!(texto.contains("erro de E/S"), "{texto}");
    // Nunca o pedido: a linha que se tentou gravar nao viaja no e-mail.
    assert!(
        !texto.contains("valores"),
        "o corpo vazou o pedido: {texto}"
    );

    // O segundo erro do MESMO tipo, dentro da janela: nada sai.
    inserir_quebrado(porta);
    assert!(
        caixa.recv_timeout(Duration::from_millis(700)).is_err(),
        "o segundo erro dentro da janela mandou segundo e-mail"
    );
    // ...mas os dois contam, e o painel diz.
    let sessao = Sessao::default();
    let painel = s.op_painel(&sessao).unwrap();
    let bloco = painel.campo("saude_do_disco").unwrap();
    assert_eq!(bloco.campo("erros_es").and_then(Json::numero), Some(2.0));
    assert_eq!(bloco.campo("estado").and_then(Json::texto), Some("erro"));
    let ev = bloco.campo("ultimo_evento").unwrap();
    assert_eq!(ev.campo("origem").and_then(Json::texto), Some("inserir"));
    assert_eq!(ev.campo("tabela").and_then(Json::texto), Some("t"));
    // O e-mail foi contado como enviado.
    let avisos = bloco.campo("avisos").unwrap();
    // O contador e incrementado pela thread do aviso DEPOIS de o rele
    // responder; espera-se por ele em vez de ler na hora.
    let fim = Instant::now() + Duration::from_secs(5);
    let mut enviados = avisos.campo("email").and_then(Json::numero).unwrap_or(0.0);
    while enviados < 1.0 && Instant::now() < fim {
        std::thread::sleep(Duration::from_millis(50));
        enviados = s
            .op_saude_disco(&sessao)
            .campo("avisos")
            .and_then(|a| a.campo("email"))
            .and_then(Json::numero)
            .unwrap_or(0.0);
    }
    assert_eq!(enviados, 1.0, "o painel tem de contar o e-mail enviado");
}

/// O SMS sai pelo MESMO rele, como `numero@gateway`, numa linha so de ate
/// 160 caracteres e SEM o caminho do disco.
#[test]
fn o_sms_sai_pelo_gateway_da_operadora_numa_linha_sem_caminho() {
    let dir = DirTemp::novo("saude-sms");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta, true);
    let s = Servidor::novo(c).unwrap();
    com_tabela_quebrada(&s, &dir);
    s.ligar_sonda_de_disco();
    inserir_quebrado(porta_de_dados_de_verdade(&s));

    let primeiro = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o e-mail nao chegou");
    let segundo = caixa
        .recv_timeout(Duration::from_secs(10))
        .expect("o SMS nao chegou ao rele");
    let (cab_email, _) = corpo_do_email(&primeiro);
    let (cab_sms, texto_sms) = corpo_do_email(&segundo);
    assert!(cab_email.contains("To: admin@exemplo.com"), "{cab_email}");
    assert!(
        cab_sms.contains("To: +5541999990000@sms.exemplo"),
        "o SMS tem de ir para numero@gateway: {cab_sms}"
    );
    assert!(
        !texto_sms.contains('\n'),
        "SMS e uma linha so: {texto_sms:?}"
    );
    assert!(
        texto_sms.chars().count() <= 160,
        "{}",
        texto_sms.chars().count()
    );
    assert!(texto_sms.contains("erro de E/S"), "{texto_sms}");
    assert!(texto_sms.contains("inserir (b/t)"), "{texto_sms}");
    let base = dir.display().to_string();
    assert!(
        !texto_sms.contains(&base),
        "o SMS carregou o caminho do disco: {texto_sms}"
    );
    assert!(
        caixa.recv_timeout(Duration::from_millis(500)).is_err(),
        "mais mensagens do que um e-mail e um SMS"
    );
}

/// Guarda nova entra pedida, nao imposta: sem e-mail ligado, o erro de
/// E/S continua sendo respondido e contado -- e nenhum aviso sai.
#[test]
fn sem_email_ligado_o_erro_conta_e_nao_avisa() {
    let dir = DirTemp::novo("saude-calado");
    let (_, caixa) = rele_falso();
    let s = Servidor::novo(config_base(&dir)).unwrap();
    com_tabela_quebrada(&s, &dir);
    assert_eq!(
        inserir_quebrado(porta_de_dados_de_verdade(&s)),
        CODIGO_DE_ES as u64
    );
    assert!(caixa.recv_timeout(Duration::from_millis(500)).is_err());
    assert_eq!(s.saude.erros_es(), 1);
    // O gancho entregou ao carteiro mesmo sem rele -- o painel e quem
    // consome isso quando o e-mail esta desligado, e a fila nao cresce
    // sem limite porque o silencio por tipo segura a entrada.
    assert!(s.saude.na_fila() <= 1);
}

/// O IRMAO do gancho: o `fsync` do fecho recusado chega a mesma saude,
/// com origem `fecho` e a base/tabela da chave -- e um erro que nao e de
/// E/S (um panico virou `None` la em cima) nao conta como disco.
///
/// Prova o METODO. O sitio que o chama (`descarregar_sujas_com`) nao tem
/// prova com defeito reposto, porque nao ha como fazer um `fsync` falhar
/// sem injecao de falha no sistema de arquivos -- dito em
/// `docs/SAUDE-DO-DISCO.md` §6.
#[test]
fn o_fsync_do_fecho_recusado_conta_como_erro_de_disco() {
    let dir = DirTemp::novo("saude-fecho");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    s.fecho_recusado("b/t", &PhxError::Esquema("nao e disco".into()));
    assert_eq!(s.saude.erros_es(), 0, "erro que nao e de E/S nao conta");
    s.fecho_recusado("b/t", &PhxError::Io(std::io::Error::from_raw_os_error(5)));
    assert_eq!(s.saude.erros_es(), 1);
    let e = s.saude.ultimo_evento().unwrap();
    assert_eq!(e.origem, "fecho");
    assert_eq!((e.database.as_str(), e.tabela.as_str()), ("b", "t"));
    assert_eq!(e.tipo, crate::saude_do_disco::Tipo::EntradaSaida);
    s.fecho_recusado("b/t", &PhxError::Io(std::io::Error::from_raw_os_error(30)));
    assert_eq!(
        s.saude.ultimo_evento().unwrap().tipo,
        crate::saude_do_disco::Tipo::SoLeitura,
        "o EROFS do fecho e classificado pelo errno, como o da sonda"
    );
}

/// O bloco reduzido para quem so le: sem o texto do erro e sem o nome da
/// base. A op inteira para quem administra.
#[test]
fn quem_so_le_nao_ve_o_texto_nem_a_base_do_erro() {
    let dir = DirTemp::novo("saude-direito");
    // O leitor existe so na SESSAO: e o `sessao.usuario` que o portao
    // le. Cadastra-lo no config obrigaria o `inserir` do cenario a fazer
    // login, e o que se prova aqui e o bloco, nao o login.
    let leitor = Usuario {
        id: 7,
        nome: "Leitor".into(),
        login: "leitor".into(),
        senha_hash: String::new(),
        email: String::new(),
        telefone: String::new(),
        supervisor: false,
        ativo: true,
        nivel: Nivel::Nenhum,
        chave_publica: None,
        bases: vec![(
            "*".into(),
            Permissoes {
                ler: true,
                ..Permissoes::default()
            },
        )],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    };
    let s = Servidor::novo(config_base(&dir)).unwrap();
    com_tabela_quebrada(&s, &dir);
    inserir_quebrado(porta_de_dados_de_verdade(&s));

    let sessao_leitor = Sessao {
        usuario: Some(leitor),
        ..Sessao::default()
    };
    let reduzido = s.op_saude_disco(&sessao_leitor);
    let ev = reduzido.campo("ultimo_evento").unwrap();
    assert!(ev.campo("texto").is_none(), "{}", reduzido.escrever());
    assert!(ev.campo("database").is_none(), "{}", reduzido.escrever());
    assert_eq!(
        ev.campo("tipo").and_then(Json::texto),
        Some("entrada_saida")
    );

    let inteiro = s.op_saude_disco(&Sessao::default());
    let ev = inteiro.campo("ultimo_evento").unwrap();
    assert_eq!(ev.campo("database").and_then(Json::texto), Some("b"));
    assert!(ev.campo("texto").and_then(Json::texto).is_some());
}

//! A FICHA COMPARTILHADA do `varrer`: o que ela ganhou e o que ela nao pode
//! ter perdido.
//!
//! # O teste que mais importa neste modulo
//!
//! Nao e o que prova o ganho: e o `sem_a_ficha_compartilhada_nada_muda`. A
//! pista de leitura e uma decisao do servidor sobre COMO atender, e quem chama
//! nao pediu nada e nao pode notar nada -- nem no resultado, nem na trilha,
//! nem no espelho. Guarda nova entra pedida, nao imposta.
use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("fc-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor com uma tabela `c` de dez linhas, no database `b`.
fn servidor(nome: &str, espelho: bool) -> (Arc<Servidor>, DirTemp) {
    let dir = dir_temp(nome);
    let mut config = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        espelho,
        ..Config::default()
    };
    config.recursos.lote_operacoes = 1;
    let s = Servidor::novo(config).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"n","tipo":"Int8"},
                               {"nome":"cpf","tipo":"Str(20)"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for i in 1..=10 {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"n":{i},"cpf":"0{i}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    (s, dir)
}

fn varrer(s: &Arc<Servidor>, extra: &str) -> Json {
    s.executar(
        "varrer",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"c","max":4{extra}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

/// **O comportamento velho, e a mesma resposta pelas DUAS fichas.**
///
/// A ficha exclusiva e forcada sem mexer no pedido: sem o `.trash`, abrir
/// a tabela CRIARIA o arquivo -- e criar arquivo e escrever, entao a pista
/// de leitura recusa. O `.trash` voltar a existir depois da segunda
/// chamada e a prova de que ela desceu mesmo para a outra ficha; sem essa
/// conferencia, este teste passaria com as duas chamadas na mesma pista.
#[test]
fn sem_a_ficha_compartilhada_nada_muda() {
    let (s, dir) = servidor("nada-muda", false);
    let pela_compartilhada = varrer(&s, "");
    let trash = dir.join("b/c.trash");
    assert!(trash.exists(), "a tabela nasce com lixeira");
    std::fs::remove_file(&trash).unwrap();

    let pela_exclusiva = varrer(&s, "");
    assert!(
        trash.exists(),
        "a lixeira nao voltou: a segunda chamada nao desceu para a ficha \
             exclusiva, e entao este teste comparou a mesma pista com ela mesma"
    );
    assert_eq!(
        pela_compartilhada.escrever(),
        pela_exclusiva.escrever(),
        "as duas fichas responderam coisas diferentes ao MESMO pedido"
    );
}

/// A trilha de dado pessoal sobrevive: a tabela marcada desce para a ficha
/// exclusiva, e o registro de acesso continua sendo gravado.
///
/// **Defeito reposto**: tirar `t.tem_dado_pessoal()` do
/// `abrir_para_ler_travada` faz a leitura passar pela pista de leitura, que
/// nao sabe escrever -- e o total da trilha fica em zero. Trilha que perde
/// registro em silencio e pior que trilha nenhuma: ela PARECE completa.
#[test]
fn a_trilha_de_dado_pessoal_sobrevive_a_pista_de_leitura() {
    let (s, _dir) = servidor("trilha", false);
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"c","colunas":{"cpf":"pessoal"}}"#),
        &Sessao::default(),
    )
    .unwrap();
    varrer(&s, "");
    let r = s
        .executar(
            "trilha",
            &pedido(r#"{"database":"b","tabela":"c","tipo":"acesso"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let quantos = r
        .campo("registros")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap_or(0);
    assert!(
        quantos >= 1,
        "a varredura de uma tabela com coluna marcada nao deixou rastro \
             na trilha: a pista de leitura engoliu o registro"
    );
}

/// **Pedido 487: a exportacao da trilha pagina por cursor, e um expurgo
/// entre duas paginas nao faz o auditor pular registro.**
///
/// Sete registros em dois volumes (r1..r3 fechado, r4..r7 ativo). A
/// primeira pagina leva r1..r4; o expurgo derruba o volume de r1..r3. Pelo
/// `pular: 4` a segunda pagina sai VAZIA -- r5, r6 e r7 estao vivos e nunca
/// saem (o controle, que prova que o cenario desliza mesmo). Pelo
/// `depois_de` ela traz r5 e r6. E o cursor que o expurgo levou (r2) nao
/// vira silencio: a pagina sai pela comparacao dos UUIDs e diz
/// `cursor_achado: false`.
///
/// **Defeito reposto** (o `op_trilha` ignorando o `depois_de` e lendo so
/// pelo `pular`): a segunda pagina volta `[r4, r5]` em vez de `[r5, r6]`,
/// e a asserção do conteudo cai.
#[test]
fn a_trilha_pagina_por_cursor_e_o_expurgo_nao_faz_pular_registro() {
    let (s, _dir) = servidor("trilha-cursor", false);
    let dono = Sessao::default();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"c","colunas":{"cpf":"pessoal"}}"#),
        &dono,
    )
    .unwrap();
    let trilha = |extra: &str| -> Json {
        s.executar(
            "trilha",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","tipo":"acesso"{extra}}}"#
            )),
            &dono,
        )
        .unwrap()
    };
    let uuids = |r: &Json| -> Vec<String> {
        r.campo("registros")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .map(|e| e.texto_ou("uuid", "").to_string())
            .collect()
    };
    for _ in 0..3 {
        varrer(&s, "");
    }
    // Fecha o ativo sem derrubar nada: `ate` no passado nao alcanca
    // registro nenhum.
    s.executar(
        "expurgar_trilha",
        &pedido(
            r#"{"database":"b","tabela":"c","motivo":"corte","ate":"2000-01-01",
                    "fechar_ativo":true}"#,
        ),
        &dono,
    )
    .unwrap();
    for _ in 0..4 {
        varrer(&s, "");
    }
    let todos = uuids(&trilha(""));
    assert_eq!(todos.len(), 7, "o cenario nao montou sete acessos");

    let p1 = trilha(r#","limite":4"#);
    assert_eq!(uuids(&p1), todos[0..4].to_vec());
    let proximo = p1.texto_ou("proximo", "").to_string();
    assert_eq!(proximo, todos[3], "o cursor da pagina e o ultimo lido");

    std::thread::sleep(std::time::Duration::from_millis(3));
    s.executar(
        "expurgar_trilha",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"c","motivo":"prazo","ate_ms":{}}}"#,
            crate::agora_ms()
        )),
        &dono,
    )
    .unwrap();

    // O controle: pela contagem, r5..r7 somem da exportacao.
    assert!(
        uuids(&trilha(r#","limite":2,"pular":4"#)).is_empty(),
        "o cenario nao deslizou"
    );

    let p2 = trilha(&format!(r#","limite":2,"depois_de":"{proximo}""#));
    assert_eq!(
        uuids(&p2),
        todos[4..6].to_vec(),
        "o expurgo entre as paginas fez a exportacao pular registro vivo: {}",
        p2.escrever()
    );
    assert_eq!(p2.campo("cursor_achado"), Some(&Json::Bool(true)));

    // O cursor que o expurgo levou: a pagina sai, e DIZ por onde saiu.
    let p3 = trilha(&format!(r#","limite":2,"depois_de":"{}""#, todos[1]));
    assert_eq!(uuids(&p3), todos[3..5].to_vec());
    assert_eq!(p3.campo("cursor_achado"), Some(&Json::Bool(false)));

    // A ultima pagina diz que acabou.
    let p4 = trilha(&format!(r#","limite":2,"depois_de":"{}""#, todos[6]));
    assert!(uuids(&p4).is_empty());
    assert_eq!(p4.campo("proximo"), Some(&Json::Nulo));

    // Cursor e contagem juntos nao tem leitura unica: recusa.
    assert!(s
        .executar(
            "trilha",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","pular":1,"depois_de":"{proximo}"}}"#
            )),
            &dono,
        )
        .is_err());
}

/// **Pedido 600 (b), MEDIDO: o `motivos` pagina por `pular` e o expurgo
/// entre duas paginas NAO o faz perder registro -- a hipotese morreu.**
///
/// As duas hipoteses, escritas antes de medir: (H1) o `.reason` perde a
/// frente como o `.lgpd` perdia (487), e o `pular` desliza; (H2) o
/// `.reason` so cresce no fim, e o `pular` e estavel. Lido no fonte e
/// medido aqui: nenhum caminho apaga registro do `.reason` (o
/// `MotivoFile::apagar_tudo` nao tem chamador; so o `excluir_tabela` leva
/// o arquivo inteiro). O expurgo da trilha e o esvaziar da lixeira
/// ACRESCENTAM o proprio rastro no FIM -- e acrescentar no fim nao mexe
/// na posicao de ninguem. H2 se sustenta; o cursor do 487 nao entra aqui,
/// porque nao ha o que consertar.
///
/// Este teste e a sentinela da premissa: no dia em que alguem der ao
/// `.reason` um expurgo pela frente, a segunda pagina desliza e ele cai.
#[test]
fn os_motivos_paginam_por_pular_sem_perder_registro_no_expurgo() {
    let (s, _dir) = servidor("motivos-pular", false);
    let dono = Sessao::default();
    let motivos = |extra: &str| -> Vec<String> {
        s.executar(
            "motivos",
            &pedido(&format!(r#"{{"database":"b","tabela":"c"{extra}}}"#)),
            &dono,
        )
        .unwrap()
        .campo("motivos")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|m| m.texto_ou("uuid", "").to_string())
        .collect()
    };
    for rowid in 1..=4 {
        s.executar(
            "excluir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","rowid":{rowid},"motivo":"m{rowid}"}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let todos = motivos("");
    assert_eq!(todos.len(), 4, "o cenario nao montou quatro motivos");
    let p1 = motivos(r#","pular":0,"limite":2"#);
    assert_eq!(p1, todos[0..2].to_vec());

    // Entre as paginas, os dois expurgos que existem: o da lixeira e o da
    // trilha (este, com o volume ativo fechado, para derrubar de fato).
    s.executar(
        "esvaziar_lixeira",
        &pedido(r#"{"database":"b","tabela":"c","motivo":"limpeza"}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"c","colunas":{"cpf":"pessoal"}}"#),
        &dono,
    )
    .unwrap();
    varrer(&s, "");
    std::thread::sleep(std::time::Duration::from_millis(3));
    s.executar(
        "expurgar_trilha",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"c","motivo":"prazo","ate_ms":{},
                    "fechar_ativo":true}}"#,
            crate::agora_ms()
        )),
        &dono,
    )
    .unwrap();

    let p2 = motivos(r#","pular":2,"limite":2"#);
    assert_eq!(
        p2,
        todos[2..4].to_vec(),
        "um expurgo entre as paginas fez o `motivos` pular ou repetir registro"
    );
    let depois = motivos("");
    assert!(
        depois.len() > todos.len() && depois[..4] == todos[..],
        "os expurgos tinham de ACRESCENTAR o rastro no fim, sem mexer na frente: \
             antes {todos:?}, depois {depois:?}"
    );
}

/// **Prova real do A8 (revisao SEC de 17/09/2026, pedido 285).** O
/// `replicar` entrega a linha INTEIRA, com o valor da coluna marcada
/// dentro, e nao deixava registro na trilha: a pergunta «quem viu o
/// prontuario do fulano?» nao tinha resposta para o caminho que entrega
/// todas as linhas de uma vez. Um registro por chamada, com o criterio
/// `replicar desde=N ate=M` e `linhas` = eventos servidos -- o mesmo
/// desenho por operacao do `varrer` (`docs/LGPD.md` §4).
///
/// **Defeito reposto**: tirar a chamada a `trilhar_acesso` do
/// `op_replicar` deixa a trilha em zero e este teste cai na contagem.
#[test]
fn replicar_numa_tabela_marcada_deixa_rastro_na_trilha() {
    let (s, dir) = servidor("trilha-replicar", false);
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"c","colunas":{"cpf":"pessoal"}}"#),
        &Sessao::default(),
    )
    .unwrap();
    let lgpd = dir.join("b/c.lgpd");
    let antes = std::fs::metadata(&lgpd).map(|m| m.len()).unwrap_or(0);
    let r = s
        .executar(
            "replicar",
            &pedido(r#"{"database":"b","tabela":"c","desde":2,"max":4}"#),
            &Sessao {
                ip: "192.0.2.9".into(),
                ..Sessao::default()
            },
        )
        .unwrap();
    let servidos = r
        .campo("eventos")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap_or(0);
    assert_eq!(
        servidos,
        4,
        "o lote nao serviu o que devia: {}",
        r.escrever()
    );

    let t = s
        .executar(
            "trilha",
            &pedido(r#"{"database":"b","tabela":"c","tipo":"acesso"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let registros: Vec<Json> = t
        .campo("registros")
        .and_then(Json::lista)
        .map(|l| l.to_vec())
        .unwrap_or_default();
    assert_eq!(
        registros.len(),
        1,
        "um `replicar` numa tabela marcada tem de deixar UM registro na \
             trilha; veio {}",
        t.escrever()
    );
    let e = &registros[0];
    assert_eq!(e.texto_ou("identidade", ""), "replicar desde=2 ate=6");
    assert_eq!(e.inteiro_ou("linhas", 0), 4, "linhas = eventos servidos");
    assert_eq!(e.texto_ou("coluna", ""), "cpf");
    assert_eq!(e.texto_ou("ip", ""), "192.0.2.9", "o IP de quem puxou");
    // O custo de um registro por lote, medido no proprio arquivo -- e o
    // numero que o relatorio desta frente cita (nao e asserido: ele muda
    // com o tamanho do criterio).
    let depois = std::fs::metadata(&lgpd).map(|m| m.len()).unwrap_or(0);
    eprintln!("trilha do replicar: +{} bytes por lote", depois - antes);
}

/// O comportamento VELHO, que e o que o conserto nao pode encarecer: uma
/// tabela SEM coluna marcada continua sem trilha nenhuma depois do
/// `replicar` -- nem arquivo, nem registro. O portao (`tem_dado_pessoal`)
/// vem antes do `format!` do criterio, como no `varrer`.
#[test]
fn replicar_sem_coluna_marcada_nao_grava_trilha() {
    let (s, dir) = servidor("trilha-replicar-sem-marca", false);
    let r = s
        .executar(
            "replicar",
            &pedido(r#"{"database":"b","tabela":"c","desde":0,"max":10}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        r.campo("eventos").and_then(Json::lista).map(|l| l.len()),
        Some(10)
    );
    assert!(
        !dir.join("b/c.lgpd").exists(),
        "tabela sem coluna marcada ganhou trilha por causa do replicar"
    );
    let t = s
        .executar(
            "trilha",
            &pedido(r#"{"database":"b","tabela":"c","tipo":"acesso"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(t.inteiro_ou("total", -1), 0, "{}", t.escrever());
}

/// O espelho continua nascendo numa leitura, como nascia antes.
///
/// **Defeito reposto**: tirar `self.espelho() && !t.tem_espelho()` do
/// `abrir_para_ler_travada` deixa o `.bkp` sem nascer -- e a tabela fica
/// sem a copia que o `reparar` usa, sem ninguem ter mudado configuracao
/// nenhuma.
#[test]
fn o_espelho_continua_nascendo_no_varrer() {
    let (s, dir) = servidor("espelho", true);
    let bkp = dir.join("b/c.bkp");
    // A insercao ja o cria; apagar poe a tabela no estado de quem ligou o
    // espelho depois de a tabela existir, que e o caso que importa.
    let _ = std::fs::remove_file(&bkp);
    varrer(&s, "");
    assert!(
        bkp.exists(),
        "o espelho nao nasceu: a pista de leitura aceitou uma tabela que \
             precisava ser espelhada, e espelhar e escrever"
    );
}

/// Quatro leitores ao mesmo tempo na mesma tabela leem a mesma pagina.
///
/// E o unico teste daqui que exercita o que a mudanca existe para permitir:
/// quatro threads DENTRO da ficha compartilhada ao mesmo tempo. Ele nao
/// mede tempo -- medir tempo em teste unitario e como o `quieta.Vigia`
/// existe para provar que nao se faz --, prova a RESPOSTA.
#[test]
fn quatro_leitores_ao_mesmo_tempo_leem_a_mesma_pagina() {
    let (s, _dir) = servidor("quatro", false);
    let esperado = varrer(&s, "").escrever();
    let mut fios = Vec::new();
    for _ in 0..4 {
        let copia = Arc::clone(&s);
        let alvo = esperado.clone();
        fios.push(std::thread::spawn(move || {
            for _ in 0..25 {
                assert_eq!(varrer(&copia, "").escrever(), alvo);
            }
        }));
    }
    for f in fios {
        f.join().expect("um leitor entrou em panico");
    }
}

/// Um escritor no meio de quatro leitores nao perde gravacao nenhuma.
///
/// A pergunta que ele responde e a que o `RwLock` levanta e o `Mutex` nao
/// levantava: **o escritor passa fome?** Quatro leitores em laco fechado
/// poderiam, num `RwLock` de preferencia ao leitor, deixar o escritor
/// esperando para sempre. Aqui ele grava vinte linhas e as vinte tem de
/// chegar -- e o prazo do teste e o que acusa a fome, porque fome nao da
/// erro: ela demora.
#[test]
fn o_escritor_nao_passa_fome_entre_leitores() {
    let (s, _dir) = servidor("fome", false);
    let parar = Arc::new(AtomicBool::new(false));
    let mut fios = Vec::new();
    for _ in 0..4 {
        let copia = Arc::clone(&s);
        let pare = Arc::clone(&parar);
        fios.push(std::thread::spawn(move || {
            while !pare.load(Ordering::Relaxed) {
                varrer(&copia, "");
            }
        }));
    }
    let comeco = Instant::now();
    for i in 11..=30 {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"n":{i},"cpf":"x"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
    let gasto = comeco.elapsed();
    parar.store(true, Ordering::Relaxed);
    for f in fios {
        f.join().expect("um leitor entrou em panico");
    }
    let r = varrer(&s, "");
    assert_eq!(
        r.campo("visiveis").and_then(Json::inteiro),
        Some(30),
        "faltou gravacao: o escritor perdeu linha entre os leitores"
    );
    assert!(
        gasto < Duration::from_secs(20),
        "vinte gravacoes levaram {gasto:?} com quatro leitores ao lado: \
             o escritor esta passando fome"
    );
}

use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("jan-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor cuja janela de durabilidade fecha a cada 4 gravacoes, e
/// cujo relogio de fundo nao acorda durante o teste.
///
/// Os dois numeros sao o teste: com a janela do padrao (200 operacoes) o
/// defeito so aparecia depois de 200 gravacoes, e com o relogio acordando
/// a cada 200 ms ele as vezes limpava o conjunto antes e escondia tudo.
/// Aqui a condicao e deterministica.
fn servidor_janela_curta(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = dir_temp(nome);
    let mut config = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    config.recursos.lote_operacoes = 4;
    config.recursos.lote_milissegundos = 600_000;
    let s = Servidor::novo(config).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    for t in ["a", "b"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{t}",
                        "colunas":[{{"nome":"n","tipo":"Int8"}},
                                   {{"nome":"x","tipo":"Str(20)"}}]}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    (s, dir)
}

fn quantas(s: &Arc<Servidor>, tabela: &str) -> usize {
    s.executar(
        "varrer",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"{tabela}","max":200}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .map(|l| l.len())
    .unwrap_or(0)
}

/// Roda o trecho numa thread com prazo.
///
/// Todo teste deste modulo que escreve em DUAS tabelas passa por aqui, e
/// nao e zelo: reposto o defeito, o pedido nao volta NUNCA — e um teste
/// que pendura nao acusa nada, pendura o `cargo test` inteiro e ainda
/// parece que a suite esta so demorando.
fn com_prazo(rotulo: &str, f: impl FnOnce() + Send + 'static) {
    let (enviar, receber) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = enviar.send(());
    });
    assert!(
        receber
            .recv_timeout(std::time::Duration::from_secs(30))
            .is_ok(),
        "{rotulo}: o servidor travou — alguem pediu a trava de dados que ja \
             estava na mao desta mesma thread"
    );
}

fn criar_gatilho(s: &Arc<Servidor>, texto: &str) {
    let corpo = Json::objeto(vec![
        ("token", Json::texto_de("t")),
        ("op", Json::texto_de("sql")),
        ("database", Json::texto_de("b")),
        ("texto", Json::texto_de(texto)),
    ])
    .escrever();
    let mut ses = Sessao::default();
    s.despachar(&corpo, &mut ses, "127.0.0.1").2.unwrap();
}

/// O defeito: `gravar_de_verdade` fechava a janela chamando
/// `descarregar_sujas()`, que pede a trava de dados — com a trava de dados
/// JA na mao de quem chamou. `Mutex` nao e reentrante: a thread parava
/// para sempre segurando o servidor inteiro, e nao havia gatilho nenhum no
/// meio. So aparecia com DUAS tabelas, porque com uma so o conjunto de
/// sujas ficava vazio e a funcao voltava antes de pedir a trava.
///
/// O teste roda numa thread com prazo: um abraco mortal sem prazo
/// penduraria o `cargo test` inteiro, e um teste que pendura nao acusa
/// nada.
#[test]
fn duas_tabelas_na_mesma_janela_nao_travam_o_servidor() {
    let (s, _guarda) = servidor_janela_curta("duas-tabelas");
    let copia = Arc::clone(&s);
    com_prazo("40 insercoes alternadas entre duas tabelas", move || {
        for i in 0..40 {
            let t = if i % 2 == 0 { "a" } else { "b" };
            copia
                .executar(
                    "inserir",
                    &pedido(&format!(
                        r#"{{"database":"b","tabela":"{t}","linha":{{"n":{i},"x":"y"}}}}"#
                    )),
                    &Sessao::default(),
                )
                .unwrap();
        }
    });
    // E as 40 estao la, vinte de cada: travar nao e a unica forma de errar.
    assert_eq!(quantas(&s, "a"), 20);
    assert_eq!(quantas(&s, "b"), 20);
    // E a janela FECHOU de verdade.
    //
    // Esta asercao entrou junto com a guarda de reentrancia, e ela e o
    // preco dela: antes da guarda, o defeito reposto PENDURAVA, e o prazo
    // do `com_prazo` era a prova inteira. Agora ele nao pendura -- a
    // segunda tomada volta com erro, o `else { return }` do
    // `descarregar_sujas` engole, e o conjunto de sujas fica cheio para
    // sempre sem ninguem reparar. Guarda que troca um travamento por um
    // erro engolido ENFRAQUECE todo teste cujo unico sintoma era o
    // travamento; quem a acrescenta tem de olhar a consequencia no lugar.
    assert!(
        s.sujas.lock().unwrap().is_empty(),
        "a janela fechou e o conjunto de sujas nao esvaziou: alguem pediu \
             a trava que ja tinha e o erro foi engolido"
    );
}

/// Quantas familias de arquivo ainda devem ao disco nesta base.
///
/// **Mede o FATO, e nao a intencao.** O `Volumes::sincronizacoes()` e o
/// `selo()` sobem ANTES do laco de `fsync`, e um teste que os conferisse
/// passaria com um fio que nunca foi lancado. Este numero so' desce quando
/// o disco confirmou.
fn devendo(s: &Arc<Servidor>) -> usize {
    phxsql_store::volume::familias_devendo_em(&s.config.base)
}

/// Deixa as duas tabelas sujas sem fechar a janela.
fn sujar_as_duas(s: &Arc<Servidor>) {
    // `lote_operacoes` e' 4 no servidor de teste: duas insercoes nao
    // fecham a janela, e as duas tabelas ficam devendo ao disco.
    for t in ["a", "b"] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{t}","linha":{{"n":1,"x":"y"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
    let sujas = s.sujas.lock().unwrap();
    assert_eq!(sujas.len(), 2, "as duas tinham de estar sujas: {sujas:?}");
    drop(sujas);
    assert!(
        devendo(s) > 0,
        "as duas foram escritas e nenhuma familia deve ao disco: o \
             instrumento nao esta medindo nada"
    );
}

/// Uma marca de commit de mentira, so para ver se ela sobrevive.
fn marca_de_mentira(s: &Arc<Servidor>, nome: &str) -> PathBuf {
    let c = s.config.base.join(nome);
    std::fs::write(&c, b"marca").unwrap();
    s.marcas_pendentes.lock().unwrap().push(c.clone());
    c
}

/// **O encontro do fecho e ATOMICO, e e isso que o torna seguro.**
///
/// `descarregar_sujas_com` sincroniza as tabelas sujas e SO ENTAO apaga as
/// marcas dos commits que esperavam por elas. Desde esta rodada as K vao
/// ao disco ao mesmo tempo em vez de em laco — o comboio da §12 do
/// `docs/CONCORRENCIA.md`, medido em 2,52x no K=16. O que nao pode mudar
/// com isso e o que acontece quando UMA delas falha.
///
/// O defeito reposto: um erro engolido no `join`, ou um arranjo que apaga
/// as marcas por ter chegado ao fim do laco em vez de por todas terem
/// sincronizado. Nos dois casos a marca de um commit confirmado sai do
/// disco sem o dado estar la — e a marca e a UNICA coisa que o traz de
/// volta.
#[test]
fn tabela_que_nao_sincroniza_segura_as_marcas() {
    let (s, _guarda) = servidor_janela_curta("fecho-falha");
    sujar_as_duas(&s);
    // A terceira chave nao abre: e a mesma perda que um erro de E/S no
    // meio do fecho, e o caminho que ela toma e o mesmo.
    s.sujas.lock().unwrap().insert("b/naoexiste".into());
    let marca = marca_de_mentira(&s, "transacao_1.tx");

    let dados = s.travar_dados().unwrap();
    s.descarregar_sujas_com(&dados);
    drop(dados);

    assert!(
        marca.exists(),
        "uma tabela nao sincronizou e a marca do commit foi apagada assim              mesmo: apagar a marca antes de o dado estar no disco e jogar fora              o unico bilhete que o traz de volta"
    );
    let sujas = s.sujas.lock().unwrap();
    assert_eq!(
        sujas.iter().cloned().collect::<Vec<_>>(),
        vec!["b/naoexiste".to_string()],
        "so a que falhou volta para as sujas — as que sincronizaram nao              podem voltar (seria fsync de novo a cada janela), e a que falhou              nao pode sumir (seria dado sem `fsync` que ninguem mais tenta)"
    );
}

/// **Pedido 536:** a tabela escrita na janela e depois excluida ou
/// renomeada ficava nas sujas pelo nome velho -- um `b/naoexiste` de
/// verdade --, e o fecho segurava TODAS as marcas de COMMIT ate o processo
/// cair; no arranque, a marca reaplicava `Atualizar` velho por cima do que
/// foi escrito depois. Agora o `excluir_tabela` tira a chave e o
/// `renomear_tabela` a muda de nome, sob a mesma trava, e o fecho seguinte
/// sincroniza pelo nome novo e apaga as marcas.
///
/// # Prova real
///
/// Sem o `renomear_nas_sujas`, a marca fica e as sujas guardam `b/a` e
/// `b/b` -- o vermelho medido antes do conserto.
#[test]
fn tabela_excluida_ou_renomeada_na_janela_nao_segura_as_marcas() {
    let (s, _guarda) = servidor_janela_curta("536-some-da-janela");
    sujar_as_duas(&s);
    let marca = marca_de_mentira(&s, "transacao_5.tx");
    let dono = Sessao::default();
    s.executar(
        "excluir_tabela",
        &pedido(r#"{"database":"b","tabela":"a","confirmar":"a"}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "renomear_tabela",
        &pedido(r#"{"database":"b","tabela":"b","destino":"c"}"#),
        &dono,
    )
    .unwrap();

    let dados = s.travar_dados().unwrap();
    s.descarregar_sujas_com(&dados);
    drop(dados);

    let sujas: Vec<String> = s.sujas.lock().unwrap().iter().cloned().collect();
    assert!(
        sujas.is_empty(),
        "a tabela que sumiu pelo nome ficou nas sujas: {sujas:?}"
    );
    assert!(
        !marca.exists(),
        "a marca ficou presa por uma tabela que nao existe mais pelo nome dela"
    );
    assert_eq!(devendo(&s), 0, "a tabela renomeada ficou devendo ao disco");
}

/// **O `fsync` que falha DENTRO do fio**, que e o irmao do de cima.
///
/// A guarda anterior falha na ABERTURA da tabela; esta falha na
/// SINCRONIZACAO, ja com o fio lancado — o unico caminho em que o erro
/// chega pelo `join`. Sem ela, trocar `Ok(Ok(())) => None, _ => Some(i)`
/// por `_ => None` passaria por toda a suite: o `join` engoliria o erro, a
/// tabela ficaria devendo ao disco e a marca do commit sairia assim mesmo.
///
/// O `.pag` vira DIRETORIO para que a falha venha do sistema operacional e
/// nao de um sinalizador de teste: `Table::sincronizar` termina gravando o
/// descritor, e nao se cria arquivo por cima de diretorio. *O que depende
/// do sistema operacional se prova contra o sistema operacional.*
#[test]
fn fsync_que_falha_no_fio_tambem_segura_as_marcas() {
    let (s, _guarda) = servidor_janela_curta("fecho-falha-no-fio");
    sujar_as_duas(&s);
    let atravessado = s.config.base.join("b").join("a.pag");
    let _ = std::fs::remove_file(&atravessado);
    std::fs::create_dir(&atravessado).unwrap();
    let marca = marca_de_mentira(&s, "transacao_3.tx");

    let dados = s.travar_dados().unwrap();
    // A tabela ABRE — se ela nao abrisse, este teste seria uma segunda
    // copia do de cima e nao cobriria o `join`.
    assert!(
        dados
            .abrir_database("b")
            .and_then(|d| d.abrir_qualificada("a"))
            .is_ok(),
        "a tabela precisa ABRIR para a falha acontecer dentro do fio"
    );
    s.descarregar_sujas_com(&dados);
    drop(dados);

    assert!(
        marca.exists(),
        "o `fsync` de uma tabela falhou dentro do fio e a marca do commit \
             foi apagada assim mesmo: erro engolido no `join` e marca perdida"
    );
    let sujas: Vec<String> = s.sujas.lock().unwrap().iter().cloned().collect();
    assert_eq!(
        sujas,
        vec!["b/a".to_string()],
        "so a que falhou volta para as sujas"
    );
    let _ = std::fs::remove_dir(&atravessado);
}

/// O outro lado do laco, e ele e o que faz a guarda de cima segurar: com
/// TODAS sincronizadas, as marcas saem. Sem este teste, um fecho que nunca
/// apagasse marca nenhuma passaria pela guarda de cima — e marca que nunca
/// sai vira uma varredura a mais em todo arranque, para sempre.
#[test]
fn com_todas_sincronizadas_as_marcas_saem() {
    let (s, _guarda) = servidor_janela_curta("fecho-ok");
    sujar_as_duas(&s);
    let marca = marca_de_mentira(&s, "transacao_2.tx");

    let dados = s.travar_dados().unwrap();
    s.descarregar_sujas_com(&dados);
    drop(dados);

    assert!(
        !marca.exists(),
        "as duas sincronizaram e a marca ficou pendurada"
    );
    assert!(
        s.sujas.lock().unwrap().is_empty(),
        "as duas sincronizaram e o conjunto de sujas nao esvaziou"
    );
    assert!(
        s.marcas_pendentes.lock().unwrap().is_empty(),
        "a lista de marcas pendentes nao esvaziou"
    );
    // **A asercao que mede o FATO**, e a unica que pega o fio que nao foi
    // lancado: uma tabela que o arranjo em paralelo deixasse de fora
    // continuaria devendo ao disco, e as tres asercoes acima passariam
    // assim mesmo — as marcas sairiam, as sujas esvaziariam, e o dado
    // ficaria so no cache do nucleo.
    assert_eq!(
        devendo(&s),
        0,
        "o fecho terminou e alguma familia continua devendo ao disco: uma \
             tabela nao foi sincronizada, e as marcas sairam assim mesmo"
    );
}

/// O comportamento VELHO, e ele e o que mais importa aqui: com UMA tabela
/// so, tudo continua exatamente como antes — a janela fecha, o dado vai
/// para o disco e ninguem repara em nada.
#[test]
fn uma_tabela_so_grava_como_sempre() {
    let (s, _guarda) = servidor_janela_curta("uma-tabela");
    for i in 0..20 {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"a","linha":{{"n":{i},"x":"y"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
    assert_eq!(quantas(&s, "a"), 20);
}

/// O outro defeito: um `AFTER INSERT ON t` que grava em `t` chamava a si
/// mesmo sem fundo. Nao era um laco lento — era recursao de pilha, e o
/// Rust ABORTA O PROCESSO com "stack overflow". Reposto o defeito, este
/// teste nao falha: ele derruba o `cargo test` inteiro, que e exatamente o
/// tamanho do estrago que a guarda impede.
#[test]
fn a_cadeia_de_gatilhos_para_no_teto_e_avisa() {
    let (s, _guarda) = servidor_janela_curta("cadeia");
    criar_gatilho(
        &s,
        "CREATE TRIGGER se_multiplica AFTER INSERT ON a FOR EACH ROW \
             INSERT INTO a (n, x) VALUES (NEW.n + 1, 'gatilho')",
    );
    let r = s
        .executar(
            "inserir",
            &pedido(r#"{"database":"b","tabela":"a","linha":{"n":1,"x":"gente"}}"#),
            &Sessao::default(),
        )
        .unwrap();
    // A resposta continua `ok` — a linha entrou —, e diz o que houve.
    let avisos = r.campo("gatilhos_avisos").and_then(Json::lista).unwrap();
    assert_eq!(avisos.len(), 1, "esperava um aviso, veio {avisos:?}");
    let texto = avisos[0].texto().unwrap_or_default().to_string();
    assert!(texto.contains("cadeia de gatilhos"), "aviso: {texto}");
    // A primeira linha mais um degrau por nivel, e nem uma a mais.
    assert_eq!(quantas(&s, "a"), 1 + CADEIA_MAXIMA as usize);
}

/// A cadeia CURTA — a de verdade — continua rodando inteira: gravar em `a`
/// dispara a auditoria em `b`, e isso e um nivel, nao oito. Sem este teste,
/// a guarda podia estar cortando a cadeia legitima e ninguem veria.
#[test]
fn a_cadeia_curta_de_auditoria_roda_inteira() {
    let (s, _guarda) = servidor_janela_curta("auditoria");
    criar_gatilho(
        &s,
        "CREATE TRIGGER audita AFTER INSERT ON a FOR EACH ROW \
             INSERT INTO b (n, x) VALUES (NEW.n, 'auditoria')",
    );
    // Sao duas tabelas por caminho do gatilho: o prazo vale aqui tambem.
    let copia = Arc::clone(&s);
    com_prazo("10 insercoes com auditoria em outra tabela", move || {
        for i in 0..10 {
            let r = copia
                .executar(
                    "inserir",
                    &pedido(&format!(
                        r#"{{"database":"b","tabela":"a","linha":{{"n":{i},"x":"y"}}}}"#
                    )),
                    &Sessao::default(),
                )
                .unwrap();
            assert!(
                r.campo("gatilhos_avisos").is_none(),
                "a cadeia curta nao devia avisar nada: {r:?}"
            );
        }
    });
    assert_eq!(quantas(&s, "b"), 10);
}

/// O fonte deste arquivo, pelo mesmo `include_str!` do conferidor de
/// textos -- e pela mesma razao dele: assim nao ha como o teste contar um
/// arquivo e o binario ter sido compilado de outro.
const FONTE: &str = FONTE_DO_SERVIDOR;

/// A catraca do ponto unico, agora com DUAS fichas: **UMA** tomada de cada
/// uma no arquivo, e cada uma dentro da funcao que a batiza.
///
/// Reposto o defeito -- um `self.dados.write()` a mais em qualquer lugar --
/// este teste falha nomeando o numero. Foi assim que as 13 apareceram: o
/// comentario do `travar_dados` afirmava ser o unico lugar havia rodadas,
/// e ninguem contava. Comentario nao conta; teste conta.
///
/// # Por que DUAS contagens de um, e nao uma de dois
///
/// Porque uma catraca de «duas tomadas no arquivo» aceitaria as duas na
/// mesma funcao, ou a de leitura solta no meio de um `op_`. Cada ficha
/// continua com a porta dela, e a catraca continua valendo 1 em cada --
/// o teto nao subiu, nasceu outro ao lado.
#[test]
fn so_um_lugar_toma_a_trava() {
    // As agulhas sao MONTADAS, e nao escritas: um literal aqui apareceria
    // no proprio fonte varrido e o teste contaria a si mesmo. Foi o
    // primeiro jeito que escrevi, e ele acusou 4 onde havia 1.
    // Linha de comentario fora: este arquivo CITA as tomadas em varios
    // comentarios, e cita-las e o oposto de faze-las.
    let codigo = |l: &&str| !l.trim_start().starts_with("//");
    for (agulha, dono) in [
        (
            format!("self.{}.write()", "dados"),
            "fn travar_dados(&self)",
        ),
        (
            format!("self.{}.read()", "dados"),
            "fn travar_dados_para_ler(&self)",
        ),
    ] {
        let tomadas = FONTE
            .lines()
            .filter(codigo)
            .filter(|l| l.contains(&agulha))
            .count();
        assert_eq!(
            tomadas, 1,
            "ha {tomadas} tomadas de `{agulha}` em servidor.rs, e so pode \
                 haver a de dentro do `{dono}`: sem isso a telemetria \
                 cronometra uma parte da fila e o `docs/TELEMETRIA.md` afirma o \
                 contrario. Quem precisa da trava chama a funcao; quem ja \
                 a tem recebe a ficha por parametro"
        );
        // E a que sobrou esta MESMO dentro da funcao que a batiza --
        // contar sem olhar onde deixaria passar o caso de mover a unica
        // para outro lugar.
        let corpo = FONTE
            .split_once(dono)
            .unwrap_or_else(|| panic!("o `{dono}` sumiu"))
            .1;
        let fim = corpo.find("\n    }\n").expect("o corpo da funcao");
        assert!(
            corpo[..fim]
                .lines()
                .filter(codigo)
                .any(|l| l.contains(&agulha)),
            "a unica tomada de `{agulha}` saiu de dentro do `{dono}`"
        );
    }
}

/// A catraca do ALCANCE da ficha compartilhada: **QUATRO** chamadas a tomam.
///
/// A decisao do dono era «so o `varrer`», e a segunda leva entrou MEDIDA
/// (15/09/2026): o `coletar_rowids`, o substrato do `UPDATE`/`DELETE` por
/// faixa. A medicao que a admitiu e de TIPO, nao de leitura: o corpo dele e
/// `coletar_os_rowids<T: Legivel>`, e `Legivel` **nao tem metodo de
/// escrita** -- entao ele nao consegue ter a varredura de escrita escondida
/// que esta catraca existe para achar (a trilha de dado pessoal, o espelho,
/// a criacao do `.trash`), e devolve so' rowids, nenhum valor de coluna.
///
/// A terceira entrou MEDIDA em 01/10/2026 (pedido 513): a COPIA do backup,
/// pelo `copiar_o_retrato` -- uma chamada so para o protocolo e o
/// agendado. A medicao que a admitiu: ela nao abre tabela nenhuma (le os
/// arquivos de `raiz` byte a byte), e o que ela escreve vai para o
/// destino, que `backup::executar`/`executar_zip` recusam dentro da raiz
/// antes do primeiro byte. Quem a acompanha e o portao do retrato, que
/// tira os escritores da fila (`crate::retrato`).
///
/// Desde o pedido 729 o retrato da replica passa pela MESMA funcao (um motor
/// so para as duas copias em duas passadas), e a contagem nao muda: a fase 2
/// dele roda com a ficha EXCLUSIVA (`FichaDaFase2::Exclusiva`), porque
/// descarrega as sujas antes de acertar -- nao entra nesta ficha.
///
/// A quarta entrou MEDIDA em 01/10/2026 (pedido 330): a pre-absorcao do
/// diario local no mapa de toques do bidirecional,
/// `pre_absorver_sob_leitura`. A medicao que a admitiu e de TIPO, como a
/// do `coletar_rowids`: o corpo e `absorver_diario_local<T:
/// DiarioLegivel>`, e `DiarioLegivel` **nao tem metodo de escrita** -- le
/// eventos do `.log` e decodifica a imagem, e a marca do diario e dica em
/// memoria. As tres escritas escondidas que esta catraca procura nao
/// alcancam ali: o diario nao registra acesso na trilha (a exclusiva de
/// antes tambem nao registrava), o espelho e a `.trash` so nascem pela
/// abertura exclusiva, e a abertura compartilhada recusa quando abrir
/// escreveria. O que ela compra esta medido: 2,26-2,63 us por evento sob a
/// exclusiva em release, 2,3-2,5 s de servidor parado num diario de 1 M.
///
/// Sem esta catraca, a proxima leva entra por distracao: `op_ler`,
/// `op_buscar` e `op_sistabelas` sao todas leituras e todas parecem obvias
/// -- e cada uma tem escrita escondida propria para achar antes.
///
/// Ela conta CHAMADAS, e nao operacoes: mover a tomada para um ajudante
/// chamado por vinte `op_` continuaria dando o mesmo. E ela vale nos dois
/// sentidos, como toda catraca desta casa -- quem tirar um dos dois da
/// pista de leitura tambem reprova aqui, e tem de dizer por que no mesmo
/// commit.
#[test]
fn so_as_duas_operacoes_medidas_usam_a_ficha_compartilhada() {
    let codigo = |l: &&str| !l.trim_start().starts_with("//");
    let agulha = format!("self.{}()?", "travar_dados_para_ler");
    let usos = FONTE
        .lines()
        .filter(codigo)
        .filter(|l| l.contains(&agulha))
        .count();
    assert_eq!(
        usos, 4,
        "ha {usos} chamadas tomando a ficha compartilhada, e as medidas \
             sao QUATRO -- o `varrer` e o `coletar_rowids`, os dois provados \
             so'-leitura pelo tipo `Legivel`, a copia do backup \
             (`copiar_o_retrato`, pedido 513), que nao abre tabela e escreve \
             so fora da raiz, e a pre-absorcao do bidirecional \
             (`pre_absorver_sob_leitura`, pedido 330), provada so'-leitura \
             pelo tipo `DiarioLegivel`. Uma quinta entra MEDIDA: cada \
             operacao nova precisa da propria varredura de escrita escondida \
             (a trilha de dado pessoal, o espelho, a criacao do .trash) \
             achada antes"
    );
}

/// A catraca da leitura SO DO DISCO: **ZERO** desligamentos depois de
/// abrir. Quem quer o disco puro usa a porta que nao monta o mapa.
///
/// # O defeito que a motivou, e ele era so' de TEMPO
///
/// O caminho que empilha abria a tabela pela porta de sempre -- que monta
/// a sobreposicao da transacao, percorrendo o conjunto de escrita inteiro
/// -- e a desligava na linha seguinte. O mapa nascia e morria sem ninguem
/// consultar, DENTRO da trava de dados, e o custo crescia com a lista
/// pendente: medido em 17/09/2026 pelo `--example reparticao-do-gatilho`,
/// o piso do `empilhar` ia de **60 us/op com 100 pendentes a 625,62 us/op
/// com 1.600** -- O(pendentes) por operacao, O(n²) por transacao. Com a
/// porta, a mesma sonda da **40,00 / 35,00 / 37,50 / 38,75 / 41,25 us**:
/// plana.
///
/// # Por que a prova e ESTA, e nao um teste de comportamento
///
/// Porque nao ha comportamento a provar: depois de `sobrepor` de um mapa
/// jogado fora e depois da porta que nao monta, o `Table` fica no MESMO
/// estado. O defeito era invisivel de fora, e e por isso que um teste de
/// resultado nunca o acharia -- quem o acha e o relogio, e quem impede a
/// volta dele e esta contagem. Reposto o defeito (abrir pela porta de
/// sempre e desligar depois), este teste falha nomeando o numero.
#[test]
fn so_o_disco_vem_da_porta_e_nao_de_desligar_depois() {
    // A agulha e MONTADA pelo mesmo motivo do `so_um_lugar_toma_a_trava`:
    // escrita como literal, ela apareceria no fonte varrido e o teste
    // contaria a si mesmo.
    let codigo = |l: &&str| !l.trim_start().starts_with("//");
    let agulha = format!("{}()", "ver_so_o_disco");
    let desligamentos = FONTE
        .lines()
        .filter(codigo)
        .filter(|l| l.contains(&agulha))
        .count();
    assert_eq!(
        desligamentos, 0,
        "ha {desligamentos} chamadas de `{agulha}` em servidor.rs. Abrir \
             pela porta de sempre e desligar depois monta o mapa da transacao \
             com a trava de dados na mao para apaga-lo na linha seguinte, e o \
             custo cresce com a lista pendente. Quem quer o disco puro abre \
             por `abrir_travada_sem_sobrepor`, que carrega a dispensa escrita"
    );
}

/// A catraca do prazo: **todo corpo que roda com a trava de dados na mao
/// leva prazo de parede.**
///
/// Ela e estatica e nao dinamica de proposito. O teste de ponta a ponta
/// abaixo prova que um corpo sem fundo nao derruba o servidor, mas ele nao
/// consegue provar QUAL dos tetos respondeu — isso depende da velocidade
/// da maquina, e teste que depende disso e teste que um dia falha sozinho.
/// Esta catraca prova a fiacao: quem apagar o `.com_prazo(...)` do
/// `rodar_gatilhos_antes` e acusado por nome, e nao por um tempo que
/// passou a caber.
///
/// O irmao dela e o `rodar_gatilhos_depois`, que **nao** leva prazo e nao
/// pode levar: o AFTER roda SEM a trava, e por-lhe um relogio em cima
/// tiraria de quem audita um tempo que ninguem esta esperando.
#[test]
fn o_before_roda_com_prazo_e_o_after_nao() {
    let corpo_antes = FONTE
        .split_once("fn rodar_gatilhos_antes(")
        .expect("o `rodar_gatilhos_antes` sumiu")
        .1;
    let fim = corpo_antes
        .find("\n    }\n")
        .expect("o corpo do rodar_gatilhos_antes");
    assert!(
        corpo_antes[..fim].contains(".com_prazo("),
        "o corpo do BEFORE roda com a trava GLOBAL de dados na mao e \
             perdeu o prazo de parede. Sem ele, um `WHILE` com passo caro \
             segura o servidor inteiro por dezenas de segundos: medido, \
             28.590 ms contra 500 ms"
    );
    let corpo_depois = FONTE
        .split_once("fn rodar_gatilhos_depois(")
        .expect("o `rodar_gatilhos_depois` sumiu")
        .1;
    let fim = corpo_depois
        .find("\n    }\n")
        .expect("o corpo do rodar_gatilhos_depois");
    assert!(
        !corpo_depois[..fim].contains(".com_prazo("),
        "o AFTER roda SEM a trava: um prazo aqui nao protege ninguem e \
             corta uma auditoria honesta pela metade"
    );
}

/// A catraca do `fsync` da RESTAURACAO: ele acontece **depois** de a ficha
/// sair da mao, e nunca antes.
///
/// # O defeito que ela impede, e o numero dele
///
/// A reaplicacao do diario do PITR (`reaplicar_diario_ate`) sincronizava
/// cada tabela reaplicada com a trava GLOBAL na mao -- uma dezena de
/// `sync_all` por tabela, o servidor inteiro parado atras de cada um. Foi
/// a secao que levantou a catraca `alcancam-fsync` do
/// `bancada/concorrencia/mapa-da-trava.py` de 22 para 23 em 08/09/2026, e
/// ela ficou vermelha ate a decisao do dono de 18/09 (pedido 252): tirar o
/// `fsync` de dentro da trava, sem subir teto e sem isentar secao.
///
/// # Por que ESTATICA, e nao um teste de relogio
///
/// Pelo mesmo motivo do `so_o_disco_vem_da_porta_e_nao_de_desligar_depois`:
/// de fora nao ha comportamento a provar. O dado restaurado e o mesmo nos
/// dois casos, e o que muda e quanto tempo a fila espera -- quem acha isso
/// e o relogio, e teste que depende da velocidade da maquina e teste que
/// um dia falha sozinho. Quem impede a volta e esta ORDEM.
///
/// # E ela vale nos dois sentidos
///
/// Apagar o `sincronizar` tambem reprova, e nao por simetria: restauracao
/// que nao sincroniza devolve um database que a proxima queda de energia
/// leva junto. A catraca exige que ele exista **e** que esteja embaixo.
#[test]
fn o_fsync_da_restauracao_fica_fora_da_trava() {
    // As agulhas sao MONTADAS pelo mesmo motivo do `so_um_lugar_toma_a_trava`:
    // escritas como literal, elas entrariam no proprio fonte varrido.
    let corpo = FONTE
        .split_once("fn reaplicar_diario_ate(")
        .expect("o `reaplicar_diario_ate` sumiu")
        .1;
    let fim = corpo
        .find("\n    }\n")
        .expect("o corpo do reaplicar_diario_ate");
    let linhas: Vec<&str> = corpo[..fim]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let solta = format!("{}(trava)", "drop");
    let sincroniza = format!(".{}(", "sincronizar");
    let onde_solta = linhas
        .iter()
        .position(|l| l.contains(&solta))
        .unwrap_or_else(|| {
            panic!(
                "o `reaplicar_diario_ate` nao solta mais a ficha por `{solta}`: \
                     sem isso a reaplicacao inteira -- `fsync` incluido -- volta a \
                     acontecer com a trava global na mao"
            )
        });
    let sincronizacoes: Vec<usize> = linhas
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(&sincroniza))
        .map(|(i, _)| i)
        .collect();
    assert!(
        !sincronizacoes.is_empty(),
        "o `reaplicar_diario_ate` parou de sincronizar o que reaplicou: o \
             database restaurado passa a depender do cache do nucleo, e a \
             proxima queda de energia leva a reaplicacao junto"
    );
    for i in sincronizacoes {
        assert!(
            i > onde_solta,
            "o `{sincroniza}` da linha {i} do corpo acontece ANTES do \
                 `{solta}` (linha {onde_solta}): e `fsync` com a trava global \
                 na mao, que e a secao que a catraca `alcancam-fsync` conta"
        );
    }
}

/// A catraca da ORDEM da restauracao: o PITR reaplica no **palco**, antes
/// de o database entrar na raiz de dados.
///
/// # Ela e a metade que TORNA a de cima possivel
///
/// Soltar a trava antes do `fsync` so e seguro porque, naquele ponto, o
/// que se escreve ainda nao esta na raiz: o palco e um diretorio vizinho
/// cujo nome so este pedido conhece, e nao ha segundo dono possivel. Com a
/// ordem invertida -- `confirmar` e so entao reaplicar --, a mesma linha
/// passaria a escrever numa tabela que qualquer sessao pode abrir, e o
/// `drop` da ficha viraria corrupcao em vez de melhoria.
///
/// E ela compra duas garantias de lambuja: ninguem enxerga o database no
/// instante da copia esperando a reaplicacao, e erro DURO na reaplicacao
/// nao deixa um database criado ao lado de uma resposta de fracasso.
#[test]
fn o_pitr_reaplica_antes_de_o_database_entrar_na_raiz() {
    let corpo = FONTE
        .split_once("fn op_restaurar_backup(")
        .expect("o `op_restaurar_backup` sumiu")
        .1;
    let fim = corpo
        .find("\n    }\n")
        .expect("o corpo do op_restaurar_backup");
    let linhas: Vec<&str> = corpo[..fim]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let reaplica = format!("self.{}(", "reaplicar_diario_ate");
    let troca = format!(".{}(", "confirmar");
    let onde = |agulha: &str| {
        linhas
            .iter()
            .position(|l| l.contains(agulha))
            .unwrap_or_else(|| panic!("o `{agulha}` sumiu do `op_restaurar_backup`"))
    };
    assert!(
        onde(&reaplica) < onde(&troca),
        "a reaplicacao do diario voltou para DEPOIS do `{troca}`: dali em \
             diante a tabela restaurada ja esta na raiz de dados e tem segundo \
             dono possivel, entao o `fsync` dela volta a precisar da trava \
             global -- e abre-se de novo a janela em que outra sessao ve o \
             database no instante da copia, sem o diario reaplicado"
    );
}

/// **O defeito reposto:** um gatilho `BEFORE` cujo corpo dobra o texto a
/// cada volta.
///
/// Sem os dois tetos este teste nao falha — ele **aborta o `cargo test`**
/// com `memory allocation failed`, que e o tamanho exato do estrago: em
/// producao seria o servidor inteiro caindo, com a trava global na mao,
/// por causa de um gatilho que o dono do banco escreveu. Medido a mao com
/// `ulimit -v 2000000`: 10,2 s e entao o aborto.
///
/// Ele afere o TEMPO, e nao so o veredito: uma insercao recusada em 30 s
/// e tao ruim quanto uma recusa que nao acontece, e conferir «recusou»
/// passaria com as duas.
#[test]
fn gatilho_before_sem_fundo_nao_derruba_o_servidor() {
    let (s, _guarda) = servidor_janela_curta("gatilho-sem-fundo");
    criar_gatilho(
        &s,
        "CREATE TRIGGER incha BEFORE INSERT ON a FOR EACH ROW \
             WHILE TRUE DO SET NEW.x = CONCAT(NEW.x, NEW.x); END WHILE",
    );
    let comeco = Instant::now();
    let r = s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"a","linha":{"n":1,"x":"a"}}"#),
        &Sessao::default(),
    );
    let gasto = comeco.elapsed();
    let erro = r.expect_err("a insercao tinha de ser recusada pelo gatilho");
    let texto = format!("{erro}");
    // Qual dos DOIS tetos responde depende da velocidade da maquina, e por
    // isso o teste aceita os dois pelo nome em vez de escolher um: o que
    // ele nao aceita e nenhum dos dois.
    assert!(
        texto.contains("prazo") || texto.contains("CONCAT"),
        "o erro tinha de nomear um dos dois tetos, e veio: {texto}"
    );
    assert!(
        gasto < Duration::from_secs(5),
        "a insercao levou {gasto:?} com a trava global na mao"
    );
    // E nada foi gravado: BEFORE que falha cancela a escrita.
    assert_eq!(quantas(&s, "a"), 0);
}

/// O terceiro abraco mortal, reposto de proposito: a mesma thread pede a
/// trava que ela ja tem.
///
/// Antes da `COM_A_TRAVA` isto **nao falhava** -- pendurava. Sem log, sem
/// pilha, e levando junto todas as outras conexoes, porque a trava fica
/// presa na thread parada. E por isso que o teste roda com prazo: se a
/// guarda for removida, ele acusa em 30 s em vez de pendurar o
/// `cargo test` inteiro.
#[test]
fn a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro() {
    let (s, _guarda) = servidor_janela_curta("reentrante");
    let copia = Arc::clone(&s);
    com_prazo("duas tomadas da trava na mesma thread", move || {
        let primeira = copia.travar_dados().expect("a primeira tem de vir");
        let segunda = copia.travar_dados();
        assert!(
            segunda.is_err(),
            "a segunda tomada da MESMA thread tinha de ser recusada"
        );
        let recado = match segunda {
            Ok(_) => unreachable!("a linha acima ja garantiu o erro"),
            Err(e) => format!("{e}"),
        };
        assert!(
            recado.contains("a propria thread ja tem"),
            "o erro tem de dizer o que houve, e nao um `Corrompido` mudo: {recado}"
        );
        drop(primeira);
        // E soltar de verdade: depois do `drop` a mesma thread volta a
        // conseguir. Sem isto a guarda trocaria um abraco mortal por uma
        // thread aleijada para o resto da vida dela.
        assert!(
            copia.travar_dados().is_ok(),
            "depois de soltar, a mesma thread tem de conseguir de novo"
        );
    });
}

/// O abraco mortal ENTRE AS DUAS FICHAS, nos dois sentidos.
///
/// # Por que ele nao e o mesmo teste de cima
///
/// Porque a `COM_A_TRAVA` e uma so para as duas portas, e isso e decisao --
/// e decisao que precisa de prova. Pedir a compartilhada com a exclusiva na
/// mao pendura igual; pedir a exclusiva com a compartilhada na mao pendura
/// PIOR, porque num `RwLock` com escritor na fila a segunda leitura da
/// mesma thread trava as tres pontas: ela, o escritor e todo leitor que
/// chegar depois.
///
/// O teste de cima nao cobre nenhum dos dois: ele pede a mesma porta duas
/// vezes. **Guarda que existe e so cobre metade dos caminhos e guarda que
/// alguem vai citar como se cobrisse todos.**
#[test]
fn as_duas_fichas_na_mesma_thread_viram_erro() {
    let (s, _guarda) = servidor_janela_curta("duas-fichas");
    let copia = Arc::clone(&s);
    com_prazo("a compartilhada depois da exclusiva", move || {
        let exclusiva = copia.travar_dados().expect("a primeira tem de vir");
        let recado = match copia.travar_dados_para_ler() {
            Ok(_) => unreachable!("a ficha compartilhada nao podia vir"),
            Err(e) => format!("{e}"),
        };
        assert!(
            recado.contains("a propria thread ja tem"),
            "o erro tem de dizer o que houve: {recado}"
        );
        drop(exclusiva);
        assert!(
            copia.travar_dados_para_ler().is_ok(),
            "depois de soltar, a compartilhada tem de vir"
        );
    });
    let copia = Arc::clone(&s);
    com_prazo("a exclusiva depois da compartilhada", move || {
        let leitura = copia
            .travar_dados_para_ler()
            .expect("a primeira tem de vir");
        assert!(
            copia.travar_dados().is_err(),
            "a exclusiva pedida com a compartilhada na mao tinha de ser recusada"
        );
        drop(leitura);
        assert!(
            copia.travar_dados().is_ok(),
            "depois de soltar, a exclusiva tem de vir"
        );
    });
}

/// O comportamento VELHO, que e o que mais importa: quem nunca esbarra na
/// reentrancia nao ve diferenca nenhuma -- nem no resultado, nem no
/// caminho. A guarda so existe para quem ja estava pendurado.
#[test]
fn sem_reentrancia_nada_muda() {
    let (s, _guarda) = servidor_janela_curta("sem-reentrancia");
    for i in 0..12 {
        let t = if i % 2 == 0 { "a" } else { "b" };
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{t}","linha":{{"n":{i},"x":"y"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
    assert_eq!(quantas(&s, "a"), 6);
    assert_eq!(quantas(&s, "b"), 6);
    // E a trava esta livre no fim: uma marca que vazasse do `Drop`
    // trancaria esta thread sem ninguem segurando nada.
    assert!(s.travar_dados().is_ok());
}

/// **Pedido 647: o inode velho morre FORA da trava global** -- a ordem, no
/// fonte, em cada FASE B do servidor (papel F, 08/10/2026).
///
/// A prova do store (`a_fase_b_segura_o_volume_velho_ate_soltar`) mostra que
/// o descritor fica preso ate o `soltar`; ela nao ve QUEM chama o `soltar`
/// nem com que trava na mao. Medido: com o `soltar` posto antes do
/// `drop(dados)` no `criptografar`, as 1.784 provas da biblioteca e as 6 da
/// migracao pelo soquete ficaram verdes -- e o 1,2 s do `close` a 10 M de
/// linhas volta a acontecer sob a trava. Aqui: cada chamada tem um
/// `drop(dados)` logo antes, sem a trava retomada no meio.
#[test]
fn o_soltar_dos_volumes_velhos_vem_depois_de_soltar_a_trava() {
    let chamada = format!(".{}()", "soltar_volumes_velhos");
    let solta = format!("{}(dados)", "drop");
    let toma = format!("{}(", "travar_dados");
    let linhas: Vec<&str> = FONTE
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let onde: Vec<usize> = linhas
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(&chamada))
        .map(|(i, _)| i)
        .collect();
    assert!(
        onde.len() >= 3,
        "as tres FASES B do servidor (cifra, acrescentar coluna, v10) deixaram de soltar \
         os velhos: {} chamada(s)",
        onde.len()
    );
    for i in onde {
        let antes = &linhas[i.saturating_sub(8)..i];
        let ultima_solta = antes.iter().rposition(|l| l.contains(&solta));
        let ultima_toma = antes.iter().rposition(|l| l.contains(&toma));
        assert!(
            ultima_solta.is_some() && ultima_toma.is_none_or(|t| t < ultima_solta.unwrap()),
            "o `soltar_volumes_velhos` da linha {:?} roda com a trava global na mao (pedido \
             647): o `close` do inode velho volta a custar 1,2 s sob a trava a 10 M de linhas",
            linhas[i].trim()
        );
    }
}

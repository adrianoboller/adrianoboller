//! **PITR** -- restaurar a um INSTANTE, e nao so ao instante da copia.
//!
//! A prova de ponta a ponta e uma so, e ela conta uma historia: linha 1, copia,
//! linha 2, alteracao da 1, linha 3. Restaurando com `ate` entre a alteracao e
//! a linha 3, o restaurado tem a 1 ALTERADA e a 2, e NAO tem a 3. Cada um dos
//! tres pedacos morre se o filtro de carimbo morrer.
use super::*;
use crate::usuarios::Cadastro;

fn ped(t: &str) -> Json {
    Json::analisar(t).unwrap()
}

/// Um servidor com a imagem da linha LIGADA -- e sem ela nao ha PITR, que
/// e o que o teste da recusa prova.
fn servidor(dir: &std::path::Path, imagem: bool) -> Arc<Servidor> {
    let mut c = Config {
        base: dir.join("dados"),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    c.replicacao.imagem_da_linha = imagem;
    Servidor::novo(c).unwrap()
}

fn criar_banco(s: &Arc<Servidor>) {
    let ses = Sessao::default();
    s.executar("criar_database", &ped(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &ped(r#"{"database":"b","tabela":"c",
                 "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                            {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &ses,
    )
    .unwrap();
}

fn inserir(s: &Arc<Servidor>, id: i64, nome: &str) {
    s.executar(
        "inserir",
        &ped(&format!(
            r#"{{"database":"b","tabela":"c","linha":{{"id":{id},"nome":"{nome}"}}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap();
}

fn backup(s: &Arc<Servidor>, dir: &std::path::Path) -> String {
    let destino = dir.join("copias").display().to_string();
    s.executar(
        "backup",
        &ped(&format!(
            r#"{{"destino":"{destino}","database":"b","zip":true}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .texto_ou("arquivo", "")
    .to_string()
}

/// Os `(carimbo_ms, operacao, rowid)` do diario vivo, em ordem.
fn diario(s: &Arc<Servidor>) -> Vec<(i64, String, u64)> {
    s.executar(
        "diario",
        &ped(r#"{"database":"b","tabela":"c","max":100}"#),
        &Sessao::default(),
    )
    .unwrap()
    .campo("eventos")
    .and_then(Json::lista)
    .unwrap()
    .iter()
    .map(|e| {
        (
            e.campo("carimbo_ms").and_then(Json::inteiro).unwrap(),
            e.texto_ou("operacao", "").to_string(),
            e.campo("rowid").and_then(Json::inteiro).unwrap() as u64,
        )
    })
    .collect()
}

fn linhas(s: &Arc<Servidor>, db: &str) -> Vec<(i64, String)> {
    s.executar(
        "varrer",
        &ped(&format!(r#"{{"database":"{db}","tabela":"c"}}"#)),
        &Sessao::default(),
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap()
    .iter()
    .map(|l| {
        (
            l.campo("id").and_then(Json::inteiro).unwrap(),
            l.texto_ou("nome", "").to_string(),
        )
    })
    .collect()
}

/// Espera o relogio andar um milissegundo. Sem isto, tres escritas seguidas
/// podem cair no MESMO carimbo, e um `ate` entre duas delas nao existiria.
fn passa_um_ms() {
    let antes = crate::agora_ms();
    while crate::agora_ms() == antes {
        std::hint::spin_loop();
    }
}

/// **613, a excecao:** a restauracao reaplica o diario do PROPRIO
/// servidor, e o anexo marcado de um servidor sem cofre volta. A recusa da
/// replica sem cofre (pedido 613) protege o dado que SAI da origem; aqui
/// ele nunca saiu -- o vivo e o restaurado moram no mesmo disco, com a
/// mesma falta de cofre.
///
/// # Prova real
///
/// Com o defeito reposto (a restauracao pelo `aplicar_evento`, que
/// recusa), a tabela para no instante da copia com «Falta o cofre» no
/// `parou_em`, e a linha 2 nao volta.
#[test]
fn restaurar_sem_cofre_reaplica_o_anexo_marcado() {
    let dir = DirTemp::novo("pitr-anexo-marcado");
    let s = servidor(&dir.0, true);
    let ses = Sessao::default();
    s.executar("criar_database", &ped(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &ped(r#"{"database":"b","tabela":"c",
                 "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                            {"nome":"ficha","tipo":"Memo","dado_pessoal":"sensivel"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &ses,
    )
    .unwrap();
    let por = |id: i64, ficha: &str| {
        s.executar(
            "inserir",
            &ped(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"id":{id},"ficha":"{ficha}"}}}}"#
            )),
            &ses,
        )
        .unwrap();
    };
    por(1, "antes da copia");
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    passa_um_ms();
    por(2, "depois da copia");

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_volta","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &ses,
        )
        .unwrap();
    let pitr = r.campo("pitr").expect("a resposta traz o bloco pitr");
    assert!(
        !pitr.escrever().contains("Falta o cofre"),
        "a restauracao recusou como replica: {}",
        pitr.escrever()
    );
    let fichas: Vec<String> = s
        .executar(
            "varrer",
            &ped(r#"{"database":"b_volta","tabela":"c"}"#),
            &ses,
        )
        .unwrap()
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.texto_ou("ficha", "").to_string())
        .collect();
    assert_eq!(fichas, vec!["antes da copia", "depois da copia"]);
}

/// A PROVA DE PONTA A PONTA.
///
/// Sabotagem que a derruba: aplicar tudo, sem olhar o carimbo -- trocar o
/// `if e.carimbo > ate_ms { pulados += 1; continue; }` por nada faz a
/// linha 3 aparecer no restaurado, e as tres asserces de baixo caem.
#[test]
fn restaura_ate_um_instante_no_meio_do_diario() {
    let dir = DirTemp::novo("pitr-ponta-a-ponta");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    passa_um_ms();
    let zip = backup(&s, &dir.0);

    passa_um_ms();
    inserir(&s, 2, "dois");
    passa_um_ms();
    s.executar(
        "atualizar",
        &ped(r#"{"database":"b","tabela":"c","rowid":1,
                     "linha":{"id":1,"nome":"um alterado"}}"#),
        &Sessao::default(),
    )
    .unwrap();
    passa_um_ms();
    inserir(&s, 3, "tres");

    // O corte fica ENTRE a alteracao da 1 (evento 3) e a inclusao da 3
    // (evento 4). Os carimbos saem do proprio diario, medidos: um `ate`
    // digitado a mao seria um numero que ninguem mediu.
    let eventos = diario(&s);
    assert_eq!(eventos.len(), 4, "{eventos:?}");
    let corte = eventos[3].0 - 1;
    assert!(corte >= eventos[2].0, "o relogio nao andou: {eventos:?}");

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_no_meio","ate_ms":{corte}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    let pitr = r.campo("pitr").expect("a resposta traz o bloco pitr");
    assert_eq!(
        pitr.campo("reaplicados").and_then(Json::inteiro),
        Some(2),
        "{}",
        pitr.escrever()
    );

    let restaurado = linhas(&s, "b_no_meio");
    assert_eq!(
        restaurado,
        vec![(1, "um alterado".into()), (2, "dois".into())],
        "a 1 alterada e a 2 entram; a 3 e depois do corte"
    );
    // E o original nao foi tocado: quatro linhas, com a 3.
    assert_eq!(linhas(&s, "b").len(), 3);

    // A resposta diz por tabela o que fez, e ate quando chegou.
    let t = &pitr.campo("tabelas").and_then(Json::lista).unwrap()[0];
    assert_eq!(t.texto_ou("tabela", ""), "c");
    assert_eq!(t.campo("reaplicados").and_then(Json::inteiro), Some(2));
    assert_eq!(t.campo("pulados").and_then(Json::inteiro), Some(1));
    assert_eq!(
        t.campo("ultimo_carimbo_ms").and_then(Json::inteiro),
        Some(eventos[2].0),
        "o ultimo aplicado e a alteracao da linha 1"
    );
    assert!(t.campo("parou_em").is_none(), "{}", t.escrever());
}

/// O carimbo do evento reaplicado e o do ORIGINAL, e nao a hora da
/// restauracao: o diario do restaurado e trilha de auditoria.
#[test]
fn o_diario_do_restaurado_guarda_o_carimbo_original() {
    let dir = DirTemp::novo("pitr-carimbo");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    passa_um_ms();
    inserir(&s, 2, "dois");

    let eventos = diario(&s);
    s.executar(
        "restaurar_backup",
        &ped(&format!(
            r#"{{"origem":"{zip}","database":"b2","ate_ms":{}}}"#,
            eventos[1].0
        )),
        &Sessao::default(),
    )
    .unwrap();

    let d = s
        .executar(
            "diario",
            &ped(r#"{"database":"b2","tabela":"c","max":100}"#),
            &Sessao::default(),
        )
        .unwrap();
    let lista = d.campo("eventos").and_then(Json::lista).unwrap();
    assert_eq!(lista.len(), 2);
    assert_eq!(
        lista[1].campo("carimbo_ms").and_then(Json::inteiro),
        Some(eventos[1].0),
        "o evento reaplicado guarda o instante em que a escrita NASCEU"
    );
}

/// **O TESTE DO COMPORTAMENTO VELHO.** Sem `ate`, nada muda: nem a
/// resposta ganha campo, nem o diario e reaplicado.
#[test]
fn quem_nao_manda_ate_nao_ve_diferenca() {
    let dir = DirTemp::novo("pitr-velho");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);
    inserir(&s, 2, "depois do backup");

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(r#"{{"origem":"{zip}","database":"b_igual"}}"#)),
            &Sessao::default(),
        )
        .unwrap();
    assert!(r.campo("pitr").is_none(), "{}", r.escrever());
    assert_eq!(
        linhas(&s, "b_igual"),
        vec![(1, "um".into())],
        "a restauracao simples continua sendo o retrato da copia"
    );
}

/// `ate` ANTES da copia: o diario so anda para a frente.
///
/// Sabotagem: trocar `if ate_ms < copia_ms` por `if false` faz a
/// restauracao passar e devolver zero reaplicados -- e o teste cai na
/// primeira linha, porque esperava erro.
#[test]
fn ate_antes_da_copia_e_recusado() {
    let dir = DirTemp::novo("pitr-antes");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);

    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_antes","ate":"2020-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(erro.contains("ANTES do instante da copia"), "{erro}");
    assert!(erro.contains("2020-01-01"), "diz o instante pedido: {erro}");
    // E nada foi criado: a recusa acontece antes de o disco ser tocado.
    assert!(!dir.0.join("dados/b_antes").exists());
}

/// Backup sem o carimbo em milissegundos: PITR recusa NOMEANDO o campo, e
/// a restauracao simples do mesmo arquivo continua funcionando.
///
/// Sabotagem: fazer o `quando_ms` cair no `unwrap_or(0)` em vez de recusar
/// faz a restauracao aceitar e reaplicar o diario INTEIRO, porque tudo e
/// depois da epoca -- e a segunda asserçao cai.
#[test]
fn backup_sem_carimbo_recusa_o_pitr_e_nao_a_restauracao() {
    let dir = DirTemp::novo("pitr-sem-carimbo");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    // Uma copia em PASTA, para dar para reescrever o manifesto como um
    // backup velho: sem `quando_ms`, e com o `quando` que um manifesto
    // escrito a mao teria.
    let pasta = dir.0.join("copia_velha");
    s.executar(
        "backup",
        &ped(&format!(
            r#"{{"destino":"{}","zip":false}}"#,
            pasta.display()
        )),
        &Sessao::default(),
    )
    .unwrap();
    let man = pasta.join("backup.json");
    let j = Json::analisar(&std::fs::read_to_string(&man).unwrap()).unwrap();
    let mut campos = vec![
        ("phxsql", Json::texto_de(j.texto_ou("phxsql", ""))),
        ("quando", Json::texto_de("agora")),
        (
            "arquivos",
            j.campo("arquivos").cloned().unwrap_or(Json::Nulo),
        ),
        ("bytes", j.campo("bytes").cloned().unwrap_or(Json::Nulo)),
    ];
    campos.push(("conteudo", j.campo("conteudo").cloned().unwrap()));
    std::fs::write(&man, Json::objeto(campos).escrever()).unwrap();

    inserir(&s, 2, "depois");
    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{}","de":"b","database":"b_velho",
                         "ate":"2099-01-01T00:00:00Z"}}"#,
                pasta.display()
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(
        erro.contains("quando_ms"),
        "nomeia o campo que falta: {erro}"
    );

    // E o MESMO arquivo restaura sem `ate`, como sempre restaurou.
    s.executar(
        "restaurar_backup",
        &ped(&format!(
            r#"{{"origem":"{}","de":"b","database":"b_velho"}}"#,
            pasta.display()
        )),
        &Sessao::default(),
    )
    .unwrap();
    assert_eq!(linhas(&s, "b_velho"), vec![(1, "um".into())]);
}

/// Sem imagem no diario o evento nao da para reaplicar, e a recusa diz
/// QUAL interruptor ligar.
///
/// Sabotagem: tirar o `if !self.config.replicacao.imagem_da_linha` faz a
/// restauracao seguir; o `aplicar_evento` recusa evento a evento e o
/// `parou_em` aparece -- ou seja, o teste cai na primeira asserçao (nao
/// houve erro) e o estrago vira "restaurou e nao reaplicou".
#[test]
fn sem_imagem_no_diario_o_pitr_recusa_dizendo_o_interruptor() {
    let dir = DirTemp::novo("pitr-sem-imagem");
    let s = servidor(&dir.0, false);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);
    inserir(&s, 2, "dois");

    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_sem","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(erro.contains("imagem_da_linha"), "{erro}");
    assert!(!dir.0.join("dados/b_sem").exists(), "nada foi escrito");
}

/// Restaurar POR CIMA tira do lugar o database cujo diario o PITR leria.
#[test]
fn por_cima_com_ate_e_recusado() {
    let dir = DirTemp::novo("pitr-por-cima");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);

    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b","modo":"por_cima",
                         "confirmar":true,"ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(erro.contains("POR CIMA"), "{erro}");
    assert!(erro.contains("diario VIVO"), "{erro}");
}

/// O database de origem sumiu: nao ha diario vivo para reaplicar.
#[test]
fn sem_o_database_vivo_o_pitr_recusa() {
    let dir = DirTemp::novo("pitr-sem-vivo");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);
    // Nao ha operacao de apagar database no protocolo; o que se simula
    // aqui e o banco tirado do lugar por fora -- que e exatamente o estado
    // em que o PITR nao tem diario vivo para ler.
    std::fs::rename(dir.0.join("dados/b"), dir.0.join("b_fora")).unwrap();

    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_orfao","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(erro.contains("nao existe mais neste servidor"), "{erro}");
}

/// `ate` que nao e instante recusa NOMEANDO o que se espera -- e o fuso
/// escrito na mao e recusado em vez de ignorado.
#[test]
fn ate_ilegivel_recusa_e_o_fuso_nao_e_engolido() {
    let dir = DirTemp::novo("pitr-ate-ruim");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    let zip = backup(&s, &dir.0);

    for ate in ["ontem", "2026-09-08T15:00:00+03:00"] {
        let erro = s
            .executar(
                "restaurar_backup",
                &ped(&format!(
                    r#"{{"origem":"{zip}","database":"b_x","ate":"{ate}"}}"#
                )),
                &Sessao::default(),
            )
            .unwrap_err()
            .to_string();
        assert!(erro.contains("nao e um instante"), "{ate}: {erro}");
    }
    // Os dois campos juntos tambem.
    let erro = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_x",
                         "ate":"2099-01-01T00:00:00Z","ate_ms":1}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(erro.contains("nao os dois"), "{erro}");
}

/// A tabela apagada e RECRIADA depois do backup: o diario vivo nao
/// continua o da copia, e a tabela para no instante da copia em vez de
/// receber linhas de outra vida.
///
/// Sabotagem: fazer `diario_vivo_continua` devolver sempre `Ok(())` faz o
/// `parou_em` sumir e a reaplicacao gravar; o teste cai nas duas ultimas
/// asserçoes.
#[test]
fn diario_que_nao_continua_a_copia_para_nomeando_a_tabela() {
    let dir = DirTemp::novo("pitr-recriada");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    inserir(&s, 2, "dois");
    passa_um_ms();
    let zip = backup(&s, &dir.0);

    // A tabela morre e volta a nascer: o `.log` dela recomeca do zero.
    s.executar(
        "excluir_tabela",
        &ped(r#"{"database":"b","tabela":"c","confirmar":"c"}"#),
        &Sessao::default(),
    )
    .unwrap();
    criar_banco_tabela_de_novo(&s);
    inserir(&s, 9, "outra vida");

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_recriada","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    let t = &r
        .campo("pitr")
        .and_then(|p| p.campo("tabelas"))
        .and_then(Json::lista)
        .unwrap()[0];
    assert_eq!(t.campo("reaplicados").and_then(Json::inteiro), Some(0));
    let motivo = t.texto_ou("parou_em", "");
    // O TEXTO da conta de tamanho, e nao so «recusou»: e ele que diz ao
    // operador o que houve, e e a unica coisa que essa metade acrescenta.
    assert!(
        motivo.contains("tem 1 evento(s) e a copia tinha 2"),
        "{motivo}"
    );
    assert!(motivo.contains("nao continua o da copia"), "{motivo}");
    assert_eq!(
        linhas(&s, "b_recriada"),
        vec![(1, "um".into()), (2, "dois".into())],
        "a tabela ficou no instante da copia"
    );
}

/// O irmao do teste de cima, e ele existe porque os dois GUARDAS de
/// `diario_vivo_continua` sao dois, e um teste que so acorda um deles
/// deixa o outro sem prova.
///
/// Ali o diario vivo ficou mais CURTO que a copia, e a conta de tamanho
/// bastava. Aqui a tabela recriada recebe TRES linhas, entao o diario
/// vivo tem tres eventos contra os dois da copia -- a conta de tamanho
/// passa, e quem acha a troca e a comparacao do evento daquela posicao.
#[test]
fn diario_vivo_maior_e_diferente_tambem_para() {
    let dir = DirTemp::novo("pitr-recriada-maior");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    inserir(&s, 2, "dois");
    passa_um_ms();
    let zip = backup(&s, &dir.0);

    s.executar(
        "excluir_tabela",
        &ped(r#"{"database":"b","tabela":"c","confirmar":"c"}"#),
        &Sessao::default(),
    )
    .unwrap();
    criar_banco_tabela_de_novo(&s);
    for (id, nome) in [(7, "sete"), (8, "oito"), (9, "nove")] {
        inserir(&s, id, nome);
    }

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_maior","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    let t = &r
        .campo("pitr")
        .and_then(|p| p.campo("tabelas"))
        .and_then(Json::lista)
        .unwrap()[0];
    assert_eq!(t.campo("reaplicados").and_then(Json::inteiro), Some(0));
    let motivo = t.texto_ou("parou_em", "");
    assert!(
        motivo.contains("nao e o mesmo que a copia tinha ali"),
        "{motivo}"
    );
    assert_eq!(
        linhas(&s, "b_maior"),
        vec![(1, "um".into()), (2, "dois".into())],
        "a tabela ficou no instante da copia, sem as linhas da outra vida"
    );
}

fn criar_banco_tabela_de_novo(s: &Arc<Servidor>) {
    s.executar(
        "criar_tabela",
        &ped(r#"{"database":"b","tabela":"c",
                 "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                            {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &Sessao::default(),
    )
    .unwrap();
}

/// Tabela que NASCEU depois da copia nao e criada, e sai nomeada em
/// `novas_na_origem`: refaze-la exigiria a historia do esquema.
#[test]
fn tabela_nova_na_origem_aparece_na_resposta_e_nao_e_criada() {
    let dir = DirTemp::novo("pitr-nova");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    s.executar(
        "criar_tabela",
        &ped(r#"{"database":"b","tabela":"nova",
                 "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &Sessao::default(),
    )
    .unwrap();

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_nova","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    let pitr = r.campo("pitr").unwrap();
    let novas: Vec<&str> = pitr
        .campo("novas_na_origem")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(Json::texto)
        .collect();
    assert_eq!(novas, vec!["nova"], "{}", pitr.escrever());
    assert!(!dir.0.join("dados/b_nova/nova.reg").exists());
}

/// **Ninguem enxerga o banco meio restaurado.**
///
/// # O que ele mede, e por que nao mede o veredito
///
/// Nao a resposta do pedido -- ela e a mesma nos dois desenhos --, e sim o
/// que um TERCEIRO ve enquanto a restauracao acontece. Um vigia em outra
/// thread fica de olho no `.reg` da tabela restaurada e guarda o **primeiro
/// tamanho** que consegue ler; ele tem de ser o tamanho FINAL.
///
/// Com a reaplicacao acontecendo depois do `confirmar` -- o desenho ate
/// 18/09/2026 --, o database entra na raiz no instante da COPIA e cresce
/// evento a evento com a trava na mao: o vigia le um arquivo pequeno e o
/// teste cai. Com ela no palco, o `rename` poe la um database que ja e o
/// resultado, e nao ha tamanho menor para ninguem ler.
///
/// # Por que trezentas linhas, e nao tres
///
/// Porque com tres a reaplicacao acaba antes de o vigia acordar, e ele
/// leria o tamanho final mesmo com o defeito de pe -- *teste que passa por
/// engano e pior que teste que falta*. Medido nesta maquina, com o defeito
/// reposto (reaplicar depois do `confirmar`, na raiz de dados): o vigia le
/// **570 bytes** onde o fim tem **17.912**, e pega o defeito em **10 de 10**
/// corridas. Com o conserto, 10 de 10 passam.
#[test]
fn o_restaurado_so_aparece_com_o_diario_ja_reaplicado() {
    const LINHAS: i64 = 300;
    let dir = DirTemp::novo("pitr-atomico");
    let s = servidor(&dir.0, true);
    criar_banco(&s);
    inserir(&s, 1, "um");
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    for id in 2..=LINHAS {
        inserir(&s, id, "x");
    }

    let alvo = dir.0.join("dados").join("b_atomico").join("c.reg");
    let parar = Arc::new(AtomicBool::new(false));
    let vigia = {
        let alvo = alvo.clone();
        let parar = Arc::clone(&parar);
        std::thread::spawn(move || {
            let comeco = Instant::now();
            loop {
                if let Ok(m) = std::fs::metadata(&alvo) {
                    return Some(m.len());
                }
                // O prazo existe para o caso de a restauracao FALHAR: sem
                // ele o vigia esperaria um arquivo que nunca vem, e o
                // `join` penduraria a suite inteira em vez de reprovar.
                if parar.load(Ordering::SeqCst) || comeco.elapsed() > Duration::from_secs(60) {
                    return std::fs::metadata(&alvo).ok().map(|m| m.len());
                }
                std::thread::yield_now();
            }
        })
    };

    s.executar(
        "restaurar_backup",
        &ped(&format!(
            r#"{{"origem":"{zip}","database":"b_atomico","ate":"2099-01-01T00:00:00Z"}}"#
        )),
        &Sessao::default(),
    )
    .unwrap();
    parar.store(true, Ordering::SeqCst);

    let primeiro = vigia
        .join()
        .expect("o vigia caiu")
        .expect("o vigia nunca viu o .reg do database restaurado");
    let depois = std::fs::metadata(&alvo).unwrap().len();
    assert_eq!(
        primeiro, depois,
        "o database restaurado apareceu na raiz de dados com {primeiro} bytes \
             e terminou com {depois}: houve uma janela em que outra sessao via o \
             banco no instante da copia, sem o diario reaplicado"
    );
    assert_eq!(linhas(&s, "b_atomico").len(), LINHAS as usize);
}

/// **O RED do R3 do pedido 299** -- o PITR corta evento a evento, e um commit
/// de varias tabelas cujo relogio atravessa um milissegundo, com o `ate`
/// dentro dele, restaura MEIA venda.
///
/// A venda e um `COMMIT` so: uma linha em `vendas`, os itens em `itens`, uma
/// em `pagamentos`. O carimbo e o relogio de CADA evento, entao uma venda
/// grande atravessa o milissegundo sozinha (a prova confere que atravessou, e
/// dobra a venda se nao). O corte vai no carimbo do PRIMEIRO item.
///
/// Vermelho medido no HEAD `edbd6180` (`(1, 15, 0)`) e de novo no
/// `2e10a6b7`, antes da F3: `(1, 1, 0)` -- a venda e o primeiro item, sem o
/// pagamento. O conserto e a F3 do desenho
/// (`docs/propostas/atomicidade-na-replica-299.md` §6): o PITR passa pelo
/// `Juntador`, e a transacao entra se o MAIOR carimbo dela <= `ate`. Com o
/// corte por evento reposto, ela volta a cair (guarda
/// `pitr-corta-evento-a-evento` do catalogo).
///
/// Alem da tupla, a prova confere que a resposta DIZ o corte: a transacao
/// que ficou fora, pelo relogio.
#[test]
fn o_pitr_nao_restaura_meia_venda() {
    let dir = DirTemp::novo("pitr-meia-venda");
    let s = servidor(&dir.0, true);
    let pede = |corpo: &str| {
        let mut ses = Sessao {
            ligacao: 299,
            ip: "127.0.0.1".into(),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        r.unwrap_or_else(|e| panic!("{corpo}: {e}"))
    };
    pede(r#""op":"criar_database","database":"b""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        pede(&format!(
            r#""op":"criar_tabela","database":"b","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
    }
    passa_um_ms();
    let destino = dir.0.join("copias").display().to_string();
    let zip = pede(&format!(
        r#""op":"backup","destino":"{destino}","database":"b","zip":true"#
    ))
    .texto_ou("arquivo", "")
    .to_string();
    passa_um_ms();

    let carimbos_dos_itens = || -> Vec<i64> {
        s.executar(
            "diario",
            &ped(r#"{"database":"b","tabela":"itens","max":100000}"#),
            &Sessao::default(),
        )
        .unwrap()
        .campo("eventos")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|e| e.campo("carimbo_ms").and_then(Json::inteiro).unwrap())
        .collect()
    };
    // A venda, num COMMIT so, grande o bastante para o relogio andar dentro
    // dela: medido, 600 inclusoes num COMMIT de `debug` levam varios ms.
    let itens = 600usize;
    pede(r#""op":"begin","database":"b""#);
    pede(r#""op":"inserir","database":"b","tabela":"vendas","linha":{"id":1,"venda":1}"#);
    for i in 1..=itens {
        pede(&format!(
            r#""op":"inserir","database":"b","tabela":"itens","linha":{{"id":{i},"venda":1}}"#
        ));
    }
    pede(r#""op":"inserir","database":"b","tabela":"pagamentos","linha":{"id":1,"venda":1}"#);
    pede(r#""op":"commit""#);
    let carimbos = carimbos_dos_itens();
    assert!(
        carimbos.first() != carimbos.last(),
        "o relogio nao andou dentro da venda de {itens} itens: a prova nao tem corte \
         no meio -- aumente a venda"
    );
    let corte = carimbos[0];

    let resposta = pede(&format!(
        r#""op":"restaurar_backup","origem":"{zip}","database":"b_meia","ate_ms":{corte}"#
    ));
    let conta = |tabela: &str| {
        s.executar(
            "varrer",
            &ped(&format!(r#"{{"database":"b_meia","tabela":"{tabela}"}}"#)),
            &Sessao::default(),
        )
        .unwrap()
        .campo("linhas")
        .and_then(Json::lista)
        .map_or(0, <[Json]>::len)
    };
    let r = (conta("vendas"), conta("itens"), conta("pagamentos"));
    assert!(
        r == (0, 0, 0) || r == (1, itens, 1),
        "o PITR restaurou MEIA venda (vendas, itens, pagamentos) = {r:?}, com o \
         corte em {corte} e os itens de {} a {}",
        carimbos[0],
        carimbos[carimbos.len() - 1]
    );
    let parada = resposta
        .campo("pitr")
        .and_then(|p| p.campo("parou_na_transacao"))
        .cloned()
        .unwrap_or(Json::Nulo);
    assert_eq!(
        parada.texto_ou("motivo", ""),
        "relogio",
        "a resposta nao diz o corte: {}",
        resposta.escrever()
    );
}

/// O despacho de um pedido com sessao de transacao -- o `BEGIN`/`COMMIT`
/// precisa da ligacao, que o `executar` nao tem.
fn pedir(s: &Arc<Servidor>, corpo: &str) -> Json {
    let mut ses = Sessao {
        ligacao: 2990,
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r.unwrap_or_else(|e| panic!("{corpo}: {e}"))
}

/// Os ids de transacao do diario de `database/tabela`, em ordem.
fn txs_do_diario(s: &Arc<Servidor>, database: &str, tabela: &str) -> Vec<u64> {
    let trava = s.travar_dados().unwrap();
    let db = trava.abrir_database(database).unwrap();
    let mut t = db.abrir_qualificada(tabela).unwrap();
    let n = t.eventos().unwrap();
    t.diario(0, n).unwrap().iter().map(|e| e.tx).collect()
}

fn criar_duas(s: &Arc<Servidor>, a: &str, b: &str) {
    pedir(s, r#""op":"criar_database","database":"b""#);
    for tabela in [a, b] {
        pedir(
            s,
            &format!(
                r#""op":"criar_tabela","database":"b","tabela":"{tabela}",
                   "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}}],
                   "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
            ),
        );
    }
}

/// **Pedido 299, F3 -- a continuidade rompida numa tabela segura o database
/// INTEIRO no instante da copia.**
///
/// `c` e apagada e recriada depois do backup; `d` so cresce. As transacoes
/// que tocaram a `c` velha entre a copia e a exclusao nao se reconhecem mais
/// -- e uma delas podia ter tocado a `d` tambem. Reaplicar a `d` sozinha era
/// o comportamento de ate 10/10/2026, e e a meia venda pela porta da
/// continuidade.
///
/// Com o defeito reposto (a parada por tabela: as outras andam), a `d` volta
/// com `reaplicados: 1` e o teste cai na primeira asserção.
#[test]
fn a_tabela_que_nao_continua_segura_as_outras_no_instante_da_copia() {
    let dir = DirTemp::novo("pitr-segura-todas");
    let s = servidor(&dir.0, true);
    criar_duas(&s, "c", "d");
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"c","linha":{"id":1}"#,
    );
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"d","linha":{"id":1}"#,
    );
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    passa_um_ms();
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"d","linha":{"id":2}"#,
    );
    s.executar(
        "excluir_tabela",
        &ped(r#"{"database":"b","tabela":"c","confirmar":"c"}"#),
        &Sessao::default(),
    )
    .unwrap();
    pedir(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"c",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"c","linha":{"id":9}"#,
    );

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_segura","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    let pitr = r.campo("pitr").unwrap();
    let tabelas = pitr.campo("tabelas").and_then(Json::lista).unwrap();
    let d = tabelas
        .iter()
        .find(|t| t.texto_ou("tabela", "") == "d")
        .unwrap();
    assert_eq!(
        d.campo("reaplicados").and_then(Json::inteiro),
        Some(0),
        "a d andou sem a c: {}",
        pitr.escrever()
    );
    assert!(
        d.texto_ou("parou_em", "")
            .contains("segurada no instante da copia pela tabela"),
        "{}",
        d.escrever()
    );
    let parada = pitr.campo("parou_na_transacao").unwrap();
    assert_eq!(parada.texto_ou("motivo", ""), "continuidade");
    assert_eq!(parada.texto_ou("tabela", ""), "c");
    let conta = |tabela: &str| {
        s.executar(
            "varrer",
            &ped(&format!(r#"{{"database":"b_segura","tabela":"{tabela}"}}"#)),
            &Sessao::default(),
        )
        .unwrap()
        .campo("linhas")
        .and_then(Json::lista)
        .map_or(0, <[Json]>::len)
    };
    assert_eq!((conta("c"), conta("d")), (1, 1), "o instante da copia");
}

/// **Pedido 299, F3 (o F10 do ⏸ 717) -- o restaurado guarda um id por
/// transacao, o MESMO da original.**
///
/// Uma venda de duas tabelas num `COMMIT`, e um autocommit depois. No
/// diario vivo a venda tem um id nas duas tabelas e o autocommit outro; o
/// restaurado tem de sair igual, senao uma replica tirada dele recebe a
/// venda juntada com o que veio depois -- ou partida.
///
/// Com o defeito reposto (a reaplicacao inteira sob a unidade da tomada),
/// os tres eventos saem com UM id so, novo, e a comparacao cai.
#[test]
fn o_restaurado_guarda_o_id_de_cada_transacao() {
    let dir = DirTemp::novo("pitr-um-id-por-transacao");
    let s = servidor(&dir.0, true);
    criar_duas(&s, "vendas", "pagamentos");
    passa_um_ms();
    let zip = backup(&s, &dir.0);
    passa_um_ms();
    pedir(&s, r#""op":"begin","database":"b""#);
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"vendas","linha":{"id":1}"#,
    );
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"pagamentos","linha":{"id":1}"#,
    );
    pedir(&s, r#""op":"commit""#);
    pedir(
        &s,
        r#""op":"inserir","database":"b","tabela":"vendas","linha":{"id":2}"#,
    );

    let r = s
        .executar(
            "restaurar_backup",
            &ped(&format!(
                r#"{{"origem":"{zip}","database":"b_ids","ate":"2099-01-01T00:00:00Z"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        r.campo("pitr")
            .and_then(|p| p.campo("transacoes"))
            .and_then(Json::inteiro),
        Some(2),
        "{}",
        r.escrever()
    );
    let vendas = txs_do_diario(&s, "b", "vendas");
    let pagamentos = txs_do_diario(&s, "b", "pagamentos");
    assert_eq!(vendas.len(), 2);
    assert_eq!(vendas[0], pagamentos[0], "a venda e uma transacao so");
    assert_ne!(vendas[0], vendas[1]);
    assert_eq!(txs_do_diario(&s, "b_ids", "vendas"), vendas);
    assert_eq!(txs_do_diario(&s, "b_ids", "pagamentos"), pagamentos);
}

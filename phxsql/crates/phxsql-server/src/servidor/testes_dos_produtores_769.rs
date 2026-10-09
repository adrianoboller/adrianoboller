//! Pedido 769 (fatia P0 do 765, A15 do 707): os alarmes que estavam
//! declarados no `enum Alarme` e sem produtor nenhum, um teste por produtor,
//! pelo caminho que o servidor usa.
//!
//! # O que cada teste confere
//!
//! - alarme de SERVIDOR: a pedra sobe no sedimento (`alarme::sedimento`);
//!   quando ha atividade amarrada, a ocorrencia chega ao correio DESTE
//!   servidor (a camada da tarefa, e nao a do processo, que e de quem subiu
//!   por ultimo neste binario de testes);
//! - alarme de TAREFA: o bit na atividade e a ocorrencia no correio.
//!
//! # RED
//!
//! Tirar a linha do `telemetria::sinal` do produtor derruba o teste dele: o
//! sedimento e do processo, mas nenhum outro teste desta crate chama o
//! `sinal` destes alarmes direto (o `alarme_de_servidor_vai_ao_sedimento`
//! deixou o `FechoRecusado` por isso), entao sem o produtor a pedra nao sobe.
//! Os testes medem `>`, e nao `==`, porque outro teste do mesmo binario pode
//! passar pelo mesmo produtor ao mesmo tempo -- e com o produtor tirado,
//! ninguem passa.

use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("p769-{rotulo}"))
}

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
    // O ESCAPE ESCRITO: estes testes nao passam pelo fio, e o que medem e o
    // produtor -- nao o portao da cifra.
    c.cifra_fio.exigir = false;
    c
}

/// Quantas vezes um alarme ja foi ao sedimento neste processo.
fn vezes(alarme: Alarme) -> u64 {
    crate::aquario::alarme::sedimento()
        .into_iter()
        .find(|p| p.alarme == alarme)
        .map_or(0, |p| p.vezes)
}

/// O prefixo da tarefa das atividades amarradas por este arquivo.
///
/// O alarme de SERVIDOR que outro teste do binario dispara sem atividade
/// amarrada vai a camada do PROCESSO -- a do ultimo servidor criado, que pode
/// ser o de um teste daqui. Medido em 09/10/2026 no irmao deste arquivo
/// (`testes_da_protecao_766`): um `FirewallBloqueou` vizinho fez o
/// `len() == 1` ver 2. Contar so o que nasceu nas atividades daqui tira o
/// vizinho da conta, sem afrouxar nada.
const TAREFA: &str = "dados:p769-";

/// As ocorrencias de `alarme` que esperam o carteiro neste servidor, nascidas
/// nas atividades deste arquivo.
fn ocorrencias_de(s: &Arc<Servidor>, alarme: Alarme) -> Vec<crate::ocorrencias::Ocorrencia> {
    s.ocorrencias
        .correio()
        .retirar(
            std::time::Duration::from_millis(1),
            |c| matches!(c, Carta::Ocorrencia(o) if o.alarme == alarme && o.tarefa.starts_with(TAREFA)),
        )
        .into_iter()
        .filter_map(|c| match c {
            Carta::Ocorrencia(o) => Some(o),
            Carta::Saude(_) => None,
        })
        .collect()
}

/// Uma atividade deste servidor amarrada a esta thread, como a conexao faz.
fn amarrada(s: &Arc<Servidor>, ip: &str) -> Arc<crate::telemetria::Atividade> {
    s.telemetria
        .entrar(
            &format!("dados:p769-{ip}"),
            "dados",
            ip,
            1,
            crate::agora_ms(),
        )
        .expect("a telemetria nasce ligada")
}

// ------------------------------------------------------- FirewallBloqueou

/// **O aceite da P0 do 765:** cinco senhas erradas (o padrao de
/// `tentativas_ate_bloquear`) bloqueiam o IP e geram UMA ocorrencia
/// `firewall_bloqueou`, e a pedra sobe. Pelo `despachar`, o mesmo caminho da
/// porta de dados. RED: tirar o `sinal` do `blacklist::aplicar_no_firewall`
/// -> zero ocorrencias e nenhuma pedra.
#[test]
fn cinco_senhas_erradas_bloqueiam_e_geram_uma_ocorrencia() {
    let dir = dir_temp("fw-leve");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let ip = "203.0.113.76";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let antes = vezes(Alarme::FirewallBloqueou);
    let mut sessao = Sessao::default();
    for tentativa in 1..=5 {
        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"login","usuario":"ana","senha":"errada"}"#,
            &mut sessao,
            ip,
        );
        assert!(r.is_err(), "tentativa {tentativa}");
        if tentativa < 5 {
            assert!(
                ocorrencias_de(&s, Alarme::FirewallBloqueou).is_empty(),
                "a tentativa {tentativa} nao bloqueia e nao pode alarmar"
            );
        }
    }
    assert!(
        s.barrado(ip, crate::agora_ms()).is_some(),
        "cinco senhas erradas tinham de bloquear"
    );
    let vistas = ocorrencias_de(&s, Alarme::FirewallBloqueou);
    assert_eq!(vistas.len(), 1, "uma ocorrencia por bloqueio: {vistas:?}");
    assert_eq!(vistas[0].ip, ip, "a ocorrencia leva o IP de quem bloqueou");
    assert!(vezes(Alarme::FirewallBloqueou) > antes, "a pedra nao subiu");
}

/// O IRMAO grave: comando proibido bloqueia na primeira, pelo mesmo ponto.
#[test]
fn comando_proibido_bloqueia_e_gera_uma_ocorrencia() {
    let dir = dir_temp("fw-grave");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    let s = Servidor::novo(c).unwrap();
    let ip = "203.0.113.77";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let antes = vezes(Alarme::FirewallBloqueou);
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        ip,
    );
    assert!(r.is_err());
    assert_eq!(ocorrencias_de(&s, Alarme::FirewallBloqueou).len(), 1);
    assert!(vezes(Alarme::FirewallBloqueou) > antes, "a pedra nao subiu");
}

/// O comportamento VELHO: a whitelist protege, nada bloqueia -- e nada
/// alarma. Alarme de bloqueio sem bloqueio seria mentira na tela.
#[test]
fn ip_protegido_nao_bloqueia_e_nao_alarma() {
    let dir = dir_temp("fw-protegido");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    c.politica.whitelist = vec!["203.0.113.78".into()];
    let s = Servidor::novo(c).unwrap();
    let ip = "203.0.113.78";
    let a = amarrada(&s, ip);
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        ip,
    );
    assert!(r.is_err(), "a operacao continua recusada");
    assert!(s.barrado(ip, crate::agora_ms()).is_none());
    assert!(ocorrencias_de(&s, Alarme::FirewallBloqueou).is_empty());
}

// ---------------------------------------------------------- FechoRecusado

/// O fecho que o `fsync` recusou vira pedra -- inclusive o erro que NAO e de
/// disco, que a saude do disco nao conta. Prova o ponto unico que os dois
/// ramos do `descarregar_sujas_com` (K = 1 e K > 1) chamam; o sitio nao tem
/// prova com defeito reposto pelo mesmo motivo do teste irmao da saude do
/// disco: nao ha como fazer o `fsync` falhar sem injecao no sistema de
/// arquivos. RED: tirar o `sinal` do `fecho_recusado`.
#[test]
fn o_fecho_recusado_vira_pedra() {
    let dir = dir_temp("fecho");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let antes = vezes(Alarme::FechoRecusado);
    s.fecho_recusado("b/t", &PhxError::Esquema("nao e disco".into()));
    assert_eq!(s.saude.erros_es(), 0, "a saude continua so com disco");
    assert!(vezes(Alarme::FechoRecusado) > antes, "a pedra nao subiu");
    let depois_do_primeiro = vezes(Alarme::FechoRecusado);
    s.fecho_recusado("b/t", &PhxError::Io(std::io::Error::from_raw_os_error(5)));
    assert!(vezes(Alarme::FechoRecusado) > depois_do_primeiro);
}

// ----------------------------------------------- os tres alarmes do arranque

/// A sentinela do 509 deixada por um boot ANTERIOR: o servidor sobe (o cache
/// daquele boot se foi) e a pedra `fsync_recusado_antes` sobe com ele. O
/// arranque de sempre nao sinaliza. RED: tirar o `sinal` do
/// `sinalizar_o_arranque` (ou a chamada dele do `Servidor::novo`).
#[test]
fn a_sentinela_de_um_boot_anterior_vira_pedra_no_arranque() {
    if super::servico_marca_01::boot_id().is_none() {
        eprintln!("PULADO: este sistema nao informa o boot_id");
        return;
    }
    let dir = dir_temp("sentinela");
    std::fs::write(
        dir.join(super::servico_marca_01::SENTINELA_509),
        "boot_id=00000000-0000-0000-0000-000000000769\ncaminho=/x/y.reg\nerro=EIO (5)\n",
    )
    .unwrap();
    let antes = vezes(Alarme::FsyncRecusadoAntes);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(
        !dir.join(super::servico_marca_01::SENTINELA_509).exists(),
        "a sentinela do boot anterior tinha de sair"
    );
    assert!(
        vezes(Alarme::FsyncRecusadoAntes) > antes,
        "a pedra nao subiu"
    );
    drop(s);
}

/// A marca de commit que a recuperacao do arranque nao consegue completar
/// -- a tabela nao vai ao disco (o `.pag` virou diretorio) -- fica, e vira a
/// pedra `marca_nao_resolvida`. RED: tirar o `sinal` do laco das marcas no
/// `sinalizar_o_arranque`.
#[test]
fn a_marca_que_o_arranque_nao_resolve_vira_pedra() {
    use crate::transacao::{gravar_marca, Acao, Escrita};
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;
    let dir = dir_temp("marca");
    let marca = {
        let inst = phxsql_store::catalogo::Instancia::nova(&*dir).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let esquema = Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = db.criar_tabela(None, esquema).unwrap();
        t.sincronizar().unwrap();
        drop(t);
        let marca = gravar_marca(
            db.caminho(),
            7,
            0,
            &[Escrita {
                database: "loja".into(),
                tabela: "clientes".into(),
                acao: Acao::Inserir,
                rowid: 1,
                linha: vec![Value::Int(1), Value::Str("Ana".into())],
                linha_antiga: Vec::new(),
                motivo: String::new(),
                cascata_na_lista: false,
                elo_do_empilhar: false,
                elo_da_cascata: false,
            }],
        )
        .unwrap();
        let pag = db.caminho().join("clientes.pag");
        let _ = std::fs::remove_file(&pag);
        std::fs::create_dir(&pag).unwrap();
        marca
    };
    let antes = vezes(Alarme::MarcaNaoResolvida);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(
        marca.exists(),
        "premissa: a marca nao resolvida fica no disco"
    );
    assert!(
        vezes(Alarme::MarcaNaoResolvida) > antes,
        "a pedra nao subiu"
    );
    drop(s);
}

/// O indice que a queda deixou para tras e o arranque reconstruiu vira a
/// pedra `indice_atrasado`, pela MESMA decisao do aviso por e-mail (o
/// `evento_do_arranque`). O arranque limpo nao sinaliza. RED: tirar o
/// `sinal` do indice no `sinalizar_o_arranque`.
#[test]
fn o_indice_que_ficou_para_tras_vira_pedra_no_arranque() {
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;
    let dir = dir_temp("indice");
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
    // A queda que deixa o `.ndx` marcado: escrita sem `sincronizar`.
    {
        let inst = phxsql_store::catalogo::Instancia::nova(&*dir).unwrap();
        let mut t = inst
            .abrir_database("loja")
            .unwrap()
            .abrir_qualificada("itens")
            .unwrap();
        t.inserir(&[Value::Int(2)]).unwrap();
    }
    phxsql_store::ndx::esquecer_atestados_para_teste(&dir);
    let antes = vezes(Alarme::IndiceAtrasado);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(
        s.saude.na_fila() > 0,
        "premissa: o arranque tinha de reconstruir o indice e avisar"
    );
    assert!(vezes(Alarme::IndiceAtrasado) > antes, "a pedra nao subiu");
    drop(s);
}

// ------------------------------------------------------ OrigemInalcancavel

/// A origem inalcancavel alem do prazo vira UMA pedra por episodio, pelo
/// `apos_a_falha_vigiando` -- o ponto por onde os lacos comum e do cluster
/// passam. Antes do prazo, nada; a origem que volta abre episodio novo. RED:
/// tirar o `sinal` do `apos_a_falha_vigiando`.
#[test]
fn a_origem_inalcancavel_alem_do_prazo_vira_uma_pedra_por_episodio() {
    use crate::replica::{Falha, Ritmo};
    let dir = dir_temp("origem");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let mut ritmo = Ritmo::novo(Duration::from_millis(1));
    let antes = vezes(Alarme::OrigemInalcancavel);
    // Dentro do prazo: a falha que goteja nao alarma.
    assert!(s.apos_a_falha_vigiando("p769", &mut ritmo, Falha::Rede, &|| true));
    assert_eq!(
        vezes(Alarme::OrigemInalcancavel),
        antes,
        "alarmou antes do prazo"
    );
    // Alem do prazo: uma pedra, e so uma, por mais tentativas que venham.
    ritmo.inalcancavel_ha(Ritmo::PRAZO_DE_INALCANCAVEL + Duration::from_secs(1));
    assert!(s.apos_a_falha_vigiando("p769", &mut ritmo, Falha::Rede, &|| true));
    let depois = vezes(Alarme::OrigemInalcancavel);
    assert!(depois > antes, "a pedra nao subiu");
    assert!(s.apos_a_falha_vigiando("p769", &mut ritmo, Falha::Limite, &|| true));
    assert!(!ritmo.cruzou_o_prazo(), "o episodio alarmou duas vezes");
    // A origem respondeu: o episodio acaba, e o seguinte conta do zero.
    ritmo.sucesso();
    assert!(s.apos_a_falha_vigiando("p769", &mut ritmo, Falha::Rede, &|| true));
    assert!(!ritmo.cruzou_o_prazo(), "o episodio novo herdou o relogio");
}

// -------------------------------------------------------------- DiscoLento

/// A sonda que passa, mas acima de `lento_ms`, vira a pedra `disco_lento`
/// -- e a rapida nao. A duracao e PLANTADA (`atraso_de_teste_us`), como no
/// teste do painel da saude: o disco nao fica lento quando o teste pede. RED:
/// tirar o `sinal` do `sondar`.
#[test]
fn a_sonda_lenta_vira_pedra() {
    let dir = dir_temp("lento");
    let mut c = config_base(&dir);
    c.alertas.disco.lento_ms = 1_000;
    let s = Servidor::novo(c).unwrap();
    let antes = vezes(Alarme::DiscoLento);
    let _ = s.saude.sondar(crate::agora_ms());
    assert_eq!(
        s.saude.estado(),
        "ok",
        "premissa: a sonda de verdade e rapida"
    );
    s.saude
        .atraso_de_teste_us
        .store(2_000_000, std::sync::atomic::Ordering::Relaxed);
    let _ = s.saude.sondar(crate::agora_ms());
    assert_eq!(
        s.saude.estado(),
        "aviso",
        "premissa: a sonda plantada e lenta"
    );
    assert!(vezes(Alarme::DiscoLento) > antes, "a pedra nao subiu");
}

// ----------------------------------------------- ForaDoHabitualReincidente

/// **A regra:** a MESMA chave fora do habitual pela terceira vez em 5 min
/// vira `fora_do_habitual_reincidente` (vermelho, PRAZO), pelo `anotar` do
/// servidor e o produtor unico; as duas primeiras continuam
/// `fora_do_habitual`. Vinte pedidos normais entre um desvio e outro, para o
/// desvio anterior nao virar habitual. RED: tirar o `reincide` do
/// `Base::observar` -> a terceira continua amarela.
#[test]
fn o_terceiro_desvio_da_mesma_chave_em_cinco_minutos_reincide() {
    let dir = dir_temp("reincide");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let a = amarrada(&s, "127.0.0.1");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    let agora = crate::agora_ms();
    let pedido = |us: u64| Acesso {
        quando_ms: agora,
        op: "inserir".into(),
        ok: true,
        database: "loja".into(),
        tabela: "p769".into(),
        duracao_ms: us / 1_000,
        us,
        ..Acesso::default()
    };
    let mut mascaras = Vec::new();
    for _ in 0..3 {
        for _ in 0..20 {
            a.comecou_pedido("inserir", "root", "loja", "p769", agora);
            s.anotar(&pedido(1_000));
        }
        a.comecou_pedido("inserir", "root", "loja", "p769", agora);
        s.anotar(&pedido(10_000_000));
        mascaras.push(a.alarmes());
    }
    let fora = Alarme::ForaDoHabitual.bit();
    let reincide = Alarme::ForaDoHabitualReincidente.bit();
    assert_eq!(mascaras[0] & (fora | reincide), fora, "o primeiro");
    assert_eq!(mascaras[1] & (fora | reincide), fora, "o segundo");
    assert_eq!(
        mascaras[2] & (fora | reincide),
        reincide,
        "o terceiro em 5 min tinha de reincidir"
    );
    assert_eq!(
        ocorrencias_de(&s, Alarme::ForaDoHabitualReincidente).len(),
        1,
        "a ocorrencia da reincidencia"
    );
}

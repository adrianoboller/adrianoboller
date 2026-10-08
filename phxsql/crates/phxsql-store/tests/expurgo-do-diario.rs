//! O expurgo do diario (`.log`) -- pedido 706.
//!
//! # Por que isto e um teste de INTEGRACAO
//!
//! O corte do diario sem paginacao e um global do PROCESSO
//! (`diario::definir_corte_do_expurgo`), como o corte do `corte-do-diario.rs`:
//! ligado aqui dentro de um binario com outros testes, faria o `.log` deles
//! virar de volume no meio da corrida. Neste binario so moram os testes do
//! 706, e um de cada vez.
//!
//! As provas sao as do parecer do papel C (`docs/propostas/expurgo-do-diario-706.md`
//! §5): a posicao nao desliza (1), a base perdida recusa (3), o disco fica
//! limitado com o expurgo e cresce sem ele, o que ninguem confirmou so sai
//! pelo prazo, e a queda entre o `rename` e o ativo novo nao perde nem
//! inventa evento.

mod comum;
use std::sync::Mutex;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::diario;
use phxsql_store::log::{LogFile, MotivoDoExpurgo, Operacao, ParadaDoExpurgo};

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// Liga o corte no piso (64 KiB) e devolve a guarda: um teste por vez, e o
/// corte desligado de novo na saida, mesmo no panico.
struct Corte(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

impl Corte {
    fn ligar() -> Corte {
        let g = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        diario::definir_corte_do_expurgo(diario::CORTE_MINIMO);
        Corte(g)
    }
}

impl Drop for Corte {
    fn drop(&mut self) {
        diario::definir_corte_do_expurgo(0);
    }
}

fn dir(rotulo: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("expurgo-diario-{rotulo}"))
}

/// Uma imagem de 100 bytes que diz qual linha e: ~430 eventos por volume de
/// 64 KiB.
fn imagem(i: u64) -> Vec<u8> {
    let mut v = vec![0u8; 100];
    v[..8].copy_from_slice(&i.to_le_bytes());
    v
}

fn gravar(l: &mut LogFile, de: u64, ate: u64) {
    for i in de..ate {
        l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem(i))
            .unwrap();
    }
    l.sincronizar().unwrap();
}

/// Bytes de todos os `.log` da tabela `t` no diretorio.
fn bytes_do_diario(d: &std::path::Path) -> u64 {
    std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n == "t.log" || (n.starts_with("t#") && n.ends_with(".log"))
        })
        .map(|e| e.metadata().unwrap().len())
        .sum()
}

/// **Sem o expurgo ligado, nada muda**: o diario sem paginacao continua um
/// `t.log` so, que nunca vira de volume -- e o que guarda todo banco de hoje.
#[test]
fn sem_o_corte_o_diario_sem_paginacao_continua_um_arquivo_so() {
    let _g = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    diario::definir_corte_do_expurgo(0);
    let d = dir("desligado");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    assert_eq!(l.volumes(), vec![1]);
    assert!(d.join("t.log").exists());
    let plano = l.planejar_expurgo(Some(u64::MAX), Some(i64::MAX)).unwrap();
    assert!(plano.vazio(), "expurgou um diario de volume unico");
    assert_eq!(plano.parada, ParadaDoExpurgo::SemFechado);
}

/// Com o corte, o diario sem paginacao vira de volume no formato B: o ativo
/// em `t.log`, os fechados em `t#NNN.log`, e a leitura atravessa todos.
#[test]
fn com_o_corte_o_diario_sem_paginacao_vira_no_formato_b() {
    let _c = Corte::ligar();
    let d = dir("formato-b");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let volumes = l.volumes();
    assert!(volumes.len() >= 5, "so {} volume(s)", volumes.len());
    assert!(d.join("t.log").exists());
    assert!(d.join("t#001.log").exists());
    assert_eq!(l.total().unwrap(), 3_000);
    drop(l);

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.volumes(), volumes, "reabrir achou outros volumes");
    assert_eq!(l.total().unwrap(), 3_000);
    let todos = l.ler(0, 0).unwrap();
    for (i, e) in todos.iter().enumerate() {
        assert_eq!(e.rowid, i as u64, "ordem quebrada no evento {i}");
    }
    // E continua gravando no ativo certo.
    gravar(&mut l, 3_000, 3_010);
    assert_eq!(l.total().unwrap(), 3_010);
    assert_eq!(l.verificar().unwrap(), 3_010);
}

/// **Prova 1 do parecer: a posicao nao desliza.** Depois do expurgo, o evento
/// N continua sendo o mesmo evento N -- rowid, versao, carimbo e imagem --, e
/// pedir abaixo da base e recusa dita, nunca o evento seguinte no lugar.
///
/// Defeito reposto (a varredura contando do zero, e nao da base): o evento
/// pedido volta trocado e este teste falha.
#[test]
fn depois_do_expurgo_a_posicao_aponta_o_mesmo_evento() {
    let _c = Corte::ligar();
    let d = dir("posicao");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let antes: Vec<_> = l.ler_com_imagem(0, 0).unwrap();
    let total = l.total().unwrap();

    let plano = l.expurgar_diario_para_teste(Some(total), None);
    assert!(!plano.vazio(), "nada saiu com tudo confirmado");
    assert!(plano
        .volumes
        .iter()
        .all(|(_, m)| *m == MotivoDoExpurgo::Confirmado));
    let base = l.base().unwrap();
    assert_eq!(base, plano.base);
    assert!(base > 0);
    assert!(!d.join("t#001.log").exists(), "o volume 1 ficou");
    assert_eq!(l.total().unwrap(), total, "o total encolheu com o expurgo");

    for p in [base, base + 3, total - 1] {
        let (e, img) = l.ler_com_imagem(p, 1).unwrap().remove(0);
        let (e0, img0) = &antes[p as usize];
        assert_eq!(
            (e.rowid, e.versao, e.carimbo, e.tx),
            (e0.rowid, e0.versao, e0.carimbo, e0.tx),
            "a posicao {p} entregou outro evento"
        );
        assert_eq!(&img, img0);
    }
    let erro = l.ler_com_imagem(base - 1, 1).unwrap_err().to_string();
    assert!(erro.contains("expurgo"), "{erro}");

    // Reaberto, igual.
    drop(l);
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.total().unwrap(), total);
    assert_eq!(l.base().unwrap(), base);
    let e = l.ler(base + 3, 1).unwrap().remove(0);
    assert_eq!(e.rowid, antes[(base + 3) as usize].0.rowid);
    assert_eq!(l.verificar().unwrap(), total - base);
}

/// **O disco fica limitado com o expurgo, e cresce sem ele.** Dez rodadas de
/// gravacao com o consumidor confirmando tudo: o diario fica em poucos
/// volumes. Defeito reposto (o plano que nunca tira nada): o diario passa do
/// teto na terceira rodada.
#[test]
fn com_o_expurgo_o_diario_fica_limitado() {
    let _c = Corte::ligar();
    let d = dir("limitado");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    let corte = diario::corte_do_expurgo();
    let mut maior = 0u64;
    for rodada in 0..10u64 {
        gravar(&mut l, rodada * 2_000, (rodada + 1) * 2_000);
        let total = l.total().unwrap();
        l.expurgar_diario_para_teste(Some(total), None);
        maior = maior.max(bytes_do_diario(&d));
    }
    assert_eq!(l.total().unwrap(), 20_000);
    // Fica o ultimo fechado, o penultimo como margem, e o ativo.
    assert!(
        maior <= 4 * corte,
        "o diario chegou a {maior} bytes com o expurgo ligado (corte {corte})"
    );
}

/// **A margem de um volume**: o volume so sai quando o consumidor confirmou
/// ele E o seguinte inteiro. Quem pede `desde` sem dizer o que ja foi ao
/// disco dele pode voltar pedindo menos depois de uma queda.
#[test]
fn o_volume_so_sai_com_o_seguinte_confirmado() {
    let _c = Corte::ligar();
    let d = dir("margem");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let plano = l.planejar_expurgo(Some(u64::MAX), None).unwrap();
    let quantos_1 = quantos_no_disco(&l, 1);
    let quantos_2 = quantos_no_disco(&l, 2);

    // Confirmado ate o fim do 1: o 1 nao sai, porque o 2 nao esta coberto.
    let p = l.planejar_expurgo(Some(quantos_1), None).unwrap();
    assert!(p.vazio());
    assert_eq!(p.parada, ParadaDoExpurgo::Retido);
    // Confirmado ate o fim do 2: o 1 sai.
    let p = l
        .planejar_expurgo(Some(quantos_1 + quantos_2), None)
        .unwrap();
    assert_eq!(p.volumes, vec![(1, MotivoDoExpurgo::Confirmado)]);
    assert_eq!(p.base, quantos_1);
    // E nunca o ultimo fechado nem o ativo, nem com tudo confirmado.
    let fechados = l.volumes().len() - 1;
    assert_eq!(plano.volumes.len(), fechados - 1);
}

/// Quantos eventos o volume `v` tem, pelo cabecalho NO DISCO (bytes 16..24)
/// -- depois do `sincronizar`, e o mesmo da memoria.
fn quantos_no_disco(l: &LogFile, v: u32) -> u64 {
    let bytes = std::fs::read(l.caminho(v)).unwrap();
    u64::from_le_bytes(bytes[16..24].try_into().unwrap())
}

/// **O que ninguem confirmou so sai pelo prazo** -- a decisao do dono: o caixa
/// segura no maximo 30 dias. Eventos de agora nao saem; eventos de 31 dias
/// atras saem, com o motivo dito.
#[test]
fn o_que_ninguem_confirmou_so_sai_pelo_prazo() {
    let _c = Corte::ligar();
    let d = dir("prazo");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    let dia = 86_400_000i64;
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let velho = agora - 31 * dia;
    // Os primeiros 1.500 eventos sao de 31 dias atras; o resto, de agora.
    for i in 0..1_500u64 {
        l.registrar_detalhado(Operacao::Inclusao, i, 1, &imagem(i), Some(velho), 0)
            .unwrap();
    }
    gravar(&mut l, 1_500, 3_000);
    let limite = agora - 30 * dia;

    let p = l.planejar_expurgo(None, None).unwrap();
    assert!(p.vazio(), "sem confirmacao e sem prazo, saiu volume");
    let p = l.planejar_expurgo(None, Some(limite)).unwrap();
    assert!(!p.vazio(), "o prazo vencido nao tirou nada");
    assert!(p.volumes.iter().all(|(_, m)| *m == MotivoDoExpurgo::Prazo));
    assert_eq!(p.parada, ParadaDoExpurgo::Retido);
    // Nenhum volume com evento de agora saiu.
    assert!(
        p.base <= 1_500,
        "saiu evento dentro do prazo: base {}",
        p.base
    );
    let total = l.total().unwrap();
    l.expurgar_diario_para_teste(None, Some(limite));
    assert_eq!(l.total().unwrap(), total);
    assert_eq!(l.ler(p.base, 1).unwrap()[0].rowid, p.base);
}

/// **Prova 3 do parecer: a base perdida recusa.** O primeiro volume que
/// existe com a base zerada (um binario de antes, ou um cabecalho
/// adulterado com o CRC refeito) nao abre contando do zero: recusa.
#[test]
fn base_perdida_no_primeiro_volume_recusa() {
    let _c = Corte::ligar();
    let d = dir("base-perdida");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let total = l.total().unwrap();
    let plano = l.expurgar_diario_para_teste(Some(total), None);
    let primeiro = l.caminho(plano.sobrevivente);
    drop(l);

    let mut bruto = std::fs::read(&primeiro).unwrap();
    bruto[104..112].fill(0);
    let crc = phxsql_core::crc::crc32(&bruto[..120]);
    bruto[120..124].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&primeiro, &bruto).unwrap();

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    let erro = l.total().unwrap_err().to_string();
    assert!(erro.contains("706"), "{erro}");
    assert!(l.ler(0, 1).is_err(), "leu contando do zero");
}

/// **A queda entre o `rename` do ativo e o nascimento do seguinte**: o
/// `t.log` sumiu e o fechado esta la. A abertura faz nascer o ativo com a
/// base herdada, e o total e as posicoes continuam os mesmos.
#[test]
fn a_queda_entre_o_rename_e_o_ativo_novo_nao_perde_evento() {
    let _c = Corte::ligar();
    let d = dir("rename");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let total = l.total().unwrap();
    let ativo = *l.volumes().last().unwrap();
    drop(l);
    // A queda: o ativo foi renomeado para fechado, e o seguinte nao nasceu.
    std::fs::rename(d.join("t.log"), d.join(format!("t#{ativo:03}.log"))).unwrap();

    // Quem so le nao escreve: pede a abertura com escrita.
    assert!(LogFile::abrir_sem_escrever(&d, "t", Paginacao::DESLIGADA)
        .unwrap()
        .is_none());
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert!(d.join("t.log").exists(), "o ativo nao renasceu");
    assert_eq!(*l.volumes().last().unwrap(), ativo + 1);
    assert_eq!(l.total().unwrap(), total);
    gravar(&mut l, 3_000, 3_001);
    assert_eq!(l.ler(3_000, 1).unwrap()[0].rowid, 3_000);
    assert_eq!(l.verificar().unwrap(), 3_001);
}

/// O diario PAGINADO ganha a base pelo mesmo caminho: os volumes `#NNN`
/// fechados saem do comeco e a posicao continua.
#[test]
fn o_diario_paginado_tambem_expurga_sem_deslizar() {
    let _g = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("paginado");
    let pag = Paginacao::nova(10, 999)
        .unwrap()
        .com_bytes_por_arquivo(64 * 1024)
        .unwrap();
    let mut l = LogFile::criar(&d, "t", pag).unwrap();
    gravar(&mut l, 0, 3_000);
    let total = l.total().unwrap();
    let plano = l.expurgar_diario_para_teste(Some(total), None);
    assert!(!plano.vazio());
    assert_eq!(l.total().unwrap(), total);
    let e = l.ler(plano.base + 1, 1).unwrap().remove(0);
    assert_eq!(e.rowid, plano.base + 1);
    drop(l);
    let mut l = LogFile::abrir(&d, "t", pag).unwrap();
    assert_eq!(l.total().unwrap(), total);
}

/// Os tres passos de uma vez, sobre o `LogFile` direto.
trait ExpurgarParaTeste {
    fn expurgar_diario_para_teste(
        &mut self,
        confirmado: Option<u64>,
        limite: Option<i64>,
    ) -> phxsql_store::log::PlanoDoExpurgo;
}

impl ExpurgarParaTeste for LogFile {
    fn expurgar_diario_para_teste(
        &mut self,
        confirmado: Option<u64>,
        limite: Option<i64>,
    ) -> phxsql_store::log::PlanoDoExpurgo {
        let plano = self.planejar_expurgo(confirmado, limite).unwrap();
        self.gravar_bases_do_expurgo(&plano).unwrap();
        plano.levar_bases_ao_disco().unwrap();
        self.concluir_expurgo(&plano).unwrap();
        plano
    }
}

/// O filho da prova 2: grava o diario, planeja o expurgo e morre por
/// `SIGKILL` no ponto que `PHX_EXPURGO_FASE` disser. Sem a variavel, nao faz
/// nada -- e um teste so de nome, para o pai poder chama-lo pelo `--exact`.
#[test]
fn filho_do_expurgo_que_morre() {
    let (Ok(fase), Ok(dir)) = (
        std::env::var("PHX_EXPURGO_FASE"),
        std::env::var("PHX_EXPURGO_DIR"),
    ) else {
        return;
    };
    let _c = Corte::ligar();
    let d = std::path::PathBuf::from(dir);
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    gravar(&mut l, 0, 3_000);
    let total = l.total().unwrap();
    let plano = l.planejar_expurgo(Some(total), None).unwrap();
    assert!(plano.volumes.len() >= 2, "o plano precisa de dois volumes");
    let morrer = || {
        std::process::Command::new("kill")
            .args(["-9", &std::process::id().to_string()])
            .status()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(60));
        unreachable!("o SIGKILL nao chegou");
    };
    l.gravar_bases_do_expurgo(&plano).unwrap();
    if fase == "bases" {
        morrer();
    }
    plano.levar_bases_ao_disco().unwrap();
    if fase == "levadas" {
        morrer();
    }
    if fase == "meio" {
        // O estado em disco da queda ENTRE os dois `unlink`: o mais velho
        // saiu, o seguinte nao. O `concluir` apaga nessa ordem.
        std::fs::remove_file(l.caminho(plano.volumes[0].0)).unwrap();
        morrer();
    }
    l.concluir_expurgo(&plano).unwrap();
    morrer();
}

/// **Prova 2 do parecer: `kill -9` entre as fases do expurgo.** Em cada
/// ponto -- base gravada sem `fsync`, base no disco, um volume apagado e o
/// seguinte nao, tudo feito --, o processo morre de verdade e a abertura
/// seguinte ve o MESMO total e o mesmo evento em cada posicao que ainda
/// existe; e um expurgo novo termina o que ficou.
///
/// O `SIGKILL` nao esvazia o cache do nucleo: isto prova a ORDEM das fases,
/// e nao o `fsync` (queda de energia nao se provoca em teste).
#[test]
fn kill_9_entre_as_fases_nao_desliza_a_posicao() {
    use std::os::unix::process::ExitStatusExt;
    let _g = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    for fase in ["bases", "levadas", "meio", "fim"] {
        let d = dir(&format!("kill9-{fase}"));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["filho_do_expurgo_que_morre", "--exact", "--test-threads=1"])
            .env("PHX_EXPURGO_FASE", fase)
            .env("PHX_EXPURGO_DIR", &d.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(
            status.signal(),
            Some(9),
            "{fase}: o filho nao morreu por SIGKILL"
        );

        diario::definir_corte_do_expurgo(diario::CORTE_MINIMO);
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA)
            .unwrap_or_else(|e| panic!("{fase}: a abertura recusou: {e}"));
        assert_eq!(l.total().unwrap(), 3_000, "{fase}: o total mudou");
        let base = l.base().unwrap();
        for p in [base, base + 7, 2_999] {
            assert_eq!(
                l.ler(p, 1).unwrap()[0].rowid,
                p,
                "{fase}: a posicao {p} entregou outro evento"
            );
        }
        assert_eq!(l.verificar().unwrap(), 3_000 - base, "{fase}");
        // E um expurgo novo termina o que ficou, sem deslizar.
        let plano = l.expurgar_diario_para_teste(Some(3_000), None);
        assert_eq!(l.total().unwrap(), 3_000, "{fase}: depois do expurgo novo");
        assert_eq!(l.ler(plano.base, 1).unwrap()[0].rowid, plano.base, "{fase}");
        diario::definir_corte_do_expurgo(0);
    }
}

/// **O caixa nao para no volume 999.** O diario SEM paginacao vira de volume
/// no formato B da trilha, que nao tem teto de numero: o expurgo tira os
/// volumes velhos e o numero continua subindo, `t#999.log`, `t#1000.log`...
/// Aqui o diario passa de 1.000 volumes, expurgando no caminho.
///
/// Defeito reposto (o teto de 999 da paginacao aplicado ao formato B): a
/// gravacao do volume 1.000 recusa e o teste cai.
#[test]
fn o_diario_sem_paginacao_passa_do_volume_999_sem_parar() {
    let _c = Corte::ligar();
    let d = dir("mil-volumes");
    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    // ~62 eventos de 1 KiB por volume de 64 KiB: 64.000 eventos sao ~1.030
    // volumes.
    let imagem = vec![7u8; 1024];
    let mut i = 0u64;
    while *l.volumes().last().unwrap() <= 1_000 {
        for _ in 0..620 {
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem)
                .unwrap_or_else(|e| panic!("o evento {i} recusou: {e}"));
            i += 1;
        }
        l.sincronizar().unwrap();
        let total = l.total().unwrap();
        l.expurgar_diario_para_teste(Some(total), None);
    }
    assert!(d.join("t#1000.log").exists() || *l.volumes().first().unwrap() > 1_000);
    let total = l.total().unwrap();
    assert_eq!(total, i);
    let base = l.base().unwrap();
    assert_eq!(l.ler(base, 1).unwrap()[0].rowid, base);
    drop(l);
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.total().unwrap(), i, "reaberto depois do volume 999");
    assert!(*l.volumes().last().unwrap() > 999);
}

//! Onde o volume do `.log`, da `.trash` e do `.reason` corta.
//!
//! # Por que isto e um teste de INTEGRACAO
//!
//! O corte e um global do PROCESSO (ver `phxsql_store::diario`), pela mesma
//! razao do teto do cache de paginas: e uma decisao do arranque, nao um
//! parametro que quatro camadas de API teriam de carregar. E, como todo global,
//! ele nao pode ser mexido no mesmo binario em que outros testes criam `.log`
//! -- um teste que corta em 1 MiB faria o diario de outro virar de volume no
//! meio da corrida.

mod comum;
use std::sync::Mutex;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::diario;
use phxsql_store::lixeira::LixeiraFile;
use phxsql_store::log::{LogFile, Operacao};
use phxsql_store::motivo::{MotivoFile, Tipo};

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("corte-{rotulo}"))
}

/// Uma paginacao normal de tabela: volume de 1 GiB nos arquivos externos.
fn paginacao_normal() -> Paginacao {
    Paginacao::nova(1_000_000, 99).unwrap()
}

/// **O teste que mais importa: sem configuracao, nada muda.**
///
/// Um `.log` de 20.000 eventos com o corte padrao de 1 GiB cabe folgado num
/// volume so -- e e por isso que compactar volume fechado poupava zero.
#[test]
fn sem_configuracao_o_diario_nao_vira_de_volume() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    diario::definir_bytes_por_volume(0);
    let d = dir("padrao");

    let mut l = LogFile::criar(&d, "t", paginacao_normal()).unwrap();
    for i in 1..=20_000u64 {
        l.registrar(Operacao::Inclusao, i, 1).unwrap();
    }
    l.sincronizar().unwrap();
    assert_eq!(l.volumes().len(), 1, "o diario virou de volume sozinho");
    assert_eq!(l.total().unwrap(), 20_000);
    std::fs::remove_dir_all(&d).unwrap();
}

/// Com o corte configurado, o diario fecha volume -- e continua sendo lido
/// inteiro, na ordem, atravessando os volumes.
#[test]
fn com_o_corte_o_diario_fecha_volume_e_continua_inteiro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("corte");
    // 64 KiB: o piso. 20.000 eventos de 44 bytes dao 880 KiB, entao sao ~14
    // volumes, 13 deles FECHADOS -- que e a unidade de tudo que se faz com
    // diario velho.
    diario::definir_bytes_por_volume(diario::CORTE_MINIMO);

    let mut l = LogFile::criar(&d, "t", paginacao_normal()).unwrap();
    for i in 1..=20_000u64 {
        l.registrar(Operacao::Inclusao, i, 1).unwrap();
    }
    l.sincronizar().unwrap();
    let volumes = l.volumes().len();
    assert!(volumes > 10, "so {volumes} volume(s) com corte de 64 KiB");
    assert_eq!(l.total().unwrap(), 20_000);
    assert_eq!(l.verificar().unwrap(), 20_000);

    // A ordem cronologica atravessa os volumes sem pular nem repetir.
    let eventos = l.ler(0, 0).unwrap();
    assert_eq!(eventos.len(), 20_000);
    for (i, e) in eventos.iter().enumerate() {
        assert_eq!(e.rowid, i as u64 + 1, "ordem quebrada no evento {i}");
    }
    // E reabrir encontra os mesmos volumes.
    drop(l);
    let mut l = LogFile::abrir(&d, "t", paginacao_normal()).unwrap();
    assert_eq!(l.volumes().len(), volumes);
    assert_eq!(l.total().unwrap(), 20_000);

    diario::definir_bytes_por_volume(0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// O corte vale para os TRES, e para nenhum outro arquivo.
#[test]
fn o_corte_pega_os_tres_diarios() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("tres");
    diario::definir_bytes_por_volume(diario::CORTE_MINIMO);

    let mut x = LixeiraFile::criar(&d, "t", paginacao_normal()).unwrap();
    for i in 1..=1_200u64 {
        x.guardar(i, &[(i % 251) as u8; 100], vec![]).unwrap();
    }
    assert!(
        x.volumes_existentes().len() > 1,
        "a .trash nao virou de volume"
    );
    assert_eq!(x.total().unwrap(), 1_200);
    assert_eq!(x.verificar().unwrap(), 1_200);

    let mut m = MotivoFile::criar(&d, "t", paginacao_normal()).unwrap();
    for i in 1..=1_500u64 {
        m.registrar(Tipo::Fisica, i, "duplicidade confirmada", "id=x")
            .unwrap();
    }
    m.sincronizar().unwrap();
    assert!(
        m.volumes_existentes().len() > 1,
        "o .reason nao virou de volume"
    );
    assert_eq!(m.total().unwrap(), 1_500);
    assert_eq!(m.verificar().unwrap(), 1_500);
    assert_eq!(m.ler(0, 0).unwrap().len(), 1_500);

    diario::definir_bytes_por_volume(0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// O corte do diario nao mexe no `.bin` nem no `.memo`.
///
/// # O defeito que este teste existe para pegar
///
/// A tentacao era mexer no `Paginacao::para_externos`, que os cinco arquivos
/// externos usam. Ali, cortar o diario em 1 MiB cortaria tambem os anexos --
/// e uma foto de 3 MiB passaria a morar sozinha num volume, com o `.bin`
/// virando um arquivo por imagem.
#[test]
fn o_corte_do_diario_nao_toca_nos_anexos() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("anexos");
    diario::definir_bytes_por_volume(diario::CORTE_MINIMO);

    let pag = paginacao_normal();
    let mut b =
        phxsql_store::BlobFile::criar(&d, "t", "bin", phxsql_store::MAGIC_BIN, pag.para_externos())
            .unwrap();
    // 4 MiB de anexos: passariam de 64 volumes se o corte do diario pegasse
    // aqui, e ficam num so porque nao pega.
    for _ in 0..64 {
        b.gravar(&[7u8; 64 * 1024]).unwrap();
    }
    b.sincronizar().unwrap();
    assert_eq!(
        b.volumes().len(),
        1,
        "o corte do diario cortou o .bin junto"
    );

    diario::definir_bytes_por_volume(0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// Corte ridiculo sobe ao piso, e zero continua sendo zero.
#[test]
fn corte_ridiculo_sobe_ao_piso() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    diario::definir_bytes_por_volume(1);
    assert_eq!(diario::bytes_por_volume(), diario::CORTE_MINIMO);
    diario::definir_bytes_por_volume(0);
    assert_eq!(diario::bytes_por_volume(), 0, "zero tem de ser zero");
}

/// O corte troca o `bytes_por_arquivo` e nada mais da paginacao.
#[test]
fn o_corte_nao_mexe_no_resto_da_paginacao() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let p = Paginacao::nova(1_000, 99).unwrap().com_digitos(4).unwrap();
    diario::definir_bytes_por_volume(8 * 1024 * 1024);
    let ajustada = diario::paginacao(p);
    assert_eq!(ajustada.bytes_por_arquivo, 8 * 1024 * 1024);
    assert_eq!(ajustada.registros_por_arquivo, p.registros_por_arquivo);
    assert_eq!(ajustada.digitos, p.digitos);
    assert_eq!(ajustada.max_arquivos, p.max_arquivos);
    assert_eq!(ajustada.modo, p.modo);
    diario::definir_bytes_por_volume(0);
}

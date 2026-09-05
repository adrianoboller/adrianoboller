//! A cifra do `.log`, da `.trash` e do `.reason`, provada em disco.
//!
//! # Por que isto e um teste de INTEGRACAO, e nao um `mod testes`
//!
//! A chave e do PROCESSO (ver `phxsql_store::cofre`), e `cargo test` roda os
//! testes de um mesmo binario em paralelo. Se estes testes morassem dentro da
//! biblioteca, ligar a cifra aqui faria o `.log` de outro teste nascer cifrado
//! no meio da corrida -- e o teste que quebraria seria o de outra pessoa. Um
//! arquivo de teste de integracao roda em outro processo, e ali o global e so
//! dele.
//!
//! Dentro DESTE arquivo os testes ainda dividem o mesmo processo, entao todos
//! passam pela trava `UM_DE_CADA_VEZ`.

mod comum;
use std::sync::Mutex;

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::lixeira::LixeiraFile;
use phxsql_store::log::{LogFile, Operacao};
use phxsql_store::motivo::{MotivoFile, Tipo};
use phxsql_store::table::Table;

/// A trava que serializa os testes: o cofre e global ao processo.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// Iteracoes baixas: o que se prova aqui e a amarracao, nao o custo do PBKDF2.
/// O piso do cofre continua sendo conferido -- este e o valor do piso.
const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;

const SENHA: &str = "a chave do cofre de teste";

fn dir(rotulo: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("cifra-diarios-{rotulo}"))
}

/// Os bytes crus de um volume, para olhar o que REALMENTE foi para o disco.
fn bytes_do_arquivo(d: &std::path::Path, nome: &str, ext: &str) -> Vec<u8> {
    std::fs::read(d.join(format!("{nome}.{ext}"))).unwrap()
}

fn contem(agulha: &[u8], palheiro: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

// ---------------------------------------------------------------------------
// O teste que mais importa: o comportamento VELHO
// ---------------------------------------------------------------------------

/// Diario escrito ANTES da cifra continua abrindo depois dela ligada.
///
/// E a regra da casa: guarda nova entra pedida, nao imposta. Um servidor que
/// liga a cifra na terca nao pode deixar de ler o que gravou na segunda.
#[test]
fn arquivo_escrito_antes_da_cifra_continua_abrindo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("velho");

    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=50u64 {
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, b"linha em claro")
                .unwrap();
        }
        l.sincronizar().unwrap();
        let mut x = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        x.guardar(7, b"payload em claro", vec![]).unwrap();
        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        m.registrar(Tipo::Fisica, 7, "duplicidade", "id=7").unwrap();
        m.sincronizar().unwrap();
    }

    // Agora a cifra liga. O que ja esta no disco esta na versao 2.
    cofre::definir(SENHA, RAPIDO).unwrap();

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.total().unwrap(), 50, "o diario velho perdeu evento");
    assert_eq!(l.verificar().unwrap(), 50);
    assert_eq!(l.ler_com_imagem(0, 1).unwrap()[0].1, b"linha em claro");

    let mut x = LixeiraFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(x.ler(0, 0, true).unwrap()[0].payload, b"payload em claro");

    let mut m = MotivoFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(m.ler(0, 0).unwrap()[0].motivo, "duplicidade");

    // E continua sendo gravavel: um evento novo entra no volume velho, em
    // claro, sem virar de versao no meio do arquivo.
    l.registrar(Operacao::Exclusao, 7, 1).unwrap();
    l.sincronizar().unwrap();
    assert_eq!(l.total().unwrap(), 51);
    assert_eq!(l.verificar().unwrap(), 51);

    cofre::desligar();
    // E sem a chave tambem: o arquivo em claro nunca precisou dela.
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.total().unwrap(), 51);
    std::fs::remove_dir_all(&d).unwrap();
}

/// Com o cofre vazio, o volume novo continua nascendo na versao 2.
#[test]
fn sem_configuracao_nada_muda_no_disco() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("padrao");

    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar_com_imagem(Operacao::Inclusao, 1, 1, b"Blumenau")
        .unwrap();
    l.sincronizar().unwrap();

    let bruto = bytes_do_arquivo(&d, "t", "log");
    assert_eq!(
        u16::from_le_bytes([bruto[8], bruto[9]]),
        2,
        "sem cifra o .log nasceu numa versao nova"
    );
    assert_eq!(
        u16::from_le_bytes([bruto[10], bruto[11]]),
        cofre::CAB_V2 as u16,
        "o cabecalho velho mudou de tamanho"
    );
    // O evento sem cifra tem 44 bytes de cabecalho e a imagem crua atras.
    assert_eq!(bruto.len(), cofre::CAB_V2 + 44 + 8);
    assert!(
        contem(b"Blumenau", &bruto),
        "a imagem deveria estar em claro"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

// ---------------------------------------------------------------------------
// A cifra ligada
// ---------------------------------------------------------------------------

/// Com a cifra ligada o dado do cliente nao aparece no arquivo, e volta inteiro
/// pela leitura.
#[test]
fn o_dado_some_do_disco_e_volta_pela_leitura() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("cifrado");

    let imagem = b"Rua das Flores, 123 -- Blumenau -- 89010-000";
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=30u64 {
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, imagem)
                .unwrap();
        }
        l.sincronizar().unwrap();

        let mut x = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        x.guardar(7, imagem, vec![(0, b"a foto do contrato".to_vec())])
            .unwrap();

        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        m.registrar(Tipo::Fisica, 7, "pedido de remocao do titular", "cpf=1")
            .unwrap();
        m.sincronizar().unwrap();
    }

    for ext in ["log", "trash", "reason"] {
        let bruto = bytes_do_arquivo(&d, "t", ext);
        assert_eq!(
            u16::from_le_bytes([bruto[8], bruto[9]]),
            3,
            "o .{ext} nao subiu para a versao 3"
        );
        assert_eq!(u16::from_le_bytes([bruto[10], bruto[11]]), 128);
        assert!(
            !contem(b"Blumenau", &bruto) && !contem(b"titular", &bruto),
            "o texto claro sobreviveu no .{ext}"
        );
    }
    // O cabecalho do evento continua legivel: e ele que diz onde o proximo
    // comeca, e e por isso que a varredura anda sem a chave.
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.verificar().unwrap(), 30);

    let com = l.ler_com_imagem(0, 0).unwrap();
    assert_eq!(com.len(), 30);
    for (e, img) in &com {
        assert_eq!(img.as_slice(), imagem, "imagem do rowid {}", e.rowid);
        assert_eq!(
            e.tam_imagem as usize,
            imagem.len() + cofre::ACRESCIMO,
            "o tamanho no cabecalho tem de ser o do ARQUIVO"
        );
    }

    let mut x = LixeiraFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    let descartada = &x.ler(0, 0, true).unwrap()[0];
    assert_eq!(descartada.payload, imagem);
    assert_eq!(descartada.externos[0].1, b"a foto do contrato");
    assert_eq!(x.verificar().unwrap(), 1);

    let mut m = MotivoFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    let motivo = &m.ler(0, 0).unwrap()[0];
    assert_eq!(motivo.motivo, "pedido de remocao do titular");
    assert_eq!(motivo.identidade, "cpf=1");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Sem chave, e com a chave errada: erro claro, e nao lixo nem panico.
#[test]
fn arquivo_cifrado_sem_a_chave_certa_da_erro_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("chave-errada");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar_com_imagem(Operacao::Inclusao, 1, 1, b"segredo")
            .unwrap();
        l.sincronizar().unwrap();
    }

    cofre::desligar();
    let Err(e) = LogFile::abrir(&d, "t", Paginacao::DESLIGADA) else {
        panic!("abriu o arquivo cifrado sem a chave certa")
    };
    assert!(
        e.to_string().contains("config.json"),
        "sem chave o erro tem de dizer o que fazer: {e}"
    );

    cofre::definir("uma senha que nao e a certa", RAPIDO).unwrap();
    let Err(e) = LogFile::abrir(&d, "t", Paginacao::DESLIGADA) else {
        panic!("abriu o arquivo cifrado sem a chave certa")
    };
    assert!(
        e.to_string().contains("senha"),
        "a chave errada tem de sair como senha errada: {e}"
    );

    // E com a certa, tudo volta.
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.ler_com_imagem(0, 0).unwrap()[0].1, b"segredo");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Trocar um byte do corpo cifrado nao vira dado: vira erro.
#[test]
fn corpo_adulterado_nao_decifra() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("adulterado");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar_com_imagem(Operacao::Inclusao, 1, 1, b"o valor certo")
            .unwrap();
        l.sincronizar().unwrap();
    }
    // Vira um bit DENTRO do corpo cifrado, e conserta o CRC para que o teste
    // exercite a ETIQUETA e nao o CRC -- que ja tem teste proprio.
    let caminho = d.join("t.log");
    let mut bruto = std::fs::read(&caminho).unwrap();
    let corpo = cofre::CAB_V3 + 44;
    bruto[corpo] ^= 0x01;
    let novo_crc = {
        let cab = &bruto[cofre::CAB_V3..cofre::CAB_V3 + 44];
        let mut crc = phxsql_core::crc::crc32(&cab[..36]);
        crc ^= phxsql_core::crc::crc32(&bruto[corpo..]);
        crc
    };
    bruto[cofre::CAB_V3 + 36..cofre::CAB_V3 + 40].copy_from_slice(&novo_crc.to_le_bytes());
    std::fs::write(&caminho, &bruto).unwrap();

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    // O CRC passa -- foi consertado de proposito -- e a etiqueta nao.
    assert_eq!(l.verificar().unwrap(), 1, "o CRC devia continuar fechando");
    let e = l.ler_com_imagem(0, 0).unwrap_err();
    assert!(e.to_string().contains("etiqueta"), "{e}");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// O nonce nunca se repete DENTRO do arquivo, e a prova sai do proprio disco.
///
/// O nonce e o tempero de quatro bytes do evento mais o offset dele no volume.
/// Este teste caminha pelo `.log` do jeito que a leitura caminha e junta os
/// pares num conjunto -- repetir um seria a unica falha que quebra a cifra sem
/// quebrar a matematica.
#[test]
fn o_nonce_nunca_se_repete_no_arquivo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    // Volumes pequenos de proposito: assim o diario VIRA de volume e o teste
    // cobre tambem o caso em que o offset recomeca do zero no volume seguinte.
    let pag = Paginacao::nova(1_000, 99)
        .unwrap()
        .com_bytes_por_arquivo(4_096)
        .unwrap();
    let d = dir("nonce");
    {
        let mut l = LogFile::criar(&d, "t", pag).unwrap();
        for i in 1..=400u64 {
            let imagem = vec![(i % 251) as u8; (i % 97) as usize];
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem)
                .unwrap();
        }
        l.sincronizar().unwrap();
    }

    let mut l = LogFile::abrir(&d, "t", pag).unwrap();
    let volumes = l.volumes();
    assert!(volumes.len() > 1, "o diario devia ter virado de volume");
    assert_eq!(l.verificar().unwrap(), 400);

    let mut vistos = std::collections::HashSet::new();
    for v in &volumes {
        let bruto = std::fs::read(l.caminho(*v)).unwrap();
        // Cada volume tem o proprio sal, entao o par (chave, nonce) so poderia
        // se repetir DENTRO de um volume. Ainda assim o conjunto e um so: se
        // dois volumes dividissem sal, isto acusaria.
        let sal = bruto[48..64].to_vec();
        let mut offset = cofre::CAB_V3;
        while offset + 44 <= bruto.len() {
            let tam = u32::from_le_bytes(bruto[offset + 32..offset + 36].try_into().unwrap());
            let tempero = bruto[offset + 40..offset + 44].to_vec();
            if tam > 0 {
                assert!(
                    vistos.insert((sal.clone(), tempero, offset)),
                    "nonce repetido no volume {v}, offset {offset}"
                );
            }
            offset += 44 + tam as usize;
        }
    }
    // Os eventos de imagem VAZIA nao tem corpo para cifrar, entao nao gastam
    // nonce: sao os que caem em `i % 97 == 0`.
    let sem_corpo = (1..=400u64).filter(|i| i % 97 == 0).count();
    assert_eq!(vistos.len(), 400 - sem_corpo, "nem todo evento foi olhado");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// A replicacao le o `.log` pela sessao autenticada, e recebe DECIFRADO.
///
/// A cifra e do arquivo em repouso: quem tem a chave -- o proprio servidor --
/// le a imagem como sempre leu, e `posicao` e `replicar` continuam contando
/// eventos, porque a contagem sai do cabecalho, que e claro.
#[test]
fn a_replicacao_continua_lendo_um_log_cifrado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("replica");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=200u64 {
            let imagem = format!("linha {i}").into_bytes();
            l.registrar_com_imagem(Operacao::Alteracao, i, 2, &imagem)
                .unwrap();
        }
        l.sincronizar().unwrap();
    }

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    // `posicao`: o total sai dos cabecalhos, sem tocar em corpo nenhum.
    assert_eq!(l.total().unwrap(), 200);

    // `replicar`: lotes seguidos, com a marca do lote anterior -- que e o
    // caminho rapido da replicacao, e o que faria a conta do nonce errar se
    // ela dependesse de um contador em vez do offset.
    let mut lidos = 0u64;
    let mut marca = None;
    while lidos < 200 {
        l.definir_marca(marca);
        let lote = l.ler_com_imagem(lidos, 50).unwrap();
        assert_eq!(lote.len(), 50);
        for (i, (e, img)) in lote.iter().enumerate() {
            let esperado = format!("linha {}", lidos + i as u64 + 1);
            assert_eq!(img.as_slice(), esperado.as_bytes(), "rowid {}", e.rowid);
        }
        marca = l.marca();
        lidos += 50;
    }
    assert!(marca.is_some(), "a marca do diario sumiu");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// A cura de uma queda continua funcionando num volume cifrado -- e ela anda
/// pelo CRC, sem precisar da chave para achar onde o arquivo acaba.
#[test]
fn a_cura_funciona_no_volume_cifrado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("cura");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=120u64 {
            let imagem = vec![(i % 251) as u8; (i % 31) as usize];
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem)
                .unwrap();
        }
        // De proposito SEM `sincronizar`: e o que uma queda do processo faz, e
        // o cabecalho fica atrasado em relacao aos eventos ja gravados.
    }
    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(l.total().unwrap(), 120, "a cura perdeu evento cifrado");
    assert_eq!(l.ler_com_imagem(0, 0).unwrap().len(), 120);
    // E o evento novo entra DEPOIS, sem comer os que ja estavam.
    l.registrar(Operacao::Exclusao, 1, 2).unwrap();
    l.sincronizar().unwrap();
    assert_eq!(l.verificar().unwrap(), 121);

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// A senha nunca aparece no arquivo -- nem ela, nem a chave derivada.
#[test]
fn a_senha_nao_vai_para_o_disco() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("segredo");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        l.sincronizar().unwrap();
    }
    let bruto = bytes_do_arquivo(&d, "t", "log");
    assert!(
        !contem(SENHA.as_bytes(), &bruto),
        "a senha foi parar no arquivo"
    );
    // O sal esta la, em claro, e e assim mesmo: sal nao e segredo, e o papel
    // dele e impedir que a mesma senha derive a mesma chave em dois arquivos.
    let sal = &bruto[48..64];
    assert_ne!(sal, [0u8; 16], "o sal nao foi gravado");

    // Dois arquivos com a MESMA senha tem sais diferentes, entao chaves
    // diferentes -- e por isso o offset pode ser o numero de ordem do nonce.
    let d2 = dir("segredo-2");
    {
        let mut l = LogFile::criar(&d2, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        l.sincronizar().unwrap();
    }
    let outro = bytes_do_arquivo(&d2, "t", "log");
    assert_ne!(sal, &outro[48..64], "dois arquivos sairam com o mesmo sal");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
    std::fs::remove_dir_all(&d2).unwrap();
}

// ---------------------------------------------------------------------------
// O cofre em si, fora dos tres arquivos
// ---------------------------------------------------------------------------

/// O cofre nasce desligado, e ligar e uma decisao.
#[test]
fn o_cofre_nasce_desligado_e_liga_por_pedido() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    assert!(!cofre::ligado(), "o cofre nao pode nascer ligado");
    let cab = cofre::Cabecalho::novo(1).unwrap();
    assert!(!cab.cifrado(), "volume novo saiu cifrado com o cofre vazio");
    assert_eq!(cab.cab_len, cofre::CAB_V2);

    cofre::definir(SENHA, RAPIDO).unwrap();
    let cab = cofre::Cabecalho::novo(1).unwrap();
    assert!(cab.cifrado());
    assert_eq!(cab.cab_len, cofre::CAB_V3);
    cofre::desligar();
}

/// Senha vazia e iteracoes de enfeite sao recusadas na hora de ligar.
#[test]
fn senha_vazia_e_pbkdf2_de_enfeite_sao_recusados() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    assert!(cofre::definir("", RAPIDO).is_err(), "senha vazia passou");
    assert!(
        cofre::definir("x", 10).is_err(),
        "iteracoes de enfeite passaram"
    );
    assert!(
        !cofre::ligado(),
        "uma recusa nao pode deixar o cofre ligado"
    );
}

/// O cabecalho em claro amarra o corpo: mudar o dado associado derruba a
/// etiqueta, e e isso que impede mover o corpo de um registro para outro.
#[test]
fn o_cabecalho_em_claro_amarra_o_corpo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let cab = cofre::Cabecalho::novo(1).unwrap();
    let aad = b"cabecalho do registro";
    let claro = b"Rua das Flores, 123 -- Blumenau";
    let guardado = cab.selar([1, 2, 3, 4], 4096, aad, claro);
    assert_eq!(guardado.len(), claro.len() + cofre::ACRESCIMO);
    assert!(!contem(b"Blumenau", &guardado), "o texto claro sobreviveu");
    assert_eq!(
        cab.abrir([1, 2, 3, 4], 4096, aad, &guardado, "t").unwrap(),
        claro
    );
    assert!(
        cab.abrir([1, 2, 3, 4], 4096, b"outro cabecalho", &guardado, "t")
            .is_err(),
        "trocar o dado associado passou"
    );
    assert!(
        cab.abrir([1, 2, 3, 4], 8192, aad, &guardado, "t").is_err(),
        "trocar o offset -- que entra no nonce -- passou"
    );
    assert!(
        cab.abrir([9, 9, 9, 9], 4096, aad, &guardado, "t").is_err(),
        "trocar o tempero -- que entra no nonce -- passou"
    );
    cofre::desligar();
}

/// Trocar o ROWID de um evento cifrado nao passa, mesmo com o CRC consertado.
///
/// # O defeito que este teste existe para pegar
///
/// O cabecalho do evento fica em claro, e por bom motivo: e ele que diz onde o
/// proximo evento comeca. O preco de deixa-lo em claro so nao vira buraco
/// porque ele entra como DADO ASSOCIADO da etiqueta. Sem isso, quem tem o
/// arquivo trocaria o rowid de um evento -- consertando o CRC, que e publico --
/// e a imagem de uma linha passaria a ser lida como sendo de outra. O CRC
/// sozinho nao protege de quem PODE recalcular o CRC.
#[test]
fn trocar_o_cabecalho_de_um_evento_cifrado_nao_passa() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("cabecalho-trocado");
    {
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar_com_imagem(Operacao::Inclusao, 42, 1, b"o salario do 42")
            .unwrap();
        l.sincronizar().unwrap();
    }

    let caminho = d.join("t.log");
    let mut bruto = std::fs::read(&caminho).unwrap();
    let cab = cofre::CAB_V3;
    // O rowid mora em 12..20 do cabecalho do evento. 42 vira 7.
    bruto[cab + 12..cab + 20].copy_from_slice(&7u64.to_le_bytes());
    // E o CRC e consertado, porque quem adultera o arquivo sabe recalcula-lo.
    let novo_crc = {
        let mut crc = phxsql_core::crc::crc32(&bruto[cab..cab + 36]);
        crc ^= phxsql_core::crc::crc32(&bruto[cab + 44..]);
        crc
    };
    bruto[cab + 36..cab + 40].copy_from_slice(&novo_crc.to_le_bytes());
    std::fs::write(&caminho, &bruto).unwrap();

    let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
    assert_eq!(
        l.verificar().unwrap(),
        1,
        "o CRC devia continuar fechando -- e por isso ele nao basta"
    );
    assert_eq!(l.ler(0, 0).unwrap()[0].rowid, 7);
    let Err(e) = l.ler_com_imagem(0, 0) else {
        panic!("a imagem do rowid 42 foi lida como sendo do rowid 7")
    };
    assert!(e.to_string().contains("etiqueta"), "{e}");

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// A tabela inteira, pelo caminho normal: `Table::criar`, inserir, excluir.
///
/// # Por que este teste existe alem dos outros
///
/// Os de cima abrem `LogFile`, `LixeiraFile` e `MotivoFile` a mao. Este passa
/// pela `Table`, que e por onde o servidor passa -- e prova que a chave chega
/// aos tres sem que nenhuma assinatura da `Table` tenha mudado. A cifra e uma
/// decisao do processo justamente para nao atravessar quatro camadas de API.
#[test]
fn a_tabela_inteira_nasce_com_os_tres_diarios_cifrados() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = dir("tabela");

    let esquema = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();

    let mut t = Table::criar(&d, esquema).unwrap();
    for i in 1..=100i64 {
        t.inserir(&[
            Value::Int(i),
            Value::Str(format!("Fulano {i:04}")),
            Value::Str("Blumenau".into()),
        ])
        .unwrap();
    }
    assert!(t.excluir_suave(7, "revisao de cadastro").unwrap());
    assert!(t.excluir_de_vez(9, "pedido de remocao do titular").unwrap());
    t.sincronizar().unwrap();
    drop(t);

    // Nenhum dos tres pode ter o dado do cliente em claro.
    for ext in ["log", "trash", "reason"] {
        let bruto = bytes_do_arquivo(&d, "clientes", ext);
        assert_eq!(
            u16::from_le_bytes([bruto[8], bruto[9]]),
            3,
            "o .{ext} da tabela nasceu na versao velha"
        );
        assert!(
            !contem(b"titular", &bruto),
            "o motivo em claro sobreviveu no .{ext}"
        );
    }
    // O `.trash` guarda a LINHA INTEIRA -- e e ela que nao pode estar legivel.
    let bruto = bytes_do_arquivo(&d, "clientes", "trash");
    assert!(!contem(b"Fulano 0009", &bruto), "a linha descartada vazou");

    // E tudo volta pela leitura normal.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(t.lixeira(0, 0, true).unwrap().len(), 1);
    let motivos = t.motivos(0, 0).unwrap();
    assert_eq!(motivos.len(), 2);
    assert_eq!(motivos[0].motivo, "revisao de cadastro");
    assert_eq!(motivos[1].motivo, "pedido de remocao do titular");
    assert_eq!(t.historico(7).unwrap().len(), 2);
    assert_eq!(t.diario(0, 0).unwrap().len(), 102);

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

// ---------------------------------------------------------------------------
// O cache de chaves derivadas responde a QUEM?
// ---------------------------------------------------------------------------

/// **O cache nao pode responder a quem nao deu a senha.**
///
/// `cofre::derivar` consulta `DERIVADAS` **antes** de olhar o `COFRE`: se a
/// entrada existe, ele devolve a chave sem nunca perguntar quem esta pedindo.
/// Com UMA senha do processo isso e correto e esta escrito assim no fonte --
/// «a chave ja esta na memoria do processo de qualquer jeito». O que segura a
/// correcao nao e o `derivar`: e o `definir_com`/`desligar` **esvaziarem o
/// cache inteiro**.
///
/// # Por que este teste existe ANTES do desenho que ele protege
///
/// Porque ha um desenho na mesa -- senha do banco vinda do login, guardada na
/// sessao -- em que tirar esse esvaziamento parece a otimizacao obvia: sem ele,
/// alternar de banco deixa de custar os 290 ms do PBKDF2. E o dia em que
/// alguem o tirar, a garantia «so quem sabe a senha le» **some sem erro, sem
/// log e sem teste vermelho**: o primeiro login que fornecesse a senha poria a
/// chave no cache do PROCESSO, e dali em diante qualquer sessao abriria a
/// tabela sem fornecer senha nenhuma.
///
/// Nao ha defeito hoje. Ha uma porta, e esta e a tranca dela.
#[test]
fn o_cache_de_derivadas_nao_responde_a_quem_nao_deu_a_senha() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let sal = [0x5au8; cofre::SAL_LEN];

    // (1) Uma sessao fornece a senha e a chave entra no cache.
    cofre::desligar();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let da_sessao_a = cofre::derivar(&sal, RAPIDO, "<a>").unwrap();

    // (2) Outra sessao chega com OUTRA senha, no mesmo sal. Se o cache
    //     respondesse, ela receberia a chave da primeira.
    cofre::definir("nao e a senha da sessao A", RAPIDO).unwrap();
    let da_sessao_b = cofre::derivar(&sal, RAPIDO, "<b>").unwrap();
    assert_ne!(
        da_sessao_a, da_sessao_b,
        "o cache respondeu a quem nao deu a senha: as duas sessoes receberam a \
         MESMA chave para o mesmo sal, e a garantia «so quem sabe a senha le» \
         durou ate o primeiro login"
    );

    // (3) E o caso extremo, que e o que a senha por sessao produz de verdade:
    //     ninguem tem a senha agora. O cache nao pode ser a resposta.
    cofre::desligar();
    let sem_ninguem = cofre::derivar(&sal, RAPIDO, "<sem sessao>");
    assert!(
        sem_ninguem.is_err(),
        "o cache abriu um arquivo cifrado com NENHUMA senha no cofre -- a \
         chave sobreviveu a quem a forneceu"
    );

    // E a recusa DIZ o que fazer, em vez de devolver lixo.
    let e = sem_ninguem.unwrap_err().to_string();
    assert!(
        e.contains("nao tem a chave"),
        "a recusa nao explica nada: {e}"
    );
    cofre::desligar();
}

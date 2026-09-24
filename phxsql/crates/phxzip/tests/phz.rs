//! A prova do PhxZip contra o 7-Zip, nos dois sentidos, e contra entrada hostil.
//!
//! # De onde vem cada fixture
//!
//! Gravados UMA vez pelo 7-Zip 23.01 (x64, pacote `7zip` do Debian), com a
//! senha INVENTADA `fixture-inventada-450` -- nunca a senha do dono --, a partir
//! do `config.json` que esta em [`CONFIG`]:
//!
//! ```text
//! S=fixture-inventada-450
//! 7z a -p$S -mhe=on           lzma2-cabecalho-cifrado.phz config.json
//! 7z a -p$S                   lzma2-cabecalho-claro.phz   config.json
//! 7z a -p$S -m0=LZMA -mhe=on  lzma-cabecalho-cifrado.phz  config.json
//! 7z a -p$S -mx=0 -mhe=on     copy-cabecalho-cifrado.phz  config.json
//! 7z a -p$S -m0=PPMd -mhe=on  legado-ppmd.phz             config.json
//! 7z a -p$S -m0=BCJ -m1=LZMA2 -mhe=on legado-bcj.phz      config.json
//! 7z a -p$S -m0=BZip2 -mhe=on legado-bzip2.phz            config.json
//! 7z a -p$S -m0=Deflate -mhe=on legado-deflate.phz        config.json
//! 7z a -p$S -mhe=on           duas-entradas.phz config.json outro.json
//! 7z a                        sem-senha.phz               config.json
//! 7z a -p$S -mhe=on           muitos-cabecalho-cifrado.phz muitos   # pasta + 60 arquivos
//! 7z a -p$S -mhe=on           arvore-cifrada.7z raiz
//! 7z a                        arvore-clara.7z   raiz
//! 7z a                        base-zip-slip.7z  XXfora.txt
//! ```
//!
//! A `raiz` tem `a.txt`, `vazio.txt` (vazio), `vazia/` (pasta vazia),
//! `sub/b.bin` e `sub/fundo/ação.txt`, todos com data 2026-09-24 12:34:56 UTC
//! (`touch -d`). O conteudo de cada um esta nas funcoes abaixo.
//!
//! O que os fixtures exercitam, conferido nos proprios bytes: `duas-entradas`,
//! `muitos` e `arvore-*` trazem o cabecalho comprimido em LZMA (`030101`) -- com
//! e sem 7zAES por cima --, que e o caminho do LZMA puro que so aparece quando o
//! cabecalho cresce.

mod comum;

use std::path::Path;

use comum::DirTemp;

use phxzip::{
    desempacotar, desempacotar_com_teto, empacotar, empacotar_com_ciclos, unix_para_filetime,
    Arquivo, Erro, Escritor, Limites, Metodo, Opcoes,
};

const SENHA: &str = "fixture-inventada-450";
const CONFIG: &[u8] = b"{\n  \"porta\": 5433,\n  \"nome\": \"configuracao inventada para teste\",\n  \"ligado\": true,\n  \"lista\": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]\n}\n";

macro_rules! fixture {
    ($nome:literal) => {
        include_bytes!(concat!("fixtures/", $nome)).as_slice()
    };
}

/// Poucas rodadas nos testes que gravam muitos arquivos: a derivacao de 2^19
/// SHA-256 e o que custa, e aqui o que se prova e o formato, nao o custo. Os
/// testes que provam as 2^19 do padrao usam o padrao.
const CICLOS_DE_TESTE: u8 = 4;

fn b_bin() -> Vec<u8> {
    (0..3000u32).map(|i| ((i * 7 + 3) % 256) as u8).collect()
}

fn acao_txt() -> Vec<u8> {
    "nome com acento e cedilha\n".repeat(20).into_bytes()
}

// ------------------------------------------------------ 7-Zip -> PhxZip

#[test]
fn le_o_lzma2_com_cabecalho_cifrado_do_7zip() {
    let (nome, dados) = desempacotar(fixture!("lzma2-cabecalho-cifrado.phz"), SENHA).unwrap();
    assert_eq!((nome.as_str(), dados.as_slice()), ("config.json", CONFIG));
}

#[test]
fn le_o_lzma2_com_cabecalho_claro_do_7zip() {
    let (nome, dados) = desempacotar(fixture!("lzma2-cabecalho-claro.phz"), SENHA).unwrap();
    assert_eq!((nome.as_str(), dados.as_slice()), ("config.json", CONFIG));
}

#[test]
fn le_o_lzma_do_7zip() {
    let (_, dados) = desempacotar(fixture!("lzma-cabecalho-cifrado.phz"), SENHA).unwrap();
    assert_eq!(dados, CONFIG);
}

#[test]
fn le_o_copy_do_7zip() {
    let (_, dados) = desempacotar(fixture!("copy-cabecalho-cifrado.phz"), SENHA).unwrap();
    assert_eq!(dados, CONFIG);
}

#[test]
fn metodo_legado_e_recusado_pelo_nome() {
    for (arquivo, nome) in [
        (fixture!("legado-ppmd.phz"), "PPMd"),
        (fixture!("legado-bcj.phz"), "BCJ"),
        (fixture!("legado-bzip2.phz"), "BZip2"),
        (fixture!("legado-deflate.phz"), "Deflate"),
    ] {
        match desempacotar(arquivo, SENHA) {
            Err(Erro::MetodoLegado(m)) => assert!(m.contains(nome), "{nome}: veio {m}"),
            outro => panic!("{nome}: esperava MetodoLegado, veio {outro:?}"),
        }
    }
}

/// O cabecalho destes dois vem em LZMA + 7zAES: chegar a contar as entradas e
/// a prova de que o LZMA puro decodificou.
#[test]
fn mais_de_uma_entrada_e_recusada_contando() {
    assert_eq!(
        desempacotar(fixture!("duas-entradas.phz"), SENHA),
        Err(Erro::MaisDeUmaEntrada(2))
    );
    // A pasta `muitos` e os 60 arquivos dela.
    assert_eq!(
        desempacotar(fixture!("muitos-cabecalho-cifrado.phz"), SENHA),
        Err(Erro::MaisDeUmaEntrada(61))
    );
}

#[test]
fn sem_senha_nao_e_phz() {
    assert_eq!(
        desempacotar(fixture!("sem-senha.phz"), SENHA),
        Err(Erro::SemCifra)
    );
}

/// O 7-Zip nao grava o CRC do dado cifrado, e sem ele o formato nao separa
/// senha errada de byte trocado: o erro diz isso, em vez de afirmar um dos dois.
#[test]
fn senha_errada_em_arquivo_do_7zip_diz_que_nao_sabe_qual() {
    for arquivo in [
        fixture!("lzma2-cabecalho-cifrado.phz"),
        fixture!("lzma2-cabecalho-claro.phz"),
        fixture!("lzma-cabecalho-cifrado.phz"),
    ] {
        assert_eq!(
            desempacotar(arquivo, "outra-senha-inventada"),
            Err(Erro::SenhaErradaOuCorrompido)
        );
    }
}

fn conferir_arvore(a: &mut Arquivo) {
    let quando = unix_para_filetime(1_790_253_296);
    let lista: Vec<(String, bool, u64)> = a
        .entradas()
        .iter()
        .map(|e| (e.nome.clone(), e.pasta, e.tamanho))
        .collect();
    assert_eq!(
        lista,
        vec![
            ("raiz".into(), true, 0),
            ("raiz/sub".into(), true, 0),
            ("raiz/sub/fundo".into(), true, 0),
            ("raiz/vazia".into(), true, 0),
            ("raiz/vazio.txt".into(), false, 0),
            ("raiz/a.txt".into(), false, 11),
            ("raiz/sub/b.bin".into(), false, 3000),
            ("raiz/sub/fundo/ação.txt".into(), false, 520),
        ]
    );
    assert!(a.entradas().iter().all(|e| e.modificado == Some(quando)));
    let todas = a.extrair_todas().unwrap();
    let conteudo = |nome: &str| {
        todas
            .iter()
            .find(|(e, _)| e.nome == nome)
            .map(|(_, d)| d.clone())
            .unwrap()
    };
    assert_eq!(conteudo("raiz/a.txt"), b"texto de a\n");
    assert_eq!(conteudo("raiz/vazio.txt"), b"");
    assert_eq!(conteudo("raiz/sub/b.bin"), b_bin());
    assert_eq!(conteudo("raiz/sub/fundo/ação.txt"), acao_txt());
    // Extrair uma so da o mesmo.
    let i = a
        .entradas()
        .iter()
        .position(|e| e.nome == "raiz/sub/b.bin")
        .unwrap();
    assert_eq!(a.extrair(i).unwrap(), b_bin());
}

#[test]
fn lista_e_extrai_a_arvore_que_o_7zip_gravou() {
    let mut a = Arquivo::abrir(fixture!("arvore-clara.7z"), None, Limites::default()).unwrap();
    assert!(!a.cabecalho_cifrado());
    conferir_arvore(&mut a);
}

#[test]
fn lista_e_extrai_a_arvore_cifrada_que_o_7zip_gravou() {
    let mut a = Arquivo::abrir(
        fixture!("arvore-cifrada.7z"),
        Some(SENHA),
        Limites::default(),
    )
    .unwrap();
    assert!(a.cabecalho_cifrado());
    conferir_arvore(&mut a);
    assert!(matches!(
        Arquivo::abrir(fixture!("arvore-cifrada.7z"), None, Limites::default()),
        Err(Erro::SenhaAusente)
    ));
}

// ------------------------------------------------------------ zip-slip

/// Troca o nome `XXfora.txt` do fixture por outro de mesmo tamanho e refaz os
/// dois CRCs do cabecalho -- o que um atacante faria.
fn com_nome(nome: &str) -> Vec<u8> {
    let mut d = fixture!("base-zip-slip.7z").to_vec();
    let de: Vec<u8> = "XXfora.txt"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let para: Vec<u8> = nome.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    assert_eq!(
        de.len(),
        para.len(),
        "o nome novo tem de ter o mesmo tamanho"
    );
    let onde = d
        .windows(de.len())
        .position(|w| w == de.as_slice())
        .unwrap();
    d[onde..onde + de.len()].copy_from_slice(&para);
    refazer_crcs(&mut d);
    d
}

/// Refaz o CRC do cabecalho e o do cabecalho de inicio.
fn refazer_crcs(d: &mut [u8]) {
    let off = u64::from_le_bytes(d[12..20].try_into().unwrap()) as usize;
    let tam = u64::from_le_bytes(d[20..28].try_into().unwrap()) as usize;
    let crc = phxsql_core::crc32(&d[32 + off..32 + off + tam]);
    d[28..32].copy_from_slice(&crc.to_le_bytes());
    let crc_inicio = phxsql_core::crc32(&d[12..32]);
    d[8..12].copy_from_slice(&crc_inicio.to_le_bytes());
}

/// O extrator que a web e o terminal vao ser: grava cada entrada em
/// `destino/nome`. Ele NAO confere o nome -- confia no motor, que e exatamente
/// o contrato: a conferencia mora num lugar so.
fn extrair_para(destino: &Path, arquivo: &[u8]) -> Result<(), Erro> {
    let mut a = Arquivo::abrir(arquivo, None, Limites::default())?;
    a.percorrer(|e, dados| {
        let caminho = destino.join(&e.nome);
        if e.pasta {
            std::fs::create_dir_all(&caminho).unwrap();
        } else {
            if let Some(pai) = caminho.parent() {
                std::fs::create_dir_all(pai).unwrap();
            }
            std::fs::write(&caminho, dados).unwrap();
        }
    })
}

/// A pasta do teste, com `destino/` dentro. O guarda apaga tudo no `Drop` --
/// inclusive o arquivo que um zip-slip tenha escrito fora do destino, que fica
/// DENTRO dela.
fn pasta_de_teste(rotulo: &str) -> DirTemp {
    let d = DirTemp::novo(rotulo);
    std::fs::create_dir_all(d.join("destino")).unwrap();
    d
}

/// O zip-slip, pelo disco: o vermelho (conferencia tirada) mede o arquivo que
/// apareceu FORA do destino. `..\` so escapa de verdade no Windows -- e por isso
/// este teste roda tambem sob o `wine`.
#[test]
fn nome_que_sobe_de_pasta_nao_escreve_fora_do_destino() {
    for (i, nome) in ["../ora.txt", "..\\ora.txt"].into_iter().enumerate() {
        let base = pasta_de_teste(&format!("slip{i}"));
        let r = extrair_para(&base.join("destino"), &com_nome(nome));
        let fora = base.join("ora.txt");
        let escapou = std::fs::read(&fora).ok();
        assert!(
            escapou.is_none(),
            "{nome:?}: escreveu {} bytes FORA do destino, em {}",
            escapou.map(|v| v.len()).unwrap_or(0),
            fora.display()
        );
        assert!(
            matches!(r, Err(Erro::NomePerigoso(_))),
            "{nome:?}: veio {r:?}"
        );
    }
}

// ------------------------------------------------------- PhxZip -> PhxZip

#[test]
fn o_phz_vai_e_volta_com_o_padrao_do_7zip() {
    let arq = empacotar("config.json", CONFIG, SENHA).unwrap();
    assert_eq!(&arq[..6], b"7z\xbc\xaf\x27\x1c");
    let (nome, dados) = desempacotar(&arq, SENHA).unwrap();
    assert_eq!((nome.as_str(), dados.as_slice()), ("config.json", CONFIG));
    // O cabecalho vai cifrado: o nome da entrada nao aparece nos bytes.
    let nome16: Vec<u8> = "config"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(!arq.windows(nome16.len()).any(|w| w == nome16.as_slice()));
}

/// No arquivo do PhxZip o CRC do dado cifrado esta gravado, entao a senha
/// errada e AFIRMADA -- e nao confundida com arquivo corrompido.
#[test]
fn senha_errada_no_phz_do_phxzip_e_afirmada() {
    let arq = empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap();
    assert_eq!(
        desempacotar(&arq, "outra-senha-inventada"),
        Err(Erro::SenhaErrada)
    );
}

/// Com `Copy` por baixo do 7zAES nao ha decodificador que tropece no lixo de
/// uma chave errada: quem pega e so o CRC do conteudo. Sem ele, a senha errada
/// devolveria lixo como se fosse o arquivo.
#[test]
fn senha_errada_com_copy_nao_devolve_lixo() {
    let mut e = Escritor::novo(Opcoes {
        metodo: Metodo::Copia,
        senha: Some(SENHA.into()),
        ciclos: CICLOS_DE_TESTE,
        cifrar_cabecalho: false,
    })
    .unwrap();
    e.arquivo("config.json", CONFIG, None).unwrap();
    let arq = e.terminar();
    let mut a = Arquivo::abrir(&arq, Some("outra-senha-inventada"), Limites::default()).unwrap();
    let r = a.extrair(0);
    assert_eq!(r, Err(Erro::SenhaErrada), "a senha errada devolveu {r:?}");
}

/// Cada byte do arquivo trocado, um de cada vez: nunca sai conteudo, nunca
/// derruba a thread, e NUNCA vira «senha errada» -- o arquivo inteiro esta
/// debaixo de algum CRC que a senha nao alcanca.
#[test]
fn byte_trocado_em_qualquer_lugar_e_corrupcao_e_nao_senha() {
    let arq = empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap();
    for i in 0..arq.len() {
        let mut v = arq.clone();
        v[i] ^= 0x10;
        match desempacotar(&v, SENHA) {
            // O byte 7 e a versao MENOR do formato: nenhum CRC a cobre e o
            // 7-Zip nao a confere (`7zArcIn.c:1489` so olha a maior). Trocada,
            // o conteudo sai igual -- e e o unico byte assim.
            Ok((_, d)) if i == 7 && d == CONFIG => {}
            Ok(_) => panic!("o byte {i} trocado passou"),
            Err(Erro::SenhaErrada | Erro::SenhaErradaOuCorrompido) => {
                panic!("o byte {i} trocado virou senha errada")
            }
            Err(_) => {}
        }
    }
    // E no meio do dado cifrado, o nome e o certo.
    let mut v = arq.clone();
    v[40] ^= 0x01;
    assert!(matches!(desempacotar(&v, SENHA), Err(Erro::Corrompido(_))));
}

#[test]
fn arquivo_cortado_em_qualquer_ponto_e_recusado_sem_panico() {
    let nosso = empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap();
    for arq in [
        nosso.as_slice(),
        fixture!("lzma2-cabecalho-cifrado.phz"),
        fixture!("arvore-clara.7z"),
    ] {
        for corte in 0..arq.len() {
            assert!(
                desempacotar(&arq[..corte], SENHA).is_err(),
                "o prefixo de {corte} passou"
            );
        }
    }
}

/// O teto e conferido contra o tamanho DECLARADO, antes de decodificar.
#[test]
fn conteudo_acima_do_teto_e_recusado_pelo_declarado() {
    let grande: Vec<u8> = CONFIG.iter().copied().cycle().take(100_000).collect();
    let arq = empacotar_com_ciclos("config.json", &grande, SENHA, CICLOS_DE_TESTE).unwrap();
    assert_eq!(
        desempacotar_com_teto(&arq, SENHA, 1_000),
        Err(Erro::GrandeDemais {
            oque: "entrada",
            declarado: 100_000,
            teto: 1_000
        })
    );
    assert_eq!(
        desempacotar_com_teto(&arq, SENHA, 100_000).unwrap().1,
        grande
    );
}

/// A tabela de probabilidades do LZMA puro e escolhida pelo FLUXO (ate 6 MiB);
/// num microcontrolador o teto dela e o que impede o arquivo de derrubar a
/// placa por falta de memoria. O `0x5D` que o 7-Zip grava pede ~16 KiB.
#[test]
fn modelo_do_lzma_acima_do_teto_e_recusado_antes_de_alocar() {
    let limites = Limites {
        modelo: 1_000,
        ..Limites::default()
    };
    let mut a =
        Arquivo::abrir(fixture!("lzma-cabecalho-cifrado.phz"), Some(SENHA), limites).unwrap();
    match a.extrair(0) {
        Err(Erro::GrandeDemais {
            oque: "modelo do LZMA",
            declarado,
            teto: 1_000,
        }) => assert!(
            (12_288..20_000).contains(&declarado),
            "declarou {declarado}"
        ),
        outro => panic!("esperava o teto do modelo, veio {outro:?}"),
    }
    let mut a = Arquivo::abrir(
        fixture!("lzma-cabecalho-cifrado.phz"),
        Some(SENHA),
        Limites::default(),
    )
    .unwrap();
    assert_eq!(a.extrair(0).unwrap(), CONFIG);
}

#[test]
fn ciclos_acima_do_que_o_7zip_le_sao_recusados() {
    assert_eq!(
        empacotar_com_ciclos("config.json", CONFIG, SENHA, 25),
        Err(Erro::CiclosDemais {
            pedidos: 25,
            teto: 24
        })
    );
}

#[test]
fn o_escritor_recusa_o_que_o_leitor_recusaria() {
    assert!(matches!(
        empacotar_com_ciclos("../config.json", CONFIG, SENHA, 1),
        Err(Erro::NomePerigoso(_))
    ));
    let mut e = Escritor::novo(Opcoes::default()).unwrap();
    e.arquivo("a.txt", b"1", None).unwrap();
    assert_eq!(
        e.arquivo("a.txt", b"2", None),
        Err(Erro::NomeRepetido("a.txt".into()))
    );
}

fn arvore_do_phxzip(opcoes: Opcoes) -> Vec<u8> {
    let quando = Some(unix_para_filetime(1_790_253_296));
    let mut e = Escritor::novo(opcoes).unwrap();
    e.pasta("raiz", quando).unwrap();
    e.pasta("raiz/sub", quando).unwrap();
    e.pasta("raiz/sub/fundo", quando).unwrap();
    e.pasta("raiz/vazia", quando).unwrap();
    e.arquivo("raiz/vazio.txt", b"", quando).unwrap();
    e.arquivo("raiz/a.txt", b"texto de a\n", quando).unwrap();
    e.arquivo("raiz/sub/b.bin", &b_bin(), quando).unwrap();
    e.arquivo("raiz/sub/fundo/ação.txt", &acao_txt(), quando)
        .unwrap();
    e.terminar()
}

#[test]
fn a_arvore_gravada_pelo_phxzip_volta_igual_em_todo_modo() {
    for (metodo, senha, cifrar_cabecalho) in [
        (Metodo::Lzma2, Some(SENHA), true),
        (Metodo::Lzma2, Some(SENHA), false),
        (Metodo::Lzma2, None, false),
        (Metodo::Copia, Some(SENHA), true),
        (Metodo::Copia, None, false),
    ] {
        let arq = arvore_do_phxzip(Opcoes {
            metodo,
            senha: senha.map(String::from),
            ciclos: CICLOS_DE_TESTE,
            cifrar_cabecalho,
        });
        let mut a = Arquivo::abrir(&arq, senha, Limites::default()).unwrap();
        assert_eq!(a.cabecalho_cifrado(), cifrar_cabecalho);
        conferir_arvore(&mut a);
    }
}

/// Mutacao do cabecalho em claro COM os CRCs refeitos: e o que chega ao
/// analisador do cabecalho -- contagens, tamanhos e deslocamentos mentirosos.
/// Nada pode derrubar a thread nem alocar pelo que o arquivo diz.
#[test]
fn cabecalho_hostil_com_crc_refeito_nunca_derruba_a_thread() {
    let base = arvore_do_phxzip(Opcoes {
        metodo: Metodo::Lzma2,
        senha: None,
        ciclos: CICLOS_DE_TESTE,
        cifrar_cabecalho: false,
    });
    let off = u64::from_le_bytes(base[12..20].try_into().unwrap()) as usize;
    let mut x: u32 = 0x1234_5678;
    let mut sorteio = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    for _ in 0..4000 {
        let mut v = base.clone();
        for _ in 0..1 + sorteio() % 3 {
            let i = 32 + off + (sorteio() as usize % (v.len() - 32 - off));
            v[i] = sorteio() as u8;
        }
        refazer_crcs(&mut v);
        if let Ok(mut a) = Arquivo::abrir(&v, None, Limites::default()) {
            let _ = a.extrair_todas();
        }
    }
}

/// O arquivo gravado nao depende da plataforma: o IV e sintetico e todo
/// inteiro sai little-endian. Este SHA-256 foi medido no x86_64; o mesmo teste
/// sob `qemu-arm` (32 bits) e sob `wine` (Windows) tem de dar o mesmo -- e
/// assim um `.phz` gravado em qualquer um abre em qualquer outro.
#[test]
fn o_arquivo_gravado_e_o_mesmo_em_toda_plataforma() {
    let phz = empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap();
    let arvore = arvore_do_phxzip(Opcoes {
        metodo: Metodo::Lzma2,
        senha: Some(SENHA.into()),
        ciclos: CICLOS_DE_TESTE,
        cifrar_cabecalho: true,
    });
    let h = |d: &[u8]| phxsql_core::hash::para_hex(&phxsql_core::hash::sha256(d));
    assert_eq!(
        h(&phz),
        "70f7e37ff5c411f2a4c4c268342b824347aee3aab600aad592d02252c225498e"
    );
    assert_eq!(
        h(&arvore),
        "dfe77ac864548fb524d6799934da41da3956e7d2287df09ff8ce9d03404fa1b3"
    );
    // E o que o x86_64 gravou, guardado como fixture, abre aqui.
    let (_, dados) = desempacotar(fixture!("gravado-pelo-phxzip.phz"), SENHA).unwrap();
    assert_eq!(dados, CONFIG);
}

/// Regrava o fixture `gravado-pelo-phxzip.phz` e imprime os dois SHA-256 que
/// o teste acima confere. Fixture gerado por teste, e nao a mao, para que
/// qualquer sessao o refaca igual:
///
/// ```text
/// cargo test -p phxzip --test phz -- --ignored gerar_o_fixture --nocapture
/// ```
#[test]
#[ignore]
fn gerar_o_fixture_gravado_pelo_phxzip() {
    let phz = empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap();
    let arvore = arvore_do_phxzip(Opcoes {
        metodo: Metodo::Lzma2,
        senha: Some(SENHA.into()),
        ciclos: CICLOS_DE_TESTE,
        cifrar_cabecalho: true,
    });
    let h = |d: &[u8]| phxsql_core::hash::para_hex(&phxsql_core::hash::sha256(d));
    let destino =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gravado-pelo-phxzip.phz");
    std::fs::write(&destino, &phz).unwrap();
    println!("phz    {} ({} bytes)", h(&phz), phz.len());
    println!("arvore {} ({} bytes)", h(&arvore), arvore.len());
}

// ---------------------------------------------- PhxZip -> 7-Zip, ao vivo

fn sete_zip() -> Option<&'static str> {
    ["7z", "7za"].into_iter().find(|p| {
        std::process::Command::new(p)
            .arg("i")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// O que o PhxZip grava, o 7-Zip testa e extrai byte a byte. Precisa do `7z`
/// instalado -- e por isso e `#[ignore]`; sem ele, o teste FALHA dizendo
/// «NAO MEDIDO», em vez de passar calado. Rodar com:
///
/// ```text
/// cargo test -p phxzip --test phz -- --ignored o_7zip_le
/// ```
#[test]
#[ignore]
fn o_7zip_le_o_que_o_phxzip_grava() {
    let Some(sete) = sete_zip() else {
        panic!("NAO MEDIDO: nao ha 7z nem 7za nesta maquina");
    };
    let base = pasta_de_teste("7z");
    let casos: Vec<(&str, Vec<u8>, Option<&str>)> = vec![
        (
            "padrao.phz",
            empacotar("config.json", CONFIG, SENHA).unwrap(),
            Some(SENHA),
        ),
        (
            "poucos-ciclos.phz",
            empacotar_com_ciclos("config.json", CONFIG, SENHA, CICLOS_DE_TESTE).unwrap(),
            Some(SENHA),
        ),
        (
            "arvore-lzma2-cifrada.7z",
            arvore_do_phxzip(Opcoes {
                metodo: Metodo::Lzma2,
                senha: Some(SENHA.into()),
                ciclos: CICLOS_DE_TESTE,
                cifrar_cabecalho: true,
            }),
            Some(SENHA),
        ),
        (
            "arvore-lzma2-clara.7z",
            arvore_do_phxzip(Opcoes {
                metodo: Metodo::Lzma2,
                senha: None,
                ciclos: CICLOS_DE_TESTE,
                cifrar_cabecalho: false,
            }),
            None,
        ),
        (
            "arvore-copy-cifrada.7z",
            arvore_do_phxzip(Opcoes {
                metodo: Metodo::Copia,
                senha: Some(SENHA.into()),
                ciclos: CICLOS_DE_TESTE,
                cifrar_cabecalho: true,
            }),
            Some(SENHA),
        ),
    ];
    for (nome, bytes, senha) in casos {
        let caminho = base.join(nome);
        std::fs::write(&caminho, &bytes).unwrap();
        let p = format!("-p{}", senha.unwrap_or(""));
        let t = std::process::Command::new(sete)
            .args(["t", &p])
            .arg(&caminho)
            .output()
            .unwrap();
        assert!(
            t.status.success(),
            "7z t {nome}: {}",
            String::from_utf8_lossy(&t.stdout)
        );
        let destino = base.join(format!("x-{nome}"));
        let x = std::process::Command::new(sete)
            .args(["x", &p, "-y"])
            .arg(format!("-o{}", destino.display()))
            .arg(&caminho)
            .output()
            .unwrap();
        assert!(x.status.success(), "7z x {nome}");
        if nome.ends_with(".phz") {
            assert_eq!(std::fs::read(destino.join("config.json")).unwrap(), CONFIG);
        } else {
            assert_eq!(
                std::fs::read(destino.join("raiz/a.txt")).unwrap(),
                b"texto de a\n"
            );
            assert_eq!(
                std::fs::read(destino.join("raiz/sub/b.bin")).unwrap(),
                b_bin()
            );
            assert_eq!(
                std::fs::read(destino.join("raiz/sub/fundo/ação.txt")).unwrap(),
                acao_txt()
            );
            assert_eq!(std::fs::read(destino.join("raiz/vazio.txt")).unwrap(), b"");
            assert!(destino.join("raiz/vazia").is_dir());
        }
    }
}

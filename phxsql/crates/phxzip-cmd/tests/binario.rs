//! O PhxZipCmd pelo binario de verdade: processo, argumentos, ambiente,
//! entrada padrao e disco. As provas de seguranca medem o DANO -- o arquivo
//! que apareceu fora da raiz, o alvo do link que mudou, a senha que ecoou --,
//! e nao so o codigo de saida: uma recusa que acontece depois do estrago
//! passaria num teste que confere so o veredito.

mod comum;

use std::fs;

use comum::{cifrado, claro, marcar_como_link, phxzipcmd, trocar_nome, DirTemp};

const SEGREDO: &str = "SEGREDO-que-nao-ecoa-71";

// ------------------------------------------------------------ a senha

/// `--senha x` (e toda grafia dela) e recusado ANTES de abrir o arquivo: o
/// destino nao nasce, o valor nao aparece na saida e o codigo e o de uso.
///
/// Prova real: com o defeito reposto -- aceitar `--senha` como o `phxsqlcmd`
/// aceita --, o arquivo cifrado com ESSA senha extrai, o destino nasce e o
/// teste cai.
#[test]
fn senha_por_argumento_e_recusada_sem_ecoar_e_sem_tocar_no_disco() {
    let base = DirTemp::novo("senha-arg");
    fs::write(
        base.join("c.7z"),
        cifrado(&[("a.txt", b"conteudo")], SEGREDO),
    )
    .unwrap();
    let colado = format!("--senha={SEGREDO}");
    let sete = format!("-p{SEGREDO}");
    let caixa = format!("--SENHA={SEGREDO}");
    let pass = format!("--pass={SEGREDO}");
    let pwd = format!("--pwd:{SEGREDO}");
    let formas: [&[&str]; 7] = [
        &["--senha", SEGREDO],
        &[&colado],
        &[&sete],
        &["--password", SEGREDO],
        &[&caixa],
        &[&pass],
        &[&pwd],
    ];
    for forma in formas {
        let mut args = vec!["extrair", "c.7z", "--destino", "saida"];
        args.extend_from_slice(forma);
        let s = phxzipcmd(&base, &args, None, b"");
        assert_eq!(s.codigo, 2, "{forma:?}: saiu {} -- {}", s.codigo, s.tudo());
        assert!(
            !base.join("saida").exists(),
            "{forma:?}: o destino nasceu -- a senha por argumento foi usada"
        );
        assert!(
            !s.tudo().contains(SEGREDO),
            "{forma:?}: a senha apareceu na saida: {}",
            s.tudo()
        );
        assert!(
            s.err.contains("nunca vem por argumento") && s.err.contains("PHXZIP_SENHA"),
            "{forma:?}: a recusa nao diz o porque nem o caminho certo: {}",
            s.err
        );
    }
    // Compactar tambem: o arquivo de saida nao nasce.
    fs::write(base.join("f.txt"), b"x").unwrap();
    let s = phxzipcmd(
        &base,
        &[
            "compactar",
            "novo.7z",
            "f.txt",
            "--cifrar",
            "--senha",
            SEGREDO,
        ],
        None,
        b"",
    );
    assert_eq!(s.codigo, 2, "{}", s.tudo());
    assert!(!base.join("novo.7z").exists());
}

/// O caminho certo funciona: pela entrada padrao (cano) e pela variavel.
/// Sem senha, o arquivo cifrado recusa dizendo onde por a senha; com a
/// errada, recusa sem extrair.
#[test]
fn senha_pela_entrada_padrao_e_pela_variavel_ida_e_volta() {
    let base = DirTemp::novo("senha-ok");
    fs::create_dir_all(base.join("docs/sub")).unwrap();
    fs::write(base.join("docs/a.txt"), b"primeiro").unwrap();
    fs::write(base.join("docs/sub/b.bin"), [0u8, 0xFF, 7, 7, 7]).unwrap();
    let linha = format!("{SEGREDO}\n");

    let s = phxzipcmd(
        &base,
        &["compactar", "c.7z", "docs", "--cifrar", "--ciclos", "4"],
        None,
        linha.as_bytes(),
    );
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    let bruto = fs::read(base.join("c.7z")).unwrap();
    let nome16: Vec<u8> = "a.txt"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(
        !bruto.windows(nome16.len()).any(|w| w == nome16.as_slice()),
        "o nome foi gravado em claro com o cabecalho cifrado"
    );

    // Sem senha: o cabecalho cifrado recusa, e a entrada vazia e dita.
    let s = phxzipcmd(&base, &["listar", "c.7z"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(s.err.contains("PHXZIP_SENHA"), "{}", s.err);

    let s = phxzipcmd(&base, &["listar", "c.7z"], Some(SEGREDO), b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    for nome in ["docs/a.txt", "docs/sub/b.bin", "cabecalho cifrado"] {
        assert!(s.out.contains(nome), "faltou {nome}: {}", s.out);
    }

    let s = phxzipcmd(
        &base,
        &["extrair", "c.7z", "--destino", "errada"],
        None,
        b"outra-senha\n",
    );
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(!base.join("errada/docs/a.txt").exists());

    let s = phxzipcmd(
        &base,
        &["extrair", "c.7z", "--destino", "saida"],
        None,
        linha.as_bytes(),
    );
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert_eq!(
        fs::read(base.join("saida/docs/a.txt")).unwrap(),
        b"primeiro"
    );
    assert_eq!(
        fs::read(base.join("saida/docs/sub/b.bin")).unwrap(),
        [0u8, 0xFF, 7, 7, 7]
    );

    let s = phxzipcmd(&base, &["testar", "c.7z"], Some(SEGREDO), b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert!(!s.tudo().contains(SEGREDO));
}

// ------------------------------------------------------------ zip-slip

/// A entrada `../fora.txt` -- e a com barra invertida -- nao escreve fora da
/// raiz. O dano medido e o arquivo na pasta-mae do destino.
///
/// Prova real: com a conferencia tirada do motor (`conferir_nome`) E deste
/// extrator (`componentes`), `fora.txt` aparece ao lado do destino e o teste
/// cai. Com so uma das duas tirada, ele passa: a outra segura.
#[test]
fn entrada_que_sobe_de_pasta_nao_escreve_fora_da_raiz() {
    for (i, nome) in ["../fora.txt", "..\\fora.txt"].into_iter().enumerate() {
        let base = DirTemp::novo(&format!("slip{i}"));
        let mut arq = claro(&[("zz/fora.txt", b"ESCAPOU")]);
        trocar_nome(&mut arq, "zz/fora.txt", nome);
        fs::write(base.join("h.7z"), &arq).unwrap();
        let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
        let fora = base.join("fora.txt");
        assert!(
            !fora.exists(),
            "{nome:?}: escreveu FORA da raiz, em {}",
            fora.display()
        );
        assert_eq!(s.codigo, 1, "{nome:?}: {}", s.tudo());
        assert!(s.err.contains("perigoso"), "{nome:?}: {}", s.err);
    }
}

/// `a` arquivo e `a/b` no mesmo 7z: recusa ANTES de gravar qualquer byte.
#[test]
fn arquivo_e_pasta_com_o_mesmo_nome_recusam_antes_de_gravar() {
    let base = DirTemp::novo("colisao");
    fs::write(base.join("h.7z"), claro(&[("a", b"um"), ("a/b", b"dois")])).unwrap();
    let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(!base.join("raiz/a").exists(), "gravou antes de recusar");
}

// ------------------------------------------------------------ link

/// A entrada que vem marcada como LINK SIMBOLICO vira arquivo comum com os
/// bytes dela, e uma segunda extracao com `l/x.txt` nao atravessa para o alvo
/// que o link teria.
///
/// Prova real: honrando os bits (criar o link com o conteudo como alvo), `l`
/// vira link e o teste cai na primeira conferencia.
#[test]
fn entrada_marcada_como_link_vira_arquivo_comum_e_nao_e_atravessada() {
    let base = DirTemp::novo("link-entrada");
    let fora = base.join("fora");
    fs::create_dir_all(&fora).unwrap();
    let alvo = fora.to_str().unwrap().to_string();

    let mut arq = claro(&[("l", alvo.as_bytes())]);
    marcar_como_link(&mut arq, 1, 0);
    fs::write(base.join("um.7z"), &arq).unwrap();

    let s = phxzipcmd(&base, &["listar", "um.7z"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert!(
        s.out.contains("--L"),
        "a lista nao mostra a marca de link: {}",
        s.out
    );

    let s = phxzipcmd(&base, &["extrair", "um.7z", "--destino", "raiz"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    let m = fs::symlink_metadata(base.join("raiz/l")).unwrap();
    assert!(
        !m.file_type().is_symlink() && m.is_file(),
        "a entrada marcada como link virou {:?} no destino",
        m.file_type()
    );
    assert_eq!(fs::read(base.join("raiz/l")).unwrap(), alvo.as_bytes());
    assert!(s.err.contains("link simbolico"), "sem aviso: {}", s.err);

    // O segundo passo do ataque: agora escrever atraves de `l`.
    fs::write(base.join("dois.7z"), claro(&[("l/x.txt", b"ESCAPOU")])).unwrap();
    let s = phxzipcmd(
        &base,
        &["extrair", "dois.7z", "--destino", "raiz"],
        None,
        b"",
    );
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(!fora.join("x.txt").exists(), "escreveu no alvo do link");
}

/// Uma PASTA do destino que ja e link para fora: a entrada que passaria por
/// ela recusa, e nada aparece no alvo.
///
/// Prova real: olhando a pasta com `metadata` (que segue link) em vez de
/// `symlink_metadata`, `fora/x.txt` aparece e o teste cai.
#[cfg(unix)]
#[test]
fn pasta_do_destino_que_e_link_nao_e_seguida() {
    let base = DirTemp::novo("link-pasta");
    let fora = base.join("fora");
    fs::create_dir_all(&fora).unwrap();
    fs::create_dir_all(base.join("raiz")).unwrap();
    std::os::unix::fs::symlink(&fora, base.join("raiz/l")).unwrap();
    fs::write(base.join("h.7z"), claro(&[("l/x.txt", b"ESCAPOU")])).unwrap();

    let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
    assert!(
        !fora.join("x.txt").exists(),
        "seguiu o link e escreveu fora da raiz"
    );
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(s.err.contains("link"), "{}", s.err);
}

/// O ARQUIVO do destino que ja e link para fora: sem `--sobrescrever`
/// recusa; com, o que se troca e o link -- o alvo fica intacto.
///
/// Prova real: abrindo com `create(true).truncate(true)` (o `File::create`)
/// em vez de apagar o link e criar com `create_new`, o alvo vira `NOVO` e o
/// teste cai.
#[cfg(unix)]
#[test]
fn arquivo_do_destino_que_e_link_nao_tem_o_alvo_escrito() {
    let base = DirTemp::novo("link-arquivo");
    let vitima = base.join("vitima.txt");
    fs::write(&vitima, b"intacta").unwrap();
    fs::create_dir_all(base.join("raiz")).unwrap();
    std::os::unix::fs::symlink(&vitima, base.join("raiz/alvo.txt")).unwrap();
    fs::write(base.join("h.7z"), claro(&[("alvo.txt", b"NOVO")])).unwrap();

    let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert_eq!(fs::read(&vitima).unwrap(), b"intacta");

    let s = phxzipcmd(
        &base,
        &["extrair", "h.7z", "--destino", "raiz", "--sobrescrever"],
        None,
        b"",
    );
    assert_eq!(
        fs::read(&vitima).unwrap(),
        b"intacta",
        "escreveu no alvo do link"
    );
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    let m = fs::symlink_metadata(base.join("raiz/alvo.txt")).unwrap();
    assert!(m.is_file() && !m.file_type().is_symlink());
    assert_eq!(fs::read(base.join("raiz/alvo.txt")).unwrap(), b"NOVO");
}

/// Compactar nao segue link: um link para fora dentro da pasta e pulado com
/// aviso, e o conteudo do alvo nao entra no 7z.
#[cfg(unix)]
#[test]
fn compactar_nao_segue_link() {
    let base = DirTemp::novo("compactar-link");
    fs::write(base.join("segredo.txt"), b"NAO-PODE-ENTRAR").unwrap();
    fs::create_dir_all(base.join("docs")).unwrap();
    fs::write(base.join("docs/a.txt"), b"ok").unwrap();
    std::os::unix::fs::symlink(base.join("segredo.txt"), base.join("docs/l.txt")).unwrap();

    let s = phxzipcmd(&base, &["compactar", "c.7z", "docs", "--copia"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert!(s.err.contains("e link, pulado"), "{}", s.err);
    let bruto = fs::read(base.join("c.7z")).unwrap();
    assert!(
        !bruto.windows(15).any(|w| w == b"NAO-PODE-ENTRAR"),
        "o alvo do link entrou no 7z"
    );
    let s = phxzipcmd(&base, &["listar", "c.7z"], None, b"");
    assert!(
        s.out.contains("docs/a.txt") && !s.out.contains("l.txt"),
        "{}",
        s.out
    );
}

// ------------------------------------------------------------ o resto

/// Testar confere o CRC: um byte trocado no conteudo reprova com codigo 1.
#[test]
fn testar_acusa_byte_trocado() {
    let base = DirTemp::novo("testar");
    let mut arq = claro(&[("a.txt", b"conteudo-conferido")]);
    fs::write(base.join("bom.7z"), &arq).unwrap();
    let s = phxzipcmd(&base, &["testar", "bom.7z"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert!(s.out.contains("ok: 1 entrada"), "{}", s.out);

    let onde = arq.windows(8).position(|w| w == b"conteudo").unwrap();
    arq[onde] ^= 0x01;
    fs::write(base.join("ruim.7z"), &arq).unwrap();
    let s = phxzipcmd(&base, &["testar", "ruim.7z"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
}

/// Extrair nao sobrescreve calado; compactar nao sobrescreve calado.
#[test]
fn nada_se_sobrescreve_sem_pedir() {
    let base = DirTemp::novo("sobrescrever");
    fs::write(base.join("h.7z"), claro(&[("a.txt", b"do 7z")])).unwrap();
    fs::create_dir_all(base.join("raiz")).unwrap();
    fs::write(base.join("raiz/a.txt"), b"de antes").unwrap();
    let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert_eq!(fs::read(base.join("raiz/a.txt")).unwrap(), b"de antes");

    let s = phxzipcmd(&base, &["compactar", "h.7z", "raiz"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert_eq!(
        fs::read(base.join("h.7z")).unwrap(),
        claro(&[("a.txt", b"do 7z")])
    );
}

/// Uso errado sai com 2 e diz o que estava errado; opcao que nao vale para o
/// comando recusa em vez de ser ignorada.
#[test]
fn uso_errado_sai_com_dois() {
    let base = DirTemp::novo("uso");
    for args in [
        &["sumir", "x.7z"][..],
        &["extrair"][..],
        &["extrair", "x.7z", "--cifrar"][..],
        &["compactar", "x.7z"][..],
        &["compactar", "x.7z", "f", "--ciclos", "4"][..],
        &["listar", "x.7z", "--opcao-que-nao-existe"][..],
    ] {
        let s = phxzipcmd(&base, args, None, b"");
        assert_eq!(s.codigo, 2, "{args:?}: {}", s.tudo());
    }
    let s = phxzipcmd(&base, &["--help"], None, b"");
    assert_eq!(s.codigo, 0);
    assert!(s.out.contains("PHXZIP_SENHA"));
    assert!(s.out.contains("./-planilhas"), "o --help nao ensina o ./-");
}

// ------------------------------------------- revisao SEC da Z9 (09/10/2026)

/// Opcao desconhecida com o valor colado nao repete o valor: pode ser uma
/// senha digitada com a grafia errada.
///
/// Prova real: com a mensagem antiga (`para_tela(a)` inteiro), o segredo
/// aparece na saida e o teste cai.
#[test]
fn opcao_desconhecida_nao_repete_o_valor() {
    let base = DirTemp::novo("opcao-valor");
    for forma in [format!("--senah={SEGREDO}"), format!("--chave:{SEGREDO}")] {
        let s = phxzipcmd(&base, &["listar", "x.7z", &forma], None, b"");
        assert_eq!(s.codigo, 2, "{}", s.tudo());
        assert!(!s.tudo().contains(SEGREDO), "{forma}: {}", s.tudo());
    }
}

/// O caminho cru numa mensagem de erro do disco nao chega ao terminal com
/// `ESC`: o nome vem do arquivo, e o erro o repete.
///
/// Prova real: sem a `Tela` na porta do cmd E sem o `{:?}` no `disco.rs`, o
/// `ESC ]0;` e o `BEL` saem crus no `stderr` e o teste cai.
#[test]
fn caractere_de_controle_nao_sai_cru_no_erro() {
    let base = DirTemp::novo("controle-erro");
    let nome = "x\u{1b}]0;PWNED\u{7}\u{1b}[2J.txt";
    fs::create_dir_all(base.join("raiz")).unwrap();
    fs::write(base.join("raiz").join(nome), b"antes").unwrap();
    fs::write(base.join("h.7z"), claro(&[(nome, b"depois")])).unwrap();
    let s = phxzipcmd(&base, &["extrair", "h.7z", "--destino", "raiz"], None, b"");
    assert_eq!(s.codigo, 1, "{}", s.tudo());
    assert!(
        !s.tudo().contains(['\u{1b}', '\u{7}']),
        "controle cru na saida: {:?}",
        s.tudo()
    );
}

/// O `.7z` nasce 0600, e o conteudo que veio cifrado sai 0600; o claro sai
/// com o `umask` (o irmao: nao se estreita o que nao foi pedido).
///
/// Prova real: sem o `mode(0o600)` do `.parcial` e sem o `arquivo_privado`,
/// os dois nascem 0644 (sob `umask` 022) e o teste cai.
#[cfg(unix)]
#[test]
fn permissao_nao_alarga_o_que_era_privado() {
    use std::os::unix::fs::PermissionsExt;
    let modo = |p: &std::path::Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    let base = DirTemp::novo("permissao");
    fs::write(base.join("f.txt"), b"x").unwrap();
    let s = phxzipcmd(&base, &["compactar", "c.7z", "f.txt", "--copia"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert_eq!(modo(&base.join("c.7z")), 0o600);

    fs::write(
        base.join("s.7z"),
        cifrado(&[("s.txt", b"segredo")], SEGREDO),
    )
    .unwrap();
    let s = phxzipcmd(
        &base,
        &["extrair", "s.7z", "--destino", "d1"],
        Some(SEGREDO),
        b"",
    );
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    assert_eq!(modo(&base.join("d1/s.txt")), 0o600);

    fs::write(base.join("p.7z"), claro(&[("p.txt", b"publico")])).unwrap();
    let s = phxzipcmd(&base, &["extrair", "p.7z", "--destino", "d2"], None, b"");
    assert_eq!(s.codigo, 0, "{}", s.tudo());
    let umask_tira = 0o666 & !modo(&base.join("d2/p.txt"));
    assert!(
        umask_tira & 0o600 == 0,
        "o claro perdeu permissao do dono: {:o}",
        modo(&base.join("d2/p.txt"))
    );
}

/// `nobody`, dono da pasta que o root compacta, troca `f0300` por um link
/// para um segredo do root, atomicamente e sem parar. O dano medido e o
/// segredo dentro do `.7z`.
///
/// Prova real: com o `fs::read` antigo no lugar de `ler_o_mesmo`, o segredo
/// entra no `.7z` (medido na rodada: ver o relatorio) e o teste cai.
#[cfg(unix)]
#[test]
fn compactar_nao_segue_link_trocado_na_corrida() {
    use std::os::unix::fs::{chown, MetadataExt, PermissionsExt};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let base = DirTemp::novo("compactar-corrida");
    let sou_root = fs::metadata(&*base).unwrap().uid() == 0;
    if !sou_root || Command::new("python3").arg("-V").output().is_err() {
        eprintln!("PULADO: a corrida pede root (atacante como nobody) e python3");
        return;
    }
    const NINGUEM: u32 = 65534;
    let segredo = base.join("segredo.txt");
    fs::write(&segredo, b"SEGREDO-DO-ROOT-0600").unwrap();
    fs::set_permissions(&segredo, fs::Permissions::from_mode(0o600)).unwrap();
    let pasta = base.join("cz");
    fs::create_dir(&pasta).unwrap();
    for i in 0..600 {
        fs::write(pasta.join(format!("f{i:04}")), b"comum").unwrap();
    }
    chown(&pasta, Some(NINGUEM), Some(NINGUEM)).unwrap();
    let corredor = base.join("corredor.py");
    fs::write(
        &corredor,
        "import os, sys, time, ctypes\n\
         libc = ctypes.CDLL(None, use_errno=True)\n\
         d, alvo, fim = sys.argv[1], sys.argv[2], time.time() + float(sys.argv[3])\n\
         f = os.path.join(d, 'f0300').encode(); l = os.path.join(d, '.l').encode()\n\
         os.symlink(alvo, l)\n\
         n = 0\n\
         while time.time() < fim:\n\
         \x20   libc.renameat2(-100, f, -100, l, 2); libc.renameat2(-100, f, -100, l, 2); n += 1\n\
         print(n)\n",
    )
    .unwrap();
    let atacante = Command::new("python3")
        .arg("-I")
        .arg(&corredor)
        .arg(&pasta)
        .arg(&segredo)
        .arg("4")
        .uid(NINGUEM)
        .gid(NINGUEM)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut vazou = 0;
    let mut abortou = 0;
    let mut voltas = 0;
    let fim = std::time::Instant::now() + std::time::Duration::from_millis(3200);
    while std::time::Instant::now() < fim {
        let s = phxzipcmd(
            &base,
            &["compactar", "c.7z", "cz", "--copia", "--sobrescrever"],
            None,
            b"",
        );
        voltas += 1;
        if s.codigo != 0 {
            abortou += 1;
            continue;
        }
        let bruto = fs::read(base.join("c.7z")).unwrap();
        if bruto.windows(20).any(|w| w == b"SEGREDO-DO-ROOT-0600") {
            vazou += 1;
        }
    }
    let trocas: u64 = String::from_utf8_lossy(&atacante.wait_with_output().unwrap().stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if trocas == 0 {
        eprintln!("PULADO: o atacante nao trocou nada (sem acesso a pasta?)");
        return;
    }
    eprintln!("corrida: {voltas} compactacoes, {abortou} abortadas, {trocas} trocas");
    assert_eq!(
        vazou, 0,
        "o segredo do root entrou no .7z em {vazou} de {voltas} compactacoes"
    );
}

/// O ctrl+C na pergunta da senha devolve o eco ao terminal, e a senha nunca
/// aparece na tela. Num pseudo-terminal de verdade (o `pty` do `python3`).
///
/// Prova real: com o `stty -echo` rodado pelo PROPRIO processo (o desenho
/// antigo), o ctrl+C o mata com o eco desligado e o teste cai.
///
/// O roteiro espera por CONDICAO, nunca por prazo de parede (pedido 754): a
/// pergunta tem de estar na tela E o eco desligado antes de digitar (eco
/// desligado prova que os `trap` do shell ja estao armados, porque vem antes
/// do `stty` no roteiro), e depois do ctrl+C espera o processo sair e o eco
/// voltar, com 30 s de teto so para nao travar a bateria. A versao com prazos
/// de 1 s caia 5 em 50 -- e o prazo nao era a causa: medido, nas 5 o eco
/// continuava desligado 2 s depois (ver o `SCRIPT_SEM_ECO`, o `trap '' HUP`).
#[cfg(unix)]
#[test]
fn o_eco_volta_no_ctrl_c_e_a_senha_nao_aparece() {
    use std::process::Command;
    if Command::new("python3").arg("-V").output().is_err() {
        eprintln!("PULADO: o pseudo-terminal pede python3");
        return;
    }
    let base = DirTemp::novo("pty");
    fs::write(base.join("alvo"), b"x").unwrap();
    let roteiro = base.join("pty.py");
    fs::write(
        &roteiro,
        "import os, pty, sys, time, termios, select\n\
         b, modo, cwd = sys.argv[1], sys.argv[2], sys.argv[3]\n\
         pid, fd = pty.fork()\n\
         if pid == 0:\n\
         \x20   os.chdir(cwd); os.environ.pop('PHXZIP_SENHA', None)\n\
         \x20   os.execv(b, [b, 'compactar', 'c.7z', 'alvo', '--cifrar', '--ciclos', '4', '--sobrescrever'])\n\
         out = b''\n\
         saiu = False\n\
         def eco():\n\
         \x20   try: return bool(termios.tcgetattr(fd)[3] & termios.ECHO)\n\
         \x20   except termios.error: return None\n\
         def esperar(cond, oque, teto=30.0):\n\
         \x20   global out, saiu\n\
         \x20   fim = time.time() + teto\n\
         \x20   while time.time() < fim:\n\
         \x20       if cond(): return\n\
         \x20       try:\n\
         \x20           r, _, _ = select.select([fd], [], [], 0.02)\n\
         \x20           if r: out += os.read(fd, 4096)\n\
         \x20       except OSError: time.sleep(0.02)\n\
         \x20       if not saiu and os.waitpid(pid, os.WNOHANG)[0] == pid: saiu = True\n\
         \x20   print('PRAZO', oque)\n\
         esperar(lambda: b'senha: ' in out and eco() is False, 'primeira pergunta sem eco')\n\
         os.write(fd, b'S3GREDO-UNICO\\n')\n\
         esperar(lambda: b'de novo' in out and eco() is False, 'segunda pergunta sem eco')\n\
         if modo == 'intr':\n\
         \x20   os.write(fd, b'\\x03')\n\
         \x20   esperar(lambda: saiu and eco() is True, 'o eco voltar depois do ctrl+C')\n\
         else:\n\
         \x20   os.write(fd, b'S3GREDO-UNICO\\n')\n\
         \x20   esperar(lambda: saiu, 'o fim da compactacao')\n\
         print('ECO', eco())\n\
         print('ECOOU', b'S3GREDO' in out)\n",
    )
    .unwrap();
    for modo in ["intr", "normal"] {
        let o = Command::new("python3")
            .arg("-I")
            .arg(&roteiro)
            .arg(env!("CARGO_BIN_EXE_phxzipcmd"))
            .arg(modo)
            .arg(&*base)
            .output()
            .unwrap();
        let saida = String::from_utf8_lossy(&o.stdout);
        assert!(!saida.contains("PRAZO"), "{modo}: {saida}");
        assert!(
            saida.contains("ECO True"),
            "{modo}: o eco nao voltou: {saida}"
        );
        assert!(
            saida.contains("ECOOU False"),
            "{modo}: a senha ecoou: {saida}"
        );
    }
    assert!(base.join("c.7z").exists(), "o modo normal nao compactou");
}

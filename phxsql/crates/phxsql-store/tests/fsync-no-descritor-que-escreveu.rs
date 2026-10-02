//! O `fsync` do backup acontece no MESMO descritor que escreveu, e a pasta
//! sincroniza antes de o manifesto novo nascer -- pedidos 552 e 579.
//!
//! # O defeito do 552
//!
//! `executar` escrevia a copia, FECHAVA o `File`, e o `sincronizar_copias`
//! reabria pelo caminho para o `fsync`. No intervalo o nucleo pode despejar o
//! inode, e com ele o erro de *writeback* guardado no `address_space` (o caso
//! *fsyncgate*): o descritor novo responde Ok sem o dado. Um descritor aberto
//! segura o inode; por isso a prova e sobre DESCRITOR, contada pelo nucleo:
//!
//! * pelo `/proc/self/fd`, os descritores das copias seguem abertos entre a
//!   escrita (sob a trava) e o `fsync` (fora dela);
//! * pelo `strace`, cada copia e aberta com sucesso UMA vez, e o `fsync` cai
//!   nesse mesmo numero de descritor sem `close` no meio.
//!
//! # O defeito do 579
//!
//! A remocao do `backup.json` velho (577) nao ganhava `fsync` do diretorio:
//! numa queda, o manifesto velho podia voltar. A prova, pelo `strace`: depois
//! do `unlink` do manifesto velho vem o `fsync` de um descritor aberto na
//! PASTA do destino, e so depois nasce o manifesto novo.
//!
//! **Nao medido:** a queda em si. Provar que o dado sobrevive pede derrubar a
//! maquina (ou um `dm-flakey`, `CAP_SYS_ADMIN`); o que se prova aqui e a
//! ORDEM e o DESCRITOR das chamadas ao nucleo, que e o que o conserto muda.
//!
//! # Por que `strace`, e o que acontece sem ele
//!
//! A pergunta e sobre chamadas ao sistema, e so quem as ve e o nucleo. O
//! corpo roda num filho deste mesmo binario, sob `strace -f`. Sem `strace` no
//! `PATH` os tres testes dele dizem «nao medido» e passam; o do `/proc` e o
//! da pasta do zip nao dependem dele.
#![cfg(target_os = "linux")]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::path::{Path, PathBuf};

const FILHO: &str = "PHX_552_FILHO_SOB_STRACE";
const QUANDO: i64 = 1_787_000_000_000;

fn raiz_com_dado(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    std::fs::create_dir_all(raiz.join("loja/matriz")).unwrap();
    std::fs::write(raiz.join("loja/clientes.reg"), b"registros").unwrap();
    std::fs::write(raiz.join("loja/clientes.ndx"), b"indice").unwrap();
    std::fs::write(raiz.join("loja/matriz/pedidos.reg"), b"pedidos").unwrap();
    raiz
}

/// Os descritores deste processo que apontam para dentro de `pasta`.
fn abertos_em(pasta: &Path) -> Vec<PathBuf> {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .filter(|alvo| alvo.starts_with(pasta))
        // Pedido 568: as pastas do destino tambem ficam abertas (a `Pasta`
        // por onde o backup chega aos nomes sem atravessar link); a pergunta
        // aqui e so das COPIAS.
        .filter(|alvo| alvo.is_file())
        .collect()
}

/// **Os descritores das copias seguem abertos ate o `fsync`.**
///
/// Entre `executar` (que o servidor chama com a trava na mao) e `concluir`
/// (fora dela), o nucleo tem de mostrar um descritor aberto por copia; depois
/// que o chamador solta as `Copias`, nenhum.
#[test]
fn os_descritores_das_copias_seguem_abertos_ate_o_fsync() {
    let d = DirTemp::novo("552-proc");
    let raiz = raiz_com_dado(&d);
    let destino = d.join("copia");
    let (r, copias) = backup::executar(&raiz, &destino, QUANDO).unwrap();
    let abertos = abertos_em(&destino);
    assert_eq!(
        abertos.len(),
        r.arquivos.len(),
        "entre a escrita e o fsync o descritor de cada copia tinha de seguir \
         aberto -- fechado, o nucleo pode despejar o inode com o erro: {abertos:?}"
    );
    backup::concluir(&destino, QUANDO, &r, &copias).unwrap();
    drop(copias);
    assert_eq!(abertos_em(&destino), Vec::<PathBuf>::new());

    let pasta = d.join("zips");
    let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", QUANDO).unwrap();
    assert_eq!(
        abertos_em(&pasta).len(),
        1,
        "o descritor do .part tinha de seguir aberto ate o finalizar_zip"
    );
    backup::finalizar_zip(&zip).unwrap();
    drop(zip);
    assert_eq!(abertos_em(&pasta), Vec::<PathBuf>::new());
}

/// **O `rename` final do zip recusa: a pasta que a corrida criou sai junto.**
///
/// O `.part` some por fora antes do `finalizar_zip`; o `fsync` no descritor
/// aberto passa, e o `rename` recusa com o `ENOENT` do proprio nucleo. As
/// duas pastas (`zips/` e `zips/sub/`) nasceram nesta corrida e ficariam
/// vazias -- saem pelo motor do 576; a de cima, que ja existia, fica.
#[test]
fn o_rename_do_zip_que_recusa_nao_deixa_a_pasta_que_criou() {
    let d = DirTemp::novo("579-rename-pasta");
    let raiz = raiz_com_dado(&d);
    let pasta = d.join("zips/sub");
    let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", QUANDO).unwrap();
    let parcial = PathBuf::from(format!("{}.part", zip.display()));
    std::fs::remove_file(&parcial).expect("a premissa: o .part existia");

    let e = backup::finalizar_zip(&zip).expect_err("o rename sem o .part tinha de recusar");
    assert!(
        e.to_string().contains("No such file") || e.to_string().contains("os error 2"),
        "a recusa nao foi o ENOENT do rename -- a premissa nao vale: {e}"
    );
    assert!(
        std::fs::symlink_metadata(d.join("zips")).is_err(),
        "o rename recusado deixou a pasta que a corrida criou: {:?}",
        std::fs::read_dir(d.join("zips"))
            .map(|l| l.flatten().map(|e| e.path()).collect::<Vec<_>>())
    );
    assert!(d.join("dados/loja/clientes.reg").is_file());
}

// ------------------------------------------------------------------ strace

/// Uma chamada ao nucleo, do registro do `strace`.
#[derive(Debug)]
enum Chamada {
    Abriu(String, i64),
    Fechou(i64),
    Sincronizou(i64),
    Apagou(String),
    Renomeou(String, String),
}

/// Os argumentos entre aspas de uma linha do `strace`, na ordem.
fn aspas(linha: &str) -> Vec<String> {
    linha
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect()
}

/// O `= ret` do fim; o `strace` alinha em coluna, entao pode haver varios
/// espacos antes do `=`.
fn retorno(linha: &str) -> Option<i64> {
    linha
        .rsplit_once(" = ")?
        .1
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn primeiro_inteiro(linha: &str) -> Option<i64> {
    let dentro = linha.split_once('(')?.1;
    dentro.split([',', ')']).next()?.trim().parse().ok()
}

fn ler_registro(texto: &str) -> Vec<Chamada> {
    let mut v = Vec::new();
    // Pedido 568: o backup abre as copias pelo descritor da pasta
    // (`/proc/self/fd/N/nome`); o caminho real sai do que o `N` abriu.
    let mut por_fd: std::collections::HashMap<i64, String> = Default::default();
    let resolver = |p: String, por_fd: &std::collections::HashMap<i64, String>| {
        let Some(resto) = p.strip_prefix("/proc/self/fd/") else {
            return p;
        };
        let (n, nome) = resto.split_once('/').unwrap_or((resto, ""));
        match n.parse::<i64>().ok().and_then(|n| por_fd.get(&n)) {
            Some(pasta) if nome.is_empty() => pasta.clone(),
            Some(pasta) => format!("{pasta}/{nome}"),
            None => p,
        }
    };
    for bruta in texto.lines() {
        // `pid chamada(...) = ret`
        let linha = bruta.split_once(' ').map_or(bruta, |(_, r)| r).trim();
        let Some(ret) = retorno(linha) else { continue };
        let nome = linha.split('(').next().unwrap_or("");
        match nome {
            "openat" | "open" if ret >= 0 => {
                if let Some(p) = aspas(linha).into_iter().next() {
                    let p = resolver(p, &por_fd);
                    por_fd.insert(ret, p.clone());
                    v.push(Chamada::Abriu(p, ret));
                }
            }
            "close" if ret == 0 => v.extend(primeiro_inteiro(linha).map(Chamada::Fechou)),
            "fsync" | "fdatasync" if ret == 0 => {
                v.extend(primeiro_inteiro(linha).map(Chamada::Sincronizou))
            }
            "unlink" | "unlinkat" if ret == 0 => v.extend(
                aspas(linha)
                    .into_iter()
                    .next()
                    .map(|p| Chamada::Apagou(resolver(p, &por_fd))),
            ),
            "rename" | "renameat" | "renameat2" if ret == 0 => {
                let a = aspas(linha);
                if a.len() >= 2 {
                    v.push(Chamada::Renomeou(a[0].clone(), a[1].clone()));
                }
            }
            _ => {}
        }
    }
    v
}

/// Reexecuta UM teste deste binario sob `strace -f` e devolve as chamadas.
/// `None` quando nao ha `strace` -- e o teste diz «nao medido».
fn sob_strace(teste: &str, alvo: &Path, d: &Path) -> Option<Vec<Chamada>> {
    let tem = std::process::Command::new("strace")
        .arg("-V")
        .output()
        .is_ok_and(|s| s.status.success());
    if !tem {
        eprintln!("strace ausente: {teste} NAO MEDIDO");
        return None;
    }
    let log = d.join("strace.txt");
    let eu = std::env::current_exe().unwrap();
    let saida = std::process::Command::new("strace")
        .args(["-f", "-qq", "-o"])
        .arg(&log)
        .args([
            "-e",
            "trace=open,openat,close,fsync,fdatasync,unlink,unlinkat,rename,renameat,renameat2",
        ])
        .arg(&eu)
        .args(["--exact", teste, "--nocapture", "--test-threads=1"])
        .env(FILHO, alvo)
        .output()
        .expect("rodar o filho sob strace");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(saida.status.success(), "o filho reprovou:\n{texto}");
    assert!(texto.contains("1 passed"), "o filho nao rodou:\n{texto}");
    let registro = std::fs::read_to_string(&log).expect("o registro do strace");
    Some(ler_registro(&registro))
}

/// Para o arquivo `p`: o descritor da UNICA abertura com sucesso, e se o
/// `fsync` caiu nele antes de qualquer `close` dele.
fn sincronizou_no_mesmo(chamadas: &[Chamada], p: &str) -> Result<usize, String> {
    let aberturas: Vec<(usize, i64)> = chamadas
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Chamada::Abriu(q, fd) if q == p => Some((i, *fd)),
            _ => None,
        })
        .collect();
    let [(i, fd)] = aberturas[..] else {
        return Err(format!(
            "{p}: {} aberturas com sucesso, esperava UMA (a de quem escreveu)",
            aberturas.len()
        ));
    };
    for (j, c) in chamadas.iter().enumerate().skip(i + 1) {
        match c {
            Chamada::Sincronizou(f) if *f == fd => return Ok(j),
            Chamada::Fechou(f) if *f == fd => {
                return Err(format!(
                    "{p}: o descritor {fd} de quem escreveu fechou antes do fsync"
                ))
            }
            _ => {}
        }
    }
    Err(format!("{p}: nenhum fsync no descritor {fd}"))
}

/// Para onde o descritor `fd` aponta no fim de `antes`: a ultima abertura
/// que o devolveu, se nenhum `close` dele veio depois.
fn aponta_para(antes: &[Chamada], fd: i64) -> Option<&str> {
    for c in antes.iter().rev() {
        match c {
            Chamada::Abriu(p, f) if *f == fd => return Some(p),
            Chamada::Fechou(f) if *f == fd => return None,
            _ => {}
        }
    }
    None
}

/// O corpo do filho da pasta: uma corrida numa pasta REAPROVEITADA (ja tem o
/// `backup.json` da corrida anterior), do `executar` ao `concluir`.
fn filho_da_pasta() -> bool {
    let Some(destino) = std::env::var_os(FILHO) else {
        return false;
    };
    let destino = PathBuf::from(destino);
    let raiz = destino.parent().unwrap().join("dados");
    let (r, copias) = backup::executar(&raiz, &destino, QUANDO + 1).unwrap();
    backup::concluir(&destino, QUANDO + 1, &r, &copias).unwrap();
    true
}

/// A pasta com uma corrida inteira ja feita, e o registro da segunda.
fn segunda_corrida_sob_strace(teste: &str, d: &DirTemp) -> Option<(PathBuf, Vec<Chamada>)> {
    let raiz = raiz_com_dado(d);
    let destino = d.join("copia");
    let (r, copias) = backup::executar(&raiz, &destino, QUANDO).unwrap();
    backup::concluir(&destino, QUANDO, &r, &copias).unwrap();
    drop(copias);
    std::fs::write(raiz.join("loja/clientes.reg"), b"registros mudados").unwrap();
    let chamadas = sob_strace(teste, &destino, d)?;
    Some((destino, chamadas))
}

/// **Cada copia -- e o manifesto -- sincroniza no descritor que a escreveu.**
#[test]
fn a_copia_sincroniza_no_descritor_que_a_escreveu() {
    if filho_da_pasta() {
        return;
    }
    let d = DirTemp::novo("552-strace-pasta");
    let Some((destino, chamadas)) =
        segunda_corrida_sob_strace("a_copia_sincroniza_no_descritor_que_a_escreveu", &d)
    else {
        return;
    };
    let copias = [
        "loja/clientes.ndx",
        "loja/clientes.reg",
        "loja/matriz/pedidos.reg",
        backup::MANIFESTO,
    ];
    let erros: Vec<String> = copias
        .iter()
        .filter_map(|c| {
            let p = destino.join(c).to_string_lossy().into_owned();
            sincronizou_no_mesmo(&chamadas, &p).err()
        })
        .collect();
    assert!(erros.is_empty(), "{erros:#?}\n{chamadas:#?}");
}

/// **A pasta sincroniza depois do `unlink` do manifesto velho e ANTES de o
/// novo nascer.**
#[test]
fn a_pasta_sincroniza_antes_do_manifesto_novo() {
    if filho_da_pasta() {
        return;
    }
    let d = DirTemp::novo("579-strace-pasta");
    let Some((destino, chamadas)) =
        segunda_corrida_sob_strace("a_pasta_sincroniza_antes_do_manifesto_novo", &d)
    else {
        return;
    };
    let manifesto = destino
        .join(backup::MANIFESTO)
        .to_string_lossy()
        .into_owned();
    let pasta = destino.to_string_lossy().into_owned();
    let apagou = chamadas
        .iter()
        .position(|c| matches!(c, Chamada::Apagou(p) if *p == manifesto))
        .expect("a premissa: o manifesto velho saiu por unlink");
    let nasceu = chamadas
        .iter()
        .position(|c| matches!(c, Chamada::Abriu(p, _) if *p == manifesto))
        .expect("a premissa: o manifesto novo nasceu");
    assert!(apagou < nasceu, "a premissa: o velho sai antes do novo");
    // Um `fsync`, entre o unlink e o manifesto novo, num descritor que
    // naquele instante aponta para a PASTA. Pedido 593: o descritor e o que a
    // corrida abriu ANTES de escrever (a `Pasta`), e nao um `open` pelo nome
    // feito ali -- por isso a pergunta e para onde o descritor aponta.
    let sincronizou_a_pasta = (apagou..nasceu).any(|i| {
        matches!(&chamadas[i], Chamada::Sincronizou(f)
            if aponta_para(&chamadas[..i], *f) == Some(pasta.as_str()))
    });
    assert!(
        sincronizou_a_pasta,
        "o manifesto novo nasceu sem o fsync da pasta de onde o velho saiu:\n{:#?}",
        &chamadas[apagou..=nasceu]
    );
    // E o nome do manifesto novo tambem: fsync da pasta depois dele.
    let depois = (nasceu..chamadas.len()).any(|i| {
        matches!(&chamadas[i], Chamada::Sincronizou(f)
            if aponta_para(&chamadas[..i], *f) == Some(pasta.as_str()))
    });
    assert!(
        depois,
        "o nome do manifesto novo ficou sem o fsync da pasta"
    );
}

/// **O `.part` do zip sincroniza no descritor que o escreveu, antes do
/// `rename`.**
#[test]
fn o_zip_sincroniza_no_descritor_que_o_escreveu() {
    if let Some(pasta) = std::env::var_os(FILHO) {
        let pasta = PathBuf::from(pasta);
        let raiz = pasta.parent().unwrap().join("dados");
        let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", QUANDO).unwrap();
        backup::finalizar_zip(&zip).unwrap();
        return;
    }
    let d = DirTemp::novo("552-strace-zip");
    raiz_com_dado(&d);
    let pasta = d.join("zips");
    let Some(chamadas) = sob_strace("o_zip_sincroniza_no_descritor_que_o_escreveu", &pasta, &d)
    else {
        return;
    };
    let zip = std::fs::read_dir(&pasta)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "zip"))
        .expect("o zip final");
    let final_ = zip.to_string_lossy().into_owned();
    let parcial = format!("{final_}.part");
    let fsync = sincronizou_no_mesmo(&chamadas, &parcial).unwrap_or_else(|e| panic!("{e}"));
    let renomeou = chamadas
        .iter()
        .position(|c| matches!(c, Chamada::Renomeou(de, para) if *de == parcial && *para == final_))
        .expect("a premissa: o .part foi renomeado");
    assert!(fsync < renomeou, "o rename veio antes do fsync do .part");
}

/// **O ZIP sincroniza o DIRETORIO depois do `rename` do `.part` -- e a mae de
/// cada pasta que a corrida criou** (pedido 524, o que sobrava: o `fsync` do
/// diretorio).
///
/// O `rename` da troca e a entrada da pasta nova sao dado de DIRETORIO: sem
/// `fsync` deles, uma queda logo depois de «concluido» podia devolver a
/// pasta sem o `.zip`, ou sem a pasta. A pergunta ao nucleo e para onde
/// aponta o descritor que recebeu o `fsync`, como no teste da pasta (593).
#[test]
fn o_zip_sincroniza_o_diretorio_depois_do_rename() {
    if let Some(pasta) = std::env::var_os(FILHO) {
        let pasta = PathBuf::from(pasta);
        let raiz = pasta.parent().unwrap().parent().unwrap().join("dados");
        let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", QUANDO).unwrap();
        backup::finalizar_zip(&zip).unwrap();
        return;
    }
    let d = DirTemp::novo("524-strace-zip-dir");
    raiz_com_dado(&d);
    // Duas pastas novas em cadeia: `novo` (mae: d) e `zips` (mae: `novo`).
    let pasta = d.join("novo").join("zips");
    let Some(chamadas) = sob_strace("o_zip_sincroniza_o_diretorio_depois_do_rename", &pasta, &d)
    else {
        return;
    };
    let renomeou = chamadas
        .iter()
        .position(|c| matches!(c, Chamada::Renomeou(_, para) if para.ends_with(".zip")))
        .expect("a premissa: o .part foi renomeado");
    let sincronizou = |alvo: &Path, de: usize| {
        let alvo = alvo.to_string_lossy().into_owned();
        (de..chamadas.len()).any(|i| {
            matches!(&chamadas[i], Chamada::Sincronizou(f)
                if aponta_para(&chamadas[..i], *f) == Some(alvo.as_str()))
        })
    };
    assert!(
        sincronizou(&pasta, renomeou),
        "o rename do .part ficou sem o fsync da pasta do zip:\n{:#?}",
        &chamadas[renomeou..]
    );
    assert!(
        sincronizou(&d.join("novo"), 0),
        "a entrada de `zips` ficou sem o fsync da mae (`novo`)"
    );
    assert!(
        sincronizou(&d, 0),
        "a entrada de `novo` ficou sem o fsync da mae (a pasta de cima)"
    );
}

/// O irmao em arvore do teste acima: o destino nasce em cadeia (`novo/copia`)
/// e a entrada de cada pasta nova sincroniza na mae dela (579/593 ja faziam;
/// aqui a prova e contra o nucleo, nos dois caminhos do 524).
#[test]
fn a_arvore_sincroniza_a_mae_de_cada_pasta_que_criou() {
    if let Some(destino) = std::env::var_os(FILHO) {
        let destino = PathBuf::from(destino);
        let raiz = destino.parent().unwrap().parent().unwrap().join("dados");
        let (r, copias) = backup::executar(&raiz, &destino, QUANDO).unwrap();
        backup::concluir(&destino, QUANDO, &r, &copias).unwrap();
        return;
    }
    let d = DirTemp::novo("524-strace-arvore-dir");
    raiz_com_dado(&d);
    let destino = d.join("novo").join("copia");
    let Some(chamadas) = sob_strace(
        "a_arvore_sincroniza_a_mae_de_cada_pasta_que_criou",
        &destino,
        &d,
    ) else {
        return;
    };
    for mae in [d.join("novo"), d.to_path_buf()] {
        let alvo = mae.to_string_lossy().into_owned();
        let achou = (0..chamadas.len()).any(|i| {
            matches!(&chamadas[i], Chamada::Sincronizou(f)
                if aponta_para(&chamadas[..i], *f) == Some(alvo.as_str()))
        });
        assert!(achou, "a pasta {alvo} ficou sem o fsync da entrada nova");
    }
}

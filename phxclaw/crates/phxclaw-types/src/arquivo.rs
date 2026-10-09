//! Escrita em disco que sobrevive a queda: troca atomica, trava entre processos e a
//! cauda cortada de um arquivo de linhas.
//!
//! Mora aqui, no crate mais baixo da base, porque sete lugares (gonogo, config.json,
//! task.json, agenda, cursor dos canais, memoria e indice BM25) faziam «temporario +
//! rename» cada um do seu jeito, em tres niveis de garantia: uns sem `fsync`, uns com
//! temporario de nome fixo (dois processos escrevendo no mesmo `.tmp` corrompem um ao
//! outro), um so com `fsync` da pasta. A decisao de como se grava e UMA, e vive aqui.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// O temporario ao lado do destino, com o pid no nome: dois processos gravando o mesmo
/// arquivo nunca disputam o mesmo temporario, e o `rename` na mesma pasta e atomico.
fn temporario_de(arq: &Path) -> PathBuf {
    let pasta = arq.parent().unwrap_or(Path::new("."));
    pasta.join(format!(
        ".{}.{}.tmp",
        arq.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ))
}

/// Cria `pasta` e os pais que faltam, com o `fsync` do pai de CADA pasta criada. O
/// `create_dir_all` sozinho deixa a entrada da pasta nova so na memoria do sistema: o
/// `fsync` da pasta de destino (o que `gravar_atomico` faz) segura o arquivo dentro dela,
/// mas nao a propria pasta dentro do pai -- e a energia que cai leva `binarios/` inteira,
/// com o arquivo «gravado» junto. A decisao de como se cria pasta duravel e UMA, e vive
/// aqui, ao lado da de como se grava arquivo.
pub fn criar_pastas(pasta: &Path) -> io::Result<()> {
    if pasta.as_os_str().is_empty() || pasta.is_dir() {
        return Ok(());
    }
    // Do mais alto que falta para baixo: o pai de cada uma ja existe quando ela nasce.
    let mut faltam = Vec::new();
    let mut atual = Some(pasta);
    while let Some(p) = atual.filter(|p| !p.as_os_str().is_empty() && !p.is_dir()) {
        faltam.push(p);
        atual = p.parent();
    }
    for p in faltam.into_iter().rev() {
        match std::fs::create_dir(p) {
            Ok(()) => {}
            // Outro processo criou no meio: a pasta existe, e o `fsync` do pai vale igual.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && p.is_dir() => {}
            Err(e) => return Err(e),
        }
        let pai = p
            .parent()
            .filter(|x| !x.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        File::open(pai)?.sync_all()?;
    }
    Ok(())
}

/// Troca atomica: temporario por pid, `fsync` do temporario, `rename`, `fsync` da pasta.
/// Sem o `fsync` da pasta a renomeacao pode nao estar no disco quando a energia cai, e o
/// leitor acha o arquivo antigo -- ou nenhum, se era a primeira gravacao.
pub fn gravar_atomico(arq: &Path, conteudo: &[u8]) -> io::Result<()> {
    let pasta = arq.parent().unwrap_or(Path::new("."));
    criar_pastas(pasta)?;
    let tmp = temporario_de(arq);
    let escrever = || -> io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(conteudo)?;
        f.sync_all()?;
        std::fs::rename(&tmp, arq)?;
        File::open(if pasta.as_os_str().is_empty() {
            Path::new(".")
        } else {
            pasta
        })?
        .sync_all()
    };
    escrever().inspect_err(|_| {
        // O temporario de uma gravacao que falhou nao pode ficar: a proxima do mesmo pid
        // o sobrescreveria, e o de outro pid nunca seria limpo.
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Trava exclusiva entre processos sobre `arq` (criado se nao existe), solta quando o
/// `File` devolvido fecha. A trava e sobre um arquivo PROPRIO, nunca sobre o que se troca
/// por `rename`: trava no inode velho nao segura quem abre o novo.
pub fn travar(arq: &Path) -> io::Result<File> {
    if let Some(p) = arq.parent() {
        criar_pastas(p)?;
    }
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(arq)?;
    f.lock()?;
    Ok(f)
}

/// O arquivo de trava ao lado de `arq` (`.nome.lock`).
pub fn trava_de(arq: &Path) -> PathBuf {
    let pasta = arq.parent().unwrap_or(Path::new("."));
    pasta.join(format!(
        ".{}.lock",
        arq.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// Quantos bytes do inicio terminam na ultima quebra de linha: o que vem depois e uma
/// linha que a queda cortou no meio.
pub fn ate_a_ultima_linha(bytes: &[u8]) -> usize {
    bytes.iter().rposition(|b| *b == b'\n').map_or(0, |p| p + 1)
}

/// Le um arquivo de linhas e APAGA do disco a linha cortada no fim, se houver, devolvendo
/// so as linhas inteiras. Deixar a cauda la faria a proxima gravacao nascer colada nela:
/// duas linhas viram uma, ilegivel, e a mensagem que a segunda carregava se perde.
/// Arquivo ausente e vazio.
pub fn aparar_cauda_cortada(arq: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = match std::fs::read(arq) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let fim = ate_a_ultima_linha(&bytes);
    if fim < bytes.len() {
        let f = OpenOptions::new().write(true).open(arq)?;
        f.set_len(fim as u64)?;
        f.sync_data()?;
        bytes.truncate(fim);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phx-arquivo-{}-{}", nome, uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn gravar_atomico_troca_inteiro_e_nao_deixa_temporario() {
        let d = tmp("atomico");
        let a = d.join("x.json");
        gravar_atomico(&a, b"um").unwrap();
        gravar_atomico(&a, b"dois").unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), b"dois");
        let sobras: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "x.json")
            .collect();
        assert!(sobras.is_empty(), "{sobras:?}");
        // Falha no meio (o destino e uma pasta) limpa o temporario.
        let pasta = d.join("sou-pasta");
        std::fs::create_dir(&pasta).unwrap();
        assert!(gravar_atomico(&pasta, b"x").is_err());
        assert!(!temporario_de(&pasta).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Os pais que faltam nascem um a um (e cada um com o `fsync` do pai, que teste nenhum
    /// enxerga sem desligar a energia); o que ja existe nao e erro, e um ARQUIVO no meio do
    /// caminho e erro com o motivo, nao pasta criada por cima.
    #[test]
    fn criar_pastas_cria_os_pais_que_faltam() {
        let d = tmp("pastas");
        let funda = d.join("a").join("b").join("c");
        criar_pastas(&funda).unwrap();
        assert!(funda.is_dir());
        criar_pastas(&funda).unwrap();
        gravar_atomico(&d.join("x").join("y").join("z.json"), b"1").unwrap();
        assert_eq!(std::fs::read(d.join("x/y/z.json")).unwrap(), b"1");
        std::fs::write(d.join("arq"), b"").unwrap();
        assert!(criar_pastas(&d.join("arq").join("dentro")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn aparar_cauda_cortada_apaga_so_a_linha_pela_metade() {
        let d = tmp("cauda");
        let a = d.join("l.jsonl");
        std::fs::write(&a, b"{\"a\":1}\n{\"b\":2}\n{\"c\":").unwrap();
        assert_eq!(aparar_cauda_cortada(&a).unwrap(), b"{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(std::fs::read(&a).unwrap(), b"{\"a\":1}\n{\"b\":2}\n");
        // Inteiro: nada muda. Sem quebra nenhuma: vira vazio. Ausente: vazio.
        assert_eq!(aparar_cauda_cortada(&a).unwrap().len(), 16);
        std::fs::write(&a, b"{\"so-meia").unwrap();
        assert!(aparar_cauda_cortada(&a).unwrap().is_empty());
        assert_eq!(std::fs::metadata(&a).unwrap().len(), 0);
        assert!(
            aparar_cauda_cortada(&d.join("nao-existe"))
                .unwrap()
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn travar_exclui_o_segundo_ate_o_primeiro_soltar() {
        let d = tmp("trava");
        let t = trava_de(&d.join("config.json"));
        let primeira = travar(&t).unwrap();
        let t2 = t.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let fio = std::thread::spawn(move || {
            let _g = travar(&t2).unwrap();
            tx.send(std::time::Instant::now()).unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(150));
        let solta = std::time::Instant::now();
        drop(primeira);
        let pegou = rx.recv().unwrap();
        fio.join().unwrap();
        assert!(
            pegou >= solta,
            "a segunda trava entrou antes de a primeira soltar"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}

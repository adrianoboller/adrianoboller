//! O leitor de traco do `strace -f -y`, compartilhado pelos medidores que
//! contam chamadas de sistema POR ARQUIVO.
//!
//! Mora em `examples/apoio/` -- fora da descoberta automatica do cargo, que
//! so' enxerga `examples/*.rs` e `examples/*/main.rs` -- e entra nos exemplos
//! por `#[path]`. Duas copias do mesmo parser e' onde elas comecam a
//! divergir, e este ja tem uma licao paga: o `strace -f` parte uma chamada
//! concorrente em `<unfinished ...>` e `<... resumed>`, e um leitor que exige
//! caminho e resultado na mesma linha perde um terco em silencio
//! (`docs/cognicao/cognicao_strace-parte-syscall-concorrente-e-o-parser-perde-um-terco_20260905_1120.md`).
//! Aqui o CAMINHO decide: a linha `unfinished` o traz, e conta; a `resumed`
//! nao o traz, e nao conta -- nunca as duas.

use std::path::Path;

/// Roda `eu args...` num processo filho sob `strace -f -y -e trace=<syscalls>`
/// e grava o traco em `log`. `None` quando esta maquina nao tem `strace`:
/// chamada de sistema que aconteceu ou nao e' fato do nucleo, e nao ha
/// substituto de teste unitario para ela.
pub fn tracar(eu: &Path, args: &[String], syscalls: &str, log: &Path) -> Option<()> {
    std::process::Command::new("strace")
        .arg("-V")
        .output()
        .ok()?;
    let saida = std::process::Command::new("strace")
        .args(["-f", "-y", "-e", &format!("trace={syscalls}"), "-o"])
        .arg(log)
        .arg(eu)
        .args(args)
        .output()
        .ok()?;
    if !saida.status.success() {
        eprintln!(
            "a sonda tracada falhou: {}",
            String::from_utf8_lossy(&saida.stderr)
        );
        return None;
    }
    Some(())
}

/// As chamadas do traco, como `(syscall, alvo)`.
///
/// O alvo e' o CAMINHO: o argumento entre aspas quando ele vem antes (o
/// `openat` e o `statx` nomeiam o arquivo pelo nome), ou a decoracao `<...>`
/// que o `-y` pendura no descritor (o `write`, o `fsync`, o `getdents64`).
/// Vale o que aparece PRIMEIRO na linha: no `write(5</x.reg>, "\0\0...", 128)`
/// a primeira aspa e' o dado, e o dado nao e' alvo de nada.
pub fn chamadas(log: &Path) -> Vec<(String, String)> {
    let texto = std::fs::read_to_string(log).unwrap_or_default();
    texto.lines().filter_map(chamada).collect()
}

fn chamada(linha: &str) -> Option<(String, String)> {
    let l = linha.trim_start();
    // Com `-f` cada linha comeca pelo pid.
    let l = l
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start();
    if l.starts_with("<...") || l.starts_with("+++") || l.starts_with("---") {
        return None;
    }
    let abre = l.find('(')?;
    let nome = &l[..abre];
    if nome.is_empty() || !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let resto = &l[abre + 1..];
    let aspa = resto.find('"');
    // O `-y` tambem decora o `AT_FDCWD` do `openat` e do `statx` com o
    // diretorio de trabalho -- e essa decoracao vem ANTES da aspa. Ela nao e'
    // o alvo de nada: a primeira versao deste leitor atribuia todo `openat`
    // ao diretorio corrente por causa dela (contagem certa, arquivo errado).
    let dec = resto
        .find('<')
        .filter(|a| !resto[..*a].ends_with("AT_FDCWD"));
    let alvo = match (aspa, dec) {
        (Some(q), Some(a)) if q < a => entre(&resto[q + 1..], '"'),
        (Some(q), None) => entre(&resto[q + 1..], '"'),
        (_, Some(a)) => entre(&resto[a + 1..], '>'),
        (None, None) => String::new(),
    };
    Some((nome.to_string(), alvo))
}

fn entre(s: &str, fim: char) -> String {
    match s.find(fim) {
        Some(f) => s[..f].to_string(),
        None => s.to_string(),
    }
}

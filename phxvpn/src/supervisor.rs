//! Sobe e vigia um processo `openvpn` por rede.
//!
//! Opcional (`--openvpn`): sem ele o painel so escreve os arquivos e quem
//! administra sobe o OpenVPN como quiser (systemd, servico do Windows). Com
//! ele, rede criada ja entra no ar. Entrar e sair de rede NAO reinicia nada:
//! o OpenVPN rele o `ccd/` a cada conexao.
//!
//! # Parar e com aviso
//!
//! No Unix, SIGTERM e prazo (`PRAZO_PARA_SAIR`) antes do SIGKILL. Com o
//! `explicit-exit-notify` do `servidor.conf` (so UDP, ver `ovpn.rs`), o
//! OpenVPN manda `RESTART` a cada membro e sai 2 s depois; o membro religa na
//! hora em vez de esperar o `ping-restart 60`. O SIGKILL de antes nao deixava
//! o OpenVPN dizer nada.
//!
//! No Windows fica o `TerminateProcess` de antes. O equivalente do SIGTERM la
//! e o `--service <evento> 0` (windows-options.rst:182-194): o OpenVPN espera
//! um evento com nome e sai limpo quando ele dispara. Nao entrou porque nao ha
//! como prova-lo aqui (sem Windows real), e o erro possivel -- a opcao mal
//! formada -- impediria o OpenVPN de SUBIR, o que e pior que os 61 s de hoje.
//!
//! # O log e nosso, e tem teto
//!
//! O OpenVPN escrevia direto num `openvpn.log` em append, para sempre. Girar
//! o arquivo por fora nao serve: o `--log` segura o descritor ate o processo
//! morrer (log-options.rst: «persistent over the entire course of an OpenVPN
//! instantiation»), entao renomear deixa ele escrevendo no velho, e copiar e
//! truncar perde o que chega entre as duas coisas. Aqui a saida vem por
//! PIPE, e quem escreve o arquivo e o supervisor: gira entre uma linha e a
//! outra (nenhuma se perde nem se parte), fecha o velho antes de renomear, e
//! o OpenVPN nunca teve o arquivo aberto. O custo e uma thread de leitura por
//! saida, e ela nunca para de drenar: pipe cheio pararia o OpenVPN.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Teto de cada arquivo de log do OpenVPN.
pub const TETO_LOG: u64 = 10 * 1024 * 1024;
/// Quantos arquivos girados se guardam (`openvpn.log.1` ... `.3`).
pub const GUARDAR_LOG: u32 = 3;
/// Quanto o OpenVPN tem para sair depois do SIGTERM. Ele precisa de 2 s
/// (manda o `RESTART` e agenda a saida); o resto e folga de maquina lenta.
pub const PRAZO_PARA_SAIR: Duration = Duration::from_secs(10);
/// Linha sem quebra maior que isto sai partida: sem teto, uma saida sem
/// `\n` cresceria a memoria do painel sem fim.
const LINHA_MAX: u64 = 64 * 1024;

pub struct Supervisor {
    binario: PathBuf,
    filhos: Mutex<HashMap<PathBuf, Child>>,
    /// Um registro por rede, que sobrevive ao filho: o OpenVPN que caiu e o
    /// que sobe no lugar dele escrevem pelo MESMO, entao nao ha dois donos
    /// girando o mesmo arquivo.
    registros: Mutex<HashMap<PathBuf, Arc<Mutex<Registro>>>>,
    teto_log: u64,
}

impl Supervisor {
    /// Acha o `openvpn` no PATH. Nao achar e erro dito, nao silencio: o
    /// administrador pediu `--openvpn` esperando a VPN no ar.
    ///
    /// `PHXVPN_OVPN_LOG_TETO` (bytes) troca o teto do log -- existe para a
    /// prova girar o arquivo em minutos em vez de dias.
    pub fn novo() -> Result<Supervisor, String> {
        let binario = achar_no_path("openvpn").ok_or(
            "--openvpn pedido, mas o binario openvpn nao esta no PATH (instale o OpenVPN 2.6+)",
        )?;
        let teto_log = match std::env::var("PHXVPN_OVPN_LOG_TETO") {
            Ok(t) => t
                .parse::<u64>()
                .ok()
                .filter(|t| *t >= 1024)
                .ok_or("PHXVPN_OVPN_LOG_TETO: inteiro de bytes, no minimo 1024")?,
            Err(_) => TETO_LOG,
        };
        Ok(Supervisor {
            binario,
            filhos: Mutex::new(HashMap::new()),
            registros: Mutex::new(HashMap::new()),
            teto_log,
        })
    }

    pub fn garantir(&self, nome: &str, dir: &Path) -> Result<(), String> {
        let mut filhos = self.filhos.lock().expect("filhos");
        if let Some(f) = filhos.get_mut(dir) {
            if matches!(f.try_wait(), Ok(None)) {
                return Ok(());
            }
            eprintln!("phxvpn: o OpenVPN da rede «{nome}» parou; subindo de novo");
        }
        let registro = self.registro(dir)?;
        let mut filho = Command::new(&self.binario)
            .arg("--config")
            .arg(dir.join("servidor.conf"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("subir openvpn da rede «{nome}»: {e}"))?;
        if let Some(s) = filho.stdout.take() {
            drenar(s, Arc::clone(&registro));
        }
        if let Some(s) = filho.stderr.take() {
            drenar(s, registro);
        }
        eprintln!(
            "phxvpn: OpenVPN da rede «{nome}» no ar (pid {})",
            filho.id()
        );
        filhos.insert(dir.to_path_buf(), filho);
        Ok(())
    }

    fn registro(&self, dir: &Path) -> Result<Arc<Mutex<Registro>>, String> {
        let mut r = self.registros.lock().expect("registros");
        if let Some(x) = r.get(dir) {
            return Ok(Arc::clone(x));
        }
        let novo = Registro::abrir(&dir.join("openvpn.log"), self.teto_log, GUARDAR_LOG)
            .map_err(|e| format!("abrir log: {e}"))?;
        let novo = Arc::new(Mutex::new(novo));
        r.insert(dir.to_path_buf(), Arc::clone(&novo));
        Ok(novo)
    }
}

impl Supervisor {
    /// Para e sobe de novo: a configuracao mudou (ligar/desligar o
    /// autenticador da rede), e o OpenVPN so le o `servidor.conf` ao subir.
    ///
    /// A trava fica segura durante a parada (ate `PRAZO_PARA_SAIR`, na pratica
    /// os 2 s do OpenVPN): soltar antes deixaria outro `garantir` subir um
    /// segundo OpenVPN na porta que o primeiro ainda nao largou.
    pub fn reiniciar(&self, nome: &str, dir: &Path) -> Result<(), String> {
        {
            let mut filhos = self.filhos.lock().expect("filhos");
            if let Some(f) = filhos.remove(dir) {
                parar(vec![f]);
            }
        }
        self.garantir(nome, dir)
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Ok(mut f) = self.filhos.lock() {
            parar(f.drain().map(|(_, c)| c).collect());
        }
    }
}

/// Pede a saida a todos de uma vez e espera um prazo so: parar dez redes
/// custa os mesmos 2 s que parar uma.
fn parar(mut filhos: Vec<Child>) {
    for f in &filhos {
        pedir_saida(f);
    }
    let fim = Instant::now() + PRAZO_PARA_SAIR;
    while Instant::now() < fim && filhos.iter_mut().any(|f| matches!(f.try_wait(), Ok(None))) {
        std::thread::sleep(Duration::from_millis(50));
    }
    for f in &mut filhos {
        if matches!(f.try_wait(), Ok(None)) {
            let _ = f.kill();
        }
        let _ = f.wait();
    }
}

#[cfg(unix)]
fn pedir_saida(f: &Child) {
    extern "C" {
        fn kill(pid: i32, sinal: i32) -> i32;
    }
    const SIGTERM: i32 = 15;
    // O pid e de um filho nosso ainda nao colhido (o `wait` vem depois), entao
    // nao pode ter sido reciclado para outro processo.
    if let Ok(pid) = i32::try_from(f.id()) {
        // SAFETY: kill(2) so recebe dois inteiros; o pior caso e ESRCH.
        unsafe {
            kill(pid, SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn pedir_saida(_f: &Child) {
    // Sem SIGTERM no Windows: o `kill` do fim do prazo e o TerminateProcess
    // de sempre. Ver o topo do arquivo.
}

/// Le a saida do OpenVPN linha a linha e a entrega ao registro. Nunca para
/// de ler enquanto o pipe estiver aberto: se o disco falhar, a linha se
/// perde, mas o OpenVPN nao trava esperando o pipe esvaziar.
fn drenar<L: Read + Send + 'static>(saida: L, registro: Arc<Mutex<Registro>>) {
    std::thread::spawn(move || {
        let mut leitor = BufReader::new(saida);
        let mut linha = Vec::new();
        let mut avisou = false;
        loop {
            linha.clear();
            match (&mut leitor).take(LINHA_MAX).read_until(b'\n', &mut linha) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let r = registro.lock().expect("registro").escrever(&linha);
            if let (Err(e), false) = (r, avisou) {
                eprintln!("phxvpn: log do OpenVPN: {e} (linhas perdidas ate voltar)");
                avisou = true;
            }
        }
    });
}

/// Um arquivo de log com teto, girado por quem escreve.
pub struct Registro {
    caminho: PathBuf,
    teto: u64,
    guardar: u32,
    /// `None` so no meio do giro: o velho e fechado ANTES de renomear (no
    /// Windows, renomear arquivo aberto pode falhar).
    arquivo: Option<File>,
    tamanho: u64,
}

impl Registro {
    pub fn abrir(caminho: &Path, teto: u64, guardar: u32) -> std::io::Result<Registro> {
        let arquivo = OpenOptions::new().create(true).append(true).open(caminho)?;
        let tamanho = arquivo.metadata()?.len();
        Ok(Registro {
            caminho: caminho.to_path_buf(),
            teto,
            guardar,
            arquivo: Some(arquivo),
            tamanho,
        })
    }

    /// Escreve uma linha inteira. Gira ANTES quando ela passaria do teto, e
    /// so entre linhas: nenhuma linha fica partida entre dois arquivos.
    pub fn escrever(&mut self, linha: &[u8]) -> std::io::Result<()> {
        if self.tamanho > 0 && self.tamanho + linha.len() as u64 > self.teto {
            self.girar()?;
        }
        if self.arquivo.is_none() {
            // Um giro anterior falhou no meio: tenta abrir de novo.
            self.arquivo = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.caminho)?,
            );
        }
        self.arquivo
            .as_mut()
            .expect("aberto logo acima")
            .write_all(linha)?;
        self.tamanho += linha.len() as u64;
        Ok(())
    }

    fn girado(&self, n: u32) -> PathBuf {
        let mut c = self.caminho.clone().into_os_string();
        c.push(format!(".{n}"));
        PathBuf::from(c)
    }

    fn girar(&mut self) -> std::io::Result<()> {
        self.arquivo = None;
        // O mais velho sai por cima: `rename` substitui o destino.
        for n in (1..self.guardar).rev() {
            match std::fs::rename(self.girado(n), self.girado(n + 1)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        if self.guardar > 0 {
            std::fs::rename(&self.caminho, self.girado(1))?;
        }
        self.arquivo = Some(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.caminho)?,
        );
        self.tamanho = 0;
        Ok(())
    }
}

pub fn achar_no_path(nome: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        [nome.to_string(), format!("{nome}.exe")]
            .iter()
            .map(|n| d.join(n))
            .find(|c| c.is_file())
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phxvpn-registro-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 2.000 linhas numeradas num teto de 4 KiB guardando 3: o que sobra e
    /// o fim da sequencia, SEM buraco nem linha partida, e nenhum arquivo
    /// passa do teto. Com guardar grande, sobram todas.
    #[test]
    fn gira_entre_linhas_sem_perder_nem_partir() {
        for (guardar, todas) in [(3, false), (200, true)] {
            let d = pasta(&format!("g{guardar}"));
            let c = d.join("openvpn.log");
            let mut r = Registro::abrir(&c, 4096, guardar).unwrap();
            for i in 0..2000 {
                r.escrever(format!("linha {i:05} do openvpn\n").as_bytes())
                    .unwrap();
            }
            let mut lidas = Vec::new();
            for n in (1..=guardar).rev() {
                if let Ok(t) = std::fs::read_to_string(r.girado(n)) {
                    assert!(t.len() as u64 <= 4096, "{n}: {} bytes", t.len());
                    lidas.extend(t.lines().map(str::to_string));
                }
            }
            assert!(!r.girado(guardar + 1).exists(), "guardou demais");
            lidas.extend(
                std::fs::read_to_string(&c)
                    .unwrap()
                    .lines()
                    .map(str::to_string),
            );
            let numeros: Vec<u32> = lidas
                .iter()
                .map(|l| {
                    assert!(
                        l.starts_with("linha ") && l.ends_with(" do openvpn"),
                        "partida: {l}"
                    );
                    l[6..11].parse().unwrap()
                })
                .collect();
            assert_eq!(*numeros.last().unwrap(), 1999);
            for w in numeros.windows(2) {
                assert_eq!(w[1], w[0] + 1, "buraco entre {} e {}", w[0], w[1]);
            }
            if todas {
                assert_eq!(numeros[0], 0, "com espaco, nenhuma linha some");
            } else {
                // 3 girados + o atual, ~150 linhas por arquivo de 4 KiB.
                assert!(
                    numeros.len() < 2000 && numeros.len() > 400,
                    "{}",
                    numeros.len()
                );
            }
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    /// O registro continua o arquivo que ja existe (painel reiniciado) e o
    /// conta no teto: senao o primeiro arquivo depois do reinicio passaria.
    #[test]
    fn continua_o_arquivo_que_ja_existe_contando_o_tamanho() {
        let d = pasta("continua");
        let c = d.join("openvpn.log");
        std::fs::write(&c, vec![b'x'; 4000]).unwrap();
        let mut r = Registro::abrir(&c, 4096, 3).unwrap();
        r.escrever(&[b'y'; 200]).unwrap();
        assert_eq!(std::fs::metadata(r.girado(1)).unwrap().len(), 4000);
        assert_eq!(std::fs::read(&c).unwrap(), vec![b'y'; 200]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Pelo pipe de verdade: um processo filho escreve 3.000 linhas e sai; o
    /// dreno entrega todas, em ordem, girando.
    #[cfg(unix)]
    #[test]
    fn dreno_do_pipe_entrega_todas_as_linhas_de_um_processo() {
        let d = pasta("pipe");
        let c = d.join("openvpn.log");
        let r = Arc::new(Mutex::new(Registro::abrir(&c, 8192, 200).unwrap()));
        let mut filho = Command::new("sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 3000 ]; do echo \"linha $i\"; i=$((i+1)); done")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        drenar(filho.stdout.take().unwrap(), Arc::clone(&r));
        filho.wait().unwrap();
        let fim = Instant::now() + Duration::from_secs(10);
        let ler = || {
            let reg = r.lock().unwrap();
            let mut t = String::new();
            for n in (1..=200).rev() {
                t.push_str(&std::fs::read_to_string(reg.girado(n)).unwrap_or_default());
            }
            t + &std::fs::read_to_string(&c).unwrap()
        };
        while ler().lines().count() < 3000 && Instant::now() < fim {
            std::thread::sleep(Duration::from_millis(20));
        }
        let linhas: Vec<String> = ler().lines().map(str::to_string).collect();
        assert_eq!(linhas.len(), 3000);
        for (i, l) in linhas.iter().enumerate() {
            assert_eq!(l, &format!("linha {i}"));
        }
        assert!(r.lock().unwrap().girado(1).exists(), "nao girou");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// SIGTERM antes do SIGKILL: um filho que trata o TERM sai sozinho e
    /// deixa o recado; um que o ignora e morto no fim do prazo.
    #[cfg(unix)]
    #[test]
    fn parar_pede_com_sigterm_antes_de_matar() {
        let d = pasta("parar");
        let marca = d.join("saiu-com-term");
        let educado = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "trap 'touch {}; exit 0' TERM; while :; do sleep 0.05; done",
                marca.display()
            ))
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let t0 = Instant::now();
        parar(vec![educado]);
        assert!(marca.exists(), "o filho nao recebeu SIGTERM");
        assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
        let _ = std::fs::remove_dir_all(&d);
    }
}

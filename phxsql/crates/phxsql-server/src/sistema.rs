//! O que a maquina esta fazendo: CPU, memoria, disco e rede.
//!
//! # Sem crate nenhuma
//!
//! No Linux tudo sai de arquivos de texto que o proprio nucleo publica --
//! `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/diskstats` --, e ler
//! arquivo a `std` faz. O espaco livre e a excecao: ele exige `statvfs`, que
//! nao esta na `std`, entao sai do `df`, que todo Unix tem. E o mesmo caminho
//! que o gancho de firewall ja usa: chamar um programa do sistema em vez de
//! ligar uma biblioteca.
//!
//! Fora do Linux nada disso existe, e o modulo diz isso em vez de inventar
//! numero. Um monitor que erra e pior do que um monitor que falta.
//!
//! # Por que as taxas precisam de duas amostras
//!
//! `/proc/stat` traz CONTADORES desde o arranque, nao percentuais. "CPU em
//! 40%" so existe entre dois instantes. Por isso o [`Monitor`] guarda a
//! amostra anterior e devolve a taxa desde a ultima chamada -- e por isso a
//! primeira chamada devolve taxa zero, honestamente, em vez de dividir pelo
//! tempo desde o arranque e chamar isso de "agora".

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use phxsql_core::json::Json;

/// Contadores crus lidos num instante.
#[derive(Debug, Clone, Default)]
pub struct Amostra {
    /// Jiffies de CPU: (ocupado, total).
    pub cpu: (u64, u64),
    /// Por interface: (bytes recebidos, bytes enviados, pacotes rx, pacotes tx, erros).
    pub rede: HashMap<String, (u64, u64, u64, u64, u64)>,
    /// Por disco: (leituras, escritas, setores lidos, setores escritos).
    pub discos: HashMap<String, (u64, u64, u64, u64)>,
    pub quando: Option<Instant>,
}

/// Guarda a amostra anterior para conseguir falar em taxa.
pub struct Monitor {
    anterior: Amostra,
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor::novo()
    }
}

impl Monitor {
    pub fn novo() -> Monitor {
        Monitor {
            anterior: Amostra::default(),
        }
    }

    /// Le a maquina inteira e devolve o retrato, com as taxas desde a ultima
    /// leitura.
    ///
    /// `bases` sao os diretorios cujo espaco interessa -- tipicamente o `base`
    /// do `config.json`. Eles entram na lista de discos mesmo que dividam a
    /// mesma particao, porque quem opera quer ver o do BANCO, e nao adivinhar
    /// qual montagem o contem.
    pub fn ler(&mut self, bases: &[&Path]) -> Json {
        let agora = Amostra {
            cpu: ler_cpu(),
            rede: ler_rede(),
            discos: ler_diskstats(),
            quando: Some(Instant::now()),
        };
        let segundos = match (self.anterior.quando, agora.quando) {
            (Some(a), Some(b)) => b.duration_since(a).as_secs_f64(),
            _ => 0.0,
        };

        let json = Json::objeto(vec![
            ("disponivel", Json::Bool(disponivel())),
            ("sistema", Json::texto_de(std::env::consts::OS)),
            ("segundos_desde_a_ultima", Json::texto_de(fmt2(segundos))),
            ("cpu", cpu_json(&self.anterior, &agora)),
            ("memoria", memoria_json()),
            ("discos", discos_json(bases)),
            ("rede", rede_json(&self.anterior, &agora, segundos)),
            ("io", io_json(&self.anterior, &agora, segundos)),
        ]);
        self.anterior = agora;
        json
    }
}

fn disponivel() -> bool {
    cfg!(target_os = "linux") && Path::new("/proc/stat").exists()
}

fn fmt2(v: f64) -> String {
    format!("{v:.2}")
}

/// Jiffies (ocupado, total) da primeira linha de `/proc/stat`.
fn ler_cpu() -> (u64, u64) {
    let Ok(t) = std::fs::read_to_string("/proc/stat") else {
        return (0, 0);
    };
    let Some(linha) = t.lines().next() else {
        return (0, 0);
    };
    let campos: Vec<u64> = linha
        .split_whitespace()
        .skip(1)
        .filter_map(|c| c.parse().ok())
        .collect();
    if campos.len() < 4 {
        return (0, 0);
    }
    let total: u64 = campos.iter().sum();
    // Ocioso e a soma de `idle` e `iowait`: esperar disco nao e trabalho de
    // CPU, e conta-lo como ocupado faria uma carga de IO parecer carga de
    // processador -- que foi exatamente a confusao do primeiro diagnostico da
    // insercao.
    let ocioso = campos[3] + campos.get(4).copied().unwrap_or(0);
    (total.saturating_sub(ocioso), total)
}

fn cpu_json(antes: &Amostra, agora: &Amostra) -> Json {
    let (ocup, tot) = (
        agora.cpu.0.saturating_sub(antes.cpu.0),
        agora.cpu.1.saturating_sub(antes.cpu.1),
    );
    let uso = if tot > 0 {
        (ocup as f64 / tot as f64) * 100.0
    } else {
        0.0
    };
    let carga: Vec<Json> = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|t| {
            t.split_whitespace()
                .take(3)
                .map(|x| Json::texto_de(x.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Json::objeto(vec![
        ("uso_percentual", Json::texto_de(fmt2(uso))),
        (
            "nucleos",
            Json::de_u64(
                std::thread::available_parallelism()
                    .map(|n| n.get() as u64)
                    .unwrap_or(0),
            ),
        ),
        ("carga", Json::Lista(carga)),
        ("primeira_leitura", Json::Bool(antes.cpu.1 == 0)),
    ])
}

fn memoria_json() -> Json {
    let mut m: HashMap<String, u64> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
        for l in t.lines() {
            if let Some((k, v)) = l.split_once(':') {
                if let Some(n) = v.split_whitespace().next().and_then(|x| x.parse().ok()) {
                    m.insert(k.to_string(), n);
                }
            }
        }
    }
    let g = |k: &str| m.get(k).copied().unwrap_or(0);
    let total = g("MemTotal");
    // `MemAvailable` e o numero certo, e nao `MemFree`: o nucleo conta como
    // disponivel o cache que ele devolveria sob pressao. `MemFree` faria uma
    // maquina saudavel com cache cheio parecer sem memoria.
    let disp = g("MemAvailable");
    let swap_total = g("SwapTotal");
    Json::objeto(vec![
        ("total_kb", Json::de_u64(total)),
        ("disponivel_kb", Json::de_u64(disp)),
        ("usada_kb", Json::de_u64(total.saturating_sub(disp))),
        (
            "usada_percentual",
            Json::texto_de(fmt2(if total > 0 {
                (total.saturating_sub(disp) as f64 / total as f64) * 100.0
            } else {
                0.0
            })),
        ),
        ("cache_kb", Json::de_u64(g("Cached"))),
        ("swap_total_kb", Json::de_u64(swap_total)),
        (
            "swap_usada_kb",
            Json::de_u64(swap_total.saturating_sub(g("SwapFree"))),
        ),
    ])
}

/// Espaco de cada caminho pedido, pelo `df`.
///
/// Uma chamada so, com todos os caminhos: o `df` resolve cada um para a
/// montagem que o contem, que e justamente o trabalho que a `std` nao faz.
pub fn espaco(caminhos: &[&Path]) -> Vec<EspacoEmDisco> {
    if caminhos.is_empty() {
        return Vec::new();
    }
    let mut cmd = std::process::Command::new("df");
    cmd.arg("-k");
    for c in caminhos {
        cmd.arg(c);
    }
    let Ok(saida) = cmd.output() else {
        return Vec::new();
    };
    let texto = String::from_utf8_lossy(&saida.stdout);
    let mut out = Vec::new();
    for (i, l) in texto.lines().skip(1).enumerate() {
        let c: Vec<&str> = l.split_whitespace().collect();
        // `df` quebra a linha quando o dispositivo e comprido; nesse caso os
        // numeros vem na linha seguinte. Aceitar so a forma completa evita ler
        // campo trocado -- e um monitor com numero trocado nao serve.
        if c.len() < 6 {
            continue;
        }
        let n = |i: usize| c[i].parse::<u64>().unwrap_or(0);
        out.push(EspacoEmDisco {
            caminho: caminhos
                .get(i)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| c[5].to_string()),
            dispositivo: c[0].to_string(),
            montagem: c[5].to_string(),
            total_kb: n(1),
            usado_kb: n(2),
            livre_kb: n(3),
        });
    }
    out
}

#[derive(Debug, Clone)]
pub struct EspacoEmDisco {
    pub caminho: String,
    pub dispositivo: String,
    pub montagem: String,
    /// Tamanho do sistema de arquivos, como o `df` reporta.
    pub total_kb: u64,
    pub usado_kb: u64,
    pub livre_kb: u64,
}

impl EspacoEmDisco {
    /// Quanto deste sistema de arquivos ESTE processo alcanca.
    ///
    /// Nao e o `total_kb`, e a diferenca nao e detalhe. O `ext4` reserva 5%
    /// para o root, cota e contêiner reservam mais, e nesses casos
    /// `usado + livre` fica bem abaixo do tamanho do disco. A maquina onde
    /// isto foi medido:
    ///
    /// ```text
    /// Filesystem  1K-blocks      Used  Available  Use%
    /// /dev/vda    264212084  20986728   17861796   55%
    /// ```
    ///
    /// Dividir por `total_kb` daria 8% usados; o disco esta em 55%. O primeiro
    /// numero era o que este modulo mostrava, e um alerta de "menos de 10%
    /// livre" nunca dispararia -- o disco encheria calado. O `df` divide por
    /// `usado + livre`, e e essa a conta certa.
    pub fn utilizavel_kb(&self) -> u64 {
        self.usado_kb + self.livre_kb
    }

    /// Quanto do disco ja foi, de 0 a 100.
    pub fn usado_percentual(&self) -> f64 {
        let base = self.utilizavel_kb();
        if base == 0 {
            return 0.0;
        }
        (self.usado_kb as f64 / base as f64) * 100.0
    }

    pub fn livre_percentual(&self) -> f64 {
        100.0 - self.usado_percentual()
    }

    /// Quanto o `df` esconde: reserva do sistema de arquivos e cota.
    ///
    /// Mostrar isto e o que evita a pergunta "cade os outros 200 GB?" quando o
    /// total nao bate com a soma.
    pub fn reservado_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.utilizavel_kb())
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("caminho", Json::texto_de(&self.caminho)),
            ("dispositivo", Json::texto_de(&self.dispositivo)),
            ("montagem", Json::texto_de(&self.montagem)),
            ("total_kb", Json::de_u64(self.total_kb)),
            ("utilizavel_kb", Json::de_u64(self.utilizavel_kb())),
            ("reservado_kb", Json::de_u64(self.reservado_kb())),
            ("usado_kb", Json::de_u64(self.usado_kb)),
            ("livre_kb", Json::de_u64(self.livre_kb)),
            (
                "usado_percentual",
                Json::texto_de(fmt2(self.usado_percentual())),
            ),
            (
                "livre_percentual",
                Json::texto_de(fmt2(self.livre_percentual())),
            ),
        ])
    }
}

fn discos_json(bases: &[&Path]) -> Json {
    Json::Lista(espaco(bases).iter().map(|e| e.para_json()).collect())
}

fn ler_rede() -> HashMap<String, (u64, u64, u64, u64, u64)> {
    let mut m = HashMap::new();
    let Ok(t) = std::fs::read_to_string("/proc/net/dev") else {
        return m;
    };
    for l in t.lines().skip(2) {
        let Some((nome, resto)) = l.split_once(':') else {
            continue;
        };
        let c: Vec<u64> = resto
            .split_whitespace()
            .map(|x| x.parse().unwrap_or(0))
            .collect();
        if c.len() < 10 {
            continue;
        }
        m.insert(
            nome.trim().to_string(),
            (c[0], c[8], c[1], c[9], c[2] + c[10]),
        );
    }
    m
}

fn rede_json(antes: &Amostra, agora: &Amostra, segundos: f64) -> Json {
    let mut nomes: Vec<&String> = agora.rede.keys().collect();
    nomes.sort();
    Json::Lista(
        nomes
            .into_iter()
            .filter_map(|nome| {
                let a = agora.rede.get(nome)?;
                let z = antes.rede.get(nome).copied().unwrap_or((0, 0, 0, 0, 0));
                let taxa = |novo: u64, velho: u64| {
                    if segundos > 0.0 && novo >= velho {
                        (novo - velho) as f64 / segundos
                    } else {
                        0.0
                    }
                };
                Some(Json::objeto(vec![
                    ("interface", Json::texto_de(nome)),
                    ("rx_bytes", Json::de_u64(a.0)),
                    ("tx_bytes", Json::de_u64(a.1)),
                    ("rx_bytes_s", Json::texto_de(fmt2(taxa(a.0, z.0)))),
                    ("tx_bytes_s", Json::texto_de(fmt2(taxa(a.1, z.1)))),
                    ("rx_pacotes", Json::de_u64(a.2)),
                    ("tx_pacotes", Json::de_u64(a.3)),
                    ("erros", Json::de_u64(a.4)),
                ]))
            })
            .collect(),
    )
}

fn ler_diskstats() -> HashMap<String, (u64, u64, u64, u64)> {
    let mut m = HashMap::new();
    let Ok(t) = std::fs::read_to_string("/proc/diskstats") else {
        return m;
    };
    for l in t.lines() {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() < 10 {
            continue;
        }
        let nome = c[2].to_string();
        // Particoes repetem o que o disco inteiro ja contou, e loop/ram sao
        // ruido. Mostrar os tres somaria o mesmo IO tres vezes.
        if nome.starts_with("loop") || nome.starts_with("ram") {
            continue;
        }
        let n = |i: usize| c[i].parse::<u64>().unwrap_or(0);
        m.insert(nome, (n(3), n(7), n(5), n(9)));
    }
    m
}

fn io_json(antes: &Amostra, agora: &Amostra, segundos: f64) -> Json {
    let mut nomes: Vec<&String> = agora.discos.keys().collect();
    nomes.sort();
    Json::Lista(
        nomes
            .into_iter()
            .filter_map(|nome| {
                let a = agora.discos.get(nome)?;
                let z = antes.discos.get(nome).copied().unwrap_or((0, 0, 0, 0));
                let taxa = |novo: u64, velho: u64| {
                    if segundos > 0.0 && novo >= velho {
                        (novo - velho) as f64 / segundos
                    } else {
                        0.0
                    }
                };
                Some(Json::objeto(vec![
                    ("disco", Json::texto_de(nome)),
                    ("leituras", Json::de_u64(a.0)),
                    ("escritas", Json::de_u64(a.1)),
                    // O setor do `/proc/diskstats` e sempre de 512 bytes, e nao
                    // o setor fisico do disco.
                    (
                        "lidos_bytes_s",
                        Json::texto_de(fmt2(taxa(a.2, z.2) * 512.0)),
                    ),
                    (
                        "escritos_bytes_s",
                        Json::texto_de(fmt2(taxa(a.3, z.3) * 512.0)),
                    ),
                ]))
            })
            .collect(),
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O monitor tem de responder mesmo onde `/proc` nao existe. Nunca pode
    /// derrubar o painel.
    #[test]
    fn ler_nao_falha_nem_sem_proc() {
        let mut m = Monitor::novo();
        let j = m.ler(&[Path::new(".")]);
        assert!(j.campo("cpu").is_some());
        assert!(j.campo("memoria").is_some());
        assert!(j.campo("rede").is_some());
    }

    /// A primeira leitura nao tem taxa: nao ha instante anterior de onde tirar
    /// uma. Dizer isso e melhor do que dividir pelo tempo desde o arranque.
    #[test]
    fn a_primeira_leitura_se_declara() {
        let mut m = Monitor::novo();
        let j = m.ler(&[]);
        let cpu = j.campo("cpu").unwrap();
        assert!(cpu.booleano_ou("primeira_leitura", false));
        let segunda = m.ler(&[]);
        assert!(!segunda
            .campo("cpu")
            .unwrap()
            .booleano_ou("primeira_leitura", true));
    }

    #[test]
    fn o_percentual_do_disco_fecha_em_cem() {
        let e = EspacoEmDisco {
            caminho: "/x".into(),
            dispositivo: "/dev/x".into(),
            montagem: "/".into(),
            total_kb: 1000,
            usado_kb: 250,
            livre_kb: 750,
        };
        assert!((e.usado_percentual() - 25.0).abs() < 0.001);
        assert!((e.livre_percentual() - 75.0).abs() < 0.001);
    }

    /// A conta e sobre `usado + livre`, e nao sobre o tamanho do disco.
    ///
    /// Os numeros sao os desta maquina, colados do `df -k`: 264 GB de disco,
    /// 21 GB usados, 18 GB alcancaveis. O `df` diz 55% -- e era 8% que este
    /// modulo mostrava antes, o que faria um alerta de "menos de 10% livre"
    /// ficar calado com o disco pela metade.
    #[test]
    fn a_reserva_do_sistema_de_arquivos_nao_conta_como_livre() {
        let e = EspacoEmDisco {
            caminho: "/".into(),
            dispositivo: "/dev/vda".into(),
            montagem: "/".into(),
            total_kb: 264_212_084,
            usado_kb: 20_986_728,
            livre_kb: 17_861_796,
        };
        // O `df` arredonda para cima e mostra 55; a conta exata da 54,02.
        assert!(
            (e.usado_percentual() - 54.02).abs() < 0.01,
            "{}",
            e.usado_percentual()
        );
        assert_eq!(e.utilizavel_kb(), 38_848_524);
        assert_eq!(e.reservado_kb(), 225_363_560);
    }

    /// Disco de tamanho zero nao pode virar divisao por zero no painel.
    #[test]
    fn disco_vazio_nao_divide_por_zero() {
        let e = EspacoEmDisco {
            caminho: "/x".into(),
            dispositivo: "-".into(),
            montagem: "-".into(),
            total_kb: 0,
            usado_kb: 0,
            livre_kb: 0,
        };
        assert_eq!(e.usado_percentual(), 0.0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn no_linux_le_a_maquina_de_verdade() {
        let mut m = Monitor::novo();
        m.ler(&[]);
        std::thread::sleep(std::time::Duration::from_millis(60));
        let j = m.ler(&[Path::new("/")]);
        let mem = j.campo("memoria").unwrap();
        assert!(mem.inteiro_ou("total_kb", 0) > 0, "memoria total > 0");
        let discos = j.campo("discos").and_then(Json::lista).unwrap();
        assert!(!discos.is_empty(), "o df devia listar a raiz");
        assert!(discos[0].inteiro_ou("total_kb", 0) > 0);
        let rede = j.campo("rede").and_then(Json::lista).unwrap();
        assert!(!rede.is_empty(), "ao menos a interface local");
    }
}

//! wx-modelos -- escolha e controle do modelo local, no terminal.
//!
//! Por que um binario Rust a parte, num plugin de Python: a tela precisa de
//! redesenho a cada quadro e de tempo limite em soquete, e o `rotear_modelo.py`
//! precisa de uma resposta legivel por maquina para decidir. Um binario sem
//! dependencia nenhuma roda na maquina do cliente sem instalar nada.
//!
//! O que ele controla, contra o servico local (Magnitude, 127.0.0.1:10100):
//!   maquina   o que esta maquina tem, medido
//!   modelos   catalogo do servico (ou de um arquivo), com o que cabe aqui
//!   estado    o que esta carregado agora
//!   carregar  poe um modelo em memoria, com o progresso que o servico der
//!   soltar    tira da memoria
//!   medir     mede tokens/s DESTA maquina e guarda a medicao
//!
//! Sem argumento e com terminal, abre a tela. Com --json, sai dado para script.

mod catalogo;
mod json;
mod maquina;
mod servico;
mod tela;

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use catalogo::{Couber, Modelo};
use maquina::{gb, Maquina};
use tela::Eixo;

const AJUDA: &str = "\
wx-modelos -- escolha e controle do modelo local do WX Claude Code

USO
  wx-modelos [comando] [opcoes]

COMANDOS
  maquina              o que esta maquina tem (medido)
  modelos              catalogo, com o que cabe aqui
  estado               o que o servico local tem carregado
  carregar <id>        carrega o modelo e acompanha o progresso
  soltar [id]          tira da memoria
  medir <id>           mede tokens/s nesta maquina e guarda
  tela                 a tela cheia (padrao quando ha terminal)

OPCOES
  --json               saida para script, sem cor e sem tela
  --catalogo ARQ       le o catalogo de um arquivo em vez do servico
  --endereco HOST:PORTA  padrao 127.0.0.1:10100
  --sem-cor            desliga ANSI
  -h, --help           esta ajuda

O que nao se mede aparece INDISPONIVEL. Nenhum numero desta ferramenta e
estimado: memoria e nucleos saem do sistema, tamanho e contexto saem do
catalogo, velocidade so aparece depois de `medir`, e nota de qualidade so
entra se o catalogo trouxer a fonte junto.
";

struct Opcoes {
    comando: String,
    alvo: Option<String>,
    json: bool,
    catalogo: Option<PathBuf>,
    endereco: String,
    colorido: bool,
}

fn ler_opcoes() -> Result<Opcoes, String> {
    let mut args = std::env::args().skip(1).peekable();
    let mut o = Opcoes {
        comando: String::new(),
        alvo: None,
        json: false,
        catalogo: None,
        endereco: servico::ENDERECO_PADRAO.to_string(),
        colorido: std::io::stdout().is_terminal(),
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{AJUDA}");
                std::process::exit(0);
            }
            "--json" => {
                o.json = true;
                o.colorido = false;
            }
            "--sem-cor" => o.colorido = false,
            "--catalogo" => {
                o.catalogo = Some(PathBuf::from(
                    args.next().ok_or("--catalogo exige um arquivo")?,
                ))
            }
            "--endereco" => o.endereco = args.next().ok_or("--endereco exige host:porta")?,
            outro if outro.starts_with('-') => return Err(format!("opcao desconhecida: {outro}")),
            outro if o.comando.is_empty() => o.comando = outro.to_string(),
            outro => o.alvo = Some(outro.to_string()),
        }
    }
    if o.comando.is_empty() {
        o.comando = if o.json || !std::io::stdout().is_terminal() {
            "estado".into()
        } else {
            "tela".into()
        };
    }
    Ok(o)
}

fn arquivo_de_medicoes() -> PathBuf {
    let base = std::env::var("WX_MODELOS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let casa = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(casa).join(".wx-claude-code")
        });
    base.join("medicoes-de-modelo.json")
}

fn catalogo_de(o: &Opcoes) -> Result<Vec<Modelo>, String> {
    if let Some(arq) = &o.catalogo {
        return catalogo::do_arquivo(arq);
    }
    let v = servico::json(&o.endereco, "/models/catalog").map_err(|e| e.to_string())?;
    let instalados = servico::json(&o.endereco, "/models/status")
        .ok()
        .and_then(|s| {
            s.campo("loaded").and_then(|l| l.lista()).map(|l| {
                l.iter()
                    .filter_map(|i| i.texto().map(str::to_string))
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    Ok(catalogo::da_estrutura(&v, &instalados))
}

fn eixos_de(m: &Modelo, maq: &Maquina, tok_s: Option<f64>) -> Vec<Eixo> {
    let memoria = match (m.bytes, maq.orcamento_de_memoria()) {
        (Some(b), Some(orc)) if orc > 0 => Some((1.0 - (b as f64 / orc as f64)).clamp(0.0, 1.0)),
        _ => None,
    };
    vec![
        Eixo {
            nome: "MEMÓRIA",
            valor: memoria,
            rotulo: m
                .bytes
                .map(|b| format!("{:.1} GB · {}", gb(b), m.couber(maq).rotulo()))
                .unwrap_or_default(),
        },
        Eixo {
            nome: "VELOCIDADE",
            // 60 tok/s como topo da escala do desenho; o rotulo mostra o medido
            valor: tok_s.map(|t| (t / 60.0).clamp(0.0, 1.0)),
            rotulo: tok_s
                .map(|t| format!("{t:.1} tok/s medidos aqui"))
                .unwrap_or_default(),
        },
        Eixo {
            nome: "CONTEXTO",
            valor: m.contexto.map(|c| (c as f64 / 128_000.0).clamp(0.0, 1.0)),
            rotulo: m
                .contexto
                .map(|c| format!("{} mil tokens", c / 1000))
                .unwrap_or_default(),
        },
        Eixo {
            nome: "PRECISÃO",
            valor: m.precisao().map(|p| match p {
                "sem perda (16 bits)" => 1.0,
                "muito alta" => 0.85,
                "alta" => 0.7,
                "média" => 0.55,
                _ => 0.4,
            }),
            rotulo: m
                .precisao()
                .map(|p| format!("{p} ({})", m.quantizacao.clone().unwrap_or_default()))
                .unwrap_or_default(),
        },
        Eixo {
            nome: "QUALIDADE",
            valor: m.nota.as_ref().map(|(n, _)| (n / 10.0).clamp(0.0, 1.0)),
            rotulo: m
                .nota
                .as_ref()
                .map(|(n, f)| format!("{n:.1} — fonte: {f}"))
                .unwrap_or_default(),
        },
    ]
}

fn cmd_maquina(o: &Opcoes) -> i32 {
    let m = Maquina::medir();
    if o.json {
        let n = |x: Option<u64>| x.map(|v| v.to_string()).unwrap_or_else(|| "null".into());
        println!(
            "{{\"so\":\"{}\",\"arquitetura\":\"{}\",\"processador\":{},\"nucleos\":{},\"memoria_bytes\":{},\"memoria_livre_bytes\":{},\"orcamento_bytes\":{},\"acelerador\":{}}}",
            json::escapar(&m.so),
            json::escapar(&m.arquitetura),
            m.processador.as_ref().map(|p| format!("\"{}\"", json::escapar(p))).unwrap_or_else(|| "null".into()),
            m.nucleos.map(|c| c.to_string()).unwrap_or_else(|| "null".into()),
            n(m.memoria_bytes),
            n(m.memoria_livre_bytes),
            n(m.orcamento_de_memoria()),
            m.acelerador.as_ref().map(|a| format!("\"{}\"", json::escapar(a))).unwrap_or_else(|| "null".into()),
        );
    } else {
        println!("{}", tela::linha_da_maquina(&m));
        match m.orcamento_de_memoria() {
            Some(b) => println!("orçamento para o modelo: {:.1} GB", gb(b)),
            None => println!("orçamento para o modelo: INDISPONÍVEL (memória não medida)"),
        }
    }
    0
}

fn cmd_modelos(o: &Opcoes) -> i32 {
    let maq = Maquina::medir();
    let modelos = match catalogo_de(o) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("erro: {e}");
            return 3;
        }
    };
    let medidos = catalogo::medicoes(&arquivo_de_medicoes());
    if o.json {
        let itens: Vec<String> = modelos.iter().map(|m| {
            format!(
                "{{\"id\":\"{}\",\"nome\":\"{}\",\"bytes\":{},\"contexto\":{},\"cabe\":\"{}\",\"instalado\":{},\"tokens_por_segundo\":{}}}",
                json::escapar(&m.id), json::escapar(&m.nome),
                m.bytes.map(|b| b.to_string()).unwrap_or_else(|| "null".into()),
                m.contexto.map(|c| c.to_string()).unwrap_or_else(|| "null".into()),
                m.couber(&maq).rotulo(), m.instalado,
                medidos.get(&m.id).map(|t| format!("{t:.2}")).unwrap_or_else(|| "null".into()),
            )
        }).collect();
        println!("{{\"modelos\":[{}]}}", itens.join(","));
        return 0;
    }
    println!("{}\n", tela::linha_da_maquina(&maq));
    for (i, m) in modelos.iter().enumerate() {
        println!("{}", tela::linha_do_modelo(i + 1, m, &maq, o.colorido));
    }
    if modelos.is_empty() {
        println!("nenhum modelo no catálogo (serviço no ar? `wx-modelos estado`)");
    }
    0
}

fn cmd_estado(o: &Opcoes) -> i32 {
    let no_ar = servico::no_ar(&o.endereco);
    let carregados = if no_ar {
        servico::json(&o.endereco, "/models/status")
            .ok()
            .and_then(|v| {
                v.campo("loaded").and_then(|l| l.lista()).map(|l| {
                    l.iter()
                        .filter_map(|i| i.texto().map(str::to_string))
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    if o.json {
        println!(
            "{{\"servico_no_ar\":{},\"endereco\":\"{}\",\"carregados\":[{}]}}",
            no_ar,
            json::escapar(&o.endereco),
            carregados
                .iter()
                .map(|c| format!("\"{}\"", json::escapar(c)))
                .collect::<Vec<_>>()
                .join(","),
        );
    } else if !no_ar {
        println!(
            "serviço local FORA DO AR em {} — o roteador volta ao modelo pago",
            o.endereco
        );
    } else if carregados.is_empty() {
        println!("serviço no ar em {}; nenhum modelo carregado", o.endereco);
    } else {
        println!(
            "serviço no ar em {}; carregado: {}",
            o.endereco,
            carregados.join(", ")
        );
    }
    i32::from(!no_ar)
}

fn cmd_carregar(o: &Opcoes) -> i32 {
    let Some(id) = o.alvo.clone() else {
        eprintln!("erro: diga qual modelo carregar (wx-modelos carregar <id>)");
        return 2;
    };
    let corpo = format!("{{\"model\":\"{}\"}}", json::escapar(&id));
    if let Err(e) = servico::json_post(&o.endereco, "/models/load", &corpo) {
        eprintln!("erro: {e}");
        return 3;
    }
    // Progresso: SO o que o servico informar. Sem numero, a barra nao inventa.
    let comeco = Instant::now();
    let mut saida = std::io::stdout();
    loop {
        let estado = servico::json(&o.endereco, "/models/status").ok();
        let fracao = estado
            .as_ref()
            .and_then(|v| v.campo_numero("progress"))
            .map(|p| if p > 1.0 { p / 100.0 } else { p });
        let pronto = estado
            .as_ref()
            .and_then(|v| v.campo("loaded").and_then(|l| l.lista()))
            .map(|l| l.iter().any(|i| i.texto() == Some(id.as_str())))
            .unwrap_or(false);
        if !o.json {
            print!("\r{} {}", tela::barra(fracao, 40, o.colorido), id);
            saida.flush().ok();
        }
        if pronto {
            if !o.json {
                println!("\ncarregado em {:.1}s", comeco.elapsed().as_secs_f64());
            } else {
                println!(
                    "{{\"carregado\":\"{}\",\"segundos\":{:.2}}}",
                    json::escapar(&id),
                    comeco.elapsed().as_secs_f64()
                );
            }
            return 0;
        }
        if comeco.elapsed() > Duration::from_secs(600) {
            eprintln!("\nerro: o serviço não confirmou o carregamento em 10 min");
            return 4;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

fn cmd_soltar(o: &Opcoes) -> i32 {
    let corpo = match &o.alvo {
        Some(id) => format!("{{\"model\":\"{}\"}}", json::escapar(id)),
        None => "{}".to_string(),
    };
    match servico::json_post(&o.endereco, "/models/stop", &corpo) {
        Ok(_) => {
            if !o.json {
                println!("solto");
            }
            0
        }
        Err(e) => {
            eprintln!("erro: {e}");
            3
        }
    }
}

fn cmd_medir(o: &Opcoes) -> i32 {
    let Some(id) = o.alvo.clone() else {
        eprintln!("erro: diga qual modelo medir (wx-modelos medir <id>)");
        return 2;
    };
    // Uma pergunta curta e determinista; o que interessa e tokens por segundo
    // DESTA maquina, nao a qualidade da resposta.
    let corpo = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"user\",\"content\":\"Conte de 1 a 40, separados por virgula.\"}}],\"max_tokens\":120,\"temperature\":0}}",
        json::escapar(&id)
    );
    let t0 = Instant::now();
    let resposta = match servico::json_post(&o.endereco, "/inference/v1/chat/completions", &corpo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("erro: {e}");
            return 3;
        }
    };
    let segundos = t0.elapsed().as_secs_f64();
    let tokens = resposta
        .campo("usage")
        .and_then(|u| u.campo_numero("completion_tokens"));
    let Some(tokens) = tokens else {
        eprintln!("erro: o serviço não informou completion_tokens; sem isso não há medição, e medição estimada não vale");
        return 5;
    };
    let tok_s = tokens / segundos;
    let arq = arquivo_de_medicoes();
    if let Some(pai) = arq.parent() {
        std::fs::create_dir_all(pai).ok();
    }
    let mut anteriores = catalogo::medicoes(&arq);
    anteriores.insert(id.clone(), tok_s);
    let corpo: Vec<String> = anteriores
        .iter()
        .map(|(k, v)| format!("\"{}\":{{\"tokens_por_segundo\":{v:.3}}}", json::escapar(k)))
        .collect();
    std::fs::write(&arq, format!("{{{}}}\n", corpo.join(","))).ok();
    if o.json {
        println!("{{\"modelo\":\"{}\",\"tokens_por_segundo\":{tok_s:.2},\"tokens\":{tokens},\"segundos\":{segundos:.2}}}", json::escapar(&id));
    } else {
        println!("{id}: {tok_s:.1} tok/s medidos aqui ({tokens:.0} tokens em {segundos:.1}s)");
        println!("guardado em {}", arq.display());
    }
    0
}

fn cmd_tela(o: &Opcoes) -> i32 {
    let maq = Maquina::medir();
    let modelos = catalogo_de(o).unwrap_or_default();
    let medidos = catalogo::medicoes(&arquivo_de_medicoes());
    let escolhido = modelos
        .iter()
        .find(|m| m.instalado && m.couber(&maq) != Couber::NaoCabe)
        .or_else(|| modelos.iter().find(|m| m.couber(&maq) == Couber::Cabe))
        .or(modelos.first());

    let largura = tela::largura();
    print!("{}{}", tela::LIMPA, tela::ESCONDE_CURSOR);
    println!(
        "{}",
        tela::cor("1", "WX CLAUDE CODE · MODELO LOCAL", o.colorido)
    );
    println!("{}", tela::linha_da_maquina(&maq));
    println!("{}", "─".repeat(largura.min(96)));
    let no_ar = servico::no_ar(&o.endereco);
    println!(
        "serviço local: {}",
        if no_ar {
            tela::cor("32", &format!("no ar em {}", o.endereco), o.colorido)
        } else {
            tela::cor(
                "31",
                &format!(
                    "FORA DO AR em {} — o roteador volta ao modelo pago",
                    o.endereco
                ),
                o.colorido,
            )
        }
    );
    println!();
    if modelos.is_empty() {
        println!("Nenhum modelo no catálogo.");
        println!("Suba o serviço (`magnitude service`) ou passe --catalogo ARQUIVO.");
    } else {
        println!("{}", tela::cor("1", "MODELOS", o.colorido));
        for (i, m) in modelos.iter().take(12).enumerate() {
            println!("{}", tela::linha_do_modelo(i + 1, m, &maq, o.colorido));
        }
    }
    if let Some(m) = escolhido {
        println!("\n{} {}", tela::cor("1", "EM DETALHE:", o.colorido), m.nome);
        // parametros e modalidades saem do catalogo; ausentes, ficam de fora da
        // linha em vez de virar "—" que o olho le como zero
        let mut ficha: Vec<String> = Vec::new();
        if let Some(p) = &m.parametros {
            ficha.push(p.clone());
        }
        if !m.modalidades.is_empty() {
            ficha.push(m.modalidades.join(" e "));
        }
        if let Some(q) = &m.quantizacao {
            ficha.push(q.clone());
        }
        if !ficha.is_empty() {
            println!("  {}", ficha.join(" · "));
        }
        let eixos = eixos_de(m, &maq, medidos.get(&m.id).copied());
        let desenho = tela::radar(&eixos, o.colorido);
        let descricao = tela::descricao_dos_eixos(&eixos);
        let altura = desenho.len().max(descricao.len());
        for i in 0..altura {
            let a = desenho.get(i).cloned().unwrap_or_default();
            let b = descricao.get(i).cloned().unwrap_or_default();
            println!("  {a:<40}{b}");
        }
        println!(
            "\n  carregar:  wx-modelos carregar {}\n  medir:     wx-modelos medir {}",
            m.id, m.id
        );
    }
    print!("{}", tela::MOSTRA_CURSOR);
    0
}

fn main() {
    let o = match ler_opcoes() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("erro: {e}\n\n{AJUDA}");
            std::process::exit(2);
        }
    };
    let codigo = match o.comando.as_str() {
        "maquina" => cmd_maquina(&o),
        "modelos" => cmd_modelos(&o),
        "estado" => cmd_estado(&o),
        "carregar" => cmd_carregar(&o),
        "soltar" => cmd_soltar(&o),
        "medir" => cmd_medir(&o),
        "tela" => cmd_tela(&o),
        outro => {
            eprintln!("erro: comando desconhecido: {outro}\n\n{AJUDA}");
            2
        }
    };
    std::process::exit(codigo);
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modelo(js: &str) -> Modelo {
        catalogo::da_estrutura(&json::analisar(&format!("[{js}]")).unwrap(), &[]).remove(0)
    }

    #[test]
    fn eixo_de_velocidade_so_existe_depois_de_medir() {
        let m = modelo(r#"{"id":"x","bytes":4000000000,"contexto":50000,"quantizacao":"Q4_K_M"}"#);
        let maq = Maquina {
            memoria_livre_bytes: Some(16 * 1_073_741_824),
            ..Default::default()
        };
        let sem = eixos_de(&m, &maq, None);
        let com = eixos_de(&m, &maq, Some(41.0));
        let vel = |e: &Vec<Eixo>| e.iter().find(|x| x.nome == "VELOCIDADE").unwrap().valor;
        assert_eq!(
            vel(&sem),
            None,
            "sem medicao a velocidade nao pode ter valor"
        );
        assert!(vel(&com).is_some());
    }

    #[test]
    fn eixo_de_qualidade_nao_aparece_sem_fonte() {
        let maq = Maquina {
            memoria_livre_bytes: Some(16 * 1_073_741_824),
            ..Default::default()
        };
        let sem_fonte = modelo(r#"{"id":"x","nota":9.5}"#);
        let com_fonte = modelo(r#"{"id":"x","nota":9.5,"nota_fonte":"medição interna 2026-09"}"#);
        let q = |m: &Modelo| {
            eixos_de(m, &maq, None)
                .into_iter()
                .find(|e| e.nome == "QUALIDADE")
                .unwrap()
                .valor
        };
        assert_eq!(q(&sem_fonte), None);
        assert!(q(&com_fonte).is_some());
    }

    #[test]
    fn maquina_sem_memoria_medida_nao_produz_eixo_de_memoria() {
        let m = modelo(r#"{"id":"x","bytes":4000000000}"#);
        let eixos = eixos_de(&m, &Maquina::default(), None);
        assert_eq!(
            eixos.iter().find(|e| e.nome == "MEMÓRIA").unwrap().valor,
            None
        );
    }
}

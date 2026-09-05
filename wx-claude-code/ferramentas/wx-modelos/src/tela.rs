//! A tela: ANSI na mao, sem crate de TUI.
//!
//! Regra que manda no desenho: eixo sem medicao NAO vira poligono. Ele aparece
//! escrito INDISPONIVEL, e o poligono se fecha so com o que existe. Radar
//! completo com metade dos valores chutados e a forma mais eficiente de mentir
//! num terminal -- o olho le a area, nao os rotulos.

use crate::catalogo::{Couber, Modelo};
use crate::maquina::{gb, Maquina};

pub const LIMPA: &str = "\x1b[2J\x1b[H";
pub const ESCONDE_CURSOR: &str = "\x1b[?25l";
pub const MOSTRA_CURSOR: &str = "\x1b[?25h";

pub fn cor(codigo: &str, texto: &str, colorido: bool) -> String {
    if colorido {
        format!("\x1b[{codigo}m{texto}\x1b[0m")
    } else {
        texto.to_string()
    }
}

pub fn largura() -> usize {
    if let Ok(c) = std::env::var("COLUMNS") {
        if let Ok(n) = c.parse::<usize>() {
            return n.clamp(60, 200);
        }
    }
    if let Ok(saida) = std::process::Command::new("stty").arg("size").output() {
        let t = String::from_utf8_lossy(&saida.stdout);
        if let Some(c) = t
            .split_whitespace()
            .nth(1)
            .and_then(|n| n.parse::<usize>().ok())
        {
            return c.clamp(60, 200);
        }
    }
    100
}

/// Barra de progresso com o numero ao lado. Sem porcentagem conhecida, ela nao
/// finge: mostra o rotulo do passo e nenhuma fracao.
pub fn barra(fracao: Option<f64>, largura: usize, colorido: bool) -> String {
    match fracao {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            let cheio = (largura as f64 * f).round() as usize;
            let corpo = format!("{}{}", "█".repeat(cheio), "░".repeat(largura - cheio));
            format!("{} {:>3.0}%", cor("34", &corpo, colorido), f * 100.0)
        }
        None => format!("{} INDISPONÍVEL", "░".repeat(largura)),
    }
}

pub struct Eixo {
    pub nome: &'static str,
    pub valor: Option<f64>,
    pub rotulo: String,
}

/// Radar em ASCII. Desenha SO os eixos medidos; os demais ficam pontilhados e
/// escritos INDISPONIVEL, e a contagem aparece embaixo para ninguem ler area
/// achando que os cinco existem.
pub fn radar(eixos: &[Eixo], colorido: bool) -> Vec<String> {
    const ALTURA: usize = 11;
    const LARGURA: usize = 41;
    let cx = LARGURA as f64 / 2.0;
    let cy = ALTURA as f64 / 2.0;
    let rx = cx - 1.0;
    let ry = cy - 1.0;
    let mut grade = vec![vec![' '; LARGURA]; ALTURA];
    let n = eixos.len().max(1);
    let ponto = |i: usize, t: f64| -> (f64, f64) {
        let ang =
            std::f64::consts::PI * 2.0 * (i as f64) / (n as f64) - std::f64::consts::FRAC_PI_2;
        (cx + ang.cos() * t * rx, cy + ang.sin() * t * ry)
    };
    let marcar = |grade: &mut Vec<Vec<char>>, x: f64, y: f64, c: char| {
        let (xi, yi) = (x.round() as isize, y.round() as isize);
        if xi >= 0 && yi >= 0 && (xi as usize) < LARGURA && (yi as usize) < ALTURA {
            let atual = grade[yi as usize][xi as usize];
            if c != '\u{b7}' || atual == ' ' {
                grade[yi as usize][xi as usize] = c;
            }
        }
    };
    // moldura: cada eixo inteiro pontilhado, medido ou nao
    for i in 0..n {
        for p in 1..=14 {
            let (x, y) = ponto(i, p as f64 / 14.0);
            marcar(&mut grade, x, y, '\u{b7}');
        }
    }
    // poligono: liga SO pares de eixos medidos. Eixo sem medicao nao vira aresta,
    // porque area desenhada e o que o olho le -- e area chutada e mentira.
    for i in 0..n {
        let j = (i + 1) % n;
        let (Some(a), Some(b)) = (eixos[i].valor, eixos[j].valor) else {
            continue;
        };
        let (x0, y0) = ponto(i, a.clamp(0.0, 1.0));
        let (x1, y1) = ponto(j, b.clamp(0.0, 1.0));
        for p in 0..=24 {
            let t = p as f64 / 24.0;
            marcar(&mut grade, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, '*');
        }
    }
    // o vertice medido, por ultimo, para nunca ser encoberto
    for (i, e) in eixos.iter().enumerate() {
        if let Some(v) = e.valor {
            let (x, y) = ponto(i, v.clamp(0.0, 1.0));
            marcar(&mut grade, x, y, '\u{25cf}');
        }
    }
    let mut linhas: Vec<String> = grade
        .into_iter()
        .map(|l| {
            let s: String = l.into_iter().collect();
            cor("36", &s, colorido)
        })
        .collect();
    let medidos = eixos.iter().filter(|e| e.valor.is_some()).count();
    linhas.push(format!("  {medidos} de {} eixos medidos", eixos.len()));
    linhas
}

pub fn descricao_dos_eixos(eixos: &[Eixo]) -> Vec<String> {
    eixos
        .iter()
        .map(|e| {
            let v = if e.valor.is_some() {
                e.rotulo.clone()
            } else {
                "INDISPONÍVEL".to_string()
            };
            format!("  {:<12} {}", e.nome, v)
        })
        .collect()
}

pub fn linha_da_maquina(m: &Maquina) -> String {
    let mut partes = vec![m
        .processador
        .clone()
        .unwrap_or_else(|| "processador INDISPONÍVEL".into())];
    partes.push(format!("{} {}", m.so, m.arquitetura));
    partes.push(match m.nucleos {
        Some(n) => format!("{n} núcleos"),
        None => "núcleos INDISPONÍVEL".into(),
    });
    partes.push(match m.memoria_bytes {
        Some(b) => format!(
            "{:.0} GB{}",
            gb(b),
            if m.memoria_unificada {
                " unificada"
            } else {
                ""
            }
        ),
        None => "memória INDISPONÍVEL".into(),
    });
    if let Some(a) = &m.acelerador {
        partes.push(a.clone());
    }
    partes.join(" · ")
}

pub fn linha_do_modelo(i: usize, m: &Modelo, maq: &Maquina, colorido: bool) -> String {
    let couber = m.couber(maq);
    let cor_estado = match couber {
        Couber::Cabe => "32",
        Couber::Apertado => "33",
        Couber::NaoCabe => "31",
        Couber::Desconhecido => "90",
    };
    let tamanho = m
        .bytes
        .map(|b| format!("{:.1} GB", gb(b)))
        .unwrap_or_else(|| "— GB".into());
    format!(
        "{:>3}. {:<34} {:>8}  {}{}",
        i,
        m.nome,
        tamanho,
        cor(cor_estado, couber.rotulo(), colorido),
        if m.instalado {
            "  [nesta máquina]"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn barra_sem_fracao_nao_finge_porcentagem() {
        let b = barra(None, 10, false);
        assert!(b.contains("INDISPONÍVEL"));
        assert!(!b.contains('%'));
    }

    #[test]
    fn barra_com_fracao_mostra_o_numero() {
        assert!(barra(Some(0.39), 10, false).contains("39%"));
        assert!(
            barra(Some(2.0), 10, false).contains("100%"),
            "acima de 1 satura, nao estoura"
        );
    }

    #[test]
    fn radar_nao_desenha_eixo_sem_medicao() {
        let eixos = vec![
            Eixo {
                nome: "MEMÓRIA",
                valor: Some(0.9),
                rotulo: "4,6 GB".into(),
            },
            Eixo {
                nome: "VELOCIDADE",
                valor: None,
                rotulo: String::new(),
            },
        ];
        let linhas = radar(&eixos, false);
        let corpo = linhas.join("\n");
        assert!(corpo.contains('\u{25cf}'), "o eixo medido marca o vertice");
        assert!(
            !corpo.contains('*'),
            "com um eixo so nao ha aresta: area chutada e mentira"
        );
        assert!(linhas.last().unwrap().contains("1 de 2 eixos medidos"));
        let desc = descricao_dos_eixos(&eixos).join("\n");
        assert!(
            desc.contains("INDISPONÍVEL"),
            "eixo sem medicao e escrito, nao desenhado"
        );
    }

    #[test]
    fn aresta_so_liga_dois_eixos_medidos() {
        let e = |v: Option<f64>| Eixo {
            nome: "X",
            valor: v,
            rotulo: String::new(),
        };
        let dois = radar(&[e(Some(0.9)), e(Some(0.8)), e(None)], false).join("\n");
        // com tres eixos, o primeiro e o ultimo SAO vizinhos no ciclo; para provar
        // a regra e preciso separa-los de verdade, com quatro
        let nenhum = radar(&[e(Some(0.9)), e(None), e(Some(0.8)), e(None)], false).join("\n");
        assert!(dois.contains('*'), "dois medidos vizinhos fecham aresta");
        assert!(
            !nenhum.contains('*'),
            "medidos separados por um sem medicao nao se ligam"
        );
    }

    #[test]
    fn linha_da_maquina_diz_o_que_nao_mediu() {
        let vazia = linha_da_maquina(&Maquina::default());
        assert!(vazia.contains("INDISPONÍVEL"));
        assert!(
            !vazia.contains("0 GB"),
            "memoria ausente nao pode virar zero"
        );
    }
}

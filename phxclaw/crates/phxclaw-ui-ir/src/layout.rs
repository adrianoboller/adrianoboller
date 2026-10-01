//! Layout de uma captura de tela pelas CAIXAS do OCR (tesseract em TSV), sem modelo novo.
//!
//! O OCR so enxerga texto: a caixa de um campo nao aparece, aparece o rotulo dele e, quando
//! ha, o texto de exemplo dentro dele ("Selecione", "dd/mm/aaaa", "0,00"). O que este modulo
//! faz e ler a GEOMETRIA desses textos:
//! - palavras viram frases (mesma linha, espaco de palavra entre elas);
//! - a barra lateral sai por um vao vertical sem texto que a separa do resto;
//! - frase com exemplo logo abaixo (ou ao lado), ou com espaco vazio abaixo, e ROTULO de
//!   campo; frase com outra frase comum colada abaixo e titulo de SECAO (legenda);
//! - uma fila de rotulos seguida de duas filas alinhadas com exemplo, ou de "Adicionar...",
//!   e cabecalho de GRADE;
//! - ordem de leitura e linha a linha; ordem de tabulacao e secao a secao.
//!
//! E heuristica de geometria, e por isso a medida dela e a prova de fidelidade
//! (`fidelidade.rs`), nao a leitura deste codigo. Os limiares estao em multiplos da altura
//! mediana do texto (`h0`), para nao dependerem da resolucao da captura.

use crate::imagem::{Rotulo, normalizar, texto_de_exemplo};
use crate::ir::{Caixa, LayoutGroup, LayoutItem, ScreenLayout};

/// Palavra lida pelo tesseract (nivel 5 do TSV), em pixels da imagem.
#[derive(Debug, Clone, PartialEq)]
pub struct Palavra {
    pub texto: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// (bloco, paragrafo, linha) do tesseract: so para refazer as linhas de texto.
    pub linha: (u32, u32, u32),
}

/// Le o TSV do tesseract: as palavras e o tamanho da pagina (nivel 1).
pub fn ler_tsv(tsv: &str) -> (Vec<Palavra>, u32, u32) {
    let mut v = vec![];
    let (mut largura, mut altura) = (0, 0);
    for l in tsv.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 12 {
            continue;
        }
        let n = |i: usize| c[i].trim().parse::<i64>().unwrap_or(0);
        match n(0) {
            1 => {
                largura = n(8).max(0) as u32;
                altura = n(9).max(0) as u32;
            }
            5 if !c[11].trim().is_empty() => v.push(Palavra {
                texto: c[11].trim().to_string(),
                x: n(6) as i32,
                y: n(7) as i32,
                w: n(8) as i32,
                h: n(9) as i32,
                linha: (n(2) as u32, n(3) as u32, n(4) as u32),
            }),
            _ => {}
        }
    }
    (v, largura, altura)
}

/// As linhas de texto como o tesseract as agrupou: o mesmo texto que a saida em texto
/// dava, para a confirmacao do modelo (`imagem::confirmar`) nao mudar de regua por o OCR
/// ter passado a sair em TSV -- e uma leitura so, em vez de duas.
pub fn linhas(palavras: &[Palavra]) -> Vec<String> {
    let mut v: Vec<((u32, u32, u32), String)> = vec![];
    for p in palavras {
        match v.last_mut() {
            Some((k, t)) if *k == p.linha => {
                t.push(' ');
                t.push_str(&p.texto);
            }
            _ => v.push((p.linha, p.texto.clone())),
        }
    }
    v.into_iter().map(|(_, t)| t).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Titulo,
    Menu,
    Secao,
    Rotulo,
    Coluna,
    Exemplo,
    Acao,
    Total,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frase {
    /// Texto sem o `*` do obrigatorio.
    pub texto: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub papel: Papel,
    pub obrigatorio: bool,
    /// Exemplo abaixo comeca com "Selecione": e lista.
    pub lista: bool,
    /// Indice em `Layout::grupos`.
    pub grupo: Option<usize>,
    /// Fila (linha visual) e posicao na leitura.
    pub fila: usize,
    pub leitura: u32,
}

impl Frase {
    fn direita(&self) -> i32 {
        self.x + self.w
    }
    fn base(&self) -> i32 {
        self.y + self.h
    }
    fn yc(&self) -> i32 {
        self.y + self.h / 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grupo {
    pub titulo: String,
    pub grade: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub largura: u32,
    pub altura: u32,
    pub frases: Vec<Frase>,
    pub grupos: Vec<Grupo>,
}

/// Botao de ERP pelo texto inteiro. A fila que tem um destes vira fila de acoes inteira,
/// porque o OCR le mal o resto dos botoes (medido: "Salvar" saiu "saver").
const ACOES: &[&str] = &[
    "salvar",
    "gravar",
    "excluir",
    "cancelar",
    "incluir",
    "novo",
    "nova",
    "pesquisar",
    "imprimir",
    "adicionaritem",
    "adicionar",
    "remover",
    "voltar",
    "fechar",
    "confirmar",
    "limpar",
    "ok",
    "sair",
    "buscar",
    "filtrar",
];

/// Botao pelo texto inteiro, ou fila de botoes que o OCR colou numa frase so (medido:
/// "Salvar h novo h excluir h pesquisar", a borda lida como "h"; e "Saver wove exauir
/// pesquisar imprimir", com os primeiros lidos mal): duas palavras de botao na frase.
/// "Novo preco" nao passa -- uma palavra so de botao.
fn e_acao(t: &str) -> bool {
    if ACOES.contains(&normalizar(t).as_str()) {
        return true;
    }
    let ps: Vec<String> = t.split_whitespace().map(normalizar).collect();
    let n = ps.iter().filter(|p| ACOES.contains(&p.as_str())).count();
    // a borda do botao lida como "(", "]" ou "|" e assinatura de botao (medido:
    // "(saver (wove ](exauir] resquisar (imprimir)")
    n >= 2 || (n >= 1 && t.contains(['(', ')', '[', ']', '|']))
}

fn e_exemplo(t: &str) -> bool {
    let n = normalizar(t);
    // o desenho de um controle lido como letra ("Oo" na caixa de marcar, "a)") tem uma ou
    // duas letras e nao e sigla; "UF" e sigla e fica
    let glifo = n.len() <= 2
        && !t
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_uppercase);
    // "R$" lido "RS"; e valor que comeca por algarismo ("0,00 by") e conteudo, nao rotulo
    let moeda = n == "rs";
    let numero = t
        .chars()
        .find(|c| c.is_alphanumeric())
        .is_some_and(|c| c.is_ascii_digit());
    texto_de_exemplo(&n) || glifo || moeda || numero || n.starts_with("nenhumregistro")
}

/// Palavras da mesma linha visual, com espaco de palavra entre elas, viram uma frase. O
/// espaco aceito vai a 1,6 altura porque o OCR as vezes perde uma letra acentuada no meio
/// (medido: "Preço unitário" saiu "Preço unit rio").
fn frases(palavras: &[Palavra]) -> Vec<(String, i32, i32, i32, i32)> {
    // risco solto ("|", "][") e a borda de um botao lida como letra: se entrasse, colava
    // os botoes de uma fila numa frase so (medido). O `*` fica: e a marca do obrigatorio.
    let mut ps: Vec<&Palavra> = palavras
        .iter()
        .filter(|p| p.w > 0 && p.h > 0)
        .filter(|p| p.texto.contains('*') || p.texto.chars().any(char::is_alphanumeric))
        .collect();
    ps.sort_by_key(|p| (p.x, p.y));
    let mut fs: Vec<(String, i32, i32, i32, i32)> = vec![];
    for p in ps {
        let (pt, pb) = (p.y, p.y + p.h);
        let melhor = fs
            .iter()
            .enumerate()
            .filter_map(|(i, (_, x, y, w, h))| {
                let sobre = pb.min(y + h) - pt.max(*y);
                let gap = p.x - (x + w);
                let alt = p.h.max(*h);
                (sobre * 2 >= p.h.min(*h) && gap >= -2 && gap * 10 <= alt * 16).then_some((gap, i))
            })
            .min();
        match melhor {
            Some((_, i)) => {
                let f = &mut fs[i];
                f.0.push(' ');
                f.0.push_str(&p.texto);
                let (x0, y0) = (f.1, f.2.min(p.y));
                let (x1, y1) = ((f.1 + f.3).max(p.x + p.w), (f.2 + f.4).max(p.y + p.h));
                *f = (std::mem::take(&mut f.0), x0, y0, x1 - x0, y1 - y0);
            }
            None => fs.push((p.texto.clone(), p.x, p.y, p.w, p.h)),
        }
    }
    fs
}

fn mediana(mut v: Vec<i32>) -> i32 {
    if v.is_empty() {
        return 12;
    }
    v.sort_unstable();
    v[v.len() / 2].max(1)
}

/// Vao vertical sem texto, no terco esquerdo, que separa uma barra lateral do conteudo.
/// So vale se a esquerda parece menu: comeca na borda, tem 3+ frases e nenhum campo
/// (exemplo ou `*`) -- uma tela de duas colunas sem menu nao pode perder a primeira.
fn corte_do_menu(fs: &[(String, i32, i32, i32, i32)], largura: i32) -> Option<i32> {
    let mut iv: Vec<(i32, i32)> = fs.iter().map(|f| (f.1, f.1 + f.3)).collect();
    iv.sort_unstable();
    let mut fim = i32::MIN;
    let mut melhor: Option<(i32, i32)> = None;
    for (a, b) in iv {
        if fim != i32::MIN && a > fim && fim < largura * 45 / 100 {
            let vao = a - fim;
            if vao * 1000 >= largura * 15 && melhor.is_none_or(|(v, _)| vao > v) {
                melhor = Some((vao, a));
            }
        }
        fim = fim.max(b);
    }
    let (_, corte) = melhor?;
    let esq: Vec<_> = fs.iter().filter(|f| f.1 + f.3 < corte).collect();
    let dir = fs.len() - esq.len();
    let borda = esq.iter().map(|f| f.1).min()? < largura * 6 / 100;
    let campo = esq.iter().any(|f| e_exemplo(&f.0) || f.0.contains('*'));
    (esq.len() >= 3 && dir >= 3 && borda && !campo).then_some(corte)
}

/// Le o layout das palavras do OCR. `largura`/`altura` sao da imagem (nivel 1 do TSV).
pub fn analisar(palavras: &[Palavra], largura: u32, altura: u32) -> Layout {
    let brutas = frases(palavras);
    let corte = corte_do_menu(&brutas, largura as i32);
    let mut fs: Vec<Frase> = brutas
        .into_iter()
        .filter(|(t, ..)| t.chars().filter(|c| c.is_alphanumeric()).count() > 0)
        .map(|(t, x, y, w, h)| {
            let menu = corte.is_some_and(|c| x + w < c);
            Frase {
                obrigatorio: t.contains('*'),
                texto: t
                    .replace('*', " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                x,
                y,
                w,
                h,
                papel: if menu { Papel::Menu } else { Papel::Rotulo },
                lista: false,
                grupo: None,
                fila: 0,
                leitura: 0,
            }
        })
        .filter(|f| !f.texto.is_empty() || f.papel == Papel::Menu)
        .collect();
    let h0 = mediana(
        fs.iter()
            .filter(|f| f.papel != Papel::Menu)
            .map(|f| f.h)
            .collect(),
    );
    // cisco: caixa com menos de 0,4 h0 de altura e pedaco de acento ou de borda (medido:
    // "js" e "5" com 2 px, sob a legenda "Itens", faziam dela um rotulo)
    fs.retain(|f| f.h * 10 >= h0 * 4);

    // filas: centro vertical a menos de 0,6 h0 do centro da primeira frase da fila
    let mut ordem: Vec<usize> = (0..fs.len())
        .filter(|&i| fs[i].papel != Papel::Menu)
        .collect();
    ordem.sort_by_key(|&i| (fs[i].yc(), fs[i].x));
    let mut filas: Vec<Vec<usize>> = vec![];
    for i in ordem {
        match filas.last_mut() {
            Some(f) if (fs[i].yc() - fs[f[0]].yc()).abs() * 10 <= h0 * 6 => f.push(i),
            _ => filas.push(vec![i]),
        }
    }
    for f in filas.iter_mut() {
        f.sort_by_key(|&i| fs[i].x);
    }
    let mut leitura = 0u32;
    for (n, f) in filas.iter().enumerate() {
        for &i in f {
            fs[i].fila = n;
            fs[i].leitura = leitura;
            leitura += 1;
        }
    }

    // titulo: a frase mais alta do topo do conteudo
    if let Some(&t) = filas
        .iter()
        .flatten()
        .find(|&&i| fs[i].h * 10 >= h0 * 13 && !e_exemplo(&fs[i].texto))
    {
        fs[t].papel = Papel::Titulo;
    }
    for f in &filas {
        let acoes = f.iter().any(|&i| e_acao(&fs[i].texto));
        // a fila de botoes inteira sai: o resto dela e botao lido mal
        for &i in f {
            if e_exemplo(&fs[i].texto) {
                fs[i].papel = Papel::Exemplo;
            } else if acoes && fs[i].papel == Papel::Rotulo {
                fs[i].papel = Papel::Acao;
            }
        }
    }

    // rotulo ou secao, pelo que esta logo abaixo
    let vivos: Vec<usize> = (0..fs.len())
        .filter(|&i| !matches!(fs[i].papel, Papel::Menu | Papel::Titulo))
        .collect();
    let abaixo = |fs: &[Frase], i: usize| -> Option<usize> {
        let p = &fs[i];
        vivos
            .iter()
            .copied()
            .filter(|&j| j != i && fs[j].fila > p.fila && fs[j].y >= p.base() - 2)
            .filter(|&j| fs[j].x <= p.direita() + h0 && fs[j].direita() >= p.x - h0)
            .min_by_key(|&j| (fs[j].y, fs[j].x))
    };
    let ao_lado = |fs: &[Frase], i: usize| -> Option<usize> {
        let p = &fs[i];
        vivos
            .iter()
            .copied()
            .filter(|&j| fs[j].fila == p.fila && fs[j].x > p.direita())
            .min_by_key(|&j| fs[j].x)
            .filter(|&j| fs[j].x - p.direita() <= 3 * h0)
    };
    for &i in &vivos {
        if fs[i].papel != Papel::Rotulo {
            continue;
        }
        let ex_lado = ao_lado(&fs, i).filter(|&j| fs[j].papel == Papel::Exemplo);
        match abaixo(&fs, i) {
            Some(j) if fs[j].papel == Papel::Exemplo && fs[j].y - fs[i].base() <= 3 * h0 => {
                fs[i].lista = normalizar(&fs[j].texto).starts_with("selecione");
            }
            _ if ex_lado.is_some() => {
                fs[i].lista =
                    ex_lado.is_some_and(|j| normalizar(&fs[j].texto).starts_with("selecione"));
            }
            Some(j) if fs[j].papel != Papel::Exemplo && (fs[j].y - fs[i].base()) * 10 < h0 * 38 => {
                fs[i].papel = Papel::Secao;
            }
            _ => {}
        }
    }

    // grades: fila com 2+ rotulos seguida de 2 filas alinhadas com exemplo, de "Adicionar"
    // ou de "Nenhum registro"
    let mut grades: Vec<(usize, Vec<usize>)> = vec![]; // (fila do cabecalho, frases do corpo)
    for (n, f) in filas.iter().enumerate() {
        let cab: Vec<usize> = f
            .iter()
            .copied()
            .filter(|&i| fs[i].papel == Papel::Rotulo)
            .collect();
        if cab.len() < 2 {
            continue;
        }
        let coluna_de = |x: i32| {
            cab.iter()
                .rposition(|&c| x + h0 >= fs[c].x)
                .filter(|_| x <= fs[*cab.last().unwrap_or(&cab[0])].direita() + 8 * h0)
        };
        let mut corpo = vec![];
        let mut dados = 0;
        let mut marca = false;
        for g in filas.iter().skip(n + 1) {
            let primeira = &fs[g[0]];
            if g.len() == 1
                && (normalizar(&primeira.texto).starts_with("nenhumregistro")
                    || normalizar(&primeira.texto).starts_with("adicionar"))
            {
                marca = true;
                corpo.extend(g.iter().copied());
                break;
            }
            let mut cols: Vec<usize> = g
                .iter()
                .filter_map(|&i| coluna_de(fs[i].x + fs[i].w / 2))
                .collect();
            cols.dedup();
            // fila de dados: tem exemplo (campo vazio da linha) debaixo do cabecalho; um
            // valor digitado na linha tambem cabe, e vira valor. Basta UMA coluna com
            // exemplo (medido: texto e numero vazios nao tem exemplo, so o "R$ 0,00"); o
            // que separa da fila de exemplos de um formulario sao as DUAS filas seguidas
            let tem_exemplo = g.iter().any(|&i| fs[i].papel == Papel::Exemplo);
            // a fila de botoes nao e linha de dados, mesmo com "Pesquisar" lido como exemplo
            // (medido: a secao Controle, de duas datas, virava grade)
            let botoes = g.iter().any(|&i| fs[i].papel == Papel::Acao);
            if tem_exemplo && !botoes && !cols.is_empty() {
                dados += 1;
                corpo.extend(g.iter().copied());
            } else {
                break;
            }
        }
        if dados >= 2 || marca {
            for &c in &cab {
                fs[c].papel = Papel::Coluna;
            }
            for &i in &corpo {
                if fs[i].papel == Papel::Rotulo {
                    fs[i].papel = Papel::Exemplo;
                }
            }
            grades.push((n, corpo));
        }
    }

    // grupos: cada secao abre um; a grade colada abaixo de uma secao vira o grupo dela
    let mut grupos: Vec<Grupo> = vec![];
    let mut secao_de: Vec<(usize, usize)> = vec![]; // (frase, grupo)
    for &i in &vivos {
        if fs[i].papel == Papel::Secao {
            secao_de.push((i, grupos.len()));
            grupos.push(Grupo {
                titulo: fs[i].texto.clone(),
                grade: false,
                x: fs[i].x,
                y: fs[i].y,
                w: fs[i].w,
                h: fs[i].h,
            });
        }
    }
    let secao_acima = |fs: &[Frase], i: usize| -> Option<usize> {
        secao_de
            .iter()
            .filter(|(s, _)| fs[*s].y < fs[i].y && fs[*s].x <= fs[i].x + 2 * h0)
            .max_by_key(|(s, _)| (fs[*s].y, fs[*s].x))
            .map(|(_, g)| *g)
    };
    let mut principal: Option<usize> = None;
    for (fila, corpo) in &grades {
        let cab: Vec<usize> = filas[*fila]
            .iter()
            .copied()
            .filter(|&i| fs[i].papel == Papel::Coluna)
            .collect();
        let g = match secao_acima(&fs, cab[0]) {
            // a legenda colada acima do cabecalho e o titulo da grade
            Some(g) if !grupos[g].grade && fs[cab[0]].y - (grupos[g].y + grupos[g].h) < 4 * h0 => {
                grupos[g].grade = true;
                g
            }
            _ => {
                grupos.push(Grupo {
                    titulo: String::new(),
                    grade: true,
                    x: fs[cab[0]].x,
                    y: fs[cab[0]].y,
                    w: 0,
                    h: 0,
                });
                grupos.len() - 1
            }
        };
        for &i in cab.iter().chain(corpo) {
            fs[i].grupo = Some(g);
        }
    }
    for &i in &vivos {
        if fs[i].grupo.is_some() || fs[i].papel == Papel::Secao {
            continue;
        }
        let g = match secao_acima(&fs, i) {
            Some(g) => g,
            None => *principal.get_or_insert_with(|| {
                grupos.push(Grupo {
                    titulo: String::new(),
                    grade: false,
                    x: fs[i].x,
                    y: fs[i].y,
                    w: 0,
                    h: 0,
                });
                grupos.len() - 1
            }),
        };
        fs[i].grupo = Some(g);
        // "Total" lido dentro ou depois de uma grade e a soma dela, nao um campo
        if fs[i].papel == Papel::Rotulo
            && normalizar(&fs[i].texto).starts_with("total")
            && grades.iter().any(|(f, _)| *f < fs[i].fila)
        {
            fs[i].papel = Papel::Total;
        }
    }
    for (f, n) in secao_de {
        fs[f].grupo = Some(n);
    }
    // caixa de cada grupo: tudo o que caiu nele
    for (g, gr) in grupos.iter_mut().enumerate() {
        let membros: Vec<&Frase> = fs.iter().filter(|f| f.grupo == Some(g)).collect();
        if membros.is_empty() {
            continue;
        }
        let x0 = membros.iter().map(|f| f.x).min().unwrap_or(gr.x);
        let y0 = membros.iter().map(|f| f.y).min().unwrap_or(gr.y);
        let x1 = membros.iter().map(|f| f.direita()).max().unwrap_or(gr.x);
        let y1 = membros.iter().map(|f| f.base()).max().unwrap_or(gr.y);
        (gr.x, gr.y, gr.w, gr.h) = (x0, y0, x1 - x0, y1 - y0);
    }
    Layout {
        largura,
        altura,
        frases: fs,
        grupos,
    }
}

impl Layout {
    fn rotulos_de(&self, papel: Papel) -> Vec<Rotulo> {
        self.em_tabulacao(
            &self
                .frases
                .iter()
                .enumerate()
                .filter(|(_, f)| f.papel == papel)
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|i| {
            let f = &self.frases[i];
            Rotulo {
                texto: f.texto.clone(),
                obrigatorio: f.obrigatorio,
                lista: f.lista,
            }
        })
        .collect()
    }

    /// Rotulos de campo do formulario, na ordem de tabulacao (o modo so-OCR, sem modelo).
    pub fn campos(&self) -> Vec<Rotulo> {
        self.rotulos_de(Papel::Rotulo)
    }

    /// Cabecalhos das grades.
    pub fn colunas(&self) -> Vec<Rotulo> {
        self.rotulos_de(Papel::Coluna)
    }

    /// Coluna de grade que o modelo apontou so vale se o layout a ve como cabecalho de
    /// grade. Medido em 01/10 (qwen2.5vl:3b, cadastro de cliente SEM grade): perguntado
    /// pelas colunas da tabela de itens, respondeu os seis rotulos do formulario -- e o
    /// cadastro virava mestre-detalhe. O texto estava na tela; o que o modelo errou foi o
    /// PAPEL, e o papel e geometria, que o layout le.
    pub fn confirmar_colunas(&self, itens: &[Rotulo]) -> Vec<Rotulo> {
        itens
            .iter()
            .filter(|r| {
                self.achar(&r.texto)
                    .is_some_and(|i| self.frases[i].papel == Papel::Coluna)
            })
            .cloned()
            .collect()
    }

    /// A frase lida com este texto (comparacao do OCR: sem acento, sem caixa). Menu e
    /// titulo ficam de fora: o rotulo de um campo nunca e o item do menu.
    pub fn achar(&self, texto: &str) -> Option<usize> {
        let n = normalizar(texto);
        self.frases.iter().position(|f| {
            !matches!(f.papel, Papel::Menu | Papel::Titulo) && normalizar(&f.texto) == n
        })
    }

    /// Ordem de tabulacao: secao a secao (na ordem em que a secao aparece), e dentro dela
    /// fila a fila -- a do HTML, em que o formulario segue o DOM de cada fieldset.
    fn em_tabulacao(&self, frases: &[usize]) -> Vec<usize> {
        let inicio = |g: Option<usize>| {
            self.frases
                .iter()
                .filter(|f| f.grupo == g)
                .map(|f| f.leitura)
                .min()
                .unwrap_or(u32::MAX)
        };
        let mut v = frases.to_vec();
        v.sort_by_key(|&i| (inicio(self.frases[i].grupo), self.frases[i].leitura));
        v
    }

    fn caixa(&self, x: i32, y: i32, w: i32, h: i32) -> Caixa {
        let (lw, lh) = (self.largura.max(1) as f32, self.altura.max(1) as f32);
        let r = |v: f32| (v * 10000.0).round() / 10000.0;
        Caixa {
            x: r(x as f32 / lw),
            y: r(y as f32 / lh),
            w: r(w as f32 / lw),
            h: r(h as f32 / lh),
        }
    }

    /// O layout no UI-IR: `escolhidos` liga cada frase lida ao campo (entidade, nome) que
    /// ela virou. So entram os grupos que ficaram com algum campo.
    pub fn para_ir(&self, tela: &str, escolhidos: &[(usize, String, String)]) -> ScreenLayout {
        let ordem = self.em_tabulacao(&escolhidos.iter().map(|e| e.0).collect::<Vec<_>>());
        let gid = |g: usize| format!("g{}", g + 1);
        let mut items: Vec<LayoutItem> = escolhidos
            .iter()
            .map(|(i, ent, campo)| {
                let f = &self.frases[*i];
                LayoutItem {
                    entity: ent.clone(),
                    field: campo.clone(),
                    group: f.grupo.map(gid).unwrap_or_default(),
                    bbox: self.caixa(f.x, f.y, f.w, f.h),
                    read_order: f.leitura,
                    tab_order: ordem.iter().position(|o| o == i).unwrap_or(0) as u32,
                }
            })
            .collect();
        items.sort_by_key(|i| i.tab_order);
        let groups = self
            .grupos
            .iter()
            .enumerate()
            .filter(|(g, _)| items.iter().any(|i| i.group == gid(*g)))
            .map(|(g, gr)| LayoutGroup {
                id: gid(g),
                title: gr.titulo.clone(),
                kind: if gr.grade { "grid" } else { "section" }.into(),
                bbox: self.caixa(gr.x, gr.y, gr.w, gr.h),
            })
            .collect();
        ScreenLayout {
            screen: tela.into(),
            groups,
            items,
        }
    }
}

//! Medidor do 495. `detector495 <dir-dos-jsonl>`
use detector495::*;
use phxsql_core::json::Json;
use phxsql_sql::lexico;
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn ler(p: &str) -> Vec<(String, String, String)> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| Json::analisar(l).ok())
        .map(|j| {
            (
                j.texto_ou("sql", "").to_string(),
                j.texto_ou("origem", "").to_string(),
                j.texto_ou("rotulo", "").to_string(),
            )
        })
        .collect()
}

fn mediana(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2], v[v.len() - 1])
}

fn cronometrar<F: FnMut() -> usize>(voltas: usize, n: usize, mut f: F) -> (f64, f64, f64) {
    let mut t = Vec::new();
    for _ in 0..voltas {
        let i = Instant::now();
        black_box(f());
        t.push(i.elapsed().as_nanos() as f64 / n as f64);
    }
    mediana(t)
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let todos = ler(&format!("{dir}/legitimo.jsonl"));
    let (repetitivo, legit): (Vec<_>, Vec<_>) =
        todos.into_iter().partition(|(_, o, _)| o.starts_with("bancada/comando.sql"));
    let det = ler(&format!("{dir}/deteccao.jsonl"));
    let esc = ler(&format!("{dir}/escapado.jsonl"));

    // ---------- falso positivo no legitimo: analisar x recortar
    let mut por_classe: HashMap<&str, usize> = HashMap::new();
    let mut por_classe_rec: HashMap<&str, usize> = HashMap::new();
    let (mut qualquer, mut forte, mut rec_qualquer, mut rec_forte, mut aceitos) = (0, 0, 0, 0, 0);
    let mut acertos = String::new();
    for (sql, origem, _) in &legit {
        let r = detectar(sql);
        let rr = recortar(sql);
        if phxsql_sql::analisar_comando(sql).is_ok() {
            aceitos += 1;
        }
        if r != 0 {
            qualquer += 1;
            acertos.push_str(&format!("{origem}\t{:?}\t{}\n", nomes(r), sql.replace('\n', " ")));
        }
        if r & FORTES != 0 {
            forte += 1;
        }
        if rr != 0 {
            rec_qualquer += 1;
        }
        if rr & FORTES != 0 {
            rec_forte += 1;
        }
        for n in nomes(r) {
            *por_classe.entry(n).or_default() += 1;
        }
        for n in nomes(rr) {
            *por_classe_rec.entry(n).or_default() += 1;
        }
    }
    std::fs::write(format!("{dir}/acusados-legitimo.tsv"), acertos).unwrap();
    println!("== LEGITIMO do repositorio: {} comandos distintos (fora os {} do comando.sql); {} aceitos pelo analisador", legit.len(), repetitivo.len(), aceitos);
    println!("   ANALISAR: acusados {} (forte {}) ; por classe {:?}", qualquer, forte, ordenado(&por_classe));
    println!("   RECORTAR: acusados {} (forte {}) ; por classe {:?}", rec_qualquer, rec_forte, ordenado(&por_classe_rec));

    // ---------- dado escapado (aplicacao segura)
    let (mut e_an, mut e_rec) = (0, 0);
    for (sql, _, _) in &esc {
        if detectar(sql) != 0 {
            e_an += 1;
        }
        if recortar(sql) != 0 {
            e_rec += 1;
        }
    }
    println!("== DADO ESCAPADO ({}): ANALISAR acusa {} ; RECORTAR acusa {}", esc.len(), e_an, e_rec);

    // ---------- deteccao no corpus do repositorio
    println!("== DETECCAO (corpus do repositorio, {} itens):", det.len());
    for (sql, origem, rot) in &det {
        let r = detectar(sql);
        let aceito = phxsql_sql::analisar_comando(sql).is_ok();
        println!("   [{rot:8}] aceito={aceito:5} forte={:5} {:?}  <- {origem}", r & FORTES != 0, nomes(r));
    }

    // ---------- impressao digital (H2)
    let mut fp_legit: Vec<(u64, String)> = Vec::new();
    for (sql, _, _) in &legit {
        if let Ok(s) = lexico::analisar(sql) {
            let n = normalizar(&s);
            fp_legit.push((fnv1a64(&n), sql.clone()));
        }
    }
    let distintas: HashSet<u64> = fp_legit.iter().map(|x| x.0).collect();
    let mut fp_rep: HashSet<u64> = HashSet::new();
    for (sql, _, _) in &repetitivo {
        if let Ok(s) = lexico::analisar(sql) {
            fp_rep.insert(fnv1a64(&normalizar(&s)));
        }
    }
    println!("== IMPRESSAO DIGITAL: legitimo {} textos lexicaveis -> {} digitais ; comando.sql {} textos -> {} digitais",
        fp_legit.len(), distintas.len(), repetitivo.len(), fp_rep.len());
    // aprendizado: metade por hash do texto aprende, a outra metade e testada
    let (mut aprend, mut teste) = (HashSet::new(), Vec::new());
    for (h, sql) in &fp_legit {
        if fnv1a64(sql) % 2 == 0 {
            aprend.insert(*h);
        } else {
            teste.push(*h);
        }
    }
    let novos = teste.iter().filter(|h| !aprend.contains(h)).count();
    println!("   aprende 50% / testa 50%: {} de {} ({:.1}%) do teste teriam digital NOVA (= recusa/alerta em modo protegendo)",
        novos, teste.len(), 100.0 * novos as f64 / teste.len() as f64);
    // ataque vs o modelo legitimo de onde saiu: a digital muda?
    let modelo = normalizar(&lexico::analisar("SELECT * FROM clientes WHERE nome = 'Alves'").unwrap());
    for (sql, _, rot) in det.iter().filter(|d| d.2 == "ataque") {
        let d = lexico::analisar(sql).map(|s| normalizar(&s)).unwrap_or_else(|_| "<nao lexica>".into());
        println!("   [{rot}] digital {} a do modelo", if d == modelo { "IGUAL" } else { "DIFERENTE de" });
    }

    // ---------- custo
    let n = legit.len();
    let voltas = 41;
    let base = cronometrar(voltas, n, || legit.iter().filter(|x| phxsql_sql::analisar_comando(&x.0).is_ok()).count());
    let lex = cronometrar(voltas, n, || legit.iter().filter(|x| lexico::analisar(&x.0).is_ok()).count());
    let det_rel = cronometrar(voltas, n, || legit.iter().map(|x| detectar(&x.0) as usize).sum());
    let simbolos: Vec<Option<Vec<lexico::Simbolo>>> = legit.iter().map(|x| lexico::analisar(&x.0).ok()).collect();
    let det_sim = cronometrar(voltas, n, || {
        legit.iter().zip(&simbolos).map(|(x, s)| match s {
            Some(s) => detectar_com_simbolos(&x.0, s, false) as usize,
            None => varrer_comentarios(&x.0) as usize,
        }).sum()
    });
    let so_sim = cronometrar(voltas, n, || simbolos.iter().flatten().map(|s| classes_dos_simbolos(s) as usize).sum());
    let so_coment = cronometrar(voltas, n, || legit.iter().map(|x| varrer_comentarios(&x.0) as usize).sum());
    let digital = cronometrar(voltas, n, || simbolos.iter().flatten().map(|s| fnv1a64(&normalizar(s)) as usize).sum());
    let flag = AtomicBool::new(black_box(false));
    let desl = cronometrar(voltas, n * 1000, || {
        let mut c = 0;
        for _ in 0..n * 1000 {
            if black_box(&flag).load(Ordering::Relaxed) {
                c += 1;
            }
        }
        c
    });
    let bytes: usize = legit.iter().map(|x| x.0.len()).sum();
    println!("== CUSTO por comando (ns, min/mediana/max de {voltas} voltas sobre {n} comandos, {:.0} bytes medios)", bytes as f64 / n as f64);
    println!("   analisar_comando (o que op_sql ja paga) : {:.0} / {:.0} / {:.0}", base.0, base.1, base.2);
    println!("   so o lexico                              : {:.0} / {:.0} / {:.0}", lex.0, lex.1, lex.2);
    println!("   detector relexando (+empilhado reanalisa): {:.0} / {:.0} / {:.0}", det_rel.0, det_rel.1, det_rel.2);
    println!("   detector com os simbolos ja prontos      : {:.0} / {:.0} / {:.0}", det_sim.0, det_sim.1, det_sim.2);
    println!("     .. so classes dos simbolos             : {:.0} / {:.0} / {:.0}", so_sim.0, so_sim.1, so_sim.2);
    println!("     .. so varredura de comentario          : {:.0} / {:.0} / {:.0}", so_coment.0, so_coment.1, so_coment.2);
    println!("   impressao digital (normalizar + fnv1a)   : {:.0} / {:.0} / {:.0}", digital.0, digital.1, digital.2);
    println!("   DESLIGADO: leitura de um AtomicBool      : {:.2} / {:.2} / {:.2}", desl.0, desl.1, desl.2);
}

fn ordenado<'a>(m: &HashMap<&'a str, usize>) -> Vec<(&'a str, usize)> {
    let mut v: Vec<_> = m.iter().map(|(k, v)| (*k, *v)).collect();
    v.sort();
    v
}

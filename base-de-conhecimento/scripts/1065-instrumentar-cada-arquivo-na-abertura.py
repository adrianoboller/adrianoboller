# Instrumentar cada arquivo na abertura
# 29/08 04:24

import io
p='crates/phxsql-store/examples/abrir-cresce.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''        let inicio = Instant::now();
        for _ in 0..AMOSTRAS {
            let aberta = Table::abrir(&dir, "precos").unwrap();
            drop(aberta);
        }
        let abrir_ms = inicio.elapsed().as_secs_f64() * 1e3 / AMOSTRAS as f64;'''
novo = '''        let inicio = Instant::now();
        for _ in 0..AMOSTRAS {
            let aberta = Table::abrir(&dir, "precos").unwrap();
            drop(aberta);
        }
        let abrir_ms = inicio.elapsed().as_secs_f64() * 1e3 / AMOSTRAS as f64;

        // Qual dos sete arquivos cresce? Cada um medido sozinho.
        let mut por_arquivo = Vec::new();
        macro_rules! medir {
            ($rotulo:expr, $expr:expr) => {{
                let i = Instant::now();
                for _ in 0..AMOSTRAS {
                    let x = $expr;
                    drop(x);
                }
                por_arquivo.push(($rotulo, i.elapsed().as_secs_f64() * 1e3 / AMOSTRAS as f64));
            }};
        }
        let pag = phxsql_store::reg::RegFile::abrir(&dir, "precos")
            .unwrap()
            .esquema()
            .paginacao();
        let ext = pag.para_externos();
        medir!(".reg", phxsql_store::reg::RegFile::abrir(&dir, "precos").unwrap());
        medir!(
            ".ndx",
            phxsql_store::ndx::NdxFile::abrir(dir.join("precos.ndx")).unwrap()
        );
        medir!(
            ".log",
            phxsql_store::log::LogFile::abrir(&dir, "precos", ext).unwrap()
        );
        if primeira_vez {
            println!("  (uma vez) tempo de abrir cada arquivo, isolado:");
        }
        let detalhe: Vec<String> = por_arquivo
            .iter()
            .map(|(r, ms)| format!("{r} {ms:.2}ms"))
            .collect();
        println!("      {}", detalhe.join("   "));
        primeira_vez = false;'''
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace('    let mut feitas = 0i64;','    let mut primeira_vez = true;\n    let mut feitas = 0i64;')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

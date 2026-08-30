# Add the CLI import command
# 28/08 19:33

import io
p='crates/phxsql-cli/src/main.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''  phxsql reparar   <dir> <tabela>
";''','''  phxsql reparar   <dir> <tabela>
  phxsql importar  <dir> <tabela> <arquivo> [--formato csv|txt|json|xml|html]
                                            [--seguir] [--conferir]
";''',1)
s=s.replace('''        "reparar" => exigir(&args, 3).and_then(|_| reparar(Path::new(&args[1]), &args[2])),''',
            '''        "reparar" => exigir(&args, 3).and_then(|_| reparar(Path::new(&args[1]), &args[2])),
        "importar" => exigir(&args, 4).and_then(|_| importar(&args)),''',1)

# a funcao, no fim do arquivo antes dos testes (ou no fim)
funcao = '''

/// `phxsql importar <dir> <tabela> <arquivo>` -- carga em lote de um arquivo.
///
/// Le pelo MESMO caminho do servidor: `phxsql_core::carga`. Uma segunda
/// implementacao do leitor aqui divergiria da do servidor no primeiro caso
/// esquisito -- e caso esquisito e o que carga de arquivo tem de sobra.
///
/// `--conferir` le e mostra o que entendeu sem gravar nada. `--seguir` pula a
/// linha ruim em vez de parar; sem ele, para na primeira -- porque **nao ha
/// transacao**, e uma carga que para na linha 700 e mais facil de consertar do
/// que uma que gravou 999 com uma faltando no meio.
fn importar(args: &[String]) -> Result<()> {
    let dir = Path::new(&args[1]);
    let tabela = &args[2];
    let arquivo = &args[3];
    let seguir = args.iter().any(|a| a == "--seguir");
    let so_conferir = args.iter().any(|a| a == "--conferir");

    let texto = std::fs::read_to_string(arquivo)?;
    let formato = match valor_da_opcao(args, "--formato") {
        Some(f) => carga::Formato::de_texto(&f)?,
        None => carga::adivinhar(&texto),
    };
    let c = carga::ler(&texto, formato)?;

    let mut t = Table::abrir(dir, tabela)?;
    let esquema = t.esquema().clone();

    diga!("arquivo .... {arquivo}");
    diga!("formato .... {} ({})", formato.nome(),
          if valor_da_opcao(args, "--formato").is_some() { "escolhido" } else { "adivinhado" });
    diga!("colunas .... {}", c.colunas.join(", "));
    diga!("linhas ..... {}", c.linhas.len());

    let desconhecidas: Vec<&String> = c
        .colunas
        .iter()
        .filter(|n| esquema.coluna_por_nome(n).is_none())
        .collect();
    if !desconhecidas.is_empty() {
        return Err(phxsql_core::PhxError::Esquema(format!(
            "a tabela {tabela} nao tem a(s) coluna(s): {}",
            desconhecidas
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    if so_conferir {
        diga!();
        diga!("-- amostra (as 5 primeiras, como o leitor entendeu) --");
        for (i, l) in c.linhas.iter().take(5).enumerate() {
            diga!("{:>4}  {}", i + 1, l.join(" | "));
        }
        diga!();
        diga!("nada foi gravado (--conferir)");
        return Ok(());
    }

    let inicio = std::time::Instant::now();
    let mut linhas = Vec::with_capacity(c.linhas.len());
    let mut recusadas: Vec<(usize, String)> = Vec::new();
    for i in 0..c.linhas.len() {
        match carga::linha_de_texto(&c, i, &esquema) {
            Ok(l) => linhas.push(l),
            Err(e) => {
                recusadas.push((i, e.to_string()));
                if !seguir {
                    break;
                }
            }
        }
    }
    let lote = t.inserir_lote(&linhas, !seguir)?;
    t.sincronizar()?;
    for (i, e) in &lote.recusadas {
        recusadas.push((*i, e.clone()));
    }
    let ms = inicio.elapsed().as_millis().max(1);

    diga!();
    diga!("gravadas ... {} em {ms} ms ({} linhas/s)",
          lote.rowids.len(), lote.rowids.len() as u128 * 1000 / ms);
    if let (Some(a), Some(b)) = (lote.rowids.first(), lote.rowids.last()) {
        diga!("rowid ...... {a} a {b}");
    }
    if !recusadas.is_empty() {
        diga!("recusadas .. {}", recusadas.len());
        for (i, e) in recusadas.iter().take(20) {
            diga!("  linha {:>5}: {e}", i + 1);
        }
        // Sem transacao, o que entrou antes do erro FICOU. Dizer aqui e melhor
        // que quem rodou descobrir contando as linhas depois.
        diga!();
        diga!("ATENCAO: nao ha transacao -- as linhas gravadas antes do erro ficaram gravadas.");
    }
    Ok(())
}

/// O valor de uma opcao `--nome valor` na linha de comando.
fn valor_da_opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
'''
s = s.rstrip() + funcao
s = s.replace("use phxsql_core::datahora::{data_iso, hora_iso};",
              "use phxsql_core::carga;\nuse phxsql_core::datahora::{data_iso, hora_iso};",1)
io.open(p,'w',encoding='utf-8').write(s)

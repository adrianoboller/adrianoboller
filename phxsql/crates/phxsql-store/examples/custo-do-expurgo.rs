//! Quanto custa DECIDIR o expurgo da trilha `.lgpd` (pedido 368).
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # binario velho mede o passado
//! cargo run --release --example custo-do-expurgo -p phxsql-store -- [MiB] [MiB_por_volume]
//! ```
//!
//! O expurgo apaga volume inteiro, e apagar e um `unlink`: nao custa nada que
//! valha medir. O que custa e o PLANO -- a varredura que confere, registro a
//! registro, se todo carimbo do volume e anterior ao limite. Ela roda com a
//! trava global na mao (fase 1), entao o numero que interessa e quanto tempo
//! ela segura o servidor. Tres casos, na mesma trilha:
//!
//! 1. **tudo vencido** (limite amanha): anda por todo volume fechado -- e o
//!    custo de derrubar a trilha inteira, pago UMA vez na vida de cada volume;
//! 2. **nada vencido** (limite em 1970): para no primeiro registro do
//!    primeiro volume -- e o custo da passada diaria numa trilha nova;
//! 3. **o `ler(0, 0)` da trilha inteira**, como referencia: e o que a op
//!    `trilha` paga para mostrar tudo. NAO e o mesmo trabalho (o `ler`
//!    decodifica os cinco textos de cada registro e aloca um `Evento`), e por
//!    isso nao se tira dele um «ganho»: ele so situa a ordem de grandeza.
//!
//! O cache do nucleo esta QUENTE -- a trilha acabou de ser escrita. Disco frio
//! custa mais, e este medidor nao o provoca: diz isso em vez de esconder.
//! Todo numero impresso saiu desta corrida.

use std::time::Instant;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::trilha::TrilhaFile;

fn mediana(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mib: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(64);
    let por_volume: u64 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(8);
    let d = std::env::temp_dir().join(format!("phx-custo-expurgo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();

    // O corte da trilha e dela (formato B, pedido 368): `lgpd.volume_mib`, e
    // nao a paginacao da tabela. A paginacao so diz a largura de um sufixo
    // legado, e aqui nao ha legado.
    phxsql_store::trilha::definir_corte(por_volume * 1024 * 1024, 30);
    let pag = Paginacao::DESLIGADA;
    let mut t = TrilhaFile::abrir(&d, "t", pag).unwrap();
    // Um criterio do tamanho dos que o `varrer` grava de verdade.
    let criterio = "varrer indice=por_nome visao=ativas modo=cursor pular=0 filtro=cidade";
    let comeco = Instant::now();
    let mut n = 0u64;
    while std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum::<u64>()
        < mib * 1024 * 1024
    {
        for _ in 0..10_000 {
            t.registrar_acesso(n, "nome,cpf,email", criterio, 37)
                .unwrap();
            n += 1;
        }
    }
    let gravar = comeco.elapsed().as_secs_f64();
    let volumes = t.volumes_existentes().unwrap();
    let bytes: u64 = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".lgpd"))
        .map(|e| e.metadata().unwrap().len())
        .sum();
    println!(
        "trilha: {n} registros, {} volumes, {:.1} MiB ({:.1} s para gravar, {:.0} bytes/registro)",
        volumes.len(),
        bytes as f64 / 1048576.0,
        gravar,
        bytes as f64 / n as f64
    );
    let fechados_mib =
        (bytes as f64 - std::fs::metadata(d.join("t.lgpd")).unwrap().len() as f64) / 1048576.0;

    let amanha = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        + 86_400_000;
    let mut tudo = Vec::new();
    let mut nada = Vec::new();
    let mut ler = Vec::new();
    for _ in 0..5 {
        let mut t = TrilhaFile::abrir(&d, "t", pag).unwrap();
        let c = Instant::now();
        let e = t.planejar_expurgo(amanha).unwrap();
        tudo.push(c.elapsed().as_secs_f64() * 1e3);
        assert_eq!(e.volumes.len(), volumes.len() - 1);

        let c = Instant::now();
        let e = t.planejar_expurgo(0).unwrap();
        nada.push(c.elapsed().as_secs_f64() * 1e3);
        assert!(e.volumes.is_empty());

        let c = Instant::now();
        let lidos = t.ler(0, 0).unwrap();
        ler.push(c.elapsed().as_secs_f64() * 1e3);
        assert_eq!(lidos.len() as u64, n);
    }
    let (m, lo, hi) = mediana(tudo);
    println!(
        "plano, tudo vencido ....... {m:8.1} ms  (min {lo:.1} / max {hi:.1})  = {:.2} ms por MiB fechado",
        m / fechados_mib
    );
    let (m, lo, hi) = mediana(nada);
    println!("plano, nada vencido ....... {m:8.3} ms  (min {lo:.3} / max {hi:.3})");
    let (m, lo, hi) = mediana(ler);
    println!(
        "ler(0,0) da trilha inteira  {m:8.1} ms  (min {lo:.1} / max {hi:.1})  -- referencia, nao o mesmo trabalho"
    );
    println!("cache do nucleo: QUENTE (a trilha acabou de ser escrita); 5 repeticoes");
    let _ = std::fs::remove_dir_all(&d);
}

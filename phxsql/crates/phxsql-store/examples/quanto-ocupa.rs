//! Quanto o `.log`, a `.trash` e o `.reason` ocupam de verdade -- e o que
//! compactar volume fechado economizaria.
//!
//! ```bash
//! cargo run --release --example quanto-ocupa -- [linhas] [%exclusoes]
//! ```
//!
//! O pedido 101 pede "cifrar e compactar" os tres arquivos, e a pendencia diz
//! que compactar esbarra em "rotacionar e reescrever". Antes de resolver, este
//! medidor responde tres perguntas que ninguem tinha respondido com numero:
//!
//! 1. **Quanto os tres ocupam**, ao lado do `.reg` e do `.ndx`? Se forem 2% do
//!    total, compactar pela metade poupa 1% -- e a resposta certa e nao fazer.
//! 2. **Quanto encolhem?** Nao por estimativa: passando os bytes de verdade
//!    pelo DEFLATE que o backup ja usa (`phxsql_core::zip::deflate`).
//! 3. **Ha volume fechado para compactar?** Esta e a pergunta que derruba as
//!    outras duas, e por isso a segunda passagem existe.
//!
//! E mede o `.reg` e o `.ndx` pelo mesmo cano, porque a decisao nao e
//! "compactar o diario e bom?" e sim "de todo o espaco que a tabela ocupa,
//! quanto esta nos tres arquivos que o pedido cita?".
//!
//! # Por que "volume fechado" e a unidade certa
//!
//! Os tres arquivos ja sao paginados: `Tabela_001.log`, `Tabela_002.log`, ...
//! Um volume que nao e o ultimo **nunca mais recebe escrita** -- entao
//! compacta-lo nao exige rotacionar nada, que era o bloqueio registrado.
//!
//! A segunda passagem existe porque um volume so fecha quando estoura
//! `bytes_por_arquivo`, e o padrao e 1 GiB. Ela repete a medida com o volume
//! curto, para saber quanto um volume fechado encolhe DE FATO -- e o relatorio
//! junta as duas para dizer a partir de que tamanho de tabela isso paga.
//!
//! # A quarta pergunta, que a rodada seguinte acrescentou
//!
//! O corte do diario virou configuravel -- `recursos.diario_volume_mib`, ver
//! `phxsql_store::diario` --, entao "nao ha volume fechado" deixou de ser uma
//! propriedade do formato e virou uma escolha. A pergunta muda de lugar:
//!
//! 4. **Com o corte pequeno, compactar volume fechado PAGA?** Nao basta saber
//!    quanto se economiza. O `.log` e lido em lote pela replicacao e inteiro
//!    pelo `historico`, e um volume compactado tem de ser inflado INTEIRO para
//!    servir um lote de 500 eventos. A quarta secao cronometra os dois lados e
//!    conta o TETO -- porque `max_arquivos` continua valendo, e cortar pequeno
//!    encolhe o diario que cabe.
//!
//! ```bash
//! cargo run --release --example quanto-ocupa -- 1000000 5 8
//! #                                             linhas  %  MiB do diario
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use phxsql_core::paginacao::{Paginacao, BYTES_POR_ARQUIVO_PADRAO};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::zip::deflate;
use phxsql_store::log::EVENTO_CAB;
use phxsql_store::table::Table;

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

const EXTENSOES: [&str; 5] = ["reg", "ndx", "log", "trash", "reason"];

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("produto", ColumnType::Str(40)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(20)),
        Column::new(
            "valor",
            ColumnType::Decimal {
                precisao: 15,
                escala: 2,
            },
        ),
        Column::new("cadastro", ColumnType::Date),
    ]
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        Value::Decimal(((i % 900_000) + 100) as i128),
        Value::Date(20_000 + (i % 400) as i32),
    ]
}

/// Todos os arquivos de uma extensao, em ordem de volume.
fn volumes(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == ext).unwrap_or(false))
        .collect();
    v.sort();
    v
}

fn bytes(dir: &Path, ext: &str) -> u64 {
    volumes(dir, ext)
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

/// O retrato de uma tabela montada: bytes e volumes por extensao, e a razao de
/// compressao medida no primeiro volume de cada uma.
struct Retrato {
    bytes: Vec<(String, u64)>,
    volumes: Vec<(String, usize)>,
    razao: Vec<(String, f64)>,
    /// MiB por segundo do DEFLATE, medidos aqui e nao citados de outro dia.
    mib_por_s: f64,
}

impl Retrato {
    fn bytes_de(&self, ext: &str) -> u64 {
        self.bytes
            .iter()
            .find(|(e, _)| e == ext)
            .map(|(_, b)| *b)
            .unwrap_or(0)
    }
    fn volumes_de(&self, ext: &str) -> usize {
        self.volumes
            .iter()
            .find(|(e, _)| e == ext)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    }
    fn razao_de(&self, ext: &str) -> f64 {
        self.razao
            .iter()
            .find(|(e, _)| e == ext)
            .map(|(_, r)| *r)
            .unwrap_or(1.0)
    }
    fn total(&self) -> u64 {
        self.bytes.iter().map(|(_, b)| *b).sum()
    }
}

fn montar(rotulo: &str, n: i64, pct_exclusao: f64, bytes_por_volume: u64, falar: bool) -> Retrato {
    let dir =
        std::env::temp_dir().join(format!("phx-quanto-ocupa-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Quatro volumes de `.reg`, e portanto tres fechados. Fica em funcao de
    // `n` para que a forma medida seja a mesma numa rodada curta de
    // conferencia e na rodada de um milhao.
    let mut paginacao = Paginacao::nova((n as u64 / 4).max(1), 999).unwrap();
    paginacao.bytes_por_arquivo = bytes_por_volume;

    let esquema = Schema::new(
        "precos",
        colunas(),
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
    .com_paginacao(paginacao)
    .unwrap();

    let mut t = Table::criar(&dir, esquema).unwrap();
    let inicio = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    if falar {
        println!(
            "  insercao .................... {:.1}s  ({:.1} us por linha)",
            inicio.elapsed().as_secs_f64(),
            inicio.elapsed().as_secs_f64() * 1e6 / n as f64
        );
    }

    // Exclusoes: metade suave (so `.reason`) e metade de vez (`.trash` +
    // `.reason`). E a mistura que uma tabela viva tem.
    let quantas = (((n as f64) * pct_exclusao / 100.0) as i64).max(1);
    let passo = (n as u64 / quantas as u64).max(1);
    let inicio = Instant::now();
    let (mut suaves, mut fisicas) = (0u64, 0u64);
    for k in 0..quantas {
        // Espalhadas pela tabela, e nao no comeco: o `.trash` guarda o payload
        // e o `.reason` guarda a identidade, e as duas variam com a linha.
        let rowid = (1 + (k as u64) * passo).min(n as u64);
        if k % 2 == 0 {
            if t.excluir_suave(rowid, "revisao de cadastro").unwrap() {
                suaves += 1;
            }
        } else if t.excluir_de_vez(rowid, "duplicidade confirmada").unwrap() {
            fisicas += 1;
        }
    }
    t.sincronizar().unwrap();
    if falar {
        println!(
            "  exclusoes ................... {:.1}s  ({suaves} suaves, {fisicas} de vez)",
            inicio.elapsed().as_secs_f64()
        );
    }
    drop(t);

    let mut retrato = Retrato {
        bytes: Vec::new(),
        volumes: Vec::new(),
        razao: Vec::new(),
        mib_por_s: 0.0,
    };
    let mut mib_totais = 0.0;
    let mut segundos_totais = 0.0;
    for ext in EXTENSOES {
        retrato.bytes.push((ext.into(), bytes(&dir, ext)));
        let v = volumes(&dir, ext);
        retrato.volumes.push((ext.into(), v.len()));
        if let Some(primeiro) = v.first() {
            let dados = std::fs::read(primeiro).unwrap();
            if !dados.is_empty() {
                let inicio = Instant::now();
                let saida = deflate(&dados);
                let s = inicio.elapsed().as_secs_f64();
                mib_totais += mib(dados.len() as u64);
                segundos_totais += s;
                retrato
                    .razao
                    .push((ext.into(), dados.len() as f64 / saida.len().max(1) as f64));
            }
        }
    }
    retrato.mib_por_s = if segundos_totais > 0.0 {
        mib_totais / segundos_totais
    } else {
        0.0
    };

    let _ = std::fs::remove_dir_all(&dir);
    retrato
}

/// Bytes dos volumes FECHADOS de uma extensao, e o que sobraria deles.
///
/// Mede os arquivos inteiros, e nao uma amostra: o que se quer saber e quanto
/// espaco some do disco, e isso e a soma dos volumes que nunca mais recebem
/// escrita.
fn fechados(dir: &Path, ext: &str) -> (u64, u64, usize, f64) {
    let v = volumes(dir, ext);
    if v.len() < 2 {
        return (0, 0, 0, 0.0);
    }
    let (mut crus, mut comprimidos) = (0u64, 0u64);
    let inicio = Instant::now();
    for caminho in &v[..v.len() - 1] {
        let dados = std::fs::read(caminho).unwrap();
        crus += dados.len() as u64;
        comprimidos += deflate(&dados).len() as u64;
    }
    (
        crus,
        comprimidos,
        v.len() - 1,
        inicio.elapsed().as_secs_f64(),
    )
}

/// A quarta secao: com o corte do diario configurado, compactar paga?
///
/// Monta a tabela com o `recursos.diario_volume_mib` valendo -- so o diario
/// corta pequeno; o `.reg` e o `.ndx` continuam com o volume do esquema -- e
/// mede os dois lados da conta: o que se economiza e o que se paga para ler.
fn quarta_secao(n: i64, pct_exclusao: f64, corte_mib: u64) {
    let corte = corte_mib * 1024 * 1024;
    phxsql_store::diario::definir_bytes_por_volume(corte);

    let dir = std::env::temp_dir().join(format!("phx-quanto-ocupa-{}-corte", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let paginacao = Paginacao::nova((n as u64 / 4).max(1), 999).unwrap();
    let esquema = Schema::new(
        "precos",
        colunas(),
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
    .com_paginacao(paginacao)
    .unwrap();

    let mut t = Table::criar(&dir, esquema).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    let quantas = (((n as f64) * pct_exclusao / 100.0) as i64).max(1);
    let passo = (n as u64 / quantas as u64).max(1);
    for k in 0..quantas {
        let rowid = (1 + (k as u64) * passo).min(n as u64);
        if k % 2 == 0 {
            t.excluir_suave(rowid, "revisao de cadastro").unwrap();
        } else {
            t.excluir_de_vez(rowid, "duplicidade confirmada").unwrap();
        }
    }
    t.sincronizar().unwrap();
    drop(t);

    let total_tabela: u64 = EXTENSOES.iter().map(|e| bytes(&dir, e)).sum();

    println!("\n=== 4. com `recursos.diario_volume_mib = {corte_mib}`, compactar paga? ===\n");
    let (mut soma_crus, mut soma_comp) = (0u64, 0u64);
    let mut segundos = 0.0;
    for ext in ["log", "trash", "reason"] {
        let (crus, comp, quantos, s) = fechados(&dir, ext);
        soma_crus += crus;
        soma_comp += comp;
        segundos += s;
        println!(
            "  .{ext:<9} {:>3} volume(s), {:>3} fechado(s), {:>8.2} MiB fechados -> {:>7.2} MiB  ({:.2}x)",
            volumes(&dir, ext).len(),
            quantos,
            mib(crus),
            mib(comp),
            crus as f64 / comp.max(1) as f64
        );
    }
    let economia = soma_crus.saturating_sub(soma_comp);
    println!(
        "\n  Economia: {:.2} MiB de uma tabela de {:.2} MiB = {:.2}% do total.",
        mib(economia),
        mib(total_tabela),
        economia as f64 / total_tabela.max(1) as f64 * 100.0
    );
    println!(
        "  Custo de comprimir uma vez: {:.2}s para {:.2} MiB ({:.1} MiB/s).",
        segundos,
        mib(soma_crus),
        mib(soma_crus) / segundos.max(1e-9)
    );

    // ------------------------------------------------- o outro lado da conta
    //
    // Um volume compactado nao se le por dentro: para servir um lote de 500
    // eventos e preciso inflar o volume INTEIRO. Este e o custo que nao
    // aparece no numero da economia.
    let vols = volumes(&dir, "log");
    if vols.len() >= 2 {
        let primeiro = std::fs::read(&vols[0]).unwrap();
        let comprimido = deflate(&primeiro);

        let inicio = Instant::now();
        let mut log = phxsql_store::log::LogFile::abrir(&dir, "precos", paginacao).unwrap();
        let lote = log.ler(0, 500).unwrap();
        let ler_lote = inicio.elapsed().as_secs_f64();
        assert_eq!(lote.len(), 500);

        let inicio = Instant::now();
        let voltou = phxsql_core::zip::inflate_fixo(&comprimido).expect("inflate falhou");
        let inflar = inicio.elapsed().as_secs_f64();
        assert_eq!(voltou.len(), primeiro.len(), "o inflate nao devolveu igual");

        println!("\n  O outro lado da conta -- o que custa LER um volume compactado:\n");
        println!(
            "    ler 500 eventos do volume como ele e ...... {:>8.3} ms",
            ler_lote * 1e3
        );
        println!(
            "    inflar o volume inteiro para servir os 500 . {:>8.3} ms   ({:.0}x)",
            inflar * 1e3,
            inflar / ler_lote.max(1e-9)
        );
        println!(
            "    (o volume inflado tem {:.2} MiB; o diario inteiro tem {} eventos\n     \
             em {} volumes)",
            mib(primeiro.len() as u64),
            log.total().unwrap(),
            vols.len()
        );

        // ------------------------------------------------------- o veredito
        //
        // Escrito a partir dos numeros medidos acima, e nao digitado: um
        // veredito digitado envelhece calado quando a medida muda.
        println!("\n  O veredito da quarta pergunta:\n");
        if economia == 0 {
            println!(
                "    Com o corte de {corte_mib} MiB ainda nao ha volume fechado nos tres.\n    \
                 Compactar continua poupando zero."
            );
        } else {
            println!(
                "    Compactar os volumes fechados poupa {:.2} MiB -- {:.2}% da tabela --\n    \
                 e custa {:.0}x no tempo de servir um lote de 500 eventos, porque um\n    \
                 volume compactado tem de ser inflado INTEIRO para entregar qualquer\n    \
                 pedaco dele. O servidor abre e fecha a tabela a cada pedido, entao\n    \
                 nao ha cache que segure o volume inflado entre um lote e o seguinte.",
                mib(economia),
                economia as f64 / total_tabela.max(1) as f64 * 100.0,
                inflar / ler_lote.max(1e-9)
            );
            println!(
                "\n    E a comparacao que decide: compactar SO O `.ndx` poupa {:.2} MiB\n    \
                 sem custo nenhum de leitura de diario. O espaco continua nao\n    \
                 estando onde o pedido olha.",
                {
                    let b = bytes(&dir, "ndx");
                    let dados = std::fs::read(&volumes(&dir, "ndx")[0]).unwrap();
                    let razao = dados.len() as f64 / deflate(&dados).len().max(1) as f64;
                    mib(b) - mib(b) / razao
                }
            );
        }
    }

    // -------------------------------------------------------------- o teto
    let eventos_por_volume = corte / EVENTO_CAB as u64;
    println!(
        "\n  E o TETO, que cortar pequeno encolhe: {} volumes x {corte_mib} MiB\n  \
         = {:.1} GiB de diario, ou ~{:.1} milhoes de eventos sem imagem.\n  \
         Com o padrao de 1 GiB seriam {:.0} TiB e ~{:.0} bilhoes.",
        paginacao.max_arquivos,
        (paginacao.max_arquivos as u64 * corte) as f64 / (1024.0 * 1024.0 * 1024.0),
        (paginacao.max_arquivos as u64 * eventos_por_volume) as f64 / 1e6,
        (paginacao.max_arquivos as u64 * BYTES_POR_ARQUIVO_PADRAO) as f64
            / (1024.0 * 1024.0 * 1024.0 * 1024.0),
        (paginacao.max_arquivos as u64 * (BYTES_POR_ARQUIVO_PADRAO / EVENTO_CAB as u64)) as f64
            / 1e9
    );

    phxsql_store::diario::definir_bytes_por_volume(0);
    let _ = std::fs::remove_dir_all(&dir);
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let pct_exclusao: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5.0);

    println!("=== 1. a tabela como ela e: {n} linhas, volume padrao de 1 GiB ===\n");
    let r = montar("padrao", n, pct_exclusao, BYTES_POR_ARQUIVO_PADRAO, true);
    let total = r.total();

    println!("\n=== o que a tabela ocupa em disco ===\n");
    for ext in EXTENSOES {
        let b = r.bytes_de(ext);
        println!(
            "  .{ext:<9} {:>10.2} MiB  {:>6.2}%   {} volume(s)",
            mib(b),
            b as f64 / total as f64 * 100.0,
            r.volumes_de(ext)
        );
    }
    println!("  {:-<10} {:>10.2} MiB  100.00%", " TOTAL ", mib(total));

    let tres = r.bytes_de("log") + r.bytes_de("trash") + r.bytes_de("reason");
    println!(
        "\n  Os TRES arquivos do pedido 101 somam {:.2} MiB = {:.2}% do total.",
        mib(tres),
        tres as f64 / total as f64 * 100.0
    );

    println!("\n=== o que o DEFLATE faz com o primeiro volume de cada um ===\n");
    for ext in EXTENSOES {
        let razao = r.razao_de(ext);
        if razao <= 1.0 {
            continue;
        }
        println!(
            "  .{ext:<9} {:>6.2}x   -- de {:.2} MiB sobrariam {:.2} MiB",
            razao,
            mib(r.bytes_de(ext)),
            mib(r.bytes_de(ext)) / razao
        );
    }
    println!(
        "\n  (o DEFLATE deste repositorio anda a {:.1} MiB/s)",
        r.mib_por_s
    );

    // ------------------------------------------------------------------
    // A pergunta que derruba as outras: ha volume FECHADO?
    // ------------------------------------------------------------------
    let fechados: usize = ["log", "trash", "reason"]
        .iter()
        .map(|e| r.volumes_de(e).saturating_sub(1))
        .sum();
    let eventos_por_volume = BYTES_POR_ARQUIVO_PADRAO / EVENTO_CAB as u64;

    println!("\n=== 2. ha volume fechado para compactar? ===\n");
    println!(
        "  Volumes FECHADOS de `.log` + `.trash` + `.reason`: {fechados}\n\
         \n  Os tres cortam volume por BYTES, nao por linhas, e o padrao e\n  \
         {:.0} GiB. Um evento sem imagem tem {EVENTO_CAB} bytes, entao o `.log`\n  \
         so fecha o primeiro volume em ~{:.1} milhoes de eventos.",
        BYTES_POR_ARQUIVO_PADRAO as f64 / (1024.0 * 1024.0 * 1024.0),
        eventos_por_volume as f64 / 1e6
    );

    // Segunda passagem, com volume curto, para saber quanto um volume que
    // FECHA de fato encolhe -- sem isso o numero acima seria so aritmetica.
    println!("\n=== 3. e quando o volume fecha? (volume de 512 KiB) ===\n");
    let curto = montar("curto", n.min(200_000), pct_exclusao, 512 * 1024, false);
    for ext in ["log", "trash", "reason"] {
        let vols = curto.volumes_de(ext);
        println!(
            "  .{ext:<9} {vols:>3} volume(s), {:>3} fechado(s), razao {:>5.2}x",
            vols.saturating_sub(1),
            curto.razao_de(ext)
        );
    }

    // ------------------------------------------------------------ veredito
    println!("\n=== o veredito ===\n");
    if fechados == 0 {
        println!(
            "  Com {n} linhas NAO HA UM SO VOLUME FECHADO nos tres arquivos.\n  \
             Compactar por volume fechado pouparia exatamente 0 byte -- nao\n  \
             porque compactar nao funcione (funciona: {:.2}x no `.log`), mas\n  \
             porque nao ha o que compactar.",
            r.razao_de("log")
        );
        let linhas_para_fechar = eventos_por_volume;
        println!(
            "\n  O primeiro volume de `.log` fecha em ~{:.1} milhoes de linhas.\n  \
             Ate la, compactar exige reescrever o volume ABERTO -- que e\n  \
             exatamente o bloqueio ja registrado na pendencia, e continua de pe.",
            linhas_para_fechar as f64 / 1e6
        );
    } else {
        println!("  Ha {fechados} volume(s) fechado(s): compactar ja tem o que fazer.");
    }

    let economia_log = tres as f64 - tres as f64 / r.razao_de("log");
    println!(
        "\n  Mesmo supondo que TUDO fosse compactavel hoje, os tres a {:.2}x\n  \
         poupariam {:.2} MiB de {:.2} MiB: {:.2}% da tabela.",
        r.razao_de("log"),
        economia_log / (1024.0 * 1024.0),
        mib(total),
        economia_log / total as f64 * 100.0
    );

    let corte_mib: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    quarta_secao(n, pct_exclusao, corte_mib);

    println!("\n=== o veredito, agora com o corte configuravel ===");
    let ndx = r.bytes_de("ndx");
    let economia_ndx = ndx as f64 - ndx as f64 / r.razao_de("ndx");
    println!(
        "\n  Para comparar: o `.ndx` ocupa {:.2}% e comprime {:.2}x. Compactar\n  \
         SO ELE poupa {:.2} MiB -- {:.1}x mais que os tres do pedido juntos.\n  \
         O espaco nao esta onde o pedido olha.",
        ndx as f64 / total as f64 * 100.0,
        r.razao_de("ndx"),
        economia_ndx / (1024.0 * 1024.0),
        economia_ndx / economia_log.max(1.0)
    );
}

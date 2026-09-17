//! Onde esta o TEMPO dentro da secao critica do gatilho `BEFORE`?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example reparticao-do-gatilho
//! ```
//!
//! # A pergunta, e por que ela nao e a que o pedido 164 escreveu
//!
//! O 164 termina pedindo «encurtar as 5 secoes do gatilho `BEFORE`», e o mapa
//! da trava as mede em LINHAS -- 419 sob a trava. **Linha de codigo nao e
//! tempo.** Encurtar uma secao so compra alguma coisa se o que sair dela
//! custava tempo, e isso ninguem tinha medido: a premissa do item e «as cinco
//! sao longas demais», e premissa se mede antes de o item virar codigo.
//!
//! O `custo-do-gatilho` do `phxsql-sql` ja responde METADE: quanto o CORPO do
//! dono do banco consegue segurar a trava (o teto de parede,
//! `PRAZO_DO_GATILHO_ANTES`). O que falta e a outra metade -- quanto custa
//! todo o RESTO da secao, e quanto dele e PREPARACAO que talvez pudesse
//! acontecer fora da trava.
//!
//! # A reparticao que ele mede
//!
//! Tres fatias, na ordem em que o `op_inserir` as paga:
//!
//! * **(a) preparacao** -- o que a secao faz antes de o corpo do dono comecar:
//!   `linha_para_json` do NEW, a montagem do `Contexto::de_gatilho`, e o
//!   caminho de volta (`json_para_valor` de cada coluna tocada);
//! * **(b) o corpo do dono** -- `rotina::executar`, que dura o que ele quiser
//!   ate o teto de parede;
//! * **(c) a gravacao e o `fsync`** -- o piso da secao, que existe com ou sem
//!   gatilho.
//!
//! # Como, e por que assim
//!
//! Quatro cenarios INTERCALADOS -- 1,2,3,4, 1,2,3,4, ... -- sobre o mesmo
//! servidor, cada um na sua tabela e com o mesmo esquema. Intercalar e o que
//! impede a deriva da maquina (frequencia mudando, vizinho entrando) de virar
//! «custo» do cenario que rodou na hora ruim.
//!
//! 1. `piso` ...... tabela SEM gatilho nenhum;
//! 2. `minimo` .... um `BEFORE INSERT` de UMA instrucao, que ainda assim paga
//!    o aparato inteiro (ida em JSON, contexto, volta da coluna tocada);
//! 3. `tipico` .... o gatilho que o dono do banco escreve de verdade: valida,
//!    normaliza, corrige;
//! 4. `pesado` .... um `WHILE` de mil voltas, o corpo que abusa.
//!
//! O numero de cada cenario **nao e o tempo do pedido**: e o tempo SOB A
//! TRAVA, lido do contador que o proprio `travar_dados` ja mantem
//! (`telemetria.totais.trava_ms`). Medir o pedido inteiro misturaria o que
//! acontece FORA da trava -- e e justamente isso que este medidor precisa
//! separar. O tempo de parede do pedido sai junto, como controle.
//!
//! E, no mesmo processo e sem servidor nenhum, o **corpo puro**: o mesmo
//! `executar` com o mesmo `Contexto::de_gatilho`, cronometrado contra um laco
//! de controle que so monta o contexto. A diferenca entre os dois e
//! `rotina::executar` sozinho -- e e ela que permite descontar (b) de dentro
//! do aparato e sobrar (a).
//!
//! # O que este medidor NAO mede, e nao esconde
//!
//! Ele nao mede disputa: uma thread so, um cliente so. A pergunta aqui e da
//! REPARTICAO dentro da secao, e ela nao depende de haver fila -- a fila
//! multiplica o resultado, nao muda a proporcao. Concorrencia com N clientes
//! e a bancada `bancada/concorrencia/`, e ela pede maquina parada.

use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::config::Durabilidade;
use phxsql_server::mcp::Executor as _;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};
use phxsql_sql::rotina::{
    analisar_corpo, executar, regras_de_gatilho, Contexto, Evento, MotorNulo, Quando,
};

/// Copiado do `servidor.rs` de proposito, como no `custo-do-gatilho`: o
/// medidor nao le a constante privada, e o numero de verdade e o
/// `PRAZO_DO_GATILHO_ANTES`.
const PRAZO_DO_GATILHO_MS: u64 = 500;

/// O corpo de cada cenario. `None` e «esta tabela nao tem gatilho».
const CORPOS: [(&str, Option<&str>); 4] = [
    ("piso", None),
    // UMA instrucao, e ela TOCA uma coluna: sem a coluna tocada o caminho de
    // volta (`json_para_valor`) nao acontece, e o aparato medido sairia menor
    // do que o que o servidor paga de verdade.
    ("minimo", Some("SET NEW.nome = NEW.nome")),
    // O gatilho que o dono do banco escreve: recusa o que nao presta e
    // normaliza o resto. E o caso que decide, porque e o comum.
    (
        "tipico",
        Some(
            "BEGIN \
               IF NEW.nome IS NULL OR NEW.nome = '' THEN \
                 SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra'; \
               END IF; \
               SET NEW.nome = UPPER(TRIM(NEW.nome)); \
               SET NEW.cidade = UPPER(TRIM(NEW.cidade)); \
               IF NEW.valor < 0 THEN SET NEW.valor = 0; END IF; \
             END",
        ),
    ),
    // O corpo que abusa -- e que ainda assim e legitimo: mil voltas nao
    // estouram nem o teto de passos nem o de texto.
    (
        "pesado",
        Some(
            "BEGIN \
               DECLARE i INT DEFAULT 0; \
               WHILE i < 1000 DO SET i = i + 1; END WHILE; \
               SET NEW.nome = UPPER(TRIM(NEW.nome)); \
             END",
        ),
    ),
];

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).expect("o pedido do medidor nao analisa")
}

fn servidor(durabilidade: Durabilidade, marca: &str) -> (ExecutorLocal, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "phx-reparticao-{marca}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Config {
        base: dir.clone(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.durabilidade = durabilidade;
    // Com id de LIGACAO: sem ele o `begin` recusa, e a bateria do `empilhar`
    // -- a maior das cinco secoes -- nao teria como rodar.
    let s = ExecutorLocal::com_ligacao(Servidor::novo(c).unwrap(), "medidor", 1);
    s.executar(&pedido(
        r#"{"token":"t","op":"criar_database","database":"m"}"#,
    ))
    .unwrap();
    for (tabela, corpo) in CORPOS {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"criar_tabela","database":"m","tabela":"{tabela}",
                "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                           {{"nome":"nome","tipo":"Str(40)"}},
                           {{"nome":"cidade","tipo":"Str(40)"}},
                           {{"nome":"valor","tipo":"Int8"}}]}}"#
        )))
        .unwrap();
        if let Some(corpo) = corpo {
            s.executar(&pedido(
                &Json::objeto(vec![
                    ("token", Json::texto_de("t")),
                    ("op", Json::texto_de("sql")),
                    ("database", Json::texto_de("m")),
                    (
                        "texto",
                        Json::texto_de(format!(
                            "CREATE TRIGGER g_{tabela} BEFORE INSERT ON {tabela} \
                             FOR EACH ROW {corpo}"
                        )),
                    ),
                ])
                .escrever(),
            ))
            .unwrap();
        }
    }
    (s, dir)
}

/// Roda `n` insercoes na tabela e devolve (microssegundos SOB A TRAVA por
/// operacao, microssegundos de PAREDE por operacao).
///
/// O primeiro sai do contador que o `travar_dados` ja mantem; o segundo, do
/// relogio daqui. Os dois juntos porque so o par distingue «a secao esta
/// cara» de «o pedido esta caro fora da secao».
fn rodada(s: &ExecutorLocal, tabela: &str, id0: i64, n: u64) -> (f64, f64) {
    let antes = trava_ms(s);
    let inicio = Instant::now();
    for k in 0..n {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"{tabela}",
                "linha":{{"id":{},"nome":" ana ","cidade":" blumenau ","valor":7}}}}"#,
            id0 + k as i64
        )))
        .unwrap();
    }
    let parede = inicio.elapsed().as_micros() as f64 / n as f64;
    let depois = trava_ms(s);
    ((depois - antes) as f64 * 1000.0 / n as f64, parede)
}

/// O mesmo, pelo caminho IRMAO: `empilhar`, a secao de 162 linhas que atende
/// o `inserir` DENTRO de uma transacao.
///
/// Ele e irmao no sentido que vale aqui -- chama as mesmas funcoes na mesma
/// ordem: `gatilhos_para`, `conferir_gatilhos_compilam`, `travar_dados`,
/// `abrir_travada`, `json_para_linha`, `rodar_gatilhos_antes`. O que muda e o
/// fim: em vez de gravar, empilha. Medir so o `op_inserir` responderia por
/// uma das cinco secoes e deixaria a maior de fora.
///
/// A janela medida vai do fim do `begin` ao comeco do `commit`: o `commit` e
/// quem grava, e o tempo dele nao e do `empilhar`.
fn rodada_empilhar(s: &ExecutorLocal, tabela: &str, id0: i64, n: u64) -> (f64, f64) {
    s.executar(&pedido(r#"{"token":"t","op":"begin","database":"m"}"#))
        .unwrap();
    let antes = trava_ms(s);
    let inicio = Instant::now();
    for k in 0..n {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"{tabela}",
                "linha":{{"id":{},"nome":" ana ","cidade":" blumenau ","valor":7}}}}"#,
            id0 + k as i64
        )))
        .unwrap();
    }
    let parede = inicio.elapsed().as_micros() as f64 / n as f64;
    let depois = trava_ms(s);
    s.executar(&pedido(r#"{"token":"t","op":"commit"}"#))
        .unwrap();
    ((depois - antes) as f64 * 1000.0 / n as f64, parede)
}

/// O outro lado da mesma moeda: quanto custa LER dentro de uma transacao com
/// `pendentes` escritas empilhadas.
///
/// O caminho que empilha pode dispensar a sobreposicao; o que LE nao pode --
/// ela e o read-your-own-writes, e sem ela a transacao nao enxerga o proprio
/// trabalho. Entao aqui o mapa nao e desperdicio: e a funcionalidade. O que
/// esta sonda responde e outra coisa -- **quanto ela custa por leitura**, que
/// e o numero que decide se vale um mapa guardado por transacao em vez de um
/// montado a cada abertura.
///
/// Devolve microssegundos sob a trava por `ler`.
fn ler_com_pendentes(s: &ExecutorLocal, tabela: &str, id0: i64, pendentes: u64, n: u64) -> f64 {
    s.executar(&pedido(r#"{"token":"t","op":"begin","database":"m"}"#))
        .unwrap();
    for k in 0..pendentes {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"{tabela}",
                "linha":{{"id":{},"nome":"a","cidade":"b","valor":1}}}}"#,
            id0 + k as i64
        )))
        .unwrap();
    }
    let antes = trava_ms(s);
    for _ in 0..n {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"ler","database":"m","tabela":"{tabela}","rowid":1}}"#
        )))
        .unwrap();
    }
    let depois = trava_ms(s);
    // `rollback` e nao `commit`: gravar mil linhas entre duas medidas poria o
    // `fsync` do commit dentro da sonda seguinte.
    s.executar(&pedido(r#"{"token":"t","op":"rollback"}"#))
        .unwrap();
    (depois - antes) as f64 * 1000.0 / n as f64
}

/// O acumulador de posse da trava, em milissegundos. E o unico numero do
/// servidor que responde «quanto tempo esta secao segurou todo mundo».
fn trava_ms(s: &ExecutorLocal) -> u64 {
    s.executar(&pedido(r#"{"token":"t","op":"telemetria","amostras":1}"#))
        .unwrap()
        .campo("totais")
        .and_then(|t| t.campo("trava_ms"))
        .and_then(Json::inteiro)
        .unwrap_or(0) as u64
}

/// O corpo do dono, cronometrado SOZINHO -- sem servidor, sem trava, sem
/// disco. Devolve microssegundos por execucao.
///
/// O laco de controle monta o mesmo contexto e NAO executa: subtrair um do
/// outro tira do numero a clonagem do JSON e a montagem do contexto, que sao
/// da preparacao e nao do corpo.
///
/// Devolve `(corpo, montagem)` -- e a segunda nao e sobra: ela e a unica
/// medida DIRETA que este medidor consegue da fatia (a), porque o
/// `linha_para_json` do servidor e privado e so se alcanca pela diferenca
/// entre cenarios, que e onde o ruido mora.
fn corpo_puro(texto: &str, n: u64) -> (f64, f64) {
    let regras = regras_de_gatilho(Quando::Antes, Evento::Inserir);
    let programa = analisar_corpo(texto, &regras).expect("o corpo do medidor nao compila");
    let nova = pedido(r#"{"id":1,"nome":" ana ","cidade":" blumenau ","valor":7}"#);

    let inicio = Instant::now();
    for _ in 0..n {
        let mut ctx = Contexto::de_gatilho(Some(nova.clone()), true, None)
            .com_prazo(Duration::from_millis(PRAZO_DO_GATILHO_MS));
        executar(&programa, &mut ctx, &mut MotorNulo).expect("o corpo do medidor falhou");
        std::hint::black_box(&ctx.tocadas);
    }
    let com = inicio.elapsed().as_micros() as f64 / n as f64;

    let inicio = Instant::now();
    for _ in 0..n {
        let ctx = Contexto::de_gatilho(Some(nova.clone()), true, None)
            .com_prazo(Duration::from_millis(PRAZO_DO_GATILHO_MS));
        std::hint::black_box(&ctx.tocadas);
    }
    let sem = inicio.elapsed().as_micros() as f64 / n as f64;
    ((com - sem).max(0.0), sem)
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn faixa(v: &[f64]) -> (f64, f64) {
    let mut o = v.to_vec();
    o.sort_by(f64::total_cmp);
    (o[0], o[o.len() - 1])
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4_000);
    let rodadas: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(5);

    println!("Reparticao do tempo DENTRO da secao critica do gatilho BEFORE");
    println!("  {n} insercoes por cenario e por rodada, {rodadas} rodadas intercaladas\n");

    // ------------------------------------------------- o servidor do padrao
    let (s, dir) = servidor(Durabilidade::PorLote, "lote");
    // A telemetria e o instrumento: sem ela o contador de posse da trava nao
    // anda, e o medidor mediria o pedido inteiro em vez da secao.
    s.executar(&pedido(r#"{"token":"t","op":"telemetria_ligar"}"#))
        .unwrap();

    let mut id: i64 = 1;
    // Aquece os quatro: a primeira insercao de cada tabela paga a criacao dos
    // sete arquivos, e isso nao e da secao.
    for (tabela, _) in CORPOS {
        rodada(&s, tabela, id, 200);
        id += 200;
    }

    let mut sob_trava: Vec<Vec<f64>> = vec![Vec::new(); CORPOS.len()];
    let mut parede: Vec<Vec<f64>> = vec![Vec::new(); CORPOS.len()];
    for _ in 0..rodadas {
        for (i, (tabela, _)) in CORPOS.iter().enumerate() {
            let (t, p) = rodada(&s, tabela, id, n);
            id += n as i64;
            sob_trava[i].push(t);
            parede[i].push(p);
        }
    }

    println!("  1. `op_inserir` -- a secao de 86 linhas do `servidor.rs`\n");
    println!("  cenario        sob a trava (us/op)        parede (us/op)");
    let mut m_trava = Vec::new();
    let mut ruido: f64 = 0.0;
    for (i, (tabela, _)) in CORPOS.iter().enumerate() {
        let mt = mediana(sob_trava[i].clone());
        let (lo, hi) = faixa(&sob_trava[i]);
        let mp = mediana(parede[i].clone());
        m_trava.push(mt);
        // O ruido do proprio medidor: o maior espalhamento DENTRO de um
        // cenario. Diferenca entre cenarios menor que ele nao e diferenca --
        // e a licao do `custo-da-trava`, e a do vencedor declarado dentro do
        // ruido que o pedido 155 pagou.
        if *tabela != "pesado" {
            ruido = ruido.max(hi - lo);
        }
        println!("  {tabela:<10} {mt:10.2}   faixa {lo:7.2}..{hi:<8.2} {mp:9.2}");
    }

    // ------------------------------------------------ o IRMAO: `empilhar`
    //
    // A maior das cinco secoes (162 linhas). Se a reparticao dela divergisse
    // da do `op_inserir`, o parecer valeria para uma so -- e o item pede as
    // cinco.
    let m_tx: u64 = (n / 4).max(200);
    let mut tx: Vec<Vec<f64>> = vec![Vec::new(); CORPOS.len()];
    for (i, (tabela, _)) in CORPOS.iter().enumerate() {
        rodada_empilhar(&s, tabela, id + i as i64 * 1_000_000, 100);
    }
    id += 10_000_000;
    for r in 0..rodadas {
        for (i, (tabela, _)) in CORPOS.iter().enumerate() {
            let base = id + (r as i64) * 1_000_000 + (i as i64) * 100_000;
            tx[i].push(rodada_empilhar(&s, tabela, base, m_tx).0);
        }
    }
    println!("\n  2. `empilhar` -- a secao de 162 linhas, o mesmo `inserir` DENTRO");
    println!("     de uma transacao ({m_tx} por transacao, `commit` fora da conta)\n");
    println!("  cenario        sob a trava (us/op)");
    let mut m_tx_med = Vec::new();
    // O ruido do IRMAO e outro, e maior: a secao do `empilhar` cresce com a
    // lista pendente. Comparar a diferenca dele contra o ruido do
    // `op_inserir` seria medir um com a regua do outro.
    let mut ruido_tx: f64 = 0.0;
    for (i, (tabela, _)) in CORPOS.iter().enumerate() {
        let mt = mediana(tx[i].clone());
        let (lo, hi) = faixa(&tx[i]);
        m_tx_med.push(mt);
        if *tabela != "pesado" {
            ruido_tx = ruido_tx.max(hi - lo);
        }
        println!("  {tabela:<10} {mt:10.2}   faixa {lo:7.2}..{hi:<8.2}");
    }

    // ------------------------------------ a SONDA: o que engorda o `empilhar`
    //
    // Achado de lado, e ele nao e do gatilho -- por isso e sonda e nao
    // cenario. O piso do `empilhar` mudou de 85 para 341 us/op quando a
    // transacao passou de 200 para 1.000 operacoes, e ha DOIS suspeitos que a
    // bateria de cima nao distingue: a lista pendente da transacao e o
    // tamanho da tabela, que crescem juntos ali.
    //
    // Aqui eles se separam: MESMA tabela, ja crescida, e transacoes de
    // tamanhos diferentes, da maior para a menor (para o tamanho da tabela
    // trabalhar CONTRA a hipotese da lista, e nao a favor).
    println!("\n  SONDA -- o piso do `empilhar` por TAMANHO DA TRANSACAO");
    println!("  (mesma tabela, da maior para a menor: o crescimento da tabela\n   joga contra a hipotese da lista pendente)\n");
    let mut base = id + 90_000_000;
    for m in [1_600u64, 800, 400, 200, 100] {
        let us = rodada_empilhar(&s, "piso", base, m).0;
        base += m as i64 * 4;
        println!("    {m:5} operacoes na transacao  {us:9.2} us/op");
    }

    // ------------------------------- e o IRMAO que NAO pode dispensar o mapa
    println!("\n  SONDA IRMA -- `ler` dentro da transacao, por pendentes");
    println!("  (aqui a sobreposicao e a funcionalidade, nao desperdicio:\n   e o read-your-own-writes. O numero diz o que ela cobra por leitura)\n");
    let mut base = id + 95_000_000;
    for pendentes in [0u64, 100, 400, 1_600] {
        let us = ler_com_pendentes(&s, "piso", base, pendentes, 2_000);
        base += pendentes as i64 * 4 + 10;
        println!("    {pendentes:5} escritas pendentes      {us:9.2} us por `ler`");
    }
    let _ = std::fs::remove_dir_all(&dir);

    // -------------------------------------------- o corpo do dono, sozinho
    println!("\n  o CORPO do dono, cronometrado sozinho (sem trava, sem disco):");
    let mut puro = vec![0.0f64; CORPOS.len()];
    let mut montagem = 0.0f64;
    for (i, (tabela, corpo)) in CORPOS.iter().enumerate() {
        if let Some(c) = corpo {
            let (b, m) = corpo_puro(c, 20_000);
            puro[i] = b;
            montagem = montagem.max(m);
            println!("    {tabela:<10} {:8.3} us por execucao", puro[i]);
        }
    }
    println!(
        "    montagem do `Contexto::de_gatilho` com o NEW ..... {montagem:8.3} us\n    \
         (e a medida DIRETA do naipe da fatia (a): o `linha_para_json` do\n     \
         servidor faz o mesmo trabalho por outro caminho)"
    );

    // ------------------------------------------------------- o fsync, a (c)
    println!("\n  a fatia (c), o `fsync`: o mesmo piso com durabilidade por_operacao");
    let (s2, dir2) = servidor(Durabilidade::PorOperacao, "operacao");
    s2.executar(&pedido(r#"{"token":"t","op":"telemetria_ligar"}"#))
        .unwrap();
    rodada(&s2, "piso", 1, 200);
    let mut sempre = Vec::new();
    for r in 0..rodadas {
        sempre.push(rodada(&s2, "piso", 1_000 + (r as i64) * n as i64, n).0);
    }
    let m_sempre = mediana(sempre.clone());
    let _ = std::fs::remove_dir_all(&dir2);
    println!(
        "    piso por_lote {:8.2} us/op   piso por_operacao {m_sempre:8.2} us/op   \
         o fsync por linha = {:+.2} us",
        m_trava[0],
        m_sempre - m_trava[0]
    );

    // ------------------------------------------------------------ a conta
    let piso = m_trava[0];
    let aparato = m_trava[1] - piso;
    let aparato_tx = m_tx_med[1] - m_tx_med[0];
    let corpo_tipico = m_trava[2] - m_trava[1];
    let corpo_pesado = m_trava[3] - m_trava[1];
    let teto_do_corpo = PRAZO_DO_GATILHO_MS as f64 * 1000.0;

    println!("\n  A REPARTICAO, por insercao, dentro da secao critica do `op_inserir`:");
    println!("    (c) gravacao e janela (o piso) ............. {piso:9.2} us");
    println!("    (a) APARATO do gatilho (minimo - piso) ..... {aparato:+9.2} us   <- o que o item 164 queria tirar da trava");
    println!("        idem no irmao `empilhar` ............... {aparato_tx:+9.2} us");
    println!("        ruido do medidor, `op_inserir` ......... {ruido:9.2} us");
    println!("        ruido do medidor, `empilhar` ........... {ruido_tx:9.2} us");
    println!(
        "    (b) corpo do dono, minimo .................. {:9.3} us",
        puro[1]
    );
    println!("    (b) corpo do dono, tipico .................. {corpo_tipico:9.2} us   (sozinho: {:.2} us)", puro[2]);
    println!("    (b) corpo do dono, pesado (mil voltas) ..... {corpo_pesado:9.2} us   (sozinho: {:.2} us)", puro[3]);
    println!("    (b) corpo do dono, TETO de parede .......... {teto_do_corpo:9.0} us   (PRAZO_DO_GATILHO_ANTES)");

    println!("\n  VEREDITO SOBRE A PREMISSA do item («as cinco secoes sao longas demais»):");
    let some_no_ruido = aparato.abs() < ruido && aparato_tx.abs() < ruido_tx;
    if some_no_ruido {
        println!(
            "    O aparato do gatilho NAO APARECE acima do ruido do proprio\n    \
             medidor, nas DUAS secoes ({aparato:+.2} us contra {ruido:.2} no `op_inserir`;\n    \
             {aparato_tx:+.2} contra {ruido_tx:.2} no `empilhar`). O teto do que se poderia\n    \
             ganhar tirando a preparacao da trava e, portanto, MENOR que\n    \
             {ruido:.2} us -- menos de {:.1}% da secao do caso tipico e menos de\n    \
             {:.4}% do que o corpo do dono pode segurar ate o teto de parede.",
            ruido / m_trava[2] * 100.0,
            ruido / teto_do_corpo * 100.0
        );
        println!(
            "    => ENCURTAR AS CINCO SECOES NAO COMPRA NADA. O que decide o tempo\n       \
             da secao e o corpo do dono ({:.0}x o teto do ganho, no caso pesado)\n       \
             e o `fsync` do piso -- nenhum dos dois sai da trava por\n       \
             refatoracao de quantas linhas a secao tem.",
            corpo_pesado / ruido.max(0.001)
        );
    } else {
        println!(
            "    O aparato APARECE acima do ruido ({aparato:+.2} contra {ruido:.2} no\n    \
             `op_inserir`; {aparato_tx:+.2} contra {ruido_tx:.2} no `empilhar`). Ai a conta de\n    \
             quanto dele e preparacao passa a valer, e o item tem codigo a\n    \
             escrever."
        );
    }
    println!(
        "\n  E a pergunta que fecha o item, e nao e de tempo: tirar a preparacao\n  \
         da trava exige o ESQUEMA, e o esquema so chega por `abrir_travada`,\n  \
         que exige a ficha da trava -- o compilador cobra isso. Prepara-la\n  \
         fora e abrir a tabela fora da trava, e ai o gatilho passa a julgar\n  \
         sobre um esquema que pode ter mudado entre preparar e executar.\n  \
         Meia garantia e pior que nenhuma."
    );
}

//! Pedidos 556 e 557: o VALOR de uma celula no fio do DbLink, nos dois
//! sentidos, provado pelo soquete contra um MySQL(R) falso que devolve o que
//! o teste manda e entrega ao teste cada instrucao que recebe.
//!
//! A regua de nome recusava o valor citando-o inteiro (556), e a celula
//! puxada que nao servia ao tipo local saia citada na recusa (557). As duas
//! provas olham o que atravessa o soquete e o texto da recusa -- nao so o
//! veredito.
use super::testes_dblink_fora_da_trava::{
    coluna, lenenc, ler_pacote, pede, quadro, saudar, servidor, EOF,
};
use super::*;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

type Linhas = Vec<Vec<Option<String>>>;

/// `clientes` la: id INT chave, nome TEXT, nasc DATE.
fn resposta(sql: &str, linhas: &Linhas) -> Vec<Vec<u8>> {
    if !sql.starts_with("SELECT") {
        return vec![vec![0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00]];
    }
    let mut r = vec![
        vec![3],
        coluna("id", 0x03, 0x0003, 11),
        coluna("nome", 0xfc, 0, 262_140),
        coluna("nasc", 0x0a, 0, 10),
        EOF.to_vec(),
    ];
    if !sql.contains("LIMIT 0") {
        for l in linhas {
            let mut p = Vec::new();
            for c in l {
                match c {
                    // 0xFB e o NULL do protocolo texto.
                    None => p.push(0xFB),
                    Some(t) => lenenc(&mut p, t.as_bytes()),
                }
            }
            r.push(p);
        }
    }
    r.push(EOF.to_vec());
    r
}

fn atender(mut s: TcpStream, linhas: Linhas, avisar: mpsc::Sender<String>) {
    if !saudar(&mut s) {
        return;
    }
    while let Some(pacote) = ler_pacote(&mut s) {
        if pacote.first() != Some(&0x03) {
            return;
        }
        let sql = String::from_utf8_lossy(&pacote[1..]).into_owned();
        let _ = avisar.send(sql.clone());
        let mut bytes = Vec::new();
        for (i, carga) in resposta(&sql, &linhas).iter().enumerate() {
            bytes.extend(quadro(i as u8 + 1, carga));
        }
        if s.write_all(&bytes).is_err() {
            return;
        }
    }
}

fn par(linhas: Linhas) -> (u16, mpsc::Receiver<String>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (avisar, avisos) = mpsc::channel();
    std::thread::spawn(move || {
        for s in ouvinte.incoming() {
            let Ok(s) = s else { return };
            let (linhas, avisar) = (linhas.clone(), avisar.clone());
            std::thread::spawn(move || atender(s, linhas, avisar));
        }
    });
    (porta, avisos)
}

/// Banco `loja`, ligacao `erp` e a tabela `clientes` ligada no sentido dado.
fn ligado(
    dir: &std::path::Path,
    linhas: Linhas,
    sentido: &str,
) -> (Arc<Servidor>, mpsc::Receiver<String>) {
    let s = servidor(dir);
    let (porta, avisos) = par(linhas);
    pede(&s, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        &format!(
            r#""op":"dblink_salvar","nome":"erp","motor":"mysql","host":"127.0.0.1",
                   "porta":{porta},"usuario":"u","database":"erp","timeout_s":5,
                   "somente_leitura":false,"cifra":false"#
        ),
    )
    .unwrap();
    pede(
        &s,
        &format!(
            r#""op":"dblink_ligar","dblink":"erp","tabelas":[{{"remota":"clientes",
                   "local_database":"loja","sentido":"{sentido}","dono":"aqui"}}]"#
        ),
    )
    .unwrap();
    (s, avisos)
}

/// Os `_utf8mb4 X'…'` de uma instrucao, desfeitos em texto.
fn textos_em_hex(sql: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut resto = sql;
    while let Some(i) = resto.find("_utf8mb4 X'") {
        let depois = &resto[i + 11..];
        let fim = depois.find('\'').expect("hexadecimal sem fecho");
        let hex = &depois[..fim];
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|k| u8::from_str_radix(&hex[k..k + 2], 16).unwrap())
            .collect();
        v.push(String::from_utf8(bytes).unwrap());
        resto = &depois[fim + 1..];
    }
    v
}

/// **556: o valor sobe inteiro.** Aspa simples, contrabarra antes de
/// aspa (a injecao classica do MySQL(R)), quebra de linha, NUL, espaco na
/// ponta e mais de 128 bytes: com a regua de nome, a rodada recusava na
/// primeira citando o valor, e «Ana » perdia o espaco.
#[test]
fn o_empurrao_leva_o_valor_byte_a_byte_sem_regua_de_nome() {
    let dir = DirTemp::novo("556-dblink-valor");
    let (s, avisos) = ligado(&dir, Vec::new(), "empurrar");
    let valores = [
        "D'Avila \\' OR 1=1 -- \n\r\0 fim".to_string(),
        "é".repeat(100),
        "Ana ".to_string(),
    ];
    for (i, v) in valores.iter().enumerate() {
        let linha = Json::objeto(vec![
            ("id", Json::de_u64(i as u64 + 1)),
            ("nome", Json::texto_de(v)),
        ]);
        pede(
            &s,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{}"#,
                linha.escrever()
            ),
        )
        .unwrap();
    }
    let r = match pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#) {
        Ok(j) => j.escrever(),
        Err(e) => panic!("o empurrao recusou valor de celula: {e}"),
    };
    assert!(r.contains("\"empurradas\":3"), "{r}");
    let insert = avisos
        .try_iter()
        .find(|sql| sql.starts_with("INSERT"))
        .expect("o par nao recebeu o INSERT");
    let mut chegou = textos_em_hex(&insert);
    chegou.sort();
    let mut esperado = valores.to_vec();
    esperado.sort();
    assert_eq!(chegou, esperado, "{insert}");
    // Nenhuma aspa de dado no fio: o sentido nao depende do sql_mode.
    assert!(!insert.contains("D'Avila"), "{insert}");
}

/// **557: a celula puxada que nao serve recusa sem citar.** A coluna
/// `nasc` e DATE la e Date aqui; o outro banco manda um CPF nela. Antes
/// do conserto a recusa trazia `"999.888.777-66"`.
#[test]
fn a_celula_remota_recusada_nao_sai_citada() {
    let dir = DirTemp::novo("557-dblink-puxar");
    let linhas = vec![vec![
        Some("1".to_string()),
        Some("Ana".to_string()),
        Some("999.888.777-66".to_string()),
    ]];
    let (s, _avisos) = ligado(&dir, linhas, "puxar");
    let e = pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#)
        .expect_err("a celula que nao e data entrou")
        .to_string();
    assert!(!e.contains("999"), "a recusa citou o dado de la: {e}");
    assert!(e.contains("\"nasc\"") && e.contains("14 bytes"), "{e}");
}

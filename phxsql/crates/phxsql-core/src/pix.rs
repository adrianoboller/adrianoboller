//! BR Code do pix (padrao EMV(R) QRCPS, Merchant-Presented Mode do Banco
//! Central do Brasil): a string "copia e cola" que o pagador le.
//!
//! Sem dependencias externas -- so a `std`. O layout e um TLV
//! (identificador-tamanho-valor): cada campo e `IISS...`, dois digitos de id,
//! dois de tamanho, e o valor. Subtemplates (`26`, `62`) carregam outros TLV
//! dentro do valor.
//!
//! # Por que o valor entra em centavos, e nao em `f64`
//!
//! Dinheiro em ponto flutuante e fonte classica de erro: `0.1 + 0.2` nao da
//! `0.30`, e o campo `54` do pix e texto exato com duas casas. Guardando
//! centavos num inteiro, "120.00" nasce de `12000` por divisao e resto -- sem
//! arredondamento nenhum pelo caminho. Valor zero significa QR estatico (o
//! pagador digita quanto pagar), e ai o campo `54` **some inteiro**.
//!
//! # A conferencia
//!
//! O CRC e um **CRC-16/CCITT-FALSE** calculado sobre a string inteira, ja
//! incluindo o "6304" do proprio campo do CRC, e escrito em quatro hex
//! maiusculos. O algoritmo se prova contra o vetor canonico do catalogo:
//! `crc16_ccitt_false(b"123456789") == 0x29B1`. E o payload inteiro se prova
//! por auto-consistencia -- o CRC dos quatro ultimos caracteres bate com o CRC
//! recomputado de todo o resto.

use crate::error::{PhxError, Result};

/// O GUI do arranjo pix dentro do Merchant Account Information (campo `26`).
const GUI_PIX: &str = "br.gov.bcb.pix";
/// Moeda: `986` e o codigo ISO 4217 do real.
const MOEDA_BRL: &str = "986";
/// Pais no padrao ISO 3166-1 alpha-2.
const PAIS: &str = "BR";
/// Merchant Category Code generico -- `0000` quando nao ha categoria.
const MCC: &str = "0000";
/// Placeholder do pix para "sem identificador de transacao" no campo `62/05`.
const TXID_VAZIO: &str = "***";

/// Limites semanticos do pix (nao os do TLV, que e 99 por caber em 2 digitos).
const MAX_NOME: usize = 25;
const MAX_CIDADE: usize = 15;
const MAX_TXID: usize = 25;

/// CRC-16/CCITT-FALSE: poly 0x1021, init 0xFFFF, sem reflexao, xorout 0x0000.
///
/// E o mesmo CRC que o BR Code exige. Provado contra o valor de conferencia
/// canonico do catalogo (`"123456789"` -> `0x29B1`); a partir dai, escrito byte
/// a byte de proposito, para nao esconder a definicao atras de uma tabela.
pub fn crc16_ccitt_false(dados: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in dados {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            // Sem reflexao: olha-se o bit mais alto e desloca-se para a
            // esquerda -- o oposto do CRC-32 refletido do resto da casa.
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Uma cobranca pix, a materia-prima do BR Code.
///
/// Tudo emprestado (`&str`) porque `montar` e funcao pura: le a cobranca e
/// devolve a string, sem guardar nada.
pub struct Cobranca<'a> {
    /// A chave pix do recebedor (e-mail, CPF/CNPJ, telefone ou chave aleatoria).
    pub chave: &'a str,
    /// Valor em centavos. `0` = QR estatico (o campo `54` nao entra).
    pub valor_centavos: u64,
    /// Nome do recebedor -- no maximo 25.
    pub nome: &'a str,
    /// Cidade do recebedor -- no maximo 15.
    pub cidade: &'a str,
    /// Identificador da transacao (campo `62/05`). Vazio vira `***`.
    pub txid: &'a str,
}

/// Um campo TLV: `id` (2 digitos) + tamanho (2 digitos) + `valor`.
///
/// Recusa em vez de estourar calado: o tamanho mora em dois digitos decimais,
/// entao um valor com mais de 99 bytes nao tem como ser representado -- e isso
/// e erro de quem chamou, nao um numero que da a volta.
fn campo(id: &str, valor: &str) -> Result<String> {
    let n = valor.len();
    if n > 99 {
        return Err(PhxError::LimiteExcedido(format!(
            "campo {id} tem {n} bytes; o tamanho EMV cabe em 2 digitos (max 99)"
        )));
    }
    Ok(format!("{id}{n:02}{valor}"))
}

/// Formata centavos como o campo `54` exige: inteiro, ponto, dois digitos.
///
/// Por inteiro e nao por `f64`: `12000` -> `"120.00"`, `5` -> `"0.05"`, sempre
/// exato.
fn valor_texto(centavos: u64) -> String {
    format!("{}.{:02}", centavos / 100, centavos % 100)
}

/// Monta o BR Code "copia e cola" a partir da cobranca.
///
/// A ordem dos campos segue o EMV, e o CRC fecha a string. O ponto de iniciacao
/// (`01`) e `12` quando ha valor definido (cobranca de uma vez) e `11` quando
/// nao ha (QR estatico reutilizavel).
pub fn montar(c: &Cobranca) -> Result<String> {
    if c.chave.trim().is_empty() {
        return Err(PhxError::Esquema("chave pix vazia".into()));
    }
    if c.nome.trim().is_empty() {
        return Err(PhxError::Esquema("nome do recebedor vazio".into()));
    }
    if c.cidade.trim().is_empty() {
        return Err(PhxError::Esquema("cidade vazia".into()));
    }
    // Limites do pix: recusa, nao truncamento silencioso -- nome cortado no
    // meio publica um recebedor que nao existe.
    if c.nome.len() > MAX_NOME {
        return Err(PhxError::LimiteExcedido(format!(
            "nome do recebedor tem {} bytes; o pix aceita ate {MAX_NOME}",
            c.nome.len()
        )));
    }
    if c.cidade.len() > MAX_CIDADE {
        return Err(PhxError::LimiteExcedido(format!(
            "cidade tem {} bytes; o pix aceita ate {MAX_CIDADE}",
            c.cidade.len()
        )));
    }
    let txid = if c.txid.trim().is_empty() {
        TXID_VAZIO
    } else {
        c.txid
    };
    if txid.len() > MAX_TXID {
        return Err(PhxError::LimiteExcedido(format!(
            "txid tem {} bytes; o pix aceita ate {MAX_TXID}",
            txid.len()
        )));
    }

    let mut s = String::new();
    // 00 Payload Format Indicator: sempre "01".
    s.push_str(&campo("00", "01")?);
    // 01 Point of Initiation Method: 12 com valor, 11 estatico.
    let ponto = if c.valor_centavos > 0 { "12" } else { "11" };
    s.push_str(&campo("01", ponto)?);
    // 26 Merchant Account Information: subtemplate do pix.
    let mai = {
        let mut m = String::new();
        m.push_str(&campo("00", GUI_PIX)?);
        m.push_str(&campo("01", c.chave)?);
        m
    };
    s.push_str(&campo("26", &mai)?);
    // 52 Merchant Category Code.
    s.push_str(&campo("52", MCC)?);
    // 53 Transaction Currency.
    s.push_str(&campo("53", MOEDA_BRL)?);
    // 54 Transaction Amount -- so quando ha valor.
    if c.valor_centavos > 0 {
        s.push_str(&campo("54", &valor_texto(c.valor_centavos))?);
    }
    // 58 Country Code, 59 Merchant Name, 60 Merchant City.
    s.push_str(&campo("58", PAIS)?);
    s.push_str(&campo("59", c.nome)?);
    s.push_str(&campo("60", c.cidade)?);
    // 62 Additional Data Field Template: subtemplate com o txid em 05.
    let adicional = campo("05", txid)?;
    s.push_str(&campo("62", &adicional)?);
    // 63 CRC: o "6304" entra ANTES de calcular, e o CRC cobre a string toda.
    s.push_str("6304");
    let crc = crc16_ccitt_false(s.as_bytes());
    s.push_str(&format!("{crc:04X}"));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Vetor externo.** `0x29B1` e o valor de conferencia canonico do
    /// CRC-16/CCITT-FALSE para a mensagem `"123456789"` -- o mesmo que o
    /// catalogo de CRCs publica. E este teste, e so ele, que prova o
    /// algoritmo; tudo o mais no modulo se apoia nele.
    #[test]
    fn crc_bate_o_vetor_canonico() {
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    }

    /// A mensagem vazia rende o proprio valor inicial (`0xFFFF`) -- nao ha
    /// nenhum byte para processar. E o unico "outro" vetor que se pode afirmar
    /// sem medir; os demais valores de borda ficam de fora justamente porque
    /// seriam numeros citados de memoria.
    #[test]
    fn crc_da_mensagem_vazia_e_o_init() {
        assert_eq!(crc16_ccitt_false(b""), 0xFFFF);
    }

    /// Extrai os campos TLV de nivel de topo, conferindo que os tamanhos batem
    /// exatamente -- nada sobra, nada estoura. E o leitor que o teste de
    /// auto-consistencia usa para achar o campo `54`, `59`, etc.
    fn campos(payload: &str) -> Vec<(String, String)> {
        let b = payload.as_bytes();
        let mut i = 0;
        let mut out = vec![];
        while i + 4 <= b.len() {
            let id = &payload[i..i + 2];
            let tam: usize = payload[i + 2..i + 4].parse().expect("tamanho nao numerico");
            let ini = i + 4;
            let fim = ini + tam;
            assert!(fim <= b.len(), "campo {id} passa do fim do payload");
            out.push((id.to_string(), payload[ini..fim].to_string()));
            i = fim;
        }
        assert_eq!(i, b.len(), "sobrou byte fora de qualquer TLV");
        out
    }

    fn cobranca_exemplo() -> String {
        montar(&Cobranca {
            chave: "adriano@phxsql.com.br",
            valor_centavos: 12000,
            nome: "PHXSQL PAGAMENTOS",
            cidade: "BLUMENAU",
            txid: "PEDIDO123",
        })
        .expect("cobranca valida")
    }

    /// **Auto-consistencia do payload inteiro.** Nao ha um payload do manual do
    /// BCB embutido aqui de proposito: um vetor citado de memoria seria numero
    /// que ninguem mede. Em vez disso, prova-se que o CRC gravado (os quatro
    /// hex finais) bate com o CRC recomputado de todo o resto -- que e a
    /// propriedade que um leitor de verdade confere. O vetor `0x29B1` acima e
    /// quem garante que esse CRC e o CRC certo.
    #[test]
    fn payload_e_autoconsistente() {
        let p = cobranca_exemplo();
        assert!(p.len() >= 8);
        let (corpo, hex) = p.split_at(p.len() - 4);
        // O corpo termina em "6304" e o CRC cobre ele por inteiro.
        assert!(corpo.ends_with("6304"), "o campo do CRC nao fecha o corpo");
        let esperado = crc16_ccitt_false(corpo.as_bytes());
        let gravado = u16::from_str_radix(hex, 16).expect("hex do CRC invalido");
        assert_eq!(gravado, esperado, "CRC gravado nao bate com o recomputado");
        // E os quatro hex sao maiusculos, como o padrao pede.
        assert_eq!(hex, hex.to_uppercase());
    }

    /// A estrutura EMV: os campos obrigatorios, na ordem, e o subtemplate do
    /// pix com o GUI certo dentro do `26`.
    #[test]
    fn estrutura_emv_bate() {
        let p = cobranca_exemplo();
        let cs = campos(&p);
        let ids: Vec<&str> = cs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["00", "01", "26", "52", "53", "54", "58", "59", "60", "62", "63"]
        );
        let achar = |id: &str| cs.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone());
        assert_eq!(achar("00").unwrap(), "01");
        assert_eq!(achar("01").unwrap(), "12"); // tem valor
        assert_eq!(achar("52").unwrap(), MCC);
        assert_eq!(achar("53").unwrap(), MOEDA_BRL);
        assert_eq!(achar("54").unwrap(), "120.00");
        assert_eq!(achar("58").unwrap(), PAIS);
        assert_eq!(achar("59").unwrap(), "PHXSQL PAGAMENTOS");
        assert_eq!(achar("60").unwrap(), "BLUMENAU");
        // O subtemplate do pix vive dentro do 26.
        let mai = achar("26").unwrap();
        let sub = campos(&mai);
        assert_eq!(sub[0], ("00".into(), GUI_PIX.into()));
        assert_eq!(sub[1], ("01".into(), "adriano@phxsql.com.br".into()));
        // E o txid dentro do 62/05.
        let add = campos(&achar("62").unwrap());
        assert_eq!(add[0], ("05".into(), "PEDIDO123".into()));
    }

    /// Sem valor: o campo `54` some e o ponto de iniciacao vira `11` (estatico).
    #[test]
    fn sem_valor_omite_o_54_e_marca_estatico() {
        let p = montar(&Cobranca {
            chave: "chave-aleatoria-uuid",
            valor_centavos: 0,
            nome: "LOJA",
            cidade: "RECIFE",
            txid: "",
        })
        .unwrap();
        let cs = campos(&p);
        assert!(!cs.iter().any(|(id, _)| id == "54"), "54 nao devia existir");
        let poi = cs.iter().find(|(id, _)| id == "01").unwrap();
        assert_eq!(poi.1, "11");
        // txid vazio virou "***".
        let add = campos(&cs.iter().find(|(id, _)| id == "62").unwrap().1);
        assert_eq!(add[0].1, "***");
        // E continua autoconsistente.
        let (corpo, hex) = p.split_at(p.len() - 4);
        assert_eq!(
            u16::from_str_radix(hex, 16).unwrap(),
            crc16_ccitt_false(corpo.as_bytes())
        );
    }

    /// Centavos viram texto sem passar por ponto flutuante.
    #[test]
    fn valor_texto_e_exato() {
        assert_eq!(valor_texto(12000), "120.00");
        assert_eq!(valor_texto(5), "0.05");
        assert_eq!(valor_texto(199), "1.99");
        assert_eq!(valor_texto(100000000), "1000000.00");
    }

    /// **Prova real nos dois sentidos:** nome grande demais tem de RECUSAR (o
    /// defeito reposto seria truncar calado), e nome no limite tem de passar.
    #[test]
    fn nome_grande_recusa_no_limite_passa() {
        let base = |nome: &str| {
            montar(&Cobranca {
                chave: "k@phxsql.com.br",
                valor_centavos: 100,
                nome,
                cidade: "SP",
                txid: "T1",
            })
        };
        assert!(base(&"X".repeat(MAX_NOME + 1)).is_err(), "26 devia recusar");
        assert!(base(&"X".repeat(MAX_NOME)).is_ok(), "25 devia passar");
    }

    /// Cidade segue a mesma lei do nome, com o seu proprio teto.
    #[test]
    fn cidade_grande_recusa() {
        let r = montar(&Cobranca {
            chave: "k@phxsql.com.br",
            valor_centavos: 100,
            nome: "N",
            cidade: &"C".repeat(MAX_CIDADE + 1),
            txid: "T1",
        });
        assert!(r.is_err());
    }

    /// Um campo TLV com mais de 99 bytes nao cabe em dois digitos: recusa.
    #[test]
    fn campo_alem_de_99_recusa() {
        assert!(campo("01", &"z".repeat(100)).is_err());
        assert!(campo("01", &"z".repeat(99)).is_ok());
    }
}

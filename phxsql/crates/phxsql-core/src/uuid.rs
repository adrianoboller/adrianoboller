//! Identificadores: UUID de 128 bits (v4 e v7) e identificador de 256 bits.
//!
//! # Por que o v7 importa AQUI, e nao so por moda
//!
//! A bancada de dez milhoes mediu o buraco do motor: a insercao cai de 5.089
//! linhas/s no primeiro milhao para 3.626/s no ultimo, com o disco parado e a
//! CPU em 99%. A causa e a B+tree do `.ndx` sendo reescrita a cada linha.
//!
//! Chave ALEATORIA -- um UUID v4, por exemplo -- espalha cada insercao por uma
//! folha diferente da arvore: toda gravacao suja uma pagina nova, e quanto
//! maior a tabela, mais longe uma da outra. Chave CRESCENTE cai sempre na
//! folha mais a direita, que ja esta na memoria. E a diferenca entre semear a
//! arvore inteira e anexar no fim dela.
//!
//! O v7 e crescente por construcao: os primeiros 48 bits sao o relogio em
//! milissegundos, em big-endian. E como a chave do `.ndx` guarda os bytes na
//! ordem natural (ver `keyenc`), comparar bytes = comparar tempo.
//!
//! # Monotonico de verdade, nao "quase"
//!
//! Dentro do mesmo milissegundo o relogio nao separa nada, e dois v7 gerados
//! juntos sairiam fora de ordem. Por isso os 12 bits de `rand_a` viram um
//! CONTADOR (o metodo 1 da secao 6.2 do RFC 9562): no primeiro id de cada
//! milissegundo ele nasce sorteado na metade de baixo da faixa, e cada id
//! seguinte do mesmo milissegundo soma 1. Estourou, o relogio anda 1 ms para
//! frente em vez de repetir.
//!
//! O resultado e que `gerar_v7()` NUNCA devolve um valor menor ou igual ao
//! anterior, nem sob concorrencia. Isso e o que o indice precisa.
//!
//! # O de 256 bits
//!
//! `Uuid256` nao e um UUID: o RFC 9562 so define 128 bits, e chamar de UUID algo
//! que nao esta no padrao seria mentir no nome do tipo. E um identificador
//! opaco de 32 bytes, e o motivo de existir e pratico: um SHA-256 cabe nele
//! exatamente, sem sobra e sem texto. Hash de bloco, hash de transacao,
//! impressao digital de arquivo.

use std::fmt;

use crate::error::{citar, PhxError, Result};

/// Bytes de um UUID.
pub const UUID_LEN: usize = 16;
/// Bytes de um identificador de 256 bits.
pub const UUID256_LEN: usize = 32;

/// UUID de 128 bits, guardado na ordem de rede (big-endian), que e a mesma
/// ordem em que ele se escreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Uuid(pub [u8; UUID_LEN]);

/// Identificador de 256 bits.
///
/// ATENCAO: **nao e um UUID do RFC 9562** -- o padrao so define 128 bits. O
/// nome carrega o 256 justamente para nao passar por um UUID comum. E um
/// identificador opaco de 32 bytes, e existe por um motivo pratico: um SHA-256
/// cabe nele exatamente, sem sobra e sem virar texto. Hash de bloco, hash de
/// transacao, impressao digital de arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid256(pub [u8; UUID256_LEN]);

impl Default for Uuid256 {
    fn default() -> Self {
        Uuid256([0u8; UUID256_LEN])
    }
}

// ------------------------------------------------------------------ sorteio

/// Bytes sorteados. Tenta `/dev/urandom`; onde ele nao existe, mistura relogio
/// e endereco -- o mesmo caminho que `senha.rs` ja usa e explica.
fn sortear(dst: &mut [u8]) {
    // ATENCAO: /dev/urandom e um dispositivo INFINITO. Ler o "arquivo inteiro"
    // trava para sempre; le-se exatamente o que se precisa.
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(dst).is_ok() {
            return;
        }
    }
    misturar(dst);
}

/// Reserva para onde nao ha `/dev/urandom` (Windows). Nao e criptografico, e
/// nao precisa ser: o que o v7 exige dos bits sorteados e que dois geradores
/// nao colidam, nao que ninguem os adivinhe.
fn misturar(dst: &mut [u8]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let endereco = dst.as_ptr() as u64;
    let mut estado = nanos ^ endereco.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    for b in dst.iter_mut() {
        // splitmix64: barato, boa dispersao, e cabe em cinco linhas.
        estado = estado.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = estado;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        *b = ((z ^ (z >> 31)) >> 24) as u8;
    }
}

fn agora_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Faixa util do contador de 12 bits. Comeca sorteado na metade de baixo para
/// sobrar espaco de contagem sem estourar dentro do mesmo milissegundo.
const CONTADOR_MASCARA: u16 = 0x0FFF;
const CONTADOR_SEMENTE: u16 = 0x07FF;

/// Um passo do relogio logico: dado o ultimo par (milissegundo, contador)
/// emitido e a leitura do relogio da maquina, devolve o proximo par.
///
/// E uma funcao PURA de proposito: nao le nem escreve o estado do gerador.
/// Quem quer provar a logica do passo -- o estouro, o relogio da maquina
/// andando para tras -- chama esta funcao com um estado local, e o gerador
/// nem fica sabendo. O estado de verdade mora em `relogio`, que so `v7()`
/// alcanca; o motivo esta escrito la.
///
/// Tres casos, e o terceiro e o que garante a monotonia: se o contador
/// estourou dentro do mesmo milissegundo, empresta-se um milissegundo do
/// futuro em vez de repetir ou esperar. O id continua crescente e a geracao
/// nunca bloqueia.
///
/// E o primeiro caso e o que absorve o relogio da maquina: `agora_ms` le
/// `SystemTime`, que e CLOCK_REALTIME, e o NTP PODE puxa-lo para tras. So um
/// instante MAIOR que o ultimo emitido abre milissegundo novo; instante igual
/// ou menor cai no segundo caso e o id cresce pelo contador. O relogio da
/// maquina retrocede; o id, nao.
fn avancar(ultimo: (u64, u16), agora: u64) -> (u64, u16) {
    let (ultimo_ms, contador) = ultimo;
    if agora > ultimo_ms {
        let mut semente = [0u8; 2];
        sortear(&mut semente);
        (agora, u16::from_be_bytes(semente) & CONTADOR_SEMENTE)
    } else if contador < CONTADOR_MASCARA {
        (ultimo_ms, contador + 1)
    } else {
        (ultimo_ms + 1, 0)
    }
}

/// O estado do relogio logico, fechado num modulo so dele.
///
/// Por que um modulo, e nao um `static` ao lado das funcoes -- pedido 247.
/// O `static RELOGIO` ficava solto no modulo `uuid`, e o teste do contador
/// estourado o ESCREVIA para tras (`ms = 5_000`) para montar o cenario. Os
/// testes de um binario rodam em paralelo; sob carga, essa escrita caiu no
/// meio do laco de `v7_nunca_repete_nem_anda_para_tras`, o gerador viu
/// «milissegundo novo» dentro do MESMO milissegundo, re-semeou o contador na
/// metade de baixo e o id andou para tras: contador `0x8ae` seguido de
/// `0x409`, 54 vezes em 1.000 corridas, medido em 16/09/2026. O gerador
/// estava certo; o estado dele e que estava ao alcance de quem nao devia.
///
/// Aqui dentro o `static` e privado. O modulo `tests` e filho de `uuid`, nao
/// deste, e nao o enxerga: nao ha como um teste repor o defeito sem antes
/// abrir este modulo -- e e isso que a guarda `relogio-ao-alcance-do-teste`
/// do catalogo repoe para provar que o teste de concorrencia cai.
mod relogio {
    use std::sync::Mutex;

    /// Ultimo milissegundo emitido e o contador dentro dele.
    ///
    /// Num mutex porque a garantia de "nunca repete e nunca anda para tras"
    /// tem de valer entre threads: duas conexoes gravando ao mesmo tempo pedem
    /// id ao mesmo gerador. E um so para o processo inteiro, nao por thread:
    /// por thread, duas conexoes poderiam emitir o mesmo par.
    static ESTADO: Mutex<(u64, u16)> = Mutex::new((0, 0));

    /// Avanca o relogio logico do processo e devolve o par emitido. Ler o
    /// estado, decidir e gravar acontecem sob a mesma guarda: nao ha janela
    /// em que duas threads leiam o mesmo par.
    pub(super) fn passo(agora: u64) -> (u64, u16) {
        let mut guarda = match ESTADO.lock() {
            Ok(g) => g,
            // Mutex envenenado nao pode derrubar a geracao de id: o estado
            // dele e so um par de numeros, e seguir com o valor de dentro e
            // seguro.
            Err(e) => e.into_inner(),
        };
        *guarda = super::avancar(*guarda, agora);
        *guarda
    }

    /// Leitura para a mensagem de diagnostico do teste. Le e solta: o unico
    /// caminho de escrita e `passo`, e e isso que o pedido 247 comprou.
    #[cfg(test)]
    pub(super) fn ler() -> (u64, u16) {
        match ESTADO.lock() {
            Ok(g) => *g,
            Err(e) => *e.into_inner(),
        }
    }
}

impl Uuid {
    /// O UUID todo-zeros, `00000000-0000-0000-0000-000000000000`.
    pub const NULO: Uuid = Uuid([0u8; UUID_LEN]);

    pub fn bytes(&self) -> &[u8; UUID_LEN] {
        &self.0
    }

    pub fn de_bytes(b: [u8; UUID_LEN]) -> Uuid {
        Uuid(b)
    }

    /// Versao declarada nos 4 bits altos do byte 6. Vale 4 ou 7 aqui.
    pub fn versao(&self) -> u8 {
        self.0[6] >> 4
    }

    /// A variante da RFC 9562, nos bits altos do byte 8.
    ///
    /// Devolve `true` so para a variante `10x` -- a unica que o v4 e o v7
    /// desta casa escrevem. `0xx` e a variante antiga da Apollo NCS, `110` e
    /// a da Microsoft e `111` esta reservada: nenhuma das tres sai daqui, e
    /// um id que se diz v7 com variante de outra familia foi escrito a mao.
    pub fn variante_rfc(&self) -> bool {
        (self.0[8] & 0xC0) == 0x80
    }

    /// Todos os 16 bytes em zero.
    ///
    /// Vale a pena ter nome proprio porque o nulo e o valor que quem monta um
    /// id a mao escreve primeiro: nem a versao nem a variante o salvam, as
    /// duas tambem sao zero.
    pub fn e_nulo(&self) -> bool {
        self.0 == [0u8; UUID_LEN]
    }

    /// Milissegundos desde a epoca, para um v7. `None` em qualquer outra
    /// versao: ler relogio de um v4 seria ler bits sorteados.
    pub fn instante_ms(&self) -> Option<i64> {
        if self.versao() != 7 {
            return None;
        }
        let mut ms: u64 = 0;
        for b in &self.0[..6] {
            ms = (ms << 8) | *b as u64;
        }
        Some(ms as i64)
    }

    /// UUID v4: 122 bits sorteados. Sem ordem nenhuma -- use quando o id NAO
    /// deve revelar quando foi criado.
    pub fn v4() -> Uuid {
        let mut b = [0u8; UUID_LEN];
        sortear(&mut b);
        b[6] = (b[6] & 0x0F) | 0x40;
        b[8] = (b[8] & 0x3F) | 0x80;
        Uuid(b)
    }

    /// UUID v7: relogio em milissegundos nos 48 bits altos, contador de 12
    /// bits e 62 bits sorteados. Estritamente crescente.
    pub fn v7() -> Uuid {
        let (ms, contador) = relogio::passo(agora_ms());
        Uuid::montar_v7(ms, contador)
    }

    /// v7 com o instante escolhido a dedo. Existe para o teste poder conferir
    /// o layout contra o vetor do RFC sem depender do relogio da maquina.
    pub fn v7_em(ms: u64, contador: u16, aleatorios: [u8; 8]) -> Uuid {
        let mut u = Uuid::montar_v7(ms, contador);
        u.0[8..16].copy_from_slice(&aleatorios);
        u.0[8] = (u.0[8] & 0x3F) | 0x80;
        u
    }

    fn montar_v7(ms: u64, contador: u16) -> Uuid {
        let mut b = [0u8; UUID_LEN];
        let t = ms.to_be_bytes();
        b[..6].copy_from_slice(&t[2..]);
        let c = contador & CONTADOR_MASCARA;
        b[6] = 0x70 | ((c >> 8) as u8 & 0x0F);
        b[7] = (c & 0xFF) as u8;
        sortear(&mut b[8..]);
        b[8] = (b[8] & 0x3F) | 0x80;
        Uuid(b)
    }

    /// Le a forma canonica `8-4-4-4-12`. Aceita sem hifens e entre chaves,
    /// porque e assim que os ids chegam colados de fora.
    pub fn de_texto(s: &str) -> Result<Uuid> {
        let limpo: String = s
            .trim()
            .trim_start_matches('{')
            .trim_end_matches('}')
            .chars()
            .filter(|c| *c != '-')
            .collect();
        if limpo.len() != 32 {
            // O texto recebido sai pelo `citar` -- pedido 453: a mensagem vai
            // ao cliente e ao `acessos.log`, e um UUID de um megabyte nao e
            // UUID nenhum para citar.
            return Err(PhxError::Tipo(format!(
                "UUID precisa de 32 digitos hexadecimais, veio {}: {}",
                limpo.len(),
                citar(s)
            )));
        }
        let mut b = [0u8; UUID_LEN];
        hex_para(&limpo, &mut b)
            .map_err(|e| PhxError::Tipo(format!("UUID invalido {}: {e}", citar(s))))?;
        Ok(Uuid(b))
    }
}

impl Uuid256 {
    pub const NULO: Uuid256 = Uuid256([0u8; UUID256_LEN]);

    pub fn bytes(&self) -> &[u8; UUID256_LEN] {
        &self.0
    }

    pub fn de_bytes(b: [u8; UUID256_LEN]) -> Uuid256 {
        Uuid256(b)
    }

    /// 256 bits sorteados.
    pub fn aleatorio() -> Uuid256 {
        let mut b = [0u8; UUID256_LEN];
        sortear(&mut b);
        Uuid256(b)
    }

    /// Le 64 digitos hexadecimais. Aceita o prefixo `0x`, que e como hash de
    /// bloco costuma vir escrito.
    pub fn de_texto(s: &str) -> Result<Uuid256> {
        let limpo = s.trim().trim_start_matches("0x").trim_start_matches("0X");
        if limpo.len() != 64 {
            return Err(PhxError::Tipo(format!(
                "identificador de 256 bits precisa de 64 digitos hexadecimais, veio {}",
                limpo.len()
            )));
        }
        let mut b = [0u8; UUID256_LEN];
        hex_para(limpo, &mut b)
            .map_err(|e| PhxError::Tipo(format!("identificador invalido {}: {e}", citar(s))))?;
        Ok(Uuid256(b))
    }
}

fn hex_para(s: &str, dst: &mut [u8]) -> std::result::Result<(), String> {
    let b = s.as_bytes();
    for (i, alvo) in dst.iter_mut().enumerate() {
        let hi = digito(b[i * 2])?;
        let lo = digito(b[i * 2 + 1])?;
        *alvo = (hi << 4) | lo;
    }
    Ok(())
}

/// O digito vem do motor (`hash::digito_hex`, pedido 446): aqui morava uma
/// segunda copia da mesma tabela, e o que fica e so a frase de erro.
fn digito(c: u8) -> std::result::Result<u8, String> {
    crate::hash::digito_hex(c).ok_or_else(|| format!("caractere {:?} nao e hexadecimal", c as char))
}

fn escrever_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in bytes {
        write!(f, "{b:02x}")?;
    }
    Ok(())
}

impl fmt::Display for Uuid {
    /// Forma canonica em minusculas, com hifens: e a que o RFC manda escrever.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        escrever_hex(&self.0[0..4], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[4..6], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[6..8], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[8..10], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[10..16], f)
    }
}

impl fmt::Display for Uuid256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        escrever_hex(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// Milissegundo e contador de um v7, lidos dos bytes. Existe para a
    /// mensagem de falha dizer QUAL das duas metades andou para tras: o log
    /// do pedido 247 guardou so a linha do panico, e a cacada custou de novo.
    fn desmontar(u: &Uuid) -> (u64, u16) {
        let ms = u.instante_ms().unwrap_or(-1) as u64;
        let contador = (((u.0[6] & 0x0F) as u16) << 8) | u.0[7] as u16;
        (ms, contador)
    }

    /// A mensagem inteira de uma falha de monotonia: os dois ids em hexa, o
    /// milissegundo e o contador de cada um, o relogio da maquina e o estado
    /// do gerador no instante. Montada so quando cai -- os argumentos do
    /// `assert!` sao preguicosos -- e por isso nao custa nada no laco.
    fn diagnostico(i: usize, anterior: &Uuid, u: &Uuid) -> String {
        let (ms_a, c_a) = desmontar(anterior);
        let (ms_u, c_u) = desmontar(u);
        let (ms_r, c_r) = relogio::ler();
        format!(
            "id {i} nao cresceu: anterior {anterior} (ms {ms_a}, contador {c_a:#05x}) \
             depois {u} (ms {ms_u}, contador {c_u:#05x}); \
             relogio da maquina agora {} ms; estado do gerador = (ms {ms_r}, contador {c_r:#05x})",
            agora_ms(),
        )
    }

    /// O cenario do contador no teto, montado num estado LOCAL: e o que o
    /// teste do emprestimo confere, e o que `a_geracao_entre_fios...` roda em
    /// laco enquanto quatro fios geram -- para provar que montar o cenario
    /// nao alcanca o gerador. A versao antiga escrevia o `static` do modulo
    /// (`*RELOGIO.lock() = (5_000, MASCARA)`), e era isso que derrubava o
    /// vizinho que rodava em paralelo (pedido 247).
    fn cenario_do_contador_estourado() -> (u64, u16) {
        avancar((5_000, CONTADOR_MASCARA), 5_000)
    }

    #[test]
    fn v7_tem_o_layout_do_rfc_9562() {
        // Vetor do apendice A.6 do RFC 9562: 2022-02-22T19:22:22.000Z.
        let ms = 0x017F_22E2_79B0u64;
        let u = Uuid::v7_em(ms, 0x0CC3, [0x18, 0xC4, 0xDC, 0x0C, 0x0C, 0x07, 0x39, 0x8F]);
        assert_eq!(u.to_string(), "017f22e2-79b0-7cc3-98c4-dc0c0c07398f");
        assert_eq!(u.versao(), 7);
        assert_eq!(u.instante_ms(), Some(ms as i64));
        // Variante: os dois bits altos do byte 8 sao 0b10.
        assert_eq!(u.0[8] & 0xC0, 0x80);
    }

    #[test]
    fn v4_declara_versao_e_variante() {
        let u = Uuid::v4();
        assert_eq!(u.versao(), 4);
        assert_eq!(u.0[8] & 0xC0, 0x80);
        assert_eq!(u.instante_ms(), None, "v4 nao tem relogio para ler");
    }

    #[test]
    fn v7_nunca_repete_nem_anda_para_tras() {
        // O caso que interessa: milhares de ids no mesmo milissegundo.
        let mut anterior = Uuid::v7();
        for i in 0..20_000 {
            let u = Uuid::v7();
            assert!(u > anterior, "{}", diagnostico(i, &anterior, &u));
            anterior = u;
        }
    }

    #[test]
    fn comparar_bytes_e_comparar_tempo() {
        // E disto que o .ndx depende: memcmp na ordem certa.
        let a = Uuid::v7_em(1_000, 0, [0xFF; 8]);
        let b = Uuid::v7_em(2_000, 0, [0x00; 8]);
        assert!(a.0 < b.0, "o mais antigo tem de ordenar primeiro");
        assert!(a < b);
    }

    #[test]
    fn contador_estourado_empresta_do_futuro() {
        // Com o contador no teto, o proximo passo anda um milissegundo em vez
        // de repetir -- senao dois ids sairiam iguais.
        assert_eq!(cenario_do_contador_estourado(), (5_001, 0));
    }

    #[test]
    fn relogio_da_maquina_para_tras_nao_leva_o_id_junto() {
        // `agora_ms` e CLOCK_REALTIME, e o NTP pode puxa-lo para tras. O passo
        // so abre milissegundo novo com instante MAIOR que o ultimo emitido;
        // igual ou menor conta no mesmo milissegundo -- e o id cresce.
        assert_eq!(avancar((1_000, 5), 1_000), (1_000, 6));
        assert_eq!(avancar((1_000, 6), 999), (1_000, 7));
        assert_eq!(avancar((1_000, 7), 0), (1_000, 8));
        // Instante maior re-semeia na metade de baixo da faixa, e e por isso
        // que um estado escrito para tras faz o id cair (pedido 247): o
        // contador seguinte nasce menor que o anterior no MESMO milissegundo.
        let (ms, c) = avancar((1_000, 0x0FFE), 1_001);
        assert_eq!(ms, 1_001);
        assert!(
            c <= CONTADOR_SEMENTE,
            "semente {c:#05x} fora da metade de baixo"
        );
    }

    #[test]
    fn a_geracao_entre_fios_nunca_anda_para_tras() {
        // O nome comeca com «a_» de proposito: o libtest despacha em ordem
        // alfabetica, e este e o teste que tem de estar rodando enquanto os
        // vizinhos do modulo rodam -- foi um vizinho em paralelo que derrubou
        // o gerador no pedido 247.
        //
        // Quatro fios geram ate a bandeira cair (e nunca menos de 2.000 cada),
        // e cada um confere a propria sequencia. No fim, o conjunto inteiro
        // nao pode ter repetido: sequencia crescente por fio nao basta, dois
        // fios poderiam sair iguais entre si.
        let parar = Arc::new(AtomicBool::new(false));
        let fios: Vec<_> = (0..4)
            .map(|f| {
                let parar = Arc::clone(&parar);
                thread::spawn(move || {
                    let mut ids = Vec::with_capacity(50_000);
                    let mut anterior = Uuid::v7();
                    let mut i = 0usize;
                    while !parar.load(Ordering::Relaxed) || i < 2_000 {
                        let u = Uuid::v7();
                        assert!(u > anterior, "fio {f}: {}", diagnostico(i, &anterior, &u));
                        ids.push(u);
                        anterior = u;
                        i += 1;
                    }
                    ids
                })
            })
            .collect();
        // Enquanto eles geram, o cenario do contador estourado -- o do teste
        // vizinho -- e montado 64 vezes, com 1 ms entre cada. Montar cenario
        // NAO pode ser sentido por quem gera. A versao que escrevia o estado
        // global era sentida em 5,4% das corridas sob carga com UMA escrita;
        // com 64, cai sempre -- e e assim que o catalogo prova esta guarda.
        for _ in 0..64 {
            assert_eq!(cenario_do_contador_estourado(), (5_001, 0));
            thread::sleep(Duration::from_millis(1));
        }
        parar.store(true, Ordering::Relaxed);
        let mut todos: Vec<Uuid> = Vec::new();
        for fio in fios {
            match fio.join() {
                Ok(ids) => todos.extend(ids),
                // O panico do fio ja saiu no log com o diagnostico; aqui so
                // se repete para o teste cair com ele, e nao com um `Err`.
                Err(e) => std::panic::resume_unwind(e),
            }
        }
        let n = todos.len();
        todos.sort();
        todos.dedup();
        assert_eq!(todos.len(), n, "houve id repetido entre fios");
    }

    #[test]
    fn texto_vai_e_volta() {
        let u = Uuid::v7();
        assert_eq!(Uuid::de_texto(&u.to_string()).unwrap(), u);
        // Sem hifens e entre chaves tambem entram.
        let s = u.to_string();
        let sem = s.replace('-', "");
        assert_eq!(Uuid::de_texto(&sem).unwrap(), u);
        assert_eq!(Uuid::de_texto(&format!("{{{s}}}")).unwrap(), u);
        assert_eq!(Uuid::de_texto(&s.to_uppercase()).unwrap(), u);
    }

    #[test]
    fn texto_torto_e_recusado() {
        for ruim in [
            "",
            "nao-e-uuid",
            "017f22e2-79b0-7cc3-98c4-dc0c0c07398",
            "zz",
        ] {
            assert!(Uuid::de_texto(ruim).is_err(), "aceitou {ruim:?}");
        }
    }

    #[test]
    fn nulo_se_escreve_todo_zero() {
        assert_eq!(
            Uuid::NULO.to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    /// O que `de_texto` NAO confere, dito por teste.
    ///
    /// Ele le a forma canonica e nada mais: versao e variante passam como
    /// vierem. Isso e de proposito -- uma coluna de tipo `Uuid` guarda id de
    /// fora, e ali o v4, o v1 e ate o nulo sao dado legitimo de quem grava.
    /// Quem precisa de id v7 DE VERDADE confere no proprio caminho; ver
    /// `valores::coluna_de_json`, que e onde o `id` de coluna entra.
    #[test]
    fn de_texto_nao_julga_versao_nem_variante() {
        let nulo = Uuid::de_texto("00000000-0000-0000-0000-000000000000").unwrap();
        assert_eq!(nulo, Uuid::NULO);
        assert_eq!(nulo.versao(), 0);
        assert!(!nulo.variante_rfc());
        assert!(nulo.e_nulo());

        let cheio = Uuid::de_texto("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        assert_eq!(cheio.versao(), 15);
        // `1111` casa `11x`, que e a variante reservada -- nao a da RFC.
        assert!(!cheio.variante_rfc());
        assert!(!cheio.e_nulo());
    }

    /// O que o motor SORTEIA sempre passa nos dois crivos.
    #[test]
    fn v7_e_v4_daqui_tem_versao_e_variante_da_rfc() {
        for _ in 0..64 {
            let u = Uuid::v7();
            assert_eq!(u.versao(), 7);
            assert!(u.variante_rfc(), "v7 com variante fora da RFC: {u}");
            assert!(!u.e_nulo());

            let q = Uuid::v4();
            assert_eq!(q.versao(), 4);
            assert!(q.variante_rfc(), "v4 com variante fora da RFC: {q}");
        }
    }

    /// A variante `0xx` (NCS) e a `110` (Microsoft) tambem sao recusadas pelo
    /// crivo -- e nao so o `11x` do id todo-um.
    #[test]
    fn variante_antiga_e_da_microsoft_nao_sao_da_rfc() {
        let mut b = *Uuid::v7().bytes();
        b[8] = 0x00; // 0xx -- NCS
        assert!(!Uuid::de_bytes(b).variante_rfc());
        b[8] = 0xC0; // 110 -- Microsoft
        assert!(!Uuid::de_bytes(b).variante_rfc());
        b[8] = 0xA5; // 101 -- a da RFC, com o resto sorteado
        assert!(Uuid::de_bytes(b).variante_rfc());
    }

    #[test]
    fn id256_cabe_um_sha256() {
        // O hash do vetor classico "abc" do FIPS 180-4, que a suite do
        // hash.rs ja confere. Aqui o que se testa e o transporte.
        let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let id = Uuid256::de_texto(hex).unwrap();
        assert_eq!(id.to_string(), hex);
        assert_eq!(Uuid256::de_texto(&format!("0x{hex}")).unwrap(), id);
    }

    #[test]
    fn id256_torto_e_recusado() {
        assert!(Uuid256::de_texto("ba7816bf").is_err());
        assert!(Uuid256::de_texto(&"z".repeat(64)).is_err());
    }

    #[test]
    fn id256_aleatorio_nao_repete() {
        let a = Uuid256::aleatorio();
        let b = Uuid256::aleatorio();
        assert_ne!(a, b);
        assert_ne!(a, Uuid256::NULO);
    }

    #[test]
    fn sorteio_de_reserva_nao_devolve_zeros() {
        // O caminho de Windows tem de produzir bytes de verdade.
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        misturar(&mut a);
        misturar(&mut b);
        assert_ne!(a, [0u8; 16]);
        assert_ne!(a, b);
    }
}

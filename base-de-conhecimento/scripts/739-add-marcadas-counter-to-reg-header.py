# Add marcadas counter to reg header
# 28/08 19:40

import re, pathlib
p = pathlib.Path("crates/phxsql-store/src/reg.rs")
s = p.read_text()

# 1. versao
antigo = """/// Versao do `.reg`.
///
/// A 3 acrescentou `proximo_rownum` nos bytes 92..100, que estavam reservados.
/// Arquivo da 2 nao abre nesta versao -- o contador nao existiria, e comecar do
/// zero num arquivo que ja tem linhas faria a coluna repetir numero.
const VERSAO: u16 = 3;"""
novo = """/// Versao do `.reg`.
///
/// A 3 acrescentou `proximo_rownum` nos bytes 92..100, que estavam reservados.
/// Arquivo da 2 nao abre nesta versao -- o contador nao existiria, e comecar do
/// zero num arquivo que ja tem linhas faria a coluna repetir numero.
///
/// A 4 acrescentou `marcadas` nos bytes 108..116, pela mesma razao: um arquivo
/// da 3 traria zero ali, e zero quer dizer "nenhuma linha marcada" -- o motor
/// concluiria que a posicao e o `rownum` sao a mesma coisa numa tabela onde
/// nao sao, e o salto por bisseccao cairia na linha errada em silencio.
const VERSAO: u16 = 4;"""
assert antigo in s
s = s.replace(antigo, novo)

# 2. campo na struct
antigo = """    /// Proximo valor da coluna de sistema `rownum`. So o volume 1 manda.
    proximo_rownum: u64,"""
novo = """    /// Proximo valor da coluna de sistema `rownum`. So o volume 1 manda.
    proximo_rownum: u64,
    /// Quantas linhas vivas estao marcadas como excluidas (soft delete).
    ///
    /// Existe para uma pergunta que precisa de resposta em tempo constante:
    /// *a posicao de uma linha na lista e o `rownum` dela?* Se ninguem apagou
    /// de vez e ninguem marcou, sim -- e ai pular para a posicao 500.000 e uma
    /// bisseccao de vinte leituras em vez de meio milhao de passos.
    ///
    /// Contar marcadas varrendo seria pagar a tabela inteira justamente para
    /// decidir se da para nao pagar a tabela inteira. Por isso o numero mora
    /// no cabecalho, ao lado do `live_count`, que ja e um contador do mesmo
    /// tipo. E, como todo contador em cache, ele pode divergir se um caminho
    /// esquecer de mexer nele: `recontar_marcadas` refaz a conta varrendo, e e
    /// o que o reparo chama.
    marcadas: u64,"""
assert antigo in s
s = s.replace(antigo, novo)

# 3. criar
antigo = """            proxima_sequencia: 0,
            proximo_rownum: 1,
            baldes: Vec::new(),"""
novo = """            proxima_sequencia: 0,
            proximo_rownum: 1,
            marcadas: 0,
            baldes: Vec::new(),"""
assert antigo in s
s = s.replace(antigo, novo)

# 4. abrir: leitura
antigo = """        let proximo_rownum = c.u64(92).max(1);"""
novo = """        let proximo_rownum = c.u64(92).max(1);
        let marcadas = c.u64(108);"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """            proxima_sequencia,
            proximo_rownum,
            baldes: Vec::new(),"""
novo = """            proxima_sequencia,
            proximo_rownum,
            marcadas,
            baldes: Vec::new(),"""
assert antigo in s
s = s.replace(antigo, novo)

# 5. gravar_cabecalho
antigo = """            por_u64(&mut buf, 92, self.proximo_rownum);
        }"""
novo = """            por_u64(&mut buf, 92, self.proximo_rownum);
            por_u64(&mut buf, 108, self.marcadas);
        }"""
assert antigo in s
s = s.replace(antigo, novo)

# 6. acessores, logo depois de rownum_atual
antigo = """    /// Proximo `rownum` que a tabela vai entregar.
    pub fn rownum_atual(&self) -> u64 {
        self.proximo_rownum.max(1)
    }"""
novo = """    /// Proximo `rownum` que a tabela vai entregar.
    pub fn rownum_atual(&self) -> u64 {
        self.proximo_rownum.max(1)
    }

    /// Quantas linhas vivas estao marcadas como excluidas.
    pub fn marcadas(&self) -> u64 {
        self.marcadas
    }

    /// Soma `delta` ao contador de marcadas. Negativo desmarca.
    ///
    /// Nao grava o cabecalho: quem chama esta no meio de uma operacao que ja
    /// vai grava-lo (o `excluir` e o `atualizar` do slot escrevem o cabecalho
    /// no fim). Escrever duas vezes seria pagar um `write` por nada.
    pub fn mudar_marcadas(&mut self, delta: i64) {
        if delta >= 0 {
            self.marcadas = self.marcadas.saturating_add(delta as u64);
        } else {
            self.marcadas = self.marcadas.saturating_sub(delta.unsigned_abs());
        }
    }

    /// Regrava o contador de marcadas e leva ao disco.
    pub fn definir_marcadas(&mut self, n: u64) -> Result<()> {
        if self.marcadas == n {
            return Ok(());
        }
        self.marcadas = n;
        self.gravar_cabecalho(1)
    }"""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("reg.rs ok")

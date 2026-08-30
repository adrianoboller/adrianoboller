# Move the memory ceiling inside the channel read
# 30/08 06:35

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()

velho = '''    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        let mut linha = String::new();
        let lidos = leitor.read_line(&mut linha)?;'''
novo = '''    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        self.ler_ate(leitor, TETO_DO_REGISTRO)
    }

    /// O mesmo, com o teto explicito -- so para quem tem motivo para outro.
    ///
    /// O teto mora AQUI, e nao em quem chama, por um motivo que a integracao
    /// de duas frentes tornou visivel: a frente da trava pos um limite no
    /// `read_line` da replica, a frente da cifra trocou aquele `read_line` por
    /// este canal, e juntar as duas sem cuidado devolveria o limite ilimitado
    /// -- com quem escolhe o tamanho sendo o outro lado do fio. No canal, o
    /// teto vale para todo mundo que le, cifrado ou claro.
    pub fn ler_ate<L: BufRead>(&mut self, leitor: &mut L, teto: u64) -> Result<Recebido> {
        let mut linha = String::new();
        // O `+1` e o que separa "coube" de "estourou" sem contar o que ainda
        // vem. Passou do teto, a conexao esta no meio de uma linha e nao serve
        // mais: por isso a recusa e erro, e a proxima rodada abre outra.
        let lidos = {
            let mut limitado = leitor.take(teto + 1);
            limitado.read_line(&mut linha)?
        };
        if lidos as u64 > teto {
            return Err(PhxError::LimiteExcedido(format!(
                "o outro lado mandou mais de {} MiB num registro so, e este \\
                 lado nao guarda isso na memoria; baixe o tamanho do lote de \\
                 quem serve ou parta a tabela",
                teto / (1024 * 1024)
            )));
        }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# A constante entra junto do enum, onde quem le o modulo a encontra.
alvo='impl Canal {'
assert s.count(alvo)==1
s=s.replace(alvo, '''/// Teto de um registro do fio, em bytes.
///
/// Sem ele o `read_line` e ilimitado e quem decide quanta memoria este lado
/// reserva e o outro lado da conexao.
pub const TETO_DO_REGISTRO: u64 = 128 * 1024 * 1024;

''' + alvo)
open(p,'w',encoding='utf-8').write(s)
print("teto movido para dentro do canal, valendo para cifrado e claro")

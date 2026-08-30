# Corrigir o hex e o sono do laco
# 29/08 03:40

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    fn laco_da_replica(self: Arc<Self>, origem: crate::config::Origem) {
        let espera = Duration::from_secs(origem.reconectar_em);
        loop {
            match self.rodada_da_replica(&origem) {
                Ok(0) => {}
                Ok(n) => eprintln!("replicacao [{}]: {n} evento(s) aplicado(s)", origem.nome),
                Err(e) => eprintln!("replicacao [{}]: {e}", origem.nome),
            }
            std::thread::sleep(espera);
        }
    }'''
novo = '''    /// O laco que puxa do source, para sempre.
    ///
    /// # Rodada produtiva nao dorme
    ///
    /// O `reconectar_em` e o intervalo entre PERGUNTAS EM VAO -- quanto tempo
    /// esperar antes de perguntar de novo a um source que nao tinha nada. Uma
    /// rodada que aplicou eventos nao espera: se o source tinha o que dar,
    /// provavelmente ainda tem, porque ele continuou escrevendo enquanto esta
    /// rodada aplicava.
    ///
    /// Dormir depois de toda rodada era o que fazia a replica parecer lenta.
    /// A bancada media `linhas / tempo_ate_alcancar` e chegava a 4.273
    /// eventos/s -- mas o caminho de CPU inteiro, dos dois lados, custa 23,9 us
    /// por evento (`--example onde-doi-na-replica`), o que da mais de 40.000/s.
    /// O que sobrava era sono, e nao trabalho: o numero media o `reconectar_em`.
    fn laco_da_replica(self: Arc<Self>, origem: crate::config::Origem) {
        let espera = Duration::from_secs(origem.reconectar_em);
        loop {
            match self.rodada_da_replica(&origem) {
                // Nada a fazer: agora sim, espera antes de perguntar de novo.
                Ok(0) => std::thread::sleep(espera),
                Ok(n) => {
                    eprintln!("replicacao [{}]: {n} evento(s) aplicado(s)", origem.nome);
                    // Sem sono: volta ja. `alcancar_tabela` recusa girar em
                    // falso -- ela erra se aplicar e a posicao nao andar --,
                    // entao um `Ok(n)` com n > 0 e progresso de verdade e este
                    // laco nao tem como virar giro em vazio.
                }
                // Erro dorme, e e de proposito: source fora do ar ou conexao
                // caida pedem espera, senao a replica bate na porta fechada
                // num laco fechado.
                Err(e) => {
                    eprintln!("replicacao [{}]: {e}", origem.nome);
                    std::thread::sleep(espera);
                }
            }
        }
    }'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('laco ok')

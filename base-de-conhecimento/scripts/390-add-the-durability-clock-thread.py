# Add the durability clock thread
# 28/08 13:50

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
marca = '''        let servidor = Arc::clone(self);
        std::thread::spawn(move || {
            let mut ultimo = 0i64;'''
assert s.count(marca) == 1
NOVO = '''    /// O relogio que fecha a janela de durabilidade quando ninguem grava.
    ///
    /// Sem ele, a gravacao em lote so sincronizaria na PROXIMA gravacao -- e um
    /// servidor que recebe a ultima venda do dia as 18h e fica quieto deixaria
    /// essa venda sem `fsync` a noite inteira. O relogio acorda a cada janela e
    /// descarrega o que ficou.
    ///
    /// Em `por_operacao` e em `sistema` ele nao tem o que fazer: um sincroniza
    /// sempre, o outro nunca.
    fn ligar_relogio_de_gravacao(self: &Arc<Self>) {
        if self.config.recursos.durabilidade != Durabilidade::PorLote {
            return;
        }
        let ms = self.config.recursos.lote_milissegundos.max(20);
        let servidor = Arc::clone(self);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(ms));
            if servidor.janela.pendente() > 0 {
                servidor.janela.fechar();
                servidor.descarregar_sujas();
            }
        });
    }

'''
# entra antes do metodo que contem esse spawn
i = s.rindex('    fn ', 0, s.index(marca))
s = s[:i] + NOVO + s[i:]
p.write_text(s)
print('relogio')

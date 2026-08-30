# Wire the reservation registry into the server
# 29/08 02:53

import pathlib
p = pathlib.Path("crates/phxsql-server/src/lib.rs")
s = p.read_text()
s = s.replace("pub mod blacklist;", "pub mod blacklist;\npub mod carga;", 1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# 1) Sessao ganha a ligacao
s = s.replace('''struct Sessao {
    usuario: Option<Usuario>,''','''struct Sessao {
    usuario: Option<Usuario>,
    /// A conexao desta sessao, do registro de ligacoes. Zero quando o pedido
    /// veio pela porta web, que nao tem conexao para amarrar nada.
    ///
    /// E a ela que a reserva de carga morre amarrada: sem um id de CONEXAO, a
    /// reserva so poderia ser identificada pelo login -- e aí duas janelas do
    /// mesmo usuario seriam o mesmo dono, o que e exatamente o contrario de
    /// exclusivo.
    ligacao: u64,''',1)

# 2) o registro de cargas no servidor
s = s.replace('''    profiler: Mutex<crate::profiler::Profiler>,''',
'''    /// Tabelas reservadas para carga (`BULKINSERT`).
    cargas: Mutex<crate::carga::Cargas>,
    profiler: Mutex<crate::profiler::Profiler>,''',1)
s = s.replace('''            profiler: Mutex::new(crate::profiler::Profiler::default()),''',
'''            cargas: Mutex::new(crate::carga::Cargas::default()),
            profiler: Mutex::new(crate::profiler::Profiler::default()),''',1)

# 3) a sessao da porta de dados leva o id da ligacao
s = s.replace('''        let mut sessao = Sessao::default();
        let (id_ligacao, morrer) = match self.ligacoes.lock() {
            Ok(mut l) => l.entrar(&ip, porta, crate::agora_ms(), para_fechar),
            Err(_) => (0, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        };''',
'''        let (id_ligacao, morrer) = match self.ligacoes.lock() {
            Ok(mut l) => l.entrar(&ip, porta, crate::agora_ms(), para_fechar),
            Err(_) => (0, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        };
        let mut sessao = Sessao {
            ligacao: id_ligacao,
            ..Sessao::default()
        };''',1)

# 4) a saida da conexao solta o que ela reservou
s = s.replace('''        let _saida_do_registro = AoSair(|| {
            if let Ok(mut l) = self.ligacoes.lock() {
                l.sair(id_ligacao);
            }
        });''',
'''        let _saida_do_registro = AoSair(|| {
            if let Ok(mut l) = self.ligacoes.lock() {
                l.sair(id_ligacao);
            }
            // A PRIMEIRA rede de protecao da reserva de carga: a conexao caiu,
            // a tabela solta. Sem isto, um cliente morto no meio de uma carga
            // deixaria a tabela reservada ate o prazo vencer -- e o prazo e
            // medido em dezenas de minutos, de proposito.
            self.soltar_cargas_da_ligacao(id_ligacao);
        });''',1)
p.write_text(s)
print("ok")

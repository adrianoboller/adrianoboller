# Wire the profiler into the request loop
# 28/08 22:57

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# 1. o campo no servidor
antigo = """            avisados: Mutex::new(HashMap::new()),
            conexoes: AtomicUsize::new(0),
        }))"""
novo = """            avisados: Mutex::new(HashMap::new()),
            conexoes: AtomicUsize::new(0),
            profiler: Mutex::new(crate::profiler::Profiler::default()),
        }))"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    avisados: Mutex<HashMap<String, i64>>,"""
novo = """    avisados: Mutex<HashMap<String, i64>>,
    /// O que esta chegando pela porta, quando alguem liga para olhar.
    profiler: Mutex<crate::profiler::Profiler>,"""
assert antigo in s
s = s.replace(antigo, novo)

# 2. a captura, exatamente entre o read_line e o despacho
antigo = """            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;"""
novo = """            // O PROFILER olha AQUI: o pedido chegou pelo soquete e nada foi
            // gravado ainda. Se a operacao travar, ele ja apareceu na tela
            // como «em curso» -- que e justamente o pedido que se quer achar.
            let marca = {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                self.profiler.lock().ok().and_then(|mut p| {
                    p.chegou(
                        &linha,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        &ip,
                        quando_ms,
                    )
                })
            };

            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;
            if let Some(serial) = marca {
                if let Ok(mut p) = self.profiler.lock() {
                    p.terminou(
                        serial,
                        duracao,
                        resultado.is_ok(),
                        &resultado.as_ref().err().map(|e| e.to_string()).unwrap_or_default(),
                    );
                }
            }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

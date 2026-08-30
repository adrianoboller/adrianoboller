# Add the atomic gate to both capture points
# 29/08 02:06

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# 1) o espelho atomico do "ligado"
alvo = '''    profiler: Mutex<crate::profiler::Profiler>,'''
novo = '''    profiler: Mutex<crate::profiler::Profiler>,
    /// Espelho de `profiler.ligado`, para o caminho quente nao tomar a trava.
    ///
    /// # Por que existe
    ///
    /// O profiler desligado nao pode custar nada. Sem este espelho, TODO
    /// pedido pagava, antes de a conferencia acontecer: dois `Json::analisar`
    /// do corpo inteiro -- um para achar database/tabela, outro para o nome da
    /// operacao --, tres `String` alocadas, e um mutex. Num `inserir_lote` de
    /// cinco mil linhas isso e analisar meio megabyte de JSON duas vezes, para
    /// no fim `chegou` olhar `ligado` e devolver `None`.
    ///
    /// Um `AtomicBool` lido com `Relaxed` custa uma instrucao e nao serializa
    /// ninguem. A trava so e tomada quando ha o que registrar.
    ///
    /// A janela de divergencia e de um pedido: quem ligar o profiler pode nao
    /// ver o pedido que ja estava em voo. Ligar a observacao no meio de um
    /// pedido nao promete pegar aquele pedido -- promete pegar os proximos.
    profiler_ligado: AtomicBool,'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''            profiler: Mutex::new(crate::profiler::Profiler::default()),''',
'''            profiler: Mutex::new(crate::profiler::Profiler::default()),
            profiler_ligado: AtomicBool::new(false),''', 1)

# 2) os dois pontos de captura: o portao barato vem primeiro
antigo_tcp = '''            let marca = {
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
            };'''
novo_tcp = '''            // Desligado, nao custa NADA: nem parse, nem alocacao, nem trava.
            let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
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
            } else {
                None
            };'''
assert s.count(antigo_tcp) == 1
s = s.replace(antigo_tcp, novo_tcp, 1)

antigo_web = '''                let marca = {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
                    let nome_op = Json::analisar(&pedido.corpo)
                        .ok()
                        .map(|j| j.texto_ou("op", "?").to_string())
                        .unwrap_or_else(|| "?".into());
                    self.profiler.lock().ok().and_then(|mut pr| {
                        pr.chegou(
                            &pedido.corpo,
                            &nome_op,
                            sessao.login(),
                            &alvo.database,
                            &alvo.tabela,
                            ip,
                            agora,
                        )
                    })
                };'''
novo_web = '''                let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
                    let nome_op = Json::analisar(&pedido.corpo)
                        .ok()
                        .map(|j| j.texto_ou("op", "?").to_string())
                        .unwrap_or_else(|| "?".into());
                    self.profiler.lock().ok().and_then(|mut pr| {
                        pr.chegou(
                            &pedido.corpo,
                            &nome_op,
                            sessao.login(),
                            &alvo.database,
                            &alvo.tabela,
                            ip,
                            agora,
                        )
                    })
                } else {
                    None
                };'''
assert s.count(antigo_web) == 1
s = s.replace(antigo_web, novo_web, 1)
p.write_text(s)
print("ok")

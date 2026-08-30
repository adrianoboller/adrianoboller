# Re-apply the atomic gate cleanly
# 29/08 02:11

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

s = s.replace("use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};",
              "use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};", 1)

# 1) o campo
alvo = "    profiler: Mutex<crate::profiler::Profiler>,"
novo = '''    profiler: Mutex<crate::profiler::Profiler>,
    /// Espelho de `profiler.ligado`, para o caminho quente nao tomar a trava.
    ///
    /// # Por que existe
    ///
    /// Observacao que nao esta ligada nao pode custar nada. Sem este espelho,
    /// TODO pedido pagava, antes de a conferencia acontecer: dois
    /// `Json::analisar` do corpo inteiro -- um para achar database/tabela,
    /// outro para o nome da operacao --, tres `String` alocadas, e um mutex.
    /// Num `inserir_lote` de cinco mil linhas isso e analisar meio megabyte de
    /// JSON duas vezes, para no fim `chegou` olhar `ligado` e devolver `None`.
    /// Medido: 7% da carga pela rede.
    ///
    /// Um `AtomicBool` lido com `Relaxed` custa uma instrucao e nao serializa
    /// ninguem. A trava so e tomada quando ha o que registrar.
    ///
    /// A janela de divergencia e de um pedido: quem liga o profiler pode nao
    /// ver o pedido que ja estava em voo. Ligar a observacao no meio de um
    /// pedido nao promete pegar aquele pedido -- promete pegar os proximos.
    profiler_ligado: AtomicBool,'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace("            profiler: Mutex::new(crate::profiler::Profiler::default()),",
"""            profiler: Mutex::new(crate::profiler::Profiler::default()),
            profiler_ligado: AtomicBool::new(false),""", 1)

# 2) os dois pontos de captura
antigo_tcp = '''            let marca = {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));'''
novo_tcp = '''            // Desligado, nao custa NADA: nem parse, nem alocacao, nem trava.
            let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));'''
assert s.count(antigo_tcp) == 1
s = s.replace(antigo_tcp, novo_tcp, 1)
antigo_fim = '''                        &ip,
                        quando_ms,
                    )
                })
            };'''
novo_fim = '''                        &ip,
                        quando_ms,
                    )
                })
            } else {
                None
            };'''
assert s.count(antigo_fim) == 1
s = s.replace(antigo_fim, novo_fim, 1)

antigo_web = '''                let marca = {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));'''
novo_web = '''                let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));'''
assert s.count(antigo_web) == 1
s = s.replace(antigo_web, novo_web, 1)
antigo_web_fim = '''                            ip,
                            agora,
                        )
                    })
                };'''
novo_web_fim = '''                            ip,
                            agora,
                        )
                    })
                } else {
                    None
                };'''
assert s.count(antigo_web_fim) == 1
s = s.replace(antigo_web_fim, novo_web_fim, 1)

# 3) o espelho acompanha ligar/desligar, dentro da trava
alvo = "        prof.ligar(filtro, &arquivo, teto, agora)?;"
novo = '''        prof.ligar(filtro, &arquivo, teto, agora)?;
        // Dentro da trava, e DEPOIS de `ligar` ter dado certo: um espelho que
        // sobe antes faria o caminho quente pagar por um profiler que nao ligou.
        self.profiler_ligado.store(true, Ordering::Relaxed);'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = "        prof.desligar(crate::agora_ms());"
novo = '''        prof.desligar(crate::agora_ms());
        self.profiler_ligado.store(false, Ordering::Relaxed);'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

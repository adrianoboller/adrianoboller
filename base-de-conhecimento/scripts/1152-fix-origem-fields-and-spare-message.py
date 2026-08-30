# Fix Origem fields and spare message
# 29/08 17:25

import pathlib
# 1) A origem sintetica do cluster nasce em STREAMING: o pulso ja marca o
#    ritmo, e agendar a origem do cluster atrasaria a deteccao de master novo.
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = """                senha_hash: c.senha_hash.clone(),
                senha: String::new(),
            };"""
novo = """                senha_hash: c.senha_hash.clone(),
                senha: String::new(),
                // A origem do cluster e sempre STREAMING: quem marca o ritmo e
                // o pulso. Agendar aqui atrasaria a deteccao de master novo.
                cada_minutos: 0,
                hora: String::new(),
            };"""
assert velho in t; p.write_text(t.replace(velho, novo, 1)); print("origem do cluster: streaming")

# 2) O spare recusa ate leitura -- e texto que gente le, entao TRADUZ.
p = pathlib.Path("crates/phxsql-server/src/mensagens.rs"); t = p.read_text()
t = t.replace('''    MensagemFabrica {
        nome: "erro.redireciona",''', '''    MensagemFabrica {
        nome: "erro.spare_em_espera",
        textos: [
            "spare em espera: {detalhe}",
            "serveur de secours en attente : {detalhe}",
            "spare on standby: {detalhe}",
            "server di riserva in attesa: {detalhe}",
            "Reserveserver im Wartezustand: {detalhe}",
            "servidor de reserva en espera: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.redireciona",''', 1)
t = t.replace('        PhxError::Redireciona(m) => ("erro.redireciona", m.clone()),',
              '        PhxError::SpareEmEspera(m) => ("erro.spare_em_espera", m.clone()),\n        PhxError::Redireciona(m) => ("erro.redireciona", m.clone()),', 1)
t = t.replace('            PhxError::Redireciona(String::new()),',
              '            PhxError::SpareEmEspera(String::new()),\n            PhxError::Redireciona(String::new()),', 1)
p.write_text(t); print("erro.spare_em_espera nos seis idiomas")

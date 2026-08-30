# Reconcile Cancelado into messages table
# 29/08 18:41

import pathlib
p = pathlib.Path("crates/phxsql-server/src/mensagens.rs"); t = p.read_text()
t = t.replace('''    MensagemFabrica {
        nome: "erro.spare_em_espera",''', '''    // O cancelamento e a TERCEIRA que nao se traduz por moldura, e por um
    // motivo proprio: o texto ja vem montado do ponto que cancelou, com quem
    // encerrou e o que estava rodando. Traduzir de verdade exigiria mover
    // essa montagem para a tabela -- trabalho que so vale quando alguem
    // pedir a tela noutro idioma e esbarrar nisto.
    MensagemFabrica {
        nome: "erro.cancelado",
        textos: [
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.spare_em_espera",''', 1)
t = t.replace('        PhxError::SpareEmEspera(m) => ("erro.spare_em_espera", m.clone()),',
              '        PhxError::Cancelado(m) => ("erro.cancelado", m.clone()),\n        PhxError::SpareEmEspera(m) => ("erro.spare_em_espera", m.clone()),', 1)
t = t.replace('            PhxError::SpareEmEspera(String::new()),',
              '            PhxError::Cancelado(String::new()),\n            PhxError::SpareEmEspera(String::new()),', 1)
p.write_text(t); print("erro.cancelado reconciliado")

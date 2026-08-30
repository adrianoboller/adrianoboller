# Add non-translatable messages for the two new errors
# 29/08 17:15

import pathlib
p = pathlib.Path("crates/phxsql-server/src/mensagens.rs")
t = p.read_text()

# 1) As duas variantes novas na FABRICA. Os seis idiomas sao IGUAIS de
#    proposito -- ver o comentario.
entrada = '''    MensagemFabrica {
        nome: "erro.acesso_negado",'''
novo = '''    // As duas mensagens que NAO se traduzem, e por motivos diferentes:
    //
    // - `erro.redireciona` comeca com `REDIRECIONA host:porta`, que e o
    //   pedaco que o cliente RECORTA para se reapontar. Traduzir quebraria
    //   todo cliente que trata o redirecionamento -- a moldura e so o
    //   detalhe, nos seis idiomas.
    // - `erro.sinal` carrega a MESSAGE_TEXT que o DONO DO BANCO escreveu no
    //   gatilho. Substitui-la por texto nosso seria apagar a voz dele; o
    //   idioma dessa mensagem e escolha de quem escreveu o gatilho.
    MensagemFabrica {
        nome: "erro.redireciona",
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
        nome: "erro.sinal",
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
        nome: "erro.acesso_negado",'''
assert entrada in t
t = t.replace(entrada, novo, 1)

# 2) O match do decompor.
alvo = '        PhxError::Io(m) => ("erro.erro_de_es", m.to_string()),'
t = t.replace(alvo, '''        PhxError::Redireciona(m) => ("erro.redireciona", m.clone()),
        PhxError::Sinal { estado, mensagem } => (
            "erro.sinal",
            format!("{mensagem} (SIGNAL SQLSTATE {estado})"),
        ),
''' + alvo, 1)

# 3) O teste que confere variante por variante ganha as duas novas.
alvo2 = '            PhxError::Io(std::io::Error::other("x")),'
t = t.replace(alvo2, '''            PhxError::Redireciona(String::new()),
            PhxError::Sinal {
                estado: String::new(),
                mensagem: String::new(),
            },
''' + alvo2, 1)
p.write_text(t)
print("mensagens.rs: fabrica, decompor e teste atualizados")

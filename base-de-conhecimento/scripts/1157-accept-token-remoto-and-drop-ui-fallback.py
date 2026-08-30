# Accept token_remoto and drop UI fallback
# 29/08 17:34

import pathlib
# 1) O motor passa a aceitar `token_remoto`, com precedencia.
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = '                    token: p.texto_ou("token", "").to_string(),'
novo = '''                    // `token_remoto` primeiro, e `token` so como resto:
                    // no `/api` da tela o campo `token` JA e o de quem pede
                    // aqui, entao mandar o do outro servidor com o mesmo nome
                    // faz um sobrescrever o outro dentro do mesmo objeto.
                    token: {
                        let remoto = p.texto_ou("token_remoto", "");
                        if remoto.is_empty() {
                            p.texto_ou("token", "").to_string()
                        } else {
                            remoto.to_string()
                        }
                    },'''
assert velho in t, "token nao casou"
p.write_text(t.replace(velho, novo, 1)); print("1. token_remoto aceito pelo motor")

# 2) A tela perde o ramo da op provisoria, como o proprio autor previu.
p = pathlib.Path("crates/phxsql-server/ui/index.html"); t = p.read_text()
velho = '''  let bruto, deQuem = "replicacao_testar";
  try {
    bruto = await api("replicacao_testar", base);
  } catch (e) {
    if (!e || e.nome !== "NAO_ENCONTRADO") throw e;
    deQuem = "replicacao_sondar";
    bruto = await api("replicacao_sondar", base);
  }'''
novo = '''  const deQuem = "replicacao_testar";
  const bruto = await api("replicacao_testar", base);'''
assert velho in t
t = t.replace(velho, novo, 1)
t = t.replace('''   `replicacao_testar` é a operação do motor novo, e diz mais: `id_servidor`,
   a chave de cada tabela e os `impedimentos` por modo. `replicacao_sondar` é
   a provisória deste worktree, que existe só para o assistente poder ser
   provado antes daquele motor chegar — quando ele entrar, esta função perde o
   segundo ramo e a operação sai do servidor.''',
'''   `replicacao_testar` é a operação do motor, e responde tudo que o assistente
   precisa antes de prometer qualquer coisa: `id_servidor`, a chave de cada
   tabela e os `impedimentos` por modo. (Houve aqui uma sonda provisória,
   enquanto as duas frentes corriam em paralelo; ela saiu na integração, junto
   com o ramo de reserva que a chamava.)''')
p.write_text(t); print("2. tela chama so a definitiva")

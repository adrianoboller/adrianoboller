# Show storage location and alerts on the config screen
# 28/08 15:06

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''  const web = c.web || {};

  folha("Configurações gerais do servidor",'''
b='''  const web = c.web || {};
  const al = c.alertas || {};
  const em = al.email || {};

  folha("Configurações gerais do servidor",'''
assert a in s; s=s.replace(a,b,1)

a='''     ${grupoDeAjustes("Armazenamento", [
       ["base", c.base, "a pasta que guarda os databases"],
       ["max_linhas", c.max_linhas, "teto de linhas por resposta"],
       ["espelho", simNao(c.espelho), "grava o .bkp junto do .reg"],
       ["somente_leitura", simNao(c.somente_leitura), "recusa toda operação de escrita"],
     ])}'''
b='''     ${grupoDeAjustes("Armazenamento", [
       ["base", c.base, "a pasta que guarda os databases — aceita caminho absoluto: "
         + "C:\\\\database, D:\\\\database, /var/lib/phxsql"],
       // Caminho relativo vale a partir de ONDE O SERVIDOR FOI INICIADO, e não
       // de onde o config.json está. Subir por outro caminho passa a ver outro
       // banco, e a única forma de tirar essa dúvida é mostrar o resolvido.
       ["(resolvido)", c.base_absoluta, "é aqui que os dados estão agora"],
       ["dblink", c.dblink, "arquivo com as ligações para bancos de fora"],
       ["max_linhas", c.max_linhas, "teto de linhas por resposta"],
       ["espelho", simNao(c.espelho), "grava o .bkp junto do .reg"],
       ["somente_leitura", simNao(c.somente_leitura), "recusa toda operação de escrita"],
     ])}

     ${grupoDeAjustes("Alerta de espaço em disco", [
       ["alertas.ligado", simNao(al.ligado), "vigia o base, o destino do backup e o que estiver em caminhos"],
       ["alertas.livre_minimo_percentual", al.livre_minimo_percentual,
        "sobre usado+livre, como o df — reserva do sistema de arquivos não conta como livre"],
       ["alertas.livre_minimo_mb", al.livre_minimo_mb, "piso absoluto; o que apertar primeiro dispara"],
       ["alertas.checar_minutos", al.checar_minutos, ""],
       ["alertas.repetir_horas", al.repetir_horas, "silêncio entre dois avisos do mesmo caminho"],
       ["alertas.caminhos", (al.caminhos || []).join(", ") || "só o base e o backup", ""],
       ["alertas.email.ligado", simNao(em.ligado), "sem TLS: serve para relé interno na porta 25"],
       ["alertas.email.servidor", em.servidor ? `${em.servidor}:${em.porta}` : "", ""],
       ["alertas.email.para", (em.para || []).join(", ") || "ninguém", ""],
       ["alertas.email.senha", em.senha, "nunca sai daqui, nem no log"],
     ])}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

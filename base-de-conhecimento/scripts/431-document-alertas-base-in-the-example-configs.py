# Document alertas + base in the example configs
# 28/08 14:38

import json,io
bloco = '''
  "_base": [
    "Onde os bancos moram. Cada banco e uma PASTA, e cada schema uma subpasta",
    "dentro dela.",
    "",
    "Aceita caminho absoluto -- e num servidor de verdade e assim que deve",
    "ficar. No Windows:  \\"base\\": \\"C:\\\\\\\\database\\"  ou  \\"D:\\\\\\\\database\\".",
    "No Linux:  \\"base\\": \\"/var/lib/phxsql\\".",
    "",
    "Caminho relativo vale a partir de ONDE O SERVIDOR FOI INICIADO, e nao de",
    "onde o config.json esta. Subir por outro caminho passa a ver outro banco.",
    "O painel mostra o caminho ja resolvido, para tirar essa duvida."
  ],

  "_alertas": [
    "Aviso de disco apertado. Vem DESLIGADO.",
    "",
    "O vigia olha o base, o destino do backup (quando agendado) e o que",
    "estiver em caminhos. O percentual e o piso em MB valem NO OU: o que",
    "chegar primeiro dispara. Sozinho cada um erra de um lado -- 10% de um",
    "disco de 8 TB sao 800 GB, que nao e aperto; e 1 GB livre num disco de",
    "20 GB e aperto sem chegar perto de 10%.",
    "",
    "A conta e sobre usado+livre, como a do df, e nao sobre o tamanho do",
    "disco: reserva de sistema de arquivos e cota nao estao a disposicao de",
    "ninguem.",
    "",
    "repetir_horas e o silencio entre dois avisos do MESMO caminho. Sem ele o",
    "alerta vira enxurrada, porque um disco cheio continua cheio.",
    "",
    "E-MAIL, O LIMITE HONESTO: este cliente NAO fala TLS -- a std nao traz",
    "TLS e o projeto nao aceita crate. A conversa e em texto claro, entao ele",
    "serve para um RELE QUE VOCE CONTROLA (postfix na maquina ou na rede",
    "local, porta 25), e nao para entregar direto em provedor publico.",
    "Se preencher usuario e senha, os dois viajam em base64, que e",
    "codificacao e nao cifra -- prefira liberar o IP no rele.",
    "",
    "senha_env aponta uma variavel de ambiente e e o caminho recomendado:",
    "config.json costuma ir para o controle de versao, variavel nao. A senha",
    "do rele nunca aparece na resposta de config nem no log."
  ],

  "alertas": {
    "ligado": false,
    "livre_minimo_percentual": 10,
    "livre_minimo_mb": 1024,
    "checar_minutos": 15,
    "repetir_horas": 6,
    "caminhos": [],
    "email": {
      "ligado": false,
      "servidor": "127.0.0.1",
      "porta": 25,
      "de": "phxsql@empresa.com.br",
      "para": ["dba@empresa.com.br"],
      "usuario": "",
      "senha_env": "PHXSQL_SMTP_SENHA",
      "assunto": "PhxSql: espaco em disco",
      "timeout_s": 10
    }
  },
'''
for n in ("01","02","03"):
    p=f'exemplos/Config_exemplo_{n}.json'
    s=open(p).read()
    if '"_alertas"' in s: continue
    marca='\n  "_backup": ['
    if marca not in s:
        marca='\n  "log_acessos"'
    assert marca in s, p
    s=s.replace(marca, bloco+marca,1)
    open(p,'w').write(s)
    json.loads(s)   # tem de continuar sendo JSON valido
    print(p,'ok')

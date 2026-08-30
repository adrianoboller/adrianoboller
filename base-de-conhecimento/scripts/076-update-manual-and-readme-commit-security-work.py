# Update manual and README, commit security work
# 27/08 19:31

p='MANUAL.txt'
s=open(p).read()
s=s.replace('''11. SEGURANCA
--------------------------------------------------------------------------------
    - TROQUE O TOKEN antes de expor a porta. O binario recusa subir com o
      token em branco.''','''11. SEGURANCA
--------------------------------------------------------------------------------
11.1 As tres formas de login

    Da mais segura para a menos:

    DESAFIO-RESPOSTA  a senha NAO trafega
        {"token":"...","op":"desafio","usuario":"adriano"}
        -> {"sal":"...","iteracoes":210000,"nonce":"..."}
        dk    = pbkdf2(senha, sal, iteracoes)
        prova = hmac(dk, nonce + "," + nonce_cliente + "," + usuario)
        {"token":"...","op":"login","usuario":"adriano",
         "nonce_cliente":"...","prova":"..."}

        O nonce vale UMA VEZ SO: gravar o dialogo e repetir nao autentica.

    BASE64            a senha trafega codificada
        {"token":"...","op":"login",
         "usuario_b64":"YWRyaWFubw==","senha_b64":"..."}

        ATENCAO: Base64 NAO E CRIPTOGRAFIA. Quem captura o pacote decodifica
        com um comando:  echo '...' | base64 -d
        Serve contra grep casual e olho de quem passa; nao contra sniffer.

    TEXTO PURO        a senha trafega legivel
        {"token":"...","op":"login","usuario":"adriano","senha":"..."}

11.2 Politica: o que ninguem pede

    Na secao "seguranca" do config.json:

        "comandos_proibidos": ["excluir","reindexar"],
        "bases_proibidas": ["financeiro"],
        "tentativas_ate_bloquear": 5,
        "janela_minutos": 10,
        "bloqueio_minutos": 60,
        "blacklist": "blacklist.json"

    Vale para TODO MUNDO, root incluso, e e conferido ANTES do token. Nao e
    permissao de usuario: e o que este servidor nao faz por esta porta.

    GRAVE (comando ou base proibida)  bloqueia o IP NA HORA
    LEVE  (token, senha, ip de fora)  conta na janela e bloqueia no limite

11.3 A lista de bloqueio

    O blacklist.json guarda IP, data e hora, motivo, comando e ate quando:

        { "ip":"203.0.113.9",
          "desde":"2026-08-27 19:30:17,323",
          "ate":"2026-08-27 20:30:17,323",
          "motivo":"token invalido", "comando":"ping",
          "tentativas":3, "firewall":false }

    IP bloqueado tem a conexao recusada ANTES de tudo. bloqueio_minutos 0
    bloqueia ate alguem desfazer.

        phxsqld --bloqueios
        phxsqld --desbloquear 203.0.113.9

    Pelo protocolo: {"op":"bloqueios"} e {"op":"desbloquear","ip":"..."}.
    O servidor rele o arquivo quando ele muda, entao o --desbloquear vale
    na hora, sem reiniciar.

11.4 A regra de firewall

        "firewall": {
          "ligado": false,
          "bloquear":    ["/usr/sbin/iptables","-I","INPUT","-s","{ip}","-j","DROP"],
          "desbloquear": ["/usr/sbin/iptables","-D","INPUT","-s","{ip}","-j","DROP"]
        }

    O BLOQUEIO NUNCA DEPENDE DISTO. Um IP na lista e recusado dentro do
    servidor, sem firewall e sem root. A regra e um extra.

    Desligada por padrao. Roda SEM SHELL, com o comando vindo inteiro do
    config.json como lista de argumentos, e o IP validado como endereco
    antes de entrar no lugar do {ip}. Se o comando falhar, o bloqueio
    continua valendo e a falha vira aviso no log.

    Para o iptables funcionar o phxsqld precisa de root ou CAP_NET_ADMIN --
    aumento de privilegio real. Recusar a conexao dentro do processo ja
    resolve quase tudo e nao pede privilegio nenhum.

    Detalhes em docs/SEGURANCA.md.

11.5 O basico

    - TROQUE O TOKEN antes de expor a porta. O binario recusa subir com o
      token em branco.''')
open(p,'w').write(s)

p='README.md'
s=open(p).read()
s=s.replace('''O motor de armazenamento está completo e testado: **166 testes**''','''O motor de armazenamento está completo e testado: **195 testes**''')
s=s.replace('''| Cadastro de usuários, senha em hash, permissão por base | pronto |''','''| Cadastro de usuários, senha em hash, permissão por base | pronto |
| Login por desafio-resposta (a senha não trafega) e Base64 | pronto |
| Blacklist com bloqueio automático e gancho de firewall | pronto |''')
s=s.replace('''  USUARIOS.md      cadastro, senha em hash e as dez permissões''','''  USUARIOS.md      cadastro, senha em hash e as dez permissões
  SEGURANCA.md     política, blacklist, firewall e as formas de login''')
s=s.replace('''**Cadastrar usuários só aperta a segurança.**''','''**Base64 não é criptografia, e o código diz isso.** O `login` aceita
`senha_b64`, mas há um teste chamado
`base64_nao_esconde_nada_de_quem_captura` que decodifica a credencial para
provar. O que protege de verdade é o desafio-resposta, onde a senha nunca sai
da máquina do cliente.

**Firewall quebrado não vira porta aberta.** O bloqueio vale sempre dentro do
servidor; a regra de `iptables` é um extra desligado por padrão, roda sem
shell e valida o IP como endereço antes de usá-lo.

**Cadastrar usuários só aperta a segurança.**''')
open(p,'w').write(s)
print("MANUAL e README atualizados")

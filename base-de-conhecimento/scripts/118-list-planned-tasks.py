# List planned tasks
# 27/08 20:17

import subprocess
tarefas = [
 ("Criar CHANGELOG.md","Historico de versoes com o bug corrigido e tudo desta rodada."),
 ("Marca registrada (R) na documentacao","Auditar docs e por (R) em todo nome de banco de terceiros."),
 ("Tabela em memoria e SelectMemory","Motor: tabela residente em RAM, com operacao selecionar_memoria/SelectMemory."),
 ("Chave assimetrica Ed25519","Assinatura no login, chave publica no config.json. Zero dependencias, vetores RFC 8032."),
 ("Login da interface: host, porta, chave, database","Campos novos e relay para servidor remoto."),
 ("Alternador sol/lua na interface","Tema claro e escuro com botao."),
 ("Start/stop do servico de dados","Parar, iniciar e trocar a porta da 5000 sem derrubar o processo."),
 ("Sistema de backup","Copia consistente dos cinco arquivos, com verificacao."),
 ("Portas de replicacao no config.json","Ida e volta do socket entre source e replicas."),
 ("Atualizar o dossie completo","Secoes novas e numeros remedidos."),
]
for s,d in tarefas: print(f"{s}\t{d}")

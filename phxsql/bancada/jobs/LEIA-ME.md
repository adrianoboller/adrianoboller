# `backup-agendado.py` — o backup rodando pela AGENDA, o histórico e a restauração

`prova-avisos.py` já prova o AVISO de jobs por e-mail (falha, parado, os
cortes do opt-in), com um SMTP falso. Faltava a outra metade do pedido do
dono, 07/09/2026: *«teste de backups agendados e listagem log se deu tudo
certo»* — o job de backup rodando pelo RELÓGIO de verdade (não por
`job_rodar` manual), o `backup.json` que sai no disco, o histórico que a op
`jobs` devolve, um job que falha de propósito, e a restauração conferida
(`docs/RESTAURACAO.md` — backup não conferido não é backup).

```bash
python3 bancada/jobs/backup-agendado.py
```

Variável: `PHX_SONDA_PORTA` (padrão 6725). O script sobe o `phxsqld` **duas
vezes** sobre a mesma raiz de dados, de propósito: `docs/JOBS.md` é claro que
"ligar o primeiro job pela tela não acorda ninguém até o próximo arranque" —
o relógio só sobe se algum job já estava ligado NO ARRANQUE. A primeira subida
grava a `loja` com dados de verdade e desce; a segunda já nasce com o
`jobs.json` escrito e o job de backup ligado, e só nela o relógio sobe — sem
essa ordem, a primeira volta do relógio (quase imediata, porque `ultimo_ms` é
zero) faria um backup vazio, correto mas sem nada para restaurar depois.

A partir daí a sonda espera DUAS corridas de verdade (a primeira quase
imediata, a segunda ~1 minuto depois, pelo relógio que confere a cada 30 s —
nunca encurtado, porque "encurtar o relógio para o teste seria provar outro
relógio"), mostra os arquivos e o `backup.json` no disco, lê o histórico pela
própria op `jobs`, cadastra um segundo job cujo destino é um caminho que passa
por um ARQUIVO comum (o sistema operacional recusa criar diretório dentro de
um arquivo — a falha é provada, não suposta), confere que o histórico registra
a falha, descreve — sem inventar um segundo SMTP falso, porque `prova-avisos.py`
já prova isso ao vivo — o e-mail que sairia com `alertas.email` ligado, e
termina restaurando o backup bom com `restaurar_backup` (modo novo) e
conferindo que a cópia tem o mesmo número de linhas que o original.

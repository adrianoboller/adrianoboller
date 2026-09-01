# O backup do repositório, e por que ele sai à mão

Este documento existe porque um **papel que não está cumprindo tem de aparecer
como não cumprindo**, em vez de sumir do relatório. O papel é o do versionador:
ele commita, e deveria empurrar para o `origin`. Ele não empurra.

## O diagnóstico, medido

Por três rodadas o texto do projeto dizia que «o `push` recusa com 403 por
identidade da sessão». Era plausível, e plausível não é medido — havia pelo
menos três causas possíveis, com consertos diferentes:

1. o **proxy** do ambiente barrando o `git-receive-pack`;
2. a **credencial** não chegando ao GitHub;
3. o **GitHub** recusando a identidade.

As três foram separadas, e o que decide é comparar leitura e escrita **no mesmo
host, com a mesma credencial, na mesma sessão TLS**:

| operação | endpoint | resultado |
|---|---|---|
| ler | `GET /info/refs?service=git-upload-pack` | **funciona** — devolve a ponta remota |
| escrever | `GET /info/refs?service=git-receive-pack` | **403 Forbidden** |

E a resposta do 403 traz `X-Github-Request-Id` e
`Content-Type: application/x-git-receive-pack-advertisement` — dois cabeçalhos
que **o GitHub carimba**, e que um proxy que tivesse barrado a conexão não
teria como forjar. O status do proxy do agente confirma pelo outro lado:
**zero** falhas de relay para `github` na janela inteira.

Sobram a causa 3 e só ela. A identidade autenticada é `EnginePrint`
(`get_me` do servidor MCP responde com ela, e o mesmo servidor **lê** o
repositório sem erro), e ela tem leitura e não tem escrita.

**Conserto:** dar direito de escrita a essa identidade em
`adrianoboller/adrianoboller`. Não há nada a consertar do lado de cá — e é
exatamente isso que o diagnóstico medido comprou: **parar de procurar** no
lugar errado.

## Enquanto isso: o pacote

A entrega sai por `git bundle`, que é o formato do próprio git para carregar
história por fora da rede:

```bash
git bundle create phxsql-$(date +%Y%m%d-%H%M).bundle claude/capacidades-disponiveis-y6auxh
git bundle verify phxsql-AAAAMMDD-HHMM.bundle    # confere ANTES de entregar
```

Do outro lado, ele é um remoto como outro qualquer:

```bash
git clone phxsql-AAAAMMDD-HHMM.bundle phxsql
# ou, num clone que ja existe:
git fetch /caminho/para/o.bundle claude/capacidades-disponiveis-y6auxh
```

O `verify` não é enfeite: ele é que diz **«the bundle records a complete
history»**, e um pacote truncado só se descobre na hora de restaurar, que é a
pior hora possível.

### Por que o `.bundle` não é versionado

Está no `.gitignore`, na mesma folha dos worktrees e pelo mesmo motivo: é uma
cópia do próprio repositório, e versioná-la poria o repositório dentro dele
mesmo. Com um agravante que o worktree não tem — **`git rm` depois não devolve
o espaço**, porque o objeto continua alcançável pelo commit que o trouxe. Um
pacote commitado por engano fica na história para sempre, e são 16 MB por
rodada.

O pacote é **artefato de entrega, não fonte**. Se um se perder, o próximo se
refaz do mesmo comando e sai idêntico, porque vem da mesma história.

### O que o pacote NÃO substitui

O `push` não é só transporte: sem ele não há CI (o
`.github/workflows/portoes.yml` nunca corre), não há revisão pelo diff na
web, e a única cópia da história fora desta máquina é a que alguém guardou.
Chamar o pacote de «backup resolvido» seria a mesma mentira que dizer que uma
guarda anotada está provada.

## O que NÃO se faz para contornar

O servidor MCP do GitHub tem `push_files` e `create_or_update_file`, e eles
**escrevem construindo um commit novo a partir do conteúdo dos arquivos**. Não
transferem história: 146 commits virariam **um**, e junto iriam as mensagens —
que neste projeto são o entregável, porque *commit conta a decisão e o motivo*.
Trocar a história por um retrato do último estado é perder exatamente o que o
versionador existe para guardar. Se algum dia for a única saída, é decisão do
dono, e não conserto de quem está no meio da rodada.

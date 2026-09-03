# O backup do repositório

## RESOLVIDO em 03/09/2026: o `push` funciona

**A recusa acabou, e a receita de conferência escrita neste documento foi
exatamente o que a mediu.** Rodada em 03/09, às 12h: `git push -u origin
claude/capacidades-disponiveis-y6auxh` empurrou `d4dc424..cd561ce` — **248
commits** —, e a confirmação não é a palavra do `git`, é a do GitHub:
`git ls-remote` devolve `cd561cec…` para a branch, idêntico ao `HEAD` local.
São **391 commits** no `origin`.

O acesso de escrita foi concedido em algum momento entre a medição de ontem e
esta; nada mudou do lado do código. E é aí que está a lição, que vale mais que
a boa notícia: **limitação registrada também envelhece.** O 403 estava medido,
escrito com cabeçalho e request-id, e por isso ninguém o pôs em dúvida — nem
eu, que passei a sessão inteira dizendo «o `push` recusa com 403» sem tentar
uma vez. Número digitado envelhece calado; **limitação diagnosticada envelhece
do mesmo jeito, e pior, porque vem com prova ao lado.**

Fica valendo a regra: **limitação que bloqueia um papel se remede a cada
rodada**, com a receita que o próprio documento carrega (o `--dry-run` da §
«Como se confere que funcionou»). O pacote por `git bundle` continua existindo,
agora como segunda via e não como única saída.

O que segue abaixo é **história**, e não descrição do presente.

---

# Por que ele saía à mão (até 03/09/2026)

Este documento existiu porque um **papel que não está cumprindo tem de aparecer
como não cumprindo**, em vez de sumir do relatório. O papel é o do versionador:
ele commitava, e deveria empurrar para o `origin`. Ele não empurrava.

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

## A decisão: o acesso, e não um contorno

Posta a medição, o dono decidiu **conceder escrita à identidade da sessão** em
vez de aceitar qualquer contorno. É a escolha certa por um motivo que vale
registrar: os dois contornos possíveis custavam mais do que resolvem.

- **Achatar pelo `push_files` do MCP** entregaria o conteúdo e perderia as
  mensagens — e aqui *commit conta a decisão e o motivo*, então o que subiria
  seria um retrato do último estado, não a história. Perder-se-ia exatamente o
  que o versionador existe para guardar.
- **Continuar só de pacote** mantém a história intacta, mas deixa o CI parado
  para sempre (ver «O que o pacote NÃO substitui»).

E há o ganho que só apareceu quando se mediu a segunda auditoria externa: das
sete prioridades que ela lista para a 0.19.0, **duas — release reproduzível e
CI — já estão construídas e não rodam por causa deste 403.** O acesso não
destrava só o backup; destrava dois itens do caminho crítico sem uma linha de
código nova.

**O que o dono faz:** reconectar a autorização do GitHub em
*claude.ai → Configurações → Conectores*, garantindo que
`adrianoboller/adrianoboller` esteja no conjunto permitido **com escrita**. A
leitura já funciona, então o que falta é só a permissão de escrita —
`git-receive-pack`.

**Como se confere que funcionou**, sem adivinhar:

```bash
git push --dry-run origin claude/capacidades-disponiveis-y6auxh
```

Se voltar a ponta em vez de `403`, está resolvido. O `--dry-run` é de
propósito: ele exerce o **mesmo** `git-receive-pack` que o push real, sem
mexer em nada.

## Enquanto isso: o pacote

A entrega sai por `git bundle`, que é o formato do próprio git para carregar
história por fora da rede:

```bash
./backup.sh                    # gera E PROVA; nao entregue pacote de outro jeito
```

O script existe porque *pacote gerado por script, nunca montado à mão* — e
porque o procedimento que estava escrito aqui **estava errado**, do jeito mais
perigoso possível: ele aprovava pacote podre. Ver a seção seguinte.

Do outro lado, ele é um remoto como outro qualquer:

```bash
git clone phxsql-AAAAMMDD-HHMM.bundle phxsql
# ou, num clone que ja existe:
git fetch /caminho/para/o.bundle claude/capacidades-disponiveis-y6auxh
```

### O `git bundle verify` NÃO confere o conteúdo — medido

A primeira versão deste documento mandava conferir com `git bundle verify`, e
dizia que era ele quem garantia a integridade. **Está errado.** Medido:

```
cortei 2 MiB do fim de um pacote bom
git bundle verify  ->  «The bundle records a complete history»,  saída 0
git clone dele     ->  «error: index-pack died»,                 saída 128
```

O `verify` lê o **cabeçalho**: quais refs o pacote traz e se a história é
auto-suficiente — nenhum commit-pré-requisito faltando. Ele **não lê o
packfile**, então não vê conteúdo corrompido nem truncado. Quem só roda o
`verify` entrega backup podre com a consciência limpa.

Por isso o `backup.sh` **restaura de verdade**, em três passos, e o critério do
terceiro não é «o clone não deu erro»: é o SHA do objeto `tree` do HEAD ser o
**mesmo** dos dois lados. Árvore igual quer dizer conteúdo idêntico byte a byte
para todo arquivo versionado — e isso um clone que morreu no meio não consegue
fingir.

O `verify` continua no passo 1, porque vale **por si**: pega pré-requisito
faltando, que é pacote incompleto por construção. Só não vale pelo que este
documento dizia que ele valia.

*Prova real nos dois sentidos, que é o que achou o erro:* com o pacote
truncado, o passo 2 reprova (saída 128) e o `verify` sozinho aprova (saída 0).
Um backup só reprova na hora de restaurar — e essa é a pior hora possível.

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

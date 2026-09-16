# Cognição: os dez papéis viram arquivos de agente — e o `model:` fica de fora

- **Assunto:** formalizar os papéis pétreos como agentes invocáveis
  (`.claude/agents/`) sem gravar identificador de modelo em artefato versionado
- **Descoberto:** 2026-09-16, ~02:10 (papel A, ao atender «adicione os agentes e
  subagentes necessários» depois do comparativo com o Phoenix Cast)
- **Arquivos:** `.claude/agents/*.md` (README + 10 agentes),
  `phxsql/docs/MODELOS.md` (seção do tier por agente)

## 1. O que aconteceu

O dono subiu uma proposta de equipe (Phoenix Cast, 25 papéis) e pediu para
adicionar os agentes e subagentes necessários. Até aqui os dez papéis viviam só
como **governança** no `CLAUDE.md` — texto, não agente invocável. Passei-os para
`.claude/agents/`, mais o SEC e dois subagentes de pesquisa que o comparativo
marcou como acréscimo medido.

## 2. O que eu concluí primeiro, e estava errado

**Ia gravar o `model:` em cada arquivo de agente** — é o campo natural do
formato, e é onde a escolha de modelo do papel «mora». Errado, e a pétrea é
clara: *esta sessão é proibida de pôr identificador de modelo em artefato
versionado (commit, comentário, documento)*. Um `.claude/agents/dba.md` com um
`model:` de nome cravado é exatamente isso — identificador de modelo num arquivo
que vai para o `origin`.

O segundo erro embutido: pensei em **abrir os 25 papéis do Cast**. A pétrea A
corta antes — «a obrigação não é abrir dez agentes por tarefa». Metade do Cast é
uma equipe de domínio SaaS (conectores, redes, anúncios), que serve a um produto
que o PhxSql-motor não é. Adicionar todos seria confundir **cobertura** com
**headcount**.

## 3. O que a medição/decisão disse

- **A tensão se resolve separando contrato de tier.** O arquivo do agente
  carrega o CONTRATO (o que faz, o que NÃO faz — revisor não escreve, ninguém
  comita sozinho) e as ferramentas; o **tier** (forte/leve + motivo, sem nome)
  vai para o `MODELOS.md`, e o orquestrador o aplica na convocação, na conversa.
  É a mesma convenção que já existia para as frentes; só passou a valer para
  agente também.
- **Ferramenta por função não é enfeite, é a garantia do papel.** `dba`, `qa`,
  `seguranca` e os `pesquisa-*` ficam com só leitura/busca — um revisor que pode
  editar deixa de ser revisor. `engenheiro`, `prova-real`, `designer`,
  `documentacao` escrevem, porque o entregável deles é código/teste/tela/doc.
- **Contagem:** 10 agentes (B, C, E, F, G, H, J + SEC + 2 subagentes de J), não
  25. A (orquestrador) é a sessão; D e I são scripts (`zelador.sh`,
  `backup.sh`); a camada de domínio SaaS ficou dispensada **por escopo**,
  registrada — dispensa registrada é decisão, não esquecimento.

## 4. A regra

**Papel vira arquivo de agente com o CONTRATO e as ferramentas; o MODELO fica
de fora do arquivo — o tier (nível + motivo, nunca o nome) mora no `MODELOS.md`
e o orquestrador o aplica na convocação. Adicionar agente é cobertura de papel,
nunca headcount: só entra quem tem domínio no nosso produto.**

## 5. Como está guardado hoje

- `.claude/agents/README.md` amarra cada agente a um dos dez papéis, diz o que
  NÃO virou agente (A/D/I) e por quê, e a recusa por escopo da camada SaaS.
- Dez arquivos de agente, sem `model:`, com ferramentas por função.
- `phxsql/docs/MODELOS.md` tem a tabela de tier por agente.
- **Onde o buraco fica:** o `.claude/agents/` não tem guarda automática ainda —
  nada reprova um agente novo que grave `model:` ou que duplique um papel. Se a
  equipe crescer, uma catraca «nenhum agente traz `model:`» e «todo agente
  aponta um papel do CLAUDE.md» fecharia isso, no molde das listas paralelas do
  servidor. Nomeado, não construído.

# O que a aula «Claude Code na Prática» ensina ao questionário

Fonte: transcrição da aula de Guilherme Lazarotto (idealização, documentação,
prototipação com Google Stitch, desenvolvimento com Claude Code, deploy com
Docker e EasyPanel, integração com n8n). Lida inteira; abaixo, o que vale
para um desenvolvedor WLanguage convertendo um projeto, e o que não vale.

## O que a aula faz bem, e o plugin ainda não fazia

| Da aula | O que o plugin tinha | O que entrou (letra L, 3.12.0) |
| --- | --- | --- |
| A documentação gera **um prompt de kickoff** com requisitos, banco e stack, e é ele que abre o Claude Code | manifesto, config e CLAUDE.md, mas nenhum prompt pronto para a primeira sessão | `.wx-migration/prompts/kickoff.md`, montado das respostas (empresa, escopo da v1, banco, destino, estratégia, ambiente) |
| A documentação gera **um prompt de prototipação** (Stitch) e o `DESIGN.md` volta de lá | `DESIGN.md` do questionário, sem prompt para a ferramenta de protótipo | `.wx-migration/prompts/prototipacao.md`, com a tela modelo (F0), os botões (F9–F13) e as telas do legado listadas |
| «O segredo está no contexto»: kickoff + DESIGN.md + prints + skills | contexto espalhado em `.wx-migration/` sem um mapa | `INDEX_FILES.md` na raiz: o mapa de tudo que o Claude Code pode ler, com uma linha por arquivo dizendo o que é e quando abrir |
| Skills do Supabase instaladas no projeto; skills como «pedaço de prompt» | skills só no plugin, nenhuma no projeto de destino | `.claude/skills/` do projeto: `regras-do-legado` (aponta para a matriz e a hierarquia de evidências) e `legado-para-destino` (o mapeamento de peças e a estratégia escolhida) |
| MCP do Supabase conectado antes do primeiro prompt | nada | `.mcp.json` gerado com os servidores marcados (Supabase, PostgreSQL, GitHub, Playwright), sempre sem chave |
| Loop «copiou o erro, colou no Claude» | portão G0 e licença como hooks do plugin | hooks do **projeto** em `.claude/settings.json`: teste ao parar e lint ao editar, com os comandos que o usuário informou |
| Requisitos funcionais da **primeira versão** («o sistema deve…») | inventário completo, sem corte de v1 | `L1 requisitos da v1`: a lista do que a primeira entrega precisa ter, cada item vira `NFR-`/`BR-` na matriz |
| Deploy: Dockerfile, variáveis de ambiente, porta, domínio, VPS com painel | `implantacao` era um texto livre em H | `L3 implantação`: alvo, domínio, porta, Dockerfile e compose gerados por perfil, `.env` com nomes, healthcheck |
| Página `/docs` da API com `curl` pronto para o n8n, protegida por API key | K7 pedia webhooks e fluxos | K7 ganhou `documentacao_da_api` e `api_key_ref`; o `integracao.md` pede a página |
| Convenção de commits (feat, fix, chore) | K6 ligava o GitHub | K6 ganhou `convencao_de_commits` |
| Fuso errado no n8n quebrou o agendamento | A pergunta o fuso do banco | o kickoff repete fuso e locale de A e K7 para o destino inteiro |

## O que a aula faz e o plugin não deve copiar

- **Vibe coding sem saber a sintaxe.** Vale para um sistema novo de quatro
  telas; não vale para converter um ERP com regras de dez anos. Aqui a regra
  de negócio vem do legado, com origem localizável e golden master, nunca da
  conversa. O kickoff gerado diz isso na primeira linha.
- **Senha colada no `.env` na tela.** A aula cola a service role e a senha do
  banco no `.env.local` e depois no painel. O plugin guarda só nomes de
  variáveis; o `.env.exemplo` sai sem valores.
- **Testar direto em produção.** A aula subiu o Dockerfile sem rodar local.
  O `instalar-ambiente.sh` e o hook de teste existem para o erro aparecer
  antes do deploy.
- **Stack decidida pela ferramenta.** A aula usa Next + Supabase porque é o
  que o autor conhece. O plugin orienta pela equipe que vai manter, pelo
  produto e pelo volume (letra H), e o Supabase é uma opção de K5, não o
  padrão.
- **RLS desligado «para arrumar depois».** Papéis por nível são perguntados
  em K2; o SQL sai com `readonly` e `readwrite` separados desde o começo.

## Onde cada coisa passa a viver

```
<projeto>/
  CLAUDE.md                 regras + aponta para INDEX_FILES.md e respostas_questionario.md
  INDEX_FILES.md            mapa de arquivos, regravado a cada aplicação
  .claude/settings.json     hooks do projeto (teste ao parar, lint ao editar) e permissões
  .claude/hooks/            testar.sh, lint.sh (gerados dos comandos de L4)
  .claude/skills/           regras-do-legado/, legado-para-destino/
  .mcp.json                 servidores MCP marcados em L5 (sem chaves)
  Dockerfile, docker-compose.yml   por perfil de H, se L3 pedir
  .wx-migration/prompts/    kickoff.md, prototipacao.md
```

O RAG do plugin continua sendo o corpus WLanguage por tema
(`query_wlanguage_help.py --group`) mais os documentos de `.wx-migration/`;
o `INDEX_FILES.md` é o índice que faz o modelo achar o documento certo sem
abrir todos.

# Configuração da sessão — o que uma sessão nova traz e o que não traz

Feito em 09/10/2026, a pedido do dono, antes de trocar de sessão. A sessão do
Claude Code roda num contêiner que é descartado; o que **não está no git** some
com ele.

## Vem sozinho, pelo clone do ramo `claude/capacidades-disponiveis-y6auxh`

| o quê | onde |
|---|---|
| a lei do projeto (todas as pétreas) | `CLAUDE.md` (raiz) |
| a lei global, versão curta, reconstruída | `kit-portatil/lei-global/CLAUDE.md` |
| as 8 leis de processo genéricas | `kit-portatil/regras/` |
| os agentes e subagentes (os 10 papéis + SEC + pesquisa + tradutor + plugins) | `.claude/agents/*.md` e `.claude/agents/README.md` |
| o escalão de cada agente (nível, nunca nome de modelo) | `phxsql/docs/MODELOS.md` |
| o plugin próprio do projeto (`phxjev@phoenix`) | `.claude/settings.json` |
| a cognição, o avoid e o reuse | `phxsql/docs/cognicao/` |
| o que falta, pedido a pedido | `phxsql/docs/PENDENCIAS.md` |
| o plano da 0.21 e as decisões do dono | `phxsql/docs/propostas/plano-0.21.md` |
| as URLs das páginas publicadas | `CLAUDE.md` (seção do dossiê) |

## NÃO vem — e como repor

| o quê | como repor |
|---|---|
| os 4 plugins (Vercel, Supabase, oh-my-claudecode) e o MCP do Chrome DevTools | `bash kit-portatil/config-da-sessao/reinstalar-ferramentas.sh`; para ficarem de vez, ligue Vercel e Supabase como conectores em https://claude.ai/customize/connectors |
| a configuração de usuário (`~/.claude/settings.json`) | cópia em `settings-do-usuario.json`, nesta pasta (só a lista de plugins; não há segredo nela) |
| os alvos ARM musl e o ligador do Windows | o mesmo script |
| trabalho NÃO comitado, cópias de frente (`.claude/worktrees/`), o scratchpad | **nada disso vai**: só se troca de sessão com a árvore limpa e as frentes integradas |
| o histórico da conversa | não vai; o contexto que importa está nos documentos acima |

## Antes de trocar de sessão — a lista

1. `git status --short` vazio na raiz.
2. `git log origin/claude/capacidades-disponiveis-y6auxh -1` igual ao `HEAD`.
3. Nenhuma frente em andamento (agente rodando em `.claude/worktrees/`).
4. O pacote de configuração (`config-phxsql-<data>.tar.gz`) entregue ao dono
   como segunda via.

## Na sessão nova — o primeiro passo

```bash
cd phxsql && python3 docs/dossie/pagina-dos-pedidos.py   # o que falta, medido
bash ../kit-portatil/config-da-sessao/reinstalar-ferramentas.sh
```

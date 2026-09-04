# Relatório do plugin WX Claude Code 3.16.0

Medido em 2026-09-04 por `docs/dossie/numeros-do-plugin.py`; nenhum número abaixo foi digitado.

## O que é

Plugin do Claude Code que converte projetos WINDEV, WEBDEV e WINDEV Mobile para outra plataforma sem inventar o que o projeto faz: questionário guiado, gates com aprovação humana, equipe de agentes WLanguage sobre o Help oficial, PMO com Scrum, Kanban e PDCA, qualidade de tela com o Impeccable, serial de ativação, e o contexto da primeira sessão do Claude Code gerado das respostas.

## Números

| medida | valor |
| --- | ---: |
| agentes | 94 |
| papéis A–J | 10 |
| subagentes PDCA | 40 |
| especialistas WLanguage por tema | 7 |
| comandos / | 5 |
| skills | 3 |
| scripts Python | 21 |
| linhas de Python (scripts e hooks) | 11780 |
| documentos de referência | 18 |
| testes de regressão | 36 |
| hooks do plugin | 8 |
| blocos do questionário (0, A–L) | 13 |
| itens do bloco 0 | 16 |
| subperguntas de F (F0–F13) | 14 |
| itens de K | 8 |
| itens de L | 5 |
| arquivos que o questionário pode gerar | 36 |
| prints de sessões reais | 30 |
| cenas do vídeo | 23 |
| duração do vídeo | 3 min 03 s |
| corpus do Help (bytes) | 26750976 |
| páginas válidas do corpus | 12035 |
| linhas do manual | 606 |
| tabelas do exemplo ESTOQUE | 7 |

## O que foi provado em sessão real

Cada print em `docs/prints/` é a saída de uma sessão do Claude Code ou de um script, sem edição; a origem de cada um está em `docs/prints/gerar.md`. Entre eles: o questionário uma letra por vez, a senha colada que não é gravada nem repetida, a letra H com o processo de conversão, a tela modelo aberta antes de registrar, o serial de ativação recusando e depois liberando, a primeira sessão lendo `INDEX_FILES.md` e o kickoff, a exportação organizada e o zelador.

## O que não foi provado

- Nenhum projeto WINDEV real passou pelos gates G1 a G7 de ponta a ponta; o exemplo ESTOQUE é sintético.
- Os scripts de ambiente (K e L) são bash; não há versão PowerShell, e o público do plugin usa Windows.
- A licença é dissuasão (hook); a proteção real, servir corpus e agentes de um servidor, ficou para depois por decisão do dono.
- O custo em tokens do questionário inteiro numa sessão real não foi medido.

## Onde está cada coisa

- Manual: `MANUAL.md` (PDF em `docs/manual-de-uso.pdf`); oito capítulos.
- Página para investidores: `docs/investidor/`.
- Análise da aula de vibe coding: `docs/analise-aula-vibe-coding.md`.
- Telas do fluxo de licença: `docs/telas-licenca/`.
- Dossiê: `docs/dossie/dossie-wx-claude-code.html`, gerado deste mesmo medidor.

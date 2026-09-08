# Relatório do plugin WX Claude Code 3.47.0

Medido em 2026-09-08 por `docs/dossie/numeros-do-plugin.py`; nenhum número abaixo foi digitado.

## O que é

Plugin do Claude Code que converte projetos WINDEV, WEBDEV e WINDEV Mobile para outra plataforma sem inventar o que o projeto faz: questionário guiado, gates com aprovação humana, equipe de agentes WLanguage sobre o Help oficial, PMO com Scrum, Kanban e PDCA, qualidade de tela com o Impeccable, serial de ativação, e o contexto da primeira sessão do Claude Code gerado das respostas.

## Números

| medida | valor |
| --- | ---: |
| agentes | 94 |
| papéis A–J | 10 |
| subagentes PDCA | 40 |
| especialistas WLanguage por tema | 7 |
| comandos / | 33 |
| skills | 21 |
| skills de ERP (pacote skills.sh) | 8 |
| scripts Python | 40 |
| linhas de Python (scripts e hooks) | 17488 |
| documentos de referência | 19 |
| testes de regressão | 109 |
| hooks do plugin | 9 |
| blocos do questionário (0, A–M) | 14 |
| itens do bloco 0 | 16 |
| subperguntas de F (F0–F13) | 14 |
| itens de K | 9 |
| itens de L | 6 |
| arquivos que o questionário pode gerar | 102 |
| prints de sessões reais | 63 |
| cenas do vídeo | 29 |
| duração do vídeo | 3 min 38 s |
| cenas do vídeo de PHP para Rust | 11 |
| duração do vídeo de PHP para Rust | 1 min 26 s |
| cenas do vídeo da bateria de testes | 7 |
| duração do vídeo da bateria de testes | 0 min 52 s |
| cenas do vídeo do primeiro projeto | 25 |
| duração do vídeo do primeiro projeto | 3 min 35 s |
| cenas do vídeo dos PDFs do WINDEV a Rust + React | 15 |
| duração do vídeo dos PDFs do WINDEV a Rust + React | 1 min 42 s |
| cenas do vídeo passo a passo | 18 |
| duração do vídeo passo a passo | 1 min 58 s |
| projetos de exemplo convertidos de ponta a ponta | 2 |
| golden master dos destinos | 5/5 (clientes-php-mysql) · 10/10 (estoque-wx) |
| testes do runtime WLanguage em Rust (wl-rt) | 16 |
| corpus do Help (bytes) | 26750976 |
| páginas válidas do corpus | 12035 |
| linhas do manual | 936 |
| tabelas do exemplo ESTOQUE | 7 |

## O que foi provado em sessão real

Cada print em `docs/prints/` é a saída de uma sessão do Claude Code ou de um script, sem edição; a origem de cada um está em `docs/prints/gerar.md`. Entre eles: o questionário uma letra por vez, a senha colada que não é gravada nem repetida, a letra H com o processo de conversão, a tela modelo aberta antes de registrar, o serial de ativação recusando e depois liberando, a primeira sessão lendo `INDEX_FILES.md` e o kickoff, a exportação organizada e o zelador, e o esqueleto de ERP (L6) com a sessão carregando a skill do módulo.

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
- Segurança (o que protege e o que não): `docs/SEGURANCA.md`.
- Emissor de serial, fora do plugin: `ferramentas/wx-serial/`.
- Dossiê: `docs/dossie/dossie-wx-claude-code.html`, gerado deste mesmo medidor.

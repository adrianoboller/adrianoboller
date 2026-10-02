# Cognição: `git checkout -- arquivo` numa árvore com frentes vivas apaga o trabalho alheio

**Data da descoberta:** 02/10/2026, ~05:10 (SP000020, frente B — config.json fase 2)
**Estado:** INFRUTÍFERO (falha observada, com causa e prevenção)

## 1. O que aconteceu

Um script de edição do `apps/phxclaw/src/main.rs` falhou no meio (um par de `replace` já
coberto por outro par) e, para «recomeçar limpo», rodei `git checkout apps/phxclaw/src/main.rs`.
O arquivo estava **modificado na árvore de trabalho por outras frentes** (W1/D, SP000013 C:
despacho `openai`/`anthropic`, `plugins catalogo|instalar|empacotar`, a assinatura nova de
`revisao::modelo`). O checkout voltou ao commit base `0e487530` e apagou essas linhas.

## 2. O que fiz

Reconstruí as linhas perdidas pelo que sobrou: a tabela de `ajuda.rs` (que lista os
comandos e cujo teste `todo_comando_do_despacho_esta_na_ajuda` exige despacho para cada um),
as APIs já existentes (`chaves::OPENAI`/`ANTHROPIC`, `loja::Loja`, `loja::empacotar`) e os
erros de compilação (`modelo` com dois argumentos). O resultado compila e os testes passam,
mas é **reconstrução**, não recuperação: o integrador confere contra a frente de origem.

## 3. O que concluí primeiro, e estava errado

«`git checkout` de UM arquivo só desfaz o que EU fiz nele.» Errado: desfaz tudo o que não
está commitado, de quem quer que seja. Numa árvore compartilhada por frentes paralelas, o
`M` no `git status` não é meu — e eu tinha lido esse `git status` no primeiro comando da
sessão (69 arquivos modificados) sem transformar isso em regra de conduta.

## 4. Causa e prevenção

- **Causa:** usar o git como «desfazer» em árvore que não é só minha.
- **Prevenção:** em árvore com frentes vivas, **nunca** `git checkout`/`git restore`/
  `git stash` num arquivo que aparece como `M` antes de eu tocá-lo. Para recomeçar uma
  edição: guardar uma cópia (`cp arquivo scratchpad/`) ANTES do primeiro `replace`, e
  reverter pela cópia. O script de edição por pares deve ser idempotente (assert
  `count >= 1`, não `in`), para não precisar de «recomeçar».

## 5. Número

1 arquivo, 3 blocos reconstruídos (2 braços de despacho, 3 subcomandos de `plugins`, 1
chamada ajustada); 0 testes perdidos (os de `apps/phxclaw` passam depois da reconstrução).

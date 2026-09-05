# Uma condição que nunca se satisfaz é lida como receita

**Descoberta:** 05/09/2026, 05:10. Frente CIFRA (papéis C, F, J), medindo as
três premissas da senha por tabela.

## 1. O que aconteceu

O `SEGURANCA.md` §11.8 traz esta frase, escrita com todo o cuidado e em
destaque:

> **Replicar uma tabela com coluna `Memo`/`Bin` marcada só funciona entre
> servidores que compartilham a senha da cifra E o sal do arquivo de origem.**

Li a frase como uma **condição operacional**: garanta as duas coisas e
funciona. Foi assim que o parecer ia sair — «replicação de coluna externa
marcada exige senha igual nos dois lados».

Medido (`cargo run --release --example senha-por-tabela -p phxsql-store`,
caso `mesma-senha`): **não funciona nem com a mesma senha nos dois lados**. A
recusa é *«a etiqueta nao confere — ou o dado foi alterado, ou a chave de
"cifra" nao e a que gravou este arquivo»*.

A causa está no `cofre::Material::novo()`, e a própria ficha dele a explica:
*«cada arquivo sorteia o proprio sal, e por isso tem a propria chave»*. **Dois
arquivos nunca compartilham sal.** O segundo termo da conjunção é impossível
por desenho — então a frase inteira quer dizer «não funciona», e não «funciona
se».

## 2. O que eu concluí primeiro, e estava errado

**«A limitação está documentada, logo está entendida.»** Ela estava
documentada, e a frase estava **tecnicamente correta**. O que estava errado era
a minha leitura — e a leitura errada é a que qualquer um faz, porque uma
conjunção com dois requisitos parece uma **lista de conferência**: dois itens,
marque os dois. Ninguém lê «E o sal do arquivo de origem» e vai conferir se o
sal é sorteável.

O agravante: eu ia **repetir a frase no parecer**, e o parecer é o documento
que o dono usa para decidir. A limitação teria atravessado uma medição inteira
sem ser medida, porque estava escrita bonito.

## 3. O que a medição disse

Três casos, dois servidores simulados por dois diretórios no mesmo processo —
que é o caminho exato do `sincronizar_replicada`, sem subir contêiner:

| origem | réplica | sal igual? | o que aconteceu |
|---|---|---|---|
| cifra ligada | **mesma senha** | **não** | recusou: «a etiqueta nao confere» |
| cifra ligada | senha diferente | não | recusou, mesmo texto |
| cifra ligada | **cifra desligada** | não | **gravou 63 bytes de texto cifrado como se fossem o anexo, sem erro nenhum** |

O terceiro é o pior, e só apareceu porque troquei a coluna de `Memo` para
`Bin`: a `Memo` para por **acidente**, no `String::from_utf8` da volta. A `Bin`
não tem essa peneira. Um erro que só acontece porque o texto cifrado quase
nunca é UTF-8 válido não é uma guarda — é sorte com boa reputação.

E o texto da recusa acusa a coisa errada: diz *«arquivo corrompido»* e manda
escolher entre «o dado foi alterado» e «a chave não é a que gravou», quando não
há corrupção nenhuma e a senha até pode estar certa.

## 4. A regra

**Condição composta em limitação escrita se confere termo a termo, e o termo
que ninguém consegue satisfazer transforma a frase inteira em «não funciona».**
Antes de repetir uma limitação, pergunte de cada termo dela: *quem, na prática,
consegue cumprir este?*

## 5. Como está guardado hoje

- A medição está em `SEGURANCA.md` §12.4, com os três casos e a coluna
  `sal igual`, que é a causa medida e não suposta.
- O gerador é `crates/phxsql-store/examples/senha-por-tabela.rs` — ele mede as
  três premissas e sai com `RESULTADO <json>`.
- A §11.8 continua onde estava, e agora a §12.4 diz em que ela induz ao erro.
  **Não a reescrevi**: a frase é verdadeira, e apagar o histórico de uma
  limitação é pior que ele estar incompleto.
- **O buraco fica, e fica nomeado**: os dois defeitos — a recusa que acusa
  corrupção, e a réplica sem cifra gravando texto cifrado em silêncio numa
  coluna `Bin` — estão no `PENDENCIAS.md`. Nenhum foi consertado nesta rodada:
  a ordem do dono era **medir**, e `reg.rs`/`table.rs` estavam com outra frente
  na mesma árvore.

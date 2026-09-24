# Cognição: um arquivo por aprendizado, com data e hora

Ordem do dono, 02/09/2026: *«Todo aprendizado novo seu deve virar um
`cognicao_assunto_data_hora.md`.»*

## Por que um arquivo por aprendizado, e não um diário

Porque um diário se lê inteiro ou não se lê. Um arquivo por aprendizado se
**procura**: quem vai mexer no gerador de PDF acha o da crase; quem vai trocar
a trava acha o do `RwLock`. E porque a data e a hora respondem a pergunta que
mais importa quando um aprendizado contradiz outro — **qual dos dois é o mais
novo**.

## O nome

```
cognicao_<assunto>_<AAAAMMDD>_<HHMM>.md
```

O `assunto` é o que se procuraria seis meses depois, em minúscula e com hífen:
`crase-no-template-literal`, e não `bug-js`. A hora é a da **descoberta**, e
não a do commit — é ela que diz o que já se sabia quando outra frente errou o
mesmo na mesma tarde.

## O que cada um carrega

Cinco seções, e a terceira é a que impede o documento de virar anedota:

1. **O que aconteceu** — o fato, com arquivo e número.
2. **O que eu concluí primeiro, e estava errado** — o diagnóstico plausível que
   veio antes do medido. Sem isto o documento ensina só a resposta, e o erro
   volta pelo mesmo caminho.
3. **O que a medição disse** — o número. *Número citado é número que não se
   mede.*
4. **A regra** — uma frase, na forma imperativa.
5. **Como está guardado hoje** — e, quando não está, **onde o buraco ficou**.
   Papel que não está cumprindo aparece como não cumprindo.

## O que NÃO vira cognição nova

Reafirmação de pétrea que já existe. Quando uma pétrea quebra de novo, o
aprendizado novo é o **alcance** dela — «a guarda existe, mas só cobre `ui/`»
—, e é isso que o arquivo registra. Uma terceira cópia da mesma lei não
acrescenta lei: acrescenta lugar onde a lei pode divergir de si mesma.

## E a diferença entre isto e o `CLAUDE.md`

O `CLAUDE.md` é a **lei**: curta, e o que ele diz vale sem discussão. A
cognição é o **processo**: como se descobriu, o que se errou antes, e o número.
Lei sem processo vira dogma que ninguém sabe defender; processo sem lei vira
história que ninguém aplica.

## O estado de cada cognição: PENDENTE, FRUTÍFERO ou INFRUTÍFERO

Pétrea do dono, 24/09/2026 (`CLAUDE.md`, «aprendizado PENDENTE não vira
FRUTÍFERO sem evidência»): todo aprendizado nasce **PENDENTE**, e só muda de
estado com prova escrita no próprio arquivo — nada promove sozinho, nem
script, nem agente, nem o integrador por conveniência.

A linha entra **logo abaixo do título**, antes da primeira seção (`##`):

```
# <título>

**Estado:** PENDENTE | FRUTÍFERO | INFRUTÍFERO
```

- **Sem a linha, o estado é PENDENTE.** É o padrão, e é onde ficam as 303 de
  hoje (24/09/2026): registraram o processo, e ninguém ainda comprovou se a
  regra pegou ou se a hipótese morreu — inclusive as que já soam certas na
  releitura. A pétrea proíbe promoção automática, e isso vale para quem edita
  este arquivo também.
- **FRUTÍFERO** exige, na mesma zona (entre o título e o primeiro `##`), a
  linha `**Evidência:**` com pelo menos **uma** referência **verificável**,
  entre crases, separadas por `;` quando houver mais de uma:
  - um teste que existe no repositório — `` `crates/xxx/src/arquivo.rs::nome_do_teste` ``
    (caminho do arquivo até o `::`) ou `` `crate::modulo::nome_do_teste` ``
    (o nome puro, achado do mesmo jeito que o `bancada/catracas/todas.py` acha
    `#[test]` para casar com `TETO_*`);
  - um commit que existe no git — `` `1a2b3c4` ``, conferido por
    `git cat-file -e <hash>^{commit}`;
  - um arquivo de medição que existe — `` `bancada/.../resultados.json` ``.

  **Prosa não é evidência.** «Funcionou», «ficou bom», «testei e passou» não
  contam — o extrator abaixo reprova quem só escreveu isso, e reprova também
  quem inventou um nome de teste ou um hash que não existe: a referência é
  **conferida**, não apenas citada.
- **INFRUTÍFERO** exige as linhas `**Causa:**` e `**Prevenção:**`, as duas
  não vazias:

  ```
  **Estado:** INFRUTÍFERO

  **Causa:** <por que a hipótese falhou, medido>

  **Prevenção:** <o que muda para o mesmo erro não voltar>
  ```

  Sem as duas, a falha observada não serve para nada — ninguém consegue
  evitar o que não sabe por que aconteceu.

Um campo pode ocupar mais de uma linha (continuação ou lista com `-`); ele
termina na primeira linha em branco, no próximo campo `**Nome:**` ou na
próxima seção `##`.

## O extrator: `avoid-e-reuse.py`

```
python3 docs/cognicao/avoid-e-reuse.py             gera AVOID.md e REUSE.md, mostra a contagem por estado
python3 docs/cognicao/avoid-e-reuse.py --catraca   confere (a) evidencia valida (b) causa+prevencao (c) paginas em dia
python3 docs/cognicao/avoid-e-reuse.py --autoteste prova real da propria regua, em memoria
python3 docs/cognicao/avoid-e-reuse.py --raiz DIR  usa DIR em vez desta pasta (prova em copia)
python3 docs/cognicao/avoid-e-reuse.py --git DIR   roda os comandos git em DIR (arvore sem .git)
PHXSQL_GIT_DIR=DIR python3 ... --catraca            o mesmo --git, por variavel de ambiente
```

**Numa árvore sem `.git`** (uma cópia de `git archive`, como o integrador e o
provador de guardas usam para conferir a árvore exata do commit) a régua
**não finge**: nenhuma evidência de commit é dada como válida por omissão.
Ela conta como **"não conferível aqui"** — reprova igual a uma evidência
inventada, porque a pétrea proíbe promover sem prova validada, e "não
consegui checar" não é validação. Teste e arquivo continuam conferidos do
jeito de sempre (não dependem de git, só da árvore, que está sempre lá).
`--git DIR` (ou `PHXSQL_GIT_DIR`, para quando não há como passar a flag —
é o caso do `bancada/catracas/todas.py`, que chama esta régua só com
`--catraca`) aponta para onde RODAR os comandos git; sem nenhum dos dois, cai
na raiz do próprio projeto, o de sempre quando há `.git` ali.

Ele varre todo `cognicao_*.md` da raiz, lê os quatro campos acima e **gera**
duas páginas — cada uma dizendo no topo que é gerada e não se edita:

- `AVOID.md` — os INFRUTÍFEROS: causa, prevenção e o link do arquivo.
- `REUSE.md` — os FRUTÍFEROS: a evidência e o link do arquivo.

A contagem por estado que aparece nas duas páginas sai do próprio extrator,
nunca é digitada à parte. O extrator **só lê** os `cognicao_*.md` — nunca
escreve neles; a única escrita dele é nas duas páginas geradas.

`--catraca` reprova quando: (a) um FRUTÍFERO não tem `**Evidência:**`, ou
nenhuma das referências dela existe de verdade; (b) um INFRUTÍFERO não tem
`**Causa:**` ou `**Prevenção:**`; (c) `AVOID.md` ou `REUSE.md` no disco
diferem do que o extrator geraria agora — lista digitada à mão envelhece
calada, e aqui a lista **é** a própria página. Os tetos nascem em 0. Ele
entra sozinho no `bancada/catracas/todas.py`, porque declara `--catraca` na
mesma linha de `sys.argv` — nenhuma lista precisa saber do nome dele.

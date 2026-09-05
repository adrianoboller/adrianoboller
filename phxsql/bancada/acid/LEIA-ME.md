# A bancada do ACID — as quatro letras, uma a uma

O `docs/ACID.md` é o documento; esta pasta é o que o prova. **Nenhum número
daquele documento é digitado**: `prova.py` mede e grava `resultado.json`,
`gerar-secoes.py` reescreve os blocos marcados do documento a partir dele.

```bash
cargo build --release
python3 bancada/acid/prova.py          # mede, grava resultado.json
python3 bancada/acid/gerar-secoes.py   # reescreve os blocos de docs/ACID.md
```

Sobe um `phxsqld` de verdade na porta **7570**, mata-o com `SIGKILL` dezenas de
vezes (nunca `pkill` — só os PIDs que o próprio script criou), anexa `strace` a
alguns deles para contar `fsync`, e leva de três a seis minutos. O portão
`bancada/esta-medindo.sh` é consultado e o estado da máquina vai para o
resultado, mas **não decide nada** aqui: ver «o que esta bancada não mede».

| arquivo | o que é |
|---|---|
| `prova.py` | a medição: 32 afirmações, cada uma com o controle da mesma corrida |
| `gerar-secoes.py` | escreve os números dentro do `docs/ACID.md`, entre `<!-- GERADO: … -->` e `<!-- FIM: … -->` |
| `resultado.json` | tudo que foi medido, cru — é dele que o documento se gera |

## A regra que esta bancada obedece: nenhuma afirmação sem controle

Um fenômeno de isolamento só se prova **acontecendo**. O que **não** acontece
não se prova sozinho — só pelo controle: o mesmo instrumento, no mesmo
servidor, na mesma corrida, mostrando o caso oposto. Esta casa já publicou um
zero com um medidor cego, e a lição virou o desenho daqui:

- «leitura suja não acontece» vale porque, na mesma corrida, o mesmo `ler`
  **acusa** a própria escrita não confirmada da transação (o
  *read-your-own-writes*). Um `ler` cego a uma linha que não está no disco
  seria cego a uma linha suja também.
- «a transação conserta a leitura de uma instrução» vale porque o **mesmo**
  `varrer`, na **mesma** tabela, viu o estado intermediário 98 vezes contra um
  escritor sem transação — a única diferença entre as duas colunas é a
  transação.
- «o `.reg` não vai ao disco em `por_lote`» vale porque, na mesma medição,
  `por_operacao` mostra o `.reg` indo.
- toda guarda da letra **C** entra em par: a violação recusada **e** o caso
  legítimo aceito, na mesma tabela. Guarda que recusa tudo protegeria o mesmo
  número e não serviria para nada.

## Quatro armadilhas que este script pagou, e as quatro valem mais que o resultado

**1. A corrida vazia.** A primeira versão da matriz de leitura consistente
rodava o leitor 400 voltas em 225 ms enquanto o escritor levava 296 ms só para
conectar e logar: ele **não rodava uma única volta**, e os dois instrumentos
devolviam zero. Zero de uma corrida em que o outro lado não existiu não é «não
acontece» — é nada. Hoje o leitor **espera** o escritor entrar no laço, só para
quando ele já deu 40 voltas, e o número de voltas dele vai para o resultado,
para que uma corrida vazia apareça como vazia.

**2. A cascata que não cascateava.** O teste da árvore `avó ← mãe ← neta`
montava a mãe com `id` próprio e uma coluna `pai` separada. A cascata mudava
`mae.pai`; a neta apontava para `mae.id`, que não mudava; nada era exercitado e
o teste passava. A coluna que a mãe aponta na avó tem de ser a **mesma** que a
neta aponta na mãe.

**3. O `excluir` que nasce suave.** O par «de vez **e** suave» da regra
primordial mandava `{"modo": "suave"}` — um campo que o servidor não lê. Os
dois casos exercitavam o mesmo caminho, e a afirmação sobre duas coisas era
sobre uma só. O modo físico se pede com `"fisico": true`.

**4. A versão que não vem na linha crua.** O `ler` só devolve `versao` quando
se pede `"com_versao": true` — há teste no servidor cobrando que ela **não**
vaze na linha comum. Lendo sem pedir, o campo saía nulo, a gravação otimista ia
sem versão nenhuma e era aceita: o controle dizia `ACEITOU` e a página quase
publicou «o controle otimista não protege».

As quatro têm a mesma forma: **o instrumento não estava medindo o que o nome
dele dizia**, e nas quatro isso apareceu como um resultado *bonito* — zero,
passou, aceitou. É por isso que cada afirmação carrega o controle ao lado.

## O que esta bancada NÃO mede, de propósito

**Tempo.** Nenhum número daqui é uma duração: são contagens, vereditos e
`fsync` contados por `strace`, todos determinísticos e imunes a máquina
ocupada. As calibrações que o `prova.py` imprime (para decidir a faixa de
atrasos do `SIGKILL`) são meio, não resultado, e não entram no documento.

**Queda de energia.** O `SIGKILL` mata o processo e o núcleo fica com as
páginas sujas — um `write` já entregue sobrevive à morte de quem o escreveu. A
seção D existe justamente para dizer isso com todas as letras: duas inserções
comuns em `por_lote`, com **zero** `fsync` no `.reg`, voltam inteiras depois do
`SIGKILL`, e isso **não é durabilidade**. Quem mede durabilidade aqui é a
contagem de `fsync`; o `SIGKILL` só prova que a marca `.tx` decide o desfecho.

**A réplica.** «A réplica aplica, ela não julga» (pedido 171) é afirmação sobre
um segundo servidor, e esta bancada sobe um só. Ela está no documento com a
fonte da prova nomeada — `docs/INTEGRIDADE.md` §3 e as guardas do pedido 171 —
e **não** com uma medição desta pasta. Papel que não está cumprindo aparece
como não cumprindo.

## Reuso, e não terceira cópia

`subir`, `matar_de_verdade`, `Ligacao`, `ler_relatorio`, as varreduras de
`SIGKILL` e a leitura do `strace` já estavam escritos em
`bancada/durabilidade/`. Este script os **importa** e troca a porta e o
diretório. Uma armadilha do reuso ficou registrada no código: `dur.Ligacao`
fixa a porta no *default* do `__init__`, que o Python avalia na definição da
classe — trocar `dur.PORTA` depois não alcança aquele valor, e o script falaria
com a porta da outra bancada.

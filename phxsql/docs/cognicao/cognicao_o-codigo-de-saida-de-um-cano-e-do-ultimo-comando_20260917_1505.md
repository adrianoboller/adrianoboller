# O código de saída de um cano é do ÚLTIMO comando — e o portão anunciou êxito reprovando

**Descoberto em 17/09/2026, 15:05 UTC**, ao rodar a suíte antes de comitar os
arquivos da bancada do adiamento do `.ndx`.

## 1. O que aconteceu

Rodei o portão da casa assim:

```bash
flock /tmp/phx-cargo.lock cargo test --workspace 2>&1 | tail -40
```

A tarefa devolveu **`[exited with code 0]`**. Dentro do arquivo de saída, quatro
linhas acima:

```
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
error: test failed, to rerun pass `-p phxsql-server --test continuidade-da-replica`
```

**O portão reprovou e anunciou êxito.** Se eu tivesse lido só o código de saída
— que é exatamente o que um portão existe para me poupar de não ler —, eu teria
comitado código com a suíte vermelha.

## 2. O que eu concluí primeiro, e estava errado

Quatro horas antes, nesta mesma sessão, o mesmo cano já tinha me pegado: rodei
o `portao-dos-geradores.py` por `| head -30`, o `SIGPIPE` matou o portão no
terceiro de vinte e dois, e a tarefa reportou `exited with code 0`. Concluí,
disse em voz alta, e **não escrevi**: *«nunca rodar o portão por cano»*.

A lição estava certa e **pequena demais de duas maneiras**:

1. **Ela nomeava o sintoma, não a causa.** `head` fecha o cano e mata o
   produtor com `SIGPIPE`; `tail` **lê tudo até o fim** e não mata ninguém. Eu
   tinha catalogado «`head` mata o portão», então `tail` me pareceu seguro. O
   problema nunca foi o `SIGPIPE`: é que `$?` de um cano é o código do
   **último** comando, e `head` e `tail` saem bem dos dois jeitos.
2. **Ela ficou só na conversa.** Não virou arquivo, não virou guarda, não virou
   nada que sobrevivesse a uma compactação de contexto. E voltou pela outra
   porta da mesma casa, na mesma tarde.

## 3. O que a medição disse

O mesmo comando, sem o cano, guardando a saída em arquivo:

```bash
flock /tmp/phx-cargo.lock cargo test -p phxsql-server \
  --test continuidade-da-replica > saida.txt 2>&1; echo "CODIGO REAL: $?"
```

| forma | o que aconteceu | o que foi reportado |
|---|---|---|
| `cargo test … \| tail -40` | 1 passed, **1 failed** | **`exited with code 0`** |
| `cargo test … > arquivo; echo $?` | 2 passed, 0 failed | `CODIGO REAL: 0` |

E o efeito colateral do cano é o segundo estrago: `tail -40` guardou **42
linhas**, então o arquivo de saída ficou sem os `test result: ok` de todos os
outros pacotes. Um `grep -c '^test result: ok'` nele devolve **0** — e um número
que vem de uma saída truncada mente com a mesma cara de um número certo.

A falha em si era conhecida e está registrada (pedido 321), e a corrida rendeu
um dado novo para ela: quem treme não é a função de teste, é o ajudante
`esperar_eventos`, e toda função que o chamar está exposta.

## 4. A regra

**Portão não se lê por cano.** Escreva a saída num arquivo e leia `$?` do
comando, não do cano — e depois filtre o arquivo à vontade. Quando o cano for
inevitável, `set -o pipefail` ou `${PIPESTATUS[0]}`, nunca o `$?` nu.

E a regra que a repetição ensinou, que é a maior das duas: **lição dita e não
escrita volta na mesma sessão.** Esta voltou em quatro horas, pela porta irmã.

## 5. Como está guardado hoje

**Agora está escrito, e antes não estava — que é a metade do aprendizado.**

- Este arquivo é a primeira vez que a lição sai da conversa. A versão de
  11:0x ficou só no que eu disse, e por isso não me alcançou às 15:05.
- **Não há guarda.** Nada impede a próxima chamada de portão por cano, e um
  conferidor que procurasse `| head` ou `| tail` no que eu digito não existe —
  não há onde pendurá-lo, porque o comando é digitado numa sessão e não mora
  em arquivo versionado. O que existe é este arquivo e o hábito.
- **O que caberia como guarda de verdade** é do outro lado: o `comunicacao.sh`
  e o `portao-dos-geradores.py` já sabem dizer quando fizeram menos do que o
  nome promete. Um portão que **grava o próprio veredito em arquivo** — e não
  só o imprime — tira o código de saída do caminho crítico da leitura. Fica
  nomeado, não feito.

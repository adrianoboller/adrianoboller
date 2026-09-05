# Lista curta na cópia acusa a frente errada

*Descoberto em 05/09/2026, 04:55, ao provar uma guarda nova.*

## 1. O que aconteceu

Acrescentei uma guarda ao `bancada/guardas/catalogo.py` e rodei a bateria
inteira para regerar a tabela do `docs/TESTES.md`. Ela parou na **linha de
base**, antes de repor defeito nenhum:

```
test conferidor_botoes::testes::nenhum_botao_novo_sem_prova ... FAILED
test result: FAILED. 697 passed; 1 failed
error: 1 target failed: `-p phxsql-server --lib`
```

O teste é de outra frente, escrito naquela tarde. E ele **passa** na árvore de
trabalho: `cargo test -p phxsql-server --lib nenhum_botao_novo_sem_prova` sai
verde no mesmo minuto.

A causa não está no teste. O executor das guardas trabalha numa **cópia** da
árvore, e a lista do que copiar é digitada:

```python
COPIAR = ["Cargo.toml", "Cargo.lock", "crates", "exemplos", "docs"]
```

`conferidor_botoes.rs` lê `testes-web/botoes-exercitados.txt` em **tempo de
execução** (`std::fs::read_to_string`, não `include_str!`). Na cópia esse
arquivo não existe, a evidência some, todo botão vira «sem prova» e a catraca
reprova.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a outra frente tinha deixado a catraca vermelha e que a minha
instrução era «relate, não conserte» — e ia relatar exatamente isso: *o teste
de botões está falhando, é dela, está em voo*.

Estava errado, e o erro é do tipo que não se descobre lendo a saída: a saída
dizia com todas as letras qual teste caiu e em qual pacote. O que ela não dizia
é **em qual árvore**. Rodar o mesmo teste na árvore de trabalho — trinta
segundos — foi o que separou «o teste está quebrado» de «a cópia está
incompleta».

Se eu tivesse relatado o diagnóstico plausível, a outra frente teria ido
procurar defeito no código dela, que estava certo; e a bateria das guardas
continuaria parada, sem ninguém saber por quê.

## 3. O que a medição disse

| | resultado |
|---|---|
| `cargo test -p phxsql-server --lib nenhum_botao_novo_sem_prova`, árvore de trabalho | **verde** |
| o mesmo teste, na cópia `~/.cache/phx-guardas` | **FAILED** |
| tamanho de `testes-web/` | 368 KB |
| bateria completa, antes do conserto | parada na linha de base, 0 guardas provadas |
| bateria completa, depois | **85 guardas: 81 provadas, 4 redundantes**, 762 s |

Conserto: `testes-web` entrou no `COPIAR`, com o comentário dizendo qual
arquivo e por quê — ao lado do comentário que já contava o mesmo caso de
`docs/ROTEIRO-1.0.md`, que é a **primeira** vez que essa lista ficou curta pelo
mesmo motivo.

## 4. A regra

**Lista de cópia curta não falha como cópia curta: falha como defeito de quem
escreveu o teste.** É um sintoma que aponta para o lugar errado com nome,
arquivo e linha — a forma mais convincente de errar.

O corolário operacional, que custa trinta segundos: **antes de relatar um teste
alheio como quebrado, rode-o onde ele foi escrito.** Se ele passa lá e falha
aqui, o defeito é do «aqui».

E o alcance da lei que já existia — *quando um gerador depende de uma lista, a
lista tem de sair do código*: aqui o «gerador» é a cópia da árvore, e a lista
que ele precisa é a dos caminhos lidos em tempo de execução. Enquanto ela for
digitada, o próximo `read_to_string` de fora de `crates/` quebra isto de novo,
e quebra parecendo culpa de outra pessoa.

## 5. Como está guardado hoje

- `bancada/guardas/provar-guardas.py`, na `COPIAR`, com o comentário que nomeia
  o arquivo, a data e o sintoma.
- `docs/TESTES.md`, com a tabela das 85 guardas regerada pela rodada — ela
  estava com 84 e não podia ser refeita enquanto a bateria não rodasse.

**Onde o buraco ficou:** a lista continua digitada. O que a tiraria do código é
uma varredura dos `read_to_string`/`File::open` com caminho literal nos fontes,
comparada com o `COPIAR` — não escrita, e por isso registrada aqui em vez de
prometida. Enquanto ela não existir, o sinal de que a lista ficou curta é este:
um teste que passa na árvore de trabalho e falha na cópia, na linha de base.

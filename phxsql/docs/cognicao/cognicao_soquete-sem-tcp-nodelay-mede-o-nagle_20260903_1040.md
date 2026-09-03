# Um soquete de prova sem `TCP_NODELAY` mede o Nagle, não o servidor

**Descoberto:** 03/09/2026, 10:40.
**Onde:** `bancada/durabilidade/prova.py` (a classe `Ligacao`), por
comparação com `bancada/profiler/comum.py` e `bancada/transacoes/provar.py`.

## 1. O que aconteceu

Escrevendo a matriz da SP000010, precisava empilhar 3.000 `inserir` dentro de
uma transação antes de medir o `COMMIT`. Uma calibração de 60 linhas (30+30)
sem matar nada acusou **1.537 ms** — quase 26 ms por linha, um número
absurdo para uma operação que só empilha em RAM (`empilhar()` não toca
disco).

## 2. O que eu concluí primeiro, e estava errado

A primeira leitura foi «o servidor está lento demais para esta prova» — e eu
cheguei a cogitar reduzir drasticamente o tamanho das transações da matriz
inteira, o que teria estreitado a janela de todos os cinco pontos de morte e
enfraquecido a prova inteira por um motivo que não existia no motor.

## 3. O que a medição disse

O soquete da classe `Ligacao` não ligava `TCP_NODELAY`. Sem ele, o algoritmo
de Nagle do lado do cliente atrasa cada `write()` pequeno esperando um ACK
que pode vir com atraso do outro lado (`delayed ACK`) — a combinação clássica
que produz atrasos de dezenas de milissegundos por ida-e-volta em protocolos
de pergunta-resposta curtos, e é exatamente o formato de `phxsqld`: uma
linha JSON, uma resposta, a próxima linha.

Ligando `socket.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)`, a
mesma calibração caiu para **2,2 ms** — **700× mais rápida**, e agora
condizente com os números de `docs/TRANSACOES.md` §8 (marca ~0,29 ms + linha
~0,05 ms).

## 4. A regra

**Todo soquete de prova pergunta-resposta liga `TCP_NODELAY` antes de medir
qualquer coisa.** Sem isso, o número que sai do script mede o Nagle do
sistema operacional, não o servidor — e é fácil o número absurdo parecer
plausível o bastante para mudar o desenho da prova em vez de mudar o
soquete.

## 5. Como está guardado hoje

* `bancada/durabilidade/prova.py`, classe `Ligacao`, liga `TCP_NODELAY` no
  construtor, com o comentário explicando o porquê.
* **Onde o buraco continua**: `bancada/profiler/comum.py` (`Conexao`) e a
  `Ligacao` de `bancada/transacoes/provar.py` **não ligam** `TCP_NODELAY`.
  Não mudei os dois — são território de outras frentes/scripts já em uso, e
  mudar o comportamento de um soquete compartilhado sem pedido é o mesmo
  risco que a pétrea da "guarda nova entra pedida, não imposta" descreve para
  código de produto. Fica registrado aqui para quem for medir tempo fino com
  eles: o atraso pode ser do soquete, não do servidor.

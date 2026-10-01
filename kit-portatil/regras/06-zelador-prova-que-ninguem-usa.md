# O zelador prova que ninguém usa antes de apagar

**Regra.** Nada se apaga sem antes provar, **por caminho real**, que nenhum
processo vivo usa aquilo: o `cwd` de cada processo, os descritores abertos e
os mapas de memória — nunca por data, nome ou palpite. E o zelador **não mata
processo**: o processo pode ser de outro agente.

Três refinamentos, cada um pago:

- **O observador não conta.** O próprio zelador (e quem o chamou) tem o `cwd`
  na árvore; sem excluí-lo **por linhagem** (ancestrais pelo `ppid`), ele
  respondia «alguém trabalha aqui» sempre, com a máquina parada.
- **`cwd` diz quem PODE usar; a trava diz quem ESTÁ usando.** Cache de
  compilador se apaga segurando a trava do próprio compilador durante o `rm`;
  trava que não existe não se cria para conferir (criaria e responderia
  «livre» sobre um caminho que ninguém usa).
- **Quando** também importa: varrer gigabytes durante uma medição reprova a
  medição. Recusar com o motivo, e deixar uma saída explícita para o disco
  acabando de verdade.

**Cicatriz.** A primeira corrida achou 80.088 diretórios de teste soltos,
6,4 GB, num disco que tinha chegado a 560 MB livres sem ninguém saber por quê.
E matar o servidor de um agente vizinho já derrubou a própria sessão.

**Como aplicar.** `scripts/zelador.sh` deste kit, com `--ver` antes de
qualquer corrida de verdade.

## E prova que nada ali está fora de commit (01/10/2026)

«Nenhum processo usa» não basta para apagar a cópia de trabalho de uma frente.
A conferência que dizia «integrada» por `git merge-base --is-ancestor ramo HEAD`
acertou por acaso por doze limpezas e, na décima terceira, apagou quatro
frentes prontas: frente não comita, então o ramo dela é sempre a base, e a base
é sempre ancestral do HEAD. Antes de apagar, exija `git -C <copia> status
--porcelain` vazio **e** a ponta alcançável do HEAD — e nunca `-f -f` numa cópia
que a conferência não provou limpa.

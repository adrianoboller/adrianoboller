# Cognição — a recuperação reusada com o servidor de pé herda a política do arranque

**Descoberto em** 24/09/2026, 00:12 · papel B (pedido 426)

## 1. O que aconteceu

O pedido 426 tinha três camadas, e a (c) era a mais fácil de consertar: o
braço de erro do `op_commit` chamava `travar_dados()` com a trava do topo
ainda viva, a trava não é reentrante, o `if let Ok(...)` engolia a recusa, e
`transacao::recuperar` **nunca rodava**. O conserto óbvio — soltar a trava
antes, ou chamar `recuperar` com a que já está na mão — faz a recuperação
**rodar**.

Lendo o `recuperar` antes de ligá-lo: ele apaga a marca `.tx` depois de
`completar`, **mesmo quando alguma operação saiu em `impossiveis`**. No
arranque isso é certo — nada está congelado nem reservado (os dois registros
são do processo), e o impossível de lá é permanente. Com o servidor de pé, o
impossível pode ser **passageiro**: a tabela congelada por uma reescrita em
FASE A, que volta a atender quando ela acabar.

## 2. O que eu concluí primeiro, e estava errado

Que a (c) se consertava tirando a segunda tomada da trava — «o código é o
mesmo da recuperação do arranque, então o comportamento também é». O código é
o mesmo; o **contexto** não. Ligada como estava, a recuperação rodaria com a
tabela ainda congelada, contaria a operação como impossível e apagaria a marca
de uma transação **já confirmada** — trocando «completa no próximo arranque»
(o defeito de antes) por **«perdida para sempre»**.

E uma segunda leitura errada, achada pela suíte: o
`p0_filha_antes_do_pai_no_commit_ainda_recusa` passava no HEAD e parecia provar
que a filha antes do pai é recusada. Ele conferia só o veredito. A marca
ficava no disco, e o arranque seguinte aplicava o pai sem a filha — a
transação **recusada** aparecia pela metade depois de reiniciar.

## 3. O que a medição disse

| medida | número |
|---|---|
| marcas depois do COMMIT pendente, com a política do arranque ligada no braço de erro (guarda `completar-apaga-a-marca-impossivel`) | **0** (esperado 1) — a transação confirmada sem bilhete |
| a mesma, com a política separada (`NoArranque::Nao`) | **1**, e o arranque completa: `(clientes, pedidos) = (1, 1)` |
| `a_filha_antes_do_pai_nao_deixa_marca_para_o_arranque` contra o braço de erro de HEAD | `(marcas, depois do arranque) = (1, (1, 0))` — pai sem filha |
| o mesmo com o conserto | `(0, (0, 0))` |

## 4. A regra

**Função reusada num contexto novo herda a política do contexto velho — confira
a política, não só o código.** Irmão é quem chama as mesmas funções na mesma
ordem; quando o irmão novo roda em outra hora (servidor de pé × arranque), a
decisão embutida na função pode deixar de valer, e compila igual.

## 5. Como está guardado hoje

- `transacao::tratar_marca` é o corpo único, e a diferença de política é um
  parâmetro com nome (`NoArranque::Sim` / `Nao`), comentado com o porquê.
- Guarda `completar-apaga-a-marca-impossivel` no catálogo, com o teste
  `a_quebra_que_a_recuperacao_nao_vence_fica_na_marca_ate_o_arranque`.
- A prova de recusa que só conferia o veredito ganhou a irmã que mede o disco
  e o arranque (`a_filha_antes_do_pai_nao_deixa_marca_para_o_arranque`).
- **Buraco que fica:** a chave estrangeira só é conferida na passada, depois da
  marca; filha sem mãe no MEIO da lista para com parte gravada. A resposta diz
  o que ficou e o arranque não muda o passado, mas a atomicidade ali só volta
  com a conferência da chave antes da marca — outro pedido.

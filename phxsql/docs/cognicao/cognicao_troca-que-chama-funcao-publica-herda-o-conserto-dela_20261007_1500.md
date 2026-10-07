# A troca que repõe o defeito CHAMANDO uma função pública herda o conserto que a função ganhou depois

**Estado:** PENDENTE

**Descoberto em 07/10/2026, 15:00**, papel F, conferindo duas guardas que o
provador deu como NÃO PEGOU. Alcance das cognições
`cognicao_troca-que-aplica-e-nao-danifica-a-protecao-mudou-de-lugar_20260924_0440.md`
e `cognicao_guarda-que-mede-o-veredito-que-outro-conserto-passou-a-garantir_20261006_2100.md`.

## 1. O que aconteceu

- `cascata-solta-sem-marca` (pedido 540): a troca repunha «a cascata solta sem
  marca» escrevendo `t.atualizar(rowid, linha)` no `alterar_solto` do
  servidor. Desde o pedido 563 o `Table::atualizar` grava ele mesmo a marca
  `.tx` da cascata. A troca continuava aplicando, mas já não repunha «sem
  marca»: repunha «a marca do store em vez da do servidor». 3 de 4 caíam.
- `ndx-novo-sobe-com-o-diretorio-vazio` (pedido 533, P1): a troca estava certa
  e o defeito continuava gravando o cabeçalho durável com **0** índices; o 575
  fez a abertura pular a conta de índices de um `.ndx` que pede reconstrução,
  e o veredito «abre e manda reconstruir» passou a sair com ou sem o defeito.
  0 de 1 caía. Este é o caso da cognição de 06/10, sem alcance novo.

## 2. O que eu concluí primeiro, e estava errado

Para a cascata, que o teste do `SIGKILL` passava por engano (mediria um
veredito que outro conserto garante). Errado pela metade: o teste mede a
causa certa (`[6, 5]`, e a marca no disco na hora da queda); quem mentia era a
troca, porque a função que ela chama mudou de comportamento. Mexer no teste
teria escondido isso.

## 3. O que a medição disse

- Troca velha, `SIGKILL`: no disco, na hora da queda, `transacao_*.tx` com a
  mãe e as duas filhas — a marca do **store**. O arranque a completou: verde.
- Troca nova (`t.atualizar_com_maes(.., &mut SemMaes)`, cascata em linha sem
  marca nenhuma): **4 de 4** caem, o `SIGKILL` com `[6, 5]`, o número que o
  comentário do teste já previa. Os 2 de `seguem` seguem.
- Troca velha guardada como entrada irmã (`cascata-solta-pela-marca-do-embutido`):
  **3 de 3** caem (os pânicos com o processo de pé — o reparo da trava não
  completa a marca do store), e o `SIGKILL` vai para `seguem`, medido verde.
- `.ndx`: com a troca, offset 16 do cabeçalho durável = **0**; sem ela, **2**.
  Asserção nova no teste: RED 1 de 1, GREEN 11 de 11 do `disco-que-recusa`.

## 4. A regra

**Troca que repõe o defeito chamando uma função pública tem de chamar a que
NÃO tem a proteção — e quando a função ganha a proteção, a troca envelhece sem
o trecho sumir.** Ao repor, pergunte se a chamada da troca ainda faz o que o
título da guarda diz que ela faz.

## 5. Como está guardado hoje

As duas entradas consertadas e a irmã nova no `bancada/guardas/catalogo.py`,
com o motivo no comentário. Buraco que fica: nenhuma régua estática vê isto —
a `trecho-vivo.py` confere o texto do trecho, não o comportamento da função
que a troca chama. Só o provador rodado pega.

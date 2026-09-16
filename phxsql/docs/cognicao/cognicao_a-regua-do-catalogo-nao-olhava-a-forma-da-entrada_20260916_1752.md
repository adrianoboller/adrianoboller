# A régua do catálogo conferia o CONTEÚDO e nunca a FORMA — e deu `ok` nos cinco

**Descoberto em 16/09/2026, 17:52.** Papel A (integrador), fechando a frente de
IO do pedido 259.

## 1. O que aconteceu

Duas guardas novas entraram em `bancada/guardas/catalogo.py` — as duas provas
reais que a frente de IO tinha medido e deixado prontas para a QA colar. Eu as
escrevi **do esquema errado**: `nome`, `defeito`, `pedido`, `petrea`, quando o
catálogo usa `id`, `titulo`, `porque`. Deduzi o esquema de um `print` truncado
de uma entrada existente em vez de ler as chaves de uma delas.

A `bancada/guardas/trecho-vivo.py` — a régua barata que existe **justamente
para o catálogo não envelhecer** — rodou e disse:

```
   145 guardas no catalogo
   ok  TETO_TRECHO_MORTO: 0 (teto 0)
   ok  TETO_TRECHO_AMBIGUO: 0 (teto 0)
   ok  TETO_TESTE_MORTO: 0 (teto 0)
   ok  TETO_TESTE_FORA_DO_BINARIO: 0 (teto 0)
   ok  PISO_DAS_ENTRADAS: 145 (piso 145)
```

Cinco `ok`. Quem acusou foi o **provador**, depois de lançado, com um
`KeyError: 'id'` e um traceback de cinco quadros — ou seja, o erro apareceu no
lugar mais caro possível, depois de a corrida já estar de pé.

## 2. O que eu concluí primeiro, e estava errado

Concluí, ao ver os cinco `ok`, que as entradas estavam **prontas** e que só
faltava o provador confirmar a queda dos testes. A régua tinha acabado de
contar 145, achar os dois trechos vivos e os quatro testes vivos: parecia a
confirmação de que a forma estava certa.

Estava errado, e o motivo é preciso: a régua confere o **conteúdo** das chaves
que ela conhece — `arquivo` existe? o `trecho` aparece uma vez só? os testes de
`caem` existem no binário? Eu tinha acertado justamente essas quatro, porque
são as que a frente de IO me entregou escritas. As que eu inventei —
`nome`/`defeito`/`pedido`/`petrea` — a régua **nunca olhou**, porque olhar a
forma nunca esteve no trabalho dela.

Segundo erro no mesmo caminho, e ele mostra que o primeiro não foi azar: no
`seguem` da primeira entrada citei `o_diretorio_com_nome_de_tabela_nao_entra`,
um teste que **não existe** — o caso do diretório é uma asserção *dentro* do
mesmo teste. Esse, sim, a régua teria pego (`TETO_TESTE_MORTO`), e foi por isso
que o conferi à mão antes de rodar. Confiei na régua para uma metade e
desconfiei dela na outra, sem saber que era a metade errada.

## 3. O que a medição disse

- **5 `ok` em 5** com a entrada malformada, código de saída **0**.
- **14 chaves** aparecem nas 143 entradas antigas, contadas no próprio
  catálogo: 6 em todas as 143 (`id`, `titulo`, `porque`, `pacote`, `alvo`,
  `caem`), e 8 opcionais — `seguem` 141, `arquivo`/`trecho`/`troca` 139,
  `prazo` 19, `espera` 6, `nota_da_redundancia` 4, `trocas` 4.
- As 4 entradas sem `arquivo`/`trecho`/`troca` são exatamente as 4 que usam
  `trocas`: são **duas formas**, e não uma forma com buraco. Um conferidor que
  exigisse o par comum de todas reprovaria quatro entradas legítimas.
- Com o conferidor novo e o defeito reposto (`id` → `nome`): **para, nomeia a
  guarda, nomeia a chave que falta e a que ninguém lê, e sai com código 1**.
  Restaurado, os cinco `ok` voltam e o código é 0.
- O provador, depois: **2 provadas, 1/1 e 3/3 caíram**, árvore limpa verde.

## 4. A regra

**Régua que confere o conteúdo de uma entrada tem de conferir a FORMA dela
antes — e a lista de chaves sai do próprio catálogo, nunca da lembrança.**

## 5. Como está guardado hoje

`conferir_o_formato` em `bancada/guardas/trecho-vivo.py`, chamada de dentro de
`catalogo()` — ou seja, **todo** consumidor da função ganha a conferência, não
só o `--catraca`. Ela é **parada com o motivo, não teto**: teto conta quantos, e
aqui não há «quantos» aceitável. É o mesmo critério do `dossie_da_pasta.py`,
que para quando acha zero ou dois dossiês em vez de chutar qual atualizar.

**Onde o buraco ficou:** as duas listas de chaves (`CHAVES_OBRIGATORIAS` e
`CHAVES_OPCIONAIS`) estão **digitadas no conferidor**, e isso é exatamente a
armadilha da «receita de um número que também envelhece» — a lista do KiB de
interface era assim, e publicou 780 quando eram 1.032. A diferença, e é ela que
me fez parar aqui: essa lista **não pode** sair do catálogo por contagem, porque
o catálogo é justamente o que ela julga — uma chave nova malformada apareceria
na contagem e se autorizaria sozinha. Ela tem de ser uma decisão escrita. O
custo real é outro e está nomeado: quando o catálogo ganhar uma chave nova de
propósito, **ela tem de entrar nesta lista no mesmo commit**, senão o conferidor
reprova uma entrada legítima. É a mesma disciplina de baixar a catraca no mesmo
commit em que se traduz.

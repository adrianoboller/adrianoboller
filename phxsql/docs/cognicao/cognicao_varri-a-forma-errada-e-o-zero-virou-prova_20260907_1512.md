# A varredura procurou a forma errada, deu zero, e o zero passou por prova

**07/09/2026, 15:12** — pedido 150, a conversão dos temporários da bateria.

## 1. O que aconteceu

A conversão do pedido 150 troca um ajudante de teste que devolvia `PathBuf`
por um guarda `DirTemp`, que apaga o diretório no `Drop`. Antes de converter
eu nomeei a armadilha certa, e por escrito: *«`DirTemp` apaga no `Drop`, então
um sítio que faça `dir("x").join("y")` perde o diretório na hora. Não dá para
converter às cegas — vou conferir cada sítio.»*

Conferi. Rodei a varredura, ela devolveu **zero**, e eu segui.

A primeira corrida convertida deu **53 testes vermelhos**, todos com a mesma
mensagem: `database b nao existe em /tmp/phxsrv-ut-…`. A causa era exatamente
a armadilha que eu tinha nomeado.

## 2. O que eu concluí primeiro, e estava errado

Concluí que **não havia sítios com temporário emprestado** — e a frase que
escrevi na hora foi «nenhum temporário não ligado a uma variável».

O erro não estava na conclusão: estava na **régua**. A varredura procurava

```
helper("x").metodo(...)      # o temporário como receptor de um método
```

e este código escreve a outra forma, que é a comum em Rust:

```
servidor(&dir_temp("x"))     # o temporário como ARGUMENTO emprestado
```

As duas têm o mesmo tempo de vida — o `DirTemp` morre no fim da instrução —,
e a segunda é a que aparece 38 vezes no `servidor.rs`. Procurei uma delas e
declarei as duas ausentes.

E o agravante que torna isto pior que um descuido comum: **o compilador não
acusa nenhuma das duas.** O empréstimo é válido, o tipo bate, o `Drop` roda
depois do uso. O código compila limpo e apaga o diretório antes do primeiro
pedido. Só o teste vermelho acusa.

## 3. O que a medição disse

| | |
|---|---:|
| sítios que a minha varredura achou | **0** |
| sítios que existiam, forma `f(&helper(…))` | **38** |
| embrulhos que largavam o guarda dentro deles | **2** |
| testes vermelhos na primeira corrida convertida | **53** |
| testes vermelhos depois de ligar os 38 + 2 | **2** (a variante `&format!`) |
| testes vermelhos depois dessa | **0** |

O fecho do pedido, medido com retrato do `/tmp` antes e depois: **265 → 0**
diretórios por corrida nos três crates de servidor, **21 → 0** no `store`,
com `cargo test --workspace` em **1.669 testes, zero falhas**.

## 4. A regra

**Varredura que devolve zero prova a régua antes de provar o código: escreva
o caso que ela DEVE achar e veja-a achá-lo.** Zero é o único resultado que
tem duas leituras — «não existe» e «não procurei direito» —, e a segunda é
indistinguível da primeira até alguém pagar.

É a mesma lei que esta casa já paga em outro lugar por outro nome: *todo
medidor precisa de um controle que prove o instrumento antes de publicar um
veredito sobre o produto*. O conferidor de grades aprendeu contando só
`<table>` cru e subcontando dezoito telas; o de idiomas aprendeu medindo cinco
sextos da tela e anunciando o número inteiro. Aqui a régua não subcontou:
contou **zero**, que é a forma da subcontagem que menos parece uma.

## 5. Como está guardado hoje

Guardado, e nos dois sentidos:

- O conserto está nos **53 sítios convertidos** e nos guardas
  (`apoio_teste::DirTemp`, `tests/comum/mod.rs`, e a cópia curta dentro do
  `phxsql-cli`, que é binário).
- A régua está no `conferidor_temporarios.rs`, com a catraca
  `TETO_TEMP_DIR_SOLTO = 0`, e **ela mesma tem o controle que faltou à minha
  varredura**: o teste `o_conferidor_acusa_quando_o_defeito_volta` põe o
  padrão velho na frente do casador e exige que ele reconheça, e o
  contra-exemplo (`DirTemp::novo(…)`) exige que ele **não** reconheça. Sem
  esse par, um casador que passasse a não achar nada continuaria imprimindo
  «nenhuma solta» — que é o zero deste documento outra vez.
- **Onde o buraco ficou:** o conferidor varre `crates/*/src` e
  `crates/*/tests`, e **não** varre `examples/`. São 48 chamadas em 42
  arquivos, contadas e não medidas em disco. Está no pedido 209, com o que
  falta medir escrito.

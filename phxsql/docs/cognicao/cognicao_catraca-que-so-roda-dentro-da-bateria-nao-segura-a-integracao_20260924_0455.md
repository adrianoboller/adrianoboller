# Catraca que só roda dentro da bateria não segura a integração

Papel A (integração), 24/09/2026. Defeito que eu deixei entrar, achado pela frente 475.

## 1. O que aconteceu

O commit `de4ca0a` (biblioteca do PhxZip), integrado por mim, trouxe
`#[derive(Debug)]` numa struct com `senha: Option<String>` (`Opcoes`, em
`crates/phxzip/src/escritor.rs`). A catraca `bancada/guardas/debug-com-segredo.py`
subiu de 0 para 1 — e ninguém viu por quatro commits, até a frente 475 rodar a parte
`ponta-a-ponta` da bateria inteira.

## 2. O que eu concluí primeiro, e estava errado

Que a minha conferência de integração cobria as catracas: eu rodava o
`trecho-vivo.py --catraca`, a suíte e o clippy na árvore do commit. Mas essa catraca
não é chamada por nenhum dos três. Ela só roda dentro do item 0 da
`bancada/bateria/prova-bateria.py` (junto com as do mapa da trava e do mapa das
threads). Rodei «as catracas» pelo nome de uma só.

## 3. O que a medição disse

- Na versão entregue (`a8f62b3`): `VAZA crates/phxzip/src/escritor.rs:55
  Opcoes.senha: Option<String> (derive(Debug))`, `SUBIU 1 (teto 0)`.
- O primeiro conserto (um `impl Debug` à mão que dizia se havia senha) continuou
  reprovado: a régua recusa `impl` que LÊ o campo, porque dizer se a senha existe já é
  contar algo dela. O conserto aceito não toca no campo (`senha: _`, `"(oculta)"`), no
  molde do `Cifra`.
- Prova real: com a senha de volta no `Debug`, o teste novo cai mostrando
  `senha: Some("s3nh4-que-nao-pode-aparecer")`; com o conserto passa, e a catraca volta
  a 0.

## 4. A regra

**A conferência de integração roda TODAS as catracas, não só a que tem «catraca» no
nome: as que moram dentro da bateria também, porque é delas que o commit escapa.**

## 5. Como está guardado hoje

Não está: o que chama essas três catracas continua sendo só o item 0 da bateria. O
buraco vira o pedido 476 — um comando só que roda todas as catracas (as do fonte e as
em Python), chamado pela bateria e pelo integrador, para ninguém escolher quais rodar.

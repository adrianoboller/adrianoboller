# A guarda da senha existia, tinha o nome certo, e media a coisa errada

Descoberto em 23/09/2026, ~20:20, pelo papel E no pedido **339(a)** (o repouso
da chave da API). Não é reafirmação de pétrea: a pétrea «senha nunca em texto
puro» já existia **e tinha guarda própria**. O aprendizado é o **alcance** dela.

## 1. O que aconteceu

O pedido 339 mandava tirar a chave da API do `localStorage`. Medindo os irmãos
— *quem mais guarda credencial no navegador?* — a varredura do DOM depois do
login achou o ativo **maior** ainda lá:

```
INPUT#s type=password   -> a senha do usuário
INPUT#t type=password   -> o token do servidor
```

Os dois com o valor vivo, em claro, **pela sessão inteira** — medido depois de
entrar e percorrer 113 telas. `type="password"` esconde os glifos; não esconde
o valor. Qualquer script na página lia os dois.

E o caso `testes-web/casos/01-entrada.mjs` **já dizia que provava isso**, desde
que nasceu, no comentário do topo: *«a senha digitada NAO sobra em lugar nenhum
do documento depois de entrar»*. Ele passava verde o tempo todo.

## 2. O que eu concluí primeiro, e estava errado

Primeiro concluí que o furo era o `localStorage` da chave da Claude, como o
parecer externo dizia — e que mover a chave para a memória resolvia. Errado por
dois motivos, e o segundo é o que importa aqui:

- o ativo maior **já estava em memória** (`est.token`, `est.sessao`) e cai no
  mesmo XSS — isso o pedido 339 já tinha medido e escrito;
- mas «em memória» **não quer dizer «fora do DOM»**. Eu li `entrar()`, vi que a
  senha ia para uma variável local, e dei o campo por resolvido. O campo
  continuava com o valor. Ler o código não mostra isso; abrir o navegador
  mostra em dez segundos.

Também concluí, antes de medir, que uma guarda chamada «a senha não sobra no
documento» cobria o campo. Cobria o **HTML**.

## 3. O que a medição disse

A guarda usava `await page.content()`. Isso devolve o documento **serializado**
— e a serialização de um `<input>` traz o **atributo** `value`, nunca a
**propriedade** `.value`. Quem digita altera a propriedade e não o atributo.
Logo a guarda media um lugar onde a senha, por construção, nunca ia estar.

Números do dia:

- credenciais vivas no DOM depois do login, antes do conserto: **2**
  (`#s` com 8 caracteres, `#t` com 7) — e **3** campos no alcance, com o `#k`
  (chave privada Ed25519) vazio naquele cenário;
- respostas `/api` que trouxeram senha ou token ao navegador: **0** — o
  servidor não ecoa credencial, e esse lado estava certo;
- gavetas do navegador depois das 113 telas: **4** no `localStorage`, nenhuma
  sensível (conexões com lista branca, tema, arranjo de telas), e **1** no
  `sessionStorage`, a chave da API.

Com o conserto (`for (const campo of ["#s","#t","#k"]) $(campo).value = ""`,
só no caminho de sucesso): **0**. Prova real nos dois sentidos — com a linha
retirada, o caso reprova nomeando os dois campos e os tamanhos.

## 4. A regra

**Guarda de segredo mede a propriedade viva, nunca a serialização.** Quando
uma guarda diz «não sobra no documento», pergunte *em que representação do
documento* — e reponha o defeito para ver se ela cai. Guarda que não cai com o
defeito reposto não é guarda: é uma frase.

E o corolário do papel E: **`type="password"` é mascaramento de glifo, do mesmo
naipe de um filtro de CSS por cima.** O que não se quer alcançável não se põe
no DOM; o que se quer mostrar mascarado se mascara no **valor**
(`chave.slice(-4)`), como a tela da Claude já fazia.

## 5. Como está guardado hoje

- O conserto está em `crates/phxsql-server/ui/index.html`, em `entrar()`, logo
  antes do `marcarUsoDaConexao()`, com o porquê escrito ao lado — inclusive o
  porquê de **não** limpar no `catch` (entrada recusada tem de deixar o que foi
  digitado).
- A guarda nova está em `testes-web/casos/01-entrada.mjs`, **ao lado** da
  velha e não no lugar dela: a de `page.content()` continua valendo para o
  HTML, e a nova varre `input.value` de todo campo `password` mais o `#t` e o
  `#k`. As duas medem coisas diferentes, e agora está escrito qual é qual.
- **Onde o buraco ficou:** a guarda nova alcança o formulário de **entrada**.
  Não há conferidor genérico varrendo os outros formulários da tela em busca de
  campo de segredo com valor vivo — e há pelo menos dois assistentes que
  guardam segredo digitado num objeto de página enquanto o assistente está
  aberto (`rz.token_remoto` e `r.senha`, medidos: nascem vazios e só carregam o
  que a pessoa acabou de digitar, então não são vazamento de segredo
  *guardado*, mas são segredo no DOM enquanto a tela existe). Quem for escrever
  esse conferidor comece pela medição deste arquivo, não por leitura.

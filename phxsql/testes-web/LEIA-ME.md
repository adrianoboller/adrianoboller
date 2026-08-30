# A bateria de frontend

O `cargo test` prova o motor. Esta bateria prova a **tela** — contra o
servidor de verdade, num navegador de verdade, sem maquete e sem mockup.

```bash
cargo build --release -p phxsql-server --bin phxsqld
node phxsql/testes-web/bateria.mjs
```

Ela sobe um `phxsqld` só dela (portas **6200** e **6201**), num diretório
temporário próprio, e o derruba **pelo PID** no fim — nunca por `pkill`.
Roda os treze casos: onze nos **dois temas** e dois num tema só (os
dois novos medem relógio, e cor não muda relógio) — **24 execuções**, ~4min30.

| chave | o que faz |
|---|---|
| `--tema claro` / `--tema escuro` | roda um tema só |
| `--caso <pedaço>` | roda os casos cujo nome contém o pedaço |
| `--capturas <dir>` | guarda os PNG de cada tela |
| `--ver` | abre o navegador na tela, devagar |
| `--porta <n>` | outra porta de dados (a web é ela + 1) |

## Atenção ao binário velho

A página está **embutida** no `phxsqld` (`include_str!`). Mexer em `ui/` e não
recompilar faz a bateria exercitar a página anterior — e passar verde numa
correção que ainda não existe. Esta casa já perdeu uma rodada inteira de
ganhos medindo com binário velho, então a bateria **recusa rodar**: ela
compara a data do binário com a do arquivo mais novo de `ui/` e diz qual.

## Os treze casos

| | o que prova |
|---|---|
| `entrada` | a tela de login, o desafio-resposta no navegador, e que a senha não sobra no documento. Falha se a página cair em modo demonstração — sem isso a bateria inteira passaria sem tocar no motor |
| `passeio` | clica **todos** os itens dos nove menus e **todos** os botões da barra — 112 telas — e reprova em qualquer erro. Vale mais que dez asserções bonitas |
| `ficha` | incluir e salvar pela tela. É o fluxo que quebrou inteiro quando o `rownum` entrou |
| `arvore` | a árvore remontando quando um banco novo aparece — e continuando **viva** depois de remontar |
| `grade` | nenhuma coluna de sistema vira coluna de dado na grade editável, e o phx-grid não perde coluna nenhuma |
| `css-global` | as três armadilhas do CSS global: controle esticado, dado em caixa alta, e caixa de marcar separada do próprio texto |
| `responsivo` | as **cinco** larguras — celular, tablet, desktop, ultrawide (3440) e dois monitores (5120). Nada rola de lado; e nas duas largas, nada estica: texto corrido tem teto, par rótulo→valor vira coluna, e texto de SVG não cresce com o monitor nem se sobrepõe. Planta um caminho de disco comprido antes de medir, senão a sobreposição não se reproduz |
| `lateral` | o painel retrátil e pinável, com volta |
| `cores` | a convenção das cinco cores (contorno, nunca fundo cheio) e o contraste **medido** de cada elemento pintado |
| `primeira-pintura` | a tela de entrada aparece mesmo quando a rede engole a fonte da marca |
| `lgpd` | a tela de Dado pessoal audita de verdade |
| `multitela` | abas vivas com estado próprio, regiões lado a lado com calha, janela solta dentro da página, e o pino. Mede os pedidos por minuto com a aba escondida, com ela fechada, e com as quatro telas nomeadas visíveis ao mesmo tempo |
| `monitores` | a emenda física entre dois monitores, o monitor pinado que sumiu, a janela destacada pegando a sessão pelo canal — e a sessão **não** aparecendo no `localStorage`. DPI de 2× num contexto próprio |

## A prova do multi-idioma, à parte

`node phxsql/testes-web/prova-idiomas.mjs --capturas <dir>` roda fora da
bateria, na faixa **6650/6651**, e prova o caminho do idioma de ponta a ponta:

1. sem escolher nada, a tela é a de sempre, **em português** — o teste do
   comportamento velho, que é o que mais importa numa guarda nova;
2. a bandeira da tela de **entrada** troca o texto na hora;
3. a escolha **atravessa o login**: o cromo entra no idioma escolhido;
4. a bandeira da tela de **configuração** troca o cromo sem recarregar e sem
   levar a pessoa para outra tela;
5. a escolha sobrevive a **sair e entrar** de novo;
6. o **alemão** (~30% mais longo) não corta rótulo da barra nem faz a página
   rolar de lado — o defeito que só aparece traduzindo;
7. a **frase que era picada**: a tela «Sobre o modo multitela» sai inteira e na
   ordem em português, com a ênfase virada `<b>`/`<code>` de verdade e sem
   marca crua à mostra; troca para alemão **sem sair da tela**, e nem o
   corpo, nem o título, nem o `title` da tira de abas ficam em português;
8. capturas da mesma tela em três idiomas × dois temas.

Foi ela que achou o `txt` declarado como `const` sendo pedido pelo
`aplicarTema` do arranque: a página morria na primeira pintura e o botão de
tema ficava sem `onclick`. Ler o código não acharia.

O passo 7 tem prova real nos dois sentidos, e os dois defeitos foram
repostos: tirando o `est.repintar` que a tela do modo repõe, ela não troca de
idioma e o passo estoura no `waitForFunction`; tirando a conversão de marcas
do `marcado()`, a página mostra `**Multitela.**` com os asteriscos à mostra e
o passo diz qual frase saiu errada.

## Os três canais de erro

`pageerror` não é o único. O `ligarMenu` manda **toda** exceção de item de
menu para `avisar(..., true)` — capturada, ela nunca vira `pageerror`. Por
isso o passeio olha três lugares:

1. `pageerror` — exceção que ninguém pegou (o runner cuida deste);
2. `#aviso.mal` — o recado vermelho da barra;
3. `#painel .aviso.mal` — o erro que uma aba deposita no painel.

Os dois últimos são limpos **antes** de cada clique: aviso deixado pela tela
anterior seria contado contra a próxima.

## O que ela deliberadamente NÃO faz

- **Não fala com a internet.** A fonte da marca vem do Google; a bateria
  recusa esses pedidos na origem. Deixá-los sair traria a rede de quem roda
  para dentro do resultado. O caso `primeira-pintura` é o dono desse assunto
  e instala a rota dele.
- **Não testa JavaScript por unidade.** A página não exporta módulo: é um
  `include_str!` de 11 mil linhas servido pelo binário. Todo caso é de ponta
  a ponta.
- **Não clica em diálogo nativo.** `confirm` e `prompt` são descartados pelo
  Playwright quando ninguém os escuta, e é por isso que o passeio pode clicar
  em «Excluir tabela» sem excluir nada. O caso `arvore` é a exceção: ele
  **responde** ao `prompt` do `[+]`, porque ali o diálogo é o caminho.
- **Não mede desempenho** do motor. Isso é a `bancada/`. O que ela mede é o
  custo da TELA em pedidos por minuto, que é outra coisa e mora no caso
  `multitela`.
- **Não exercita a Window Management API.** Ela existe no Chromium sem cabeça
  mas rejeita sem a permissão `window-management`, que o Playwright 1.56 não
  sabe conceder. O caso `monitores` a **dubla** e prova o caminho nosso —
  achar a emenda, alinhar as calhas, cair para o monitor principal. O que fica
  sem prova real é a resposta do navegador; ver `../docs/MULTITELA.md`.

O que cada caso cobre, o que ficou de fora e por quê está em
`../docs/TESTES.md`.

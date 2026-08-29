# A bateria de frontend

O `cargo test` prova o motor. Esta bateria prova a **tela** — contra o
servidor de verdade, num navegador de verdade, sem maquete e sem mockup.

```bash
cargo build --release -p phxsql-server --bin phxsqld
node phxsql/testes-web/bateria.mjs
```

Ela sobe um `phxsqld` só dela (portas **6200** e **6201**), num diretório
temporário próprio, e o derruba **pelo PID** no fim — nunca por `pkill`.
Roda os onze casos nos **dois temas**: 22 execuções, ~2min20.

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

## Os onze casos

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
- **Não mede desempenho.** Isso é a `bancada/`.

O que cada caso cobre, o que ficou de fora e por quê está em
`../docs/TESTES.md`.

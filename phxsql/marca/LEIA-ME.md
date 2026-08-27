# Marca PhxSql

Arquivos oficiais, como recebidos:

| Arquivo | O que é |
|---|---|
| `phxsql-manual-de-marca.png` | Folha de marca: logo primário, ícone, versão horizontal, ícone de app, paleta e tipografia |
| `phxsql-logo.png` | Logotipo quadrado com assinatura *Phoenix Database Engine* |
| `phxsql-abertura.png` | Tela de abertura |

Em `derivados/`, o que a documentação usa — gerados dos originais, não
redesenhados:

| Arquivo | Onde entra |
|---|---|
| `phxsql-logo-560.png` | cabeçalho do `README.md` |
| `phxsql-simbolo-420.png` | capa do dossiê (embutido como data URI) |
| `phxsql-icone-128.png` | uso geral em miniatura |
| `phxsql-simbolo-224.png` | cartão de entrada do Centro de Controle |
| `phxsql-icone-64.png` | barra do Centro de Controle |
| `phxsql-icone-32.png` | ícone da aba do navegador |

Os três últimos são os que entram na interface web, embutidos no
`ui/index.html` como data URI — a página é um arquivo só, e não há de onde
buscar imagem.

**Eles têm alfa; os originais não.** O fundo `#010418` foi retirado
desfazendo a pré-multiplicação: subtrai o fundo, tira `alfa = max(r,g,b)` e
divide a cor por ele. Como o logo é brilho sobre quase preto, isso recupera
a cor real de cada pixel e a borda do brilho sai suave, em vez de recortada
com halo. É o que permite a fênix assentar em cima do painel `#0a1122` sem
deixar emenda de retângulo.

O ícone da barra usa um **recorte mais fechado** que o do cartão — só a ave e
o cilindro, sem as pontas da asa nem as trilhas de circuito. Em 30 px o
desenho completo vira borrão; menos desenho é mais legível.

## Especificação

| | |
|---|---|
| Tipografia | **Exo 2** — SemiBold / Medium / Regular |
| Fundo | `#010418` (medido dos originais) |
| Assinatura | *Built to store. Engineered to scale.* |
| Subtítulo | *Phoenix Database Engine* |

```
#FFC43D   âmbar        destaque, números
#FF8A1C   laranja      acento no tema escuro
#FF4D10   vermelhão    acento (escurecido para #C63C0A sobre papel)
#D71A1A   vermelho     o .log nos diagramas
#8B0D0D   vinho
#DDE2EB   prata        cor do texto no tema escuro
```

## Como a marca entra no dossiê

O acento e a tipografia vêm daqui. Duas adaptações deliberadas, para o
documento continuar sendo um documento:

- **O corpo do texto não é Exo 2.** Exo 2 é a voz da marca e leva títulos,
  rótulos e o cromo da página; parágrafos longos ficam numa serifada. É
  extensão de marca, não desvio: uma face geométrica cansa em texto corrido.
- **O vermelhão escurece no tema claro.** `#FF4D10` sobre papel dá ~3,5:1 de
  contraste, abaixo do mínimo para texto. Vira `#C63C0A`, que mantém a cor da
  marca e passa dos 4,5:1.

As cinco cores de arquivo dos diagramas (`.reg` `.ndx` `.bin` `.memo` `.log`)
precisam se distinguir **entre si e do acento**. Por isso o `.bin`, que era
âmbar, virou ciano: âmbar ao lado do laranja da marca vira ruído. O `.log`
ficou com o vermelho `#D71A1A` da paleta, que é onde ele encaixa sozinho.

## Atenção: a folha de marca promete o que o motor ainda não faz

Dois dos quatro pilares da folha **não são verdade hoje**:

- *"Reliable storage — ACID compliant"* — **não há transações**. Sem elas não
  há o A nem o I do ACID. O que existe é durabilidade por CRC e o desfazer de
  uma inserção que falha no índice.
- *"Built-in replication — high availability and failover ready"* — a
  replicação está **desenhada** (`docs/REPLICACAO.md`), não implementada.

Isso é normal numa marca feita antes do produto ficar pronto, mas os dois
precisam virar verdade antes de a folha ir para cliente. O dossiê e o
`README.md` dizem o estado real; a folha de marca, não.

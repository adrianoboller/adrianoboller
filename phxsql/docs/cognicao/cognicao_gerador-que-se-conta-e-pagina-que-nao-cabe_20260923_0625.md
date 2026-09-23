# O teto de uma página gerada não é estético — é a janela de quem a republica

Descoberto em 23/09/2026, entre 06:05 e 06:25 UTC, fechando a rodada 0.19.0.
A hora do nome é a da descoberta, não a do commit.

## 1. O que aconteceu

Faltava publicar a sétima página da rodada, a dos pedidos. O serviço recusou, e
a recusa não falava de erro: falava de leitura. *«The newer version … still does
not count as viewed … its saved source … (781 lines) has not yet been Read
whole.»* Republicar uma página exige ter lido, linha por linha, a versão que já
está publicada — para que a republicação seja uma mescla e não um atropelo.

O arquivo publicado tem **1.304.729 bytes em 781 linhas**.

Puxando o fio, o **portão dos geradores** devolveu um segundo achado que eu não
estava procurando: o `docs/tecnologias/extrair.py` continuava **VELHO depois de
rodado**. Três corridas seguidas dele, sem nenhuma edição entre elas, deram
**106.750**, **106.752** e **106.753** linhas de Markdown na mesma célula.

## 2. O que eu concluí primeiro, e estava errado

**Nas duas coisas, e as duas vezes por achar que já sabia o formato do problema.**

Na página: concluí que era **problema de fatiamento**. A ferramenta de leitura
tem teto de 25.000 fichas por chamada, eu tinha batido nele duas vezes, e a
conclusão saiu pronta — *leio em pedaços de 60 linhas e pago*. Cheguei a
escrever isso como plano. O que eu não tinha feito era a única coisa que
decidia: **medir quanto custa o arquivo inteiro**. Pedaço menor resolve chamada
que estoura; não resolve arquivo que não cabe na janela. São problemas
diferentes com o mesmo sintoma.

No gerador: concluí que ele estava velho por **eu** ter mexido no `PENDENCIAS.md`
— rodei, dei por resolvido, e o portão continuou vermelho. A causa não era a
minha edição.

## 3. O que a medição disse

**A página.** A razão saiu do próprio arquivo, e não de uma estimativa: a
ferramenta, ao recusar 240 linhas, *informou* o custo delas — **146.596 bytes =
65.129 fichas**, ou **2,251 bytes por ficha**. Logo:

| o que | medido |
|---|---|
| página publicada | 1.304.729 bytes, 781 linhas |
| leitura inteira | **~580.000 fichas** |
| chamadas necessárias | **>= 24**, ao teto de 25.000 |
| janela de contexto | ~200.000 fichas |
| **logo** | **~3 compactações só para VER uma página** |

Daí sai o número que interessa e que ninguém tinha: **~450 KiB de página
publicada é uma janela inteira.** Acima disso a página deixa de ser republicável
por este caminho. O dossiê tem 2,6 MB; a dos pedidos, 1,3.

E a armadilha que transforma isto em decisão em vez de otimização: **encolher a
página não compra a PRIMEIRA publicação**, porque o guarda lê a versão
*publicada*, que é a grande. Página menor só fica barata da publicação seguinte
em diante.

**O gerador.** Ele conta `docs/` **recursivo** — e `docs/TECNOLOGIAS.md` está
dentro de `docs/`. Quando um bloco `<!-- GERADO -->` muda de **número de
linhas** (aqui a lista dos pedidos com a palavra RECUSADO passou de 66 para 67
itens, +1 linha), a própria gravação muda o número que a gravação seguinte vai
ler. Converge em **duas** passagens — a segunda só troca dígitos —, mas quem
roda a corrente **uma vez**, como manda o LEIA-ME, termina com o portão VERMELHO
e o número atrasado em um.

## 4. A regra

**Antes de pagar um custo em pedaços, meça o custo INTEIRO** — teto por chamada
e teto por janela são limites diferentes, e só o segundo decide se a coisa cabe.

**Gerador cuja saída está dentro da própria entrada não chega a ponto fixo numa
passagem** — e o portão, que existe para perguntar «re-rodar mudaria algum
número visível?», responde *sim* para sempre nesse caso, sem que haja nada de
errado com o dado.

## 5. Como está guardado hoje

**Está guardado como pedido, não como conserto**, e nos dois casos de propósito:

- **403** carrega os números da página, o teto de 450 KiB e as três rotas
  (pagar uma vez, partir em faixas de 100, parar de publicar). São trocas
  diferentes e nenhuma é obviamente melhor, então a escolha é do dono.
- **404** carrega a sequência 106.750 → 106.752 → 106.753 e o conserto, que
  **esta casa já inventou**: a §17 da sétima página escreve «— (esta página)» na
  célula de si mesma, com o motivo ao lado. Aplicá-lo aqui muda **dois números
  publicados** (435 arquivos viram 434), e mudar número publicado é de quem
  responde pelo documento — papel H —, não do integrador que passava por ali.

**Onde o buraco ficou:** a página dos pedidos continua publicada com **394**
onde são **404**. Isso está escrito no 403 em vez de escondido, e as outras seis
páginas carregam os números de hoje — a vista do dono não ficou no escuro,
ficou sem esta tabela.

E um terceiro achado, pequeno, que estreitou um pedido antigo em vez de abrir um
novo: o **326** dizia que o leitor vê uma versão *fixada*. Medido no cabeçalho
de cada página, a fixação é do **dossiê**, que é o compartilhado; a dos pedidos
respondeu *«owned by you, private»*. Quem for consertar confere o cabeçalho de
cada uma antes de mexer — **há irmão só quando há irmão**.

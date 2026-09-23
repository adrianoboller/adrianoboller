# «BLUMENAU» tem um irmão: o `<code>` em caixa alta

Descoberto em 23/09/2026, ~20:25, pelo papel E, enquanto se olhava a tela do
pedido **339(a)** no navegador. O aprendizado novo é o **alcance** da guarda do
CSS global, não a lei — a lei «rótulo se estiliza, dado nunca» já existe e já
tinha caso próprio (`testes-web/casos/06-css-global.mjs`).

## 1. O que aconteceu

A tela *Configurações → Integração com a Claude* publicava o endereço da API
assim:

```
ENDEREÇO DA API: HTTPS://API.ANTHROPIC.COM/V1/MESSAGES
```

O `textContent` era `https://api.anthropic.com/v1/messages`; o `innerText` saía
em caixa alta. A URL está num `<code>` — o elemento que existe justamente para
dizer «isto é exatamente o que está escrito».

E dói mais aqui do que num nome de cidade: **o endereço é o que a pessoa tem de
julgar antes de deixar a chave sair por ele.** Dobrar a caixa de uma URL apaga
a diferença que denunciaria um endereço sósia.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a regra culpada era `label{text-transform:uppercase}` — a do
«Blumenau». Não era: o endereço não está dentro de um `<label>`.

E escrevi uma conferência minha que **passou** dizendo «nenhum
`text-transform` sobre dado», varrendo `#folha code, #folha .leg, …`. O `#folha`
não existe nesta página: `querySelectorAll` devolveu vazio e a conferência
passou por não ter o que reprovar. Teste que passa por engano, de novo, e desta
vez escrito por mim no mesmo dia em que eu estava caçando outro igual.

## 3. O que a medição disse

Perguntando ao navegador quais regras casam com o elemento:

```
.form-dbl .cmp                    -> text-transform: none
.form-dbl .cmp > span:first-child -> text-transform: uppercase
.form-dbl .cmp .leg               -> text-transform: none
```

A regra do rótulo (`> span:first-child`) vale **(0,3,1)** de especificidade
contra **(0,3,0)** do `.leg` — ganha por **um elemento**. A linha do endereço
tinha rótulo e valor dentro do mesmo `<span class="leg">`, que era o primeiro
filho. O `text-transform` é herdado, e desceu até o `<code>`.

Estendida a guarda do caso `06-css-global` para tratar `<code>` e `<pre>` como
dado, ela achou **um segundo** defeito do mesmo naipe numa tela que ninguém
estava olhando: o cabeçalho «campo no `config.json`» da tela de Configurações
do servidor publicava `CONFIG.JSON`, por `thead th{text-transform:uppercase}`.

Total no repasse de **13 telas**: 2 defeitos, ambos em `<code>`, ambos
invisíveis para as duas metades que a guarda já tinha (a lista de seletores de
célula de grade não os alcançava, e a heurística do «texto MISTO» não podia
pegá-los porque as duas cadeias são todas minúsculas na origem).

## 4. A regra

**`<code>` e `<pre>` são dado por construção: nenhum `text-transform` passa por
eles.** Não é heurística e não precisa de lista de exceções — esses dois
elementos existem para dizer «literalmente isto».

E o corolário de método: **rótulo e valor na mesma caixa é uma bomba-relógio de
especificidade.** Quando um formulário estiliza «o primeiro filho», o valor
mora num segundo filho, sempre.

## 5. Como está guardado hoje

- Os dois consertos: em `ui/claude.js` (rótulo e endereço viraram dois
  `<span>`, e a linha deixou de ser `.linha-chk`, que é layout de caixa de
  marcar) e em `ui/index.html` (`thead th code,thead th pre{text-transform:none}`,
  irmã da `table.montar th em` que já existia logo acima).
- A guarda estendida está em `testes-web/casos/06-css-global.mjs`, com o
  defeito que a motivou escrito ao lado, e o caso passou a visitar a tela da
  Claude. **Prova real feita nos dois sentidos, para os dois defeitos**: com
  cada um reposto, o caso reprova nomeando a tela e o valor.
- **Onde o buraco ficou:** a guarda só mede as **13 telas** que o caso abre. O
  `02-passeio` abre 114 e não mede caixa alta. Juntar os dois (medir em toda
  tela que o passeio abre) é o próximo passo óbvio e não está feito.

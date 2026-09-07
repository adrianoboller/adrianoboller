# wx-modelos

Escolha e controle do **modelo local** no terminal: o que esta máquina aguenta,
o que o serviço local tem carregado, carregar, soltar e **medir**.

Binário Rust à parte, `std` pura — **nenhuma crate**. É o que faz o executável
rodar na máquina do cliente sem instalar nada e compilar cruzado sem drama.

```bash
cargo build --release            # gera target/release/wx-modelos
wx-modelos maquina               # o que esta máquina tem, medido
wx-modelos modelos               # catálogo, com o que cabe aqui
wx-modelos estado                # o que o serviço local tem carregado
wx-modelos carregar <id>         # carrega, com o progresso que o serviço der
wx-modelos medir <id>            # mede tok/s NESTA máquina e guarda
wx-modelos                       # a tela cheia, quando há terminal
```

Toda saída tem `--json`, que é como o `rotear_modelo.py` consulta o degrau
local sem depender de texto para humano.

## Entrega: Linux e Windows

```bash
python3 publicar.py                # os dois alvos
python3 publicar.py --alvo linux   # só um
```

Sai em `dist/` (fora do git) com `ENTREGA.md` e `entrega.json`: versão, alvo,
tamanho, **SHA-256** e o `rustc` que compilou. Binário não entra na árvore —
commitado, ele envelhece calado e mostra números de um código que já mudou.

Três recusas do publicador, todas provadas:

- **teste vermelho não vira binário** — `cargo test` roda antes;
- **ferramenta ausente é PULADO, não falha**: sem `rustup target` ou sem o
  mingw, o alvo sai da lista com o comando que resolve, e o outro é publicado;
- **o `.exe` não pode depender de DLL do compilador** — `libgcc_s`,
  `libwinpthread` e `libstdc++` são procuradas dentro do binário; achando
  qualquer uma, a publicação falha, porque o cliente não tem esse arquivo.

O hash identifica o arquivo entregue e **não** afirma build reprodutível:
medido, recompilar o mesmo código muda o hash do `.exe` (o formato PE carrega
carimbo de tempo). Confira uma entrega contra a ficha que veio com ela.

Medido: o `.exe` importa só `KERNEL32`, `WS2_32`, `ntdll`, `msvcrt` e
`api-ms-win-core-synch` — tudo do próprio Windows. Um arquivo, sem instalador.

## A regra que manda no desenho

**Eixo sem medição não vira polígono.** A tela que originou esta ferramenta
mostrava um radar com «INTELIGÊNCIA 10 %» — número que decide compra de
hardware. Aqui:

| o que aparece | de onde vem |
| --- | --- |
| núcleos, memória, processador, acelerador | do sistema operacional, medido na hora |
| cabe / apertado / não cabe | calculado com a memória medida e o tamanho do modelo |
| tamanho, contexto, quantização | do catálogo (serviço local ou `--catalogo`) |
| tokens por segundo | **só depois de `wx-modelos medir`**, nesta máquina |
| qualidade | só se o catálogo trouxer a **fonte** junto; sem fonte, é descartada |

O que não se mede aparece **INDISPONÍVEL**, e o radar diz «3 de 5 eixos
medidos» embaixo — porque o olho lê área, não rótulo. Aresta só liga dois eixos
medidos: área chutada é mentira desenhada.

E o orçamento de memória é regra escrita, não mágica: 70 % do que está livre
agora, ou 60 % do total quando o livre não se mede.

## O que ele NÃO faz

Não sobe o serviço, não baixa modelo e **não redistribui nada do Magnitude** —
ele controla o que já está instalado (veja `skills/modelos-locais/`). Sem
serviço no ar, `estado` sai com código 1 e diz que o roteador volta ao modelo
pago, que é o comportamento que os testes do `rotear_modelo.py` já travam.

`exemplo-catalogo.json` é **exemplo de formato**, não afirmação sobre modelo de
terceiro: os valores são de demonstração, e o catálogo de verdade vem de
`/models/catalog` do serviço local.

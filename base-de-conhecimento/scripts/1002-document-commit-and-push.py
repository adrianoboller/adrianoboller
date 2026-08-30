# Document, commit and push
# 29/08 02:13

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''### 2.1 De quanto tem de ser o teto'''
novo = '''### 2.3 O Profiler desligado custava 7% da carga

Não estava no `onde-doi` porque não é do motor: é do **servidor**, e só aparece
quando o pedido passa pela porta. A bancada da carga em lote é que mostrou.

O ponto de captura ficava assim:

```rust
let marca = {
    let alvo = objeto_do_pedido(&linha, ..);   // Json::analisar #1 + 2 String
    let nome_op = Json::analisar(&linha)..;    // Json::analisar #2 + 1 String
    self.profiler.lock().ok().and_then(|mut p| p.chegou(..))
    //           ^ mutex por pedido        ^ e SÓ AQUI ele olha `ligado`
};
```

Com o Profiler **desligado**, todo pedido pagava dois `Json::analisar` do corpo
inteiro, três `String` e um mutex — para no fim `chegou` devolver `None`. Num
`inserir_lote` de 5.000 linhas isso é analisar meio megabyte de JSON **duas
vezes, para nada**.

O portão passou a ser um `AtomicBool` lido com `Relaxed`, antes de qualquer
trabalho:

| carga em lote pela rede, Profiler desligado | linhas/s |
|---|---:|
| antes | 40.597 · 40.653 |
| depois | **43.612 · 43.302** |

**1,07×** — dois pares de corridas, o mesmo binário sem mais nada mudando.

A regra que fica: **instrumentação desligada tem de custar zero, e o portão que
decide isso vem antes do trabalho, não depois.** Um mutex por pedido é pior do
que parece: além do custo, ele *serializa* — todo mundo esperando na mesma
fila para descobrir que não havia nada a registrar.

O que o `AtomicBool` custa em troca é uma janela de um pedido: quem liga a
observação pode não ver o que já estava em voo. Ligar a observação no meio de um
pedido não promete pegar aquele pedido — promete pegar os próximos.

### 2.1 De quanto tem de ser o teto'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace("| Inserção pela rede, linha a linha vs. lote | 2.659/s | 39.287/s | **14,8×** |",
              "| Inserção pela rede, linha a linha vs. lote | 2.659/s | 43.302/s | **16,3×** |")
p.write_text(s)
print("ok")

# Defeito reposto que não repõe: a marca chegava ao disco por outra escrita

**Descoberta:** 16/09/2026, 08:01 UTC, no `provar-guardas.py --so
ndx-queda-com-cabecalho-limpo` (papel F, «chutar a tomada»).

## 1. O que aconteceu

Escrevi uma guarda para o contrato que torna o write-back do `.ndx`
aceitável: a marca de sujo (byte 52) vai ao disco **antes** da primeira
página suja existir, para que uma queda no meio de uma carga seja detectada
e não silenciosa. A troca que «repunha o defeito» era, em `gravar_pagina`:

```rust
// antes                                   // troca 1 (errada)
if !self.sujo {                            self.sujo = true;
    self.sujo = true;                      self.guardar_no_cache(n, p, true)
    self.gravar_cabecalho()?;
}
self.guardar_no_cache(n, p, true)
```

O executor respondeu **NAO PEGOU: 0/2 caíram** — os dois testes que tinham
de cair (`a_queda_sem_sincronizar_e_detectada_e_nao_silenciosa` e
`a_marca_sai_depois_das_paginas_e_nao_antes`) continuaram verdes com o
«defeito» dentro.

## 2. O que eu concluí primeiro, e estava errado

Que tirar a chamada a `gravar_cabecalho()` daquele ponto bastava para o
cabeçalho no disco ficar dizendo «limpo». Eu tinha lido que aquele era **o**
lugar onde a marca era escrita antes da página, e tratei o ponto como se
fosse o único caminho do byte 52 até o arquivo.

## 3. O que a medição disse

Não era o único caminho. O cabeçalho do `.ndx` é regravado a **cada
`inserir`** — o `qtd_chaves` do descritor muda e `gravar_cabecalho()` roda
no fim da operação (o `remover` faz o mesmo) — e `montar_cabecalho` serializa
`self.sujo` da memória (`buf[52] = u8::from(self.sujo)`). Com a troca 1 a
flag estava levantada em RAM, então a primeira inserção seguinte levava o
byte 52 = 1 ao disco de qualquer jeito. O contrato «marca antes da página»
ficava violado por microssegundos e reparado pela escrita vizinha; o teste,
que só olha o estado depois de 20.000 inserções, não tinha como ver.

A troca que repõe o defeito de verdade é **ninguém** levantar a marca:

```rust
// troca 2 (certa)
self.guardar_no_cache(n, p, true)
```

Reprovada: **PROVADA, 2/2 caíram** em 1,9 s, com os quatro `seguem`
(`sincronizar_fecha_o_arquivo_de_verdade`,
`despejo_de_pagina_suja_chega_ao_arquivo`, `lote_sobrevive_a_reabrir_o_arquivo`,
`o_cache_nao_serve_pagina_velha`) de pé.

## 4. A regra

**Repor um defeito é repor o ESTADO no disco, não apagar a linha que o
evita.** Antes de escrever a troca, pergunte por onde mais aquele estado
chega ao arquivo — quem mais grava o cabeçalho, quem mais fecha o arquivo —
e faça a troca cobrir todos, ou a guarda prova só que o executor funciona.
E o veredito NAO PEGOU do executor é a prova real da prova real: sem ele, esta
guarda teria entrado no catálogo como PROVADA por leitura.

## 5. Como está guardado hoje

* `bancada/guardas/catalogo.py`, entrada `ndx-queda-com-cabecalho-limpo`: a
  troca 2, com o comentário contando a troca 1 e por que ela não repunha.
* O executor `provar-guardas.py` continua sendo quem julga; a corrida
  completa do fim da rodada é a que diz se a guarda ainda pega amanhã.
* O buraco: o catálogo tem outras entradas cuja troca «apaga a linha» de um
  estado que pode chegar ao disco por caminho irmão. Não as medi; a pergunta
  está aberta para o papel G — a corrida completa das 136 guardas é o
  instrumento certo, e uma NAO PEGOU lá é este mesmo achado com outro nome.

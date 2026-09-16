# O número estava certo e a receita ao lado mentia — e isso audita limpo

Descoberto em 16/09/2026, 15:10, ao ver que a sétima página não aparecia em
lugar nenhum do inventário de tecnologias.

## 1. O que aconteceu

O `docs/TECNOLOGIAS.md` publica, num bloco `<!-- GERADO -->`, quantas linhas de
cada linguagem existem e **onde**. Duas linhas dessa tabela estavam assim:

```
| Markdown (documentacao tecnica) | `docs/` (nao recursivo em `dossie/`, `design/`, `video/`) | 353 | 85013 |
| Python (geradores de dossie/pedidos) | `docs/dossie/` | 17 | 5582 |
```

A primeira **publica o número recursivo com um rótulo que jura o contrário**. O
`contar_arquivos()` do `extrair.py` usa `rglob`, e nenhuma exclusão era passada
naquela chamada: o 353 sempre foi a conta com `cognicao/`, `dossie/`,
`propostas/` e o resto dentro. O número nunca esteve errado. A frase ao lado
dele é que estava.

A segunda contava **uma pasta só**, e os geradores moram em seis desde que o
PMO, a planilha, o extrator de tecnologias e a página de status nasceram.

## 2. O que eu concluí primeiro, e estava errado

Concluí que faltava acrescentar a pasta nova (`docs/status/`) à conta — um
item de manutenção, do tamanho de uma linha. Fui conferir quanto ela pesava e
tropecei no resto: `docs/pmo/`, `docs/planilha/`, `docs/tecnologias/` e
`docs/geradores/` também estavam de fora, e há semanas.

E o diagnóstico seguinte também nasceu errado: «então o número do Markdown
deve estar subcontado do mesmo jeito». Medido, é o oposto — o do Markdown está
**super**-contado em relação ao que o rótulo promete. Dois defeitos de sinais
opostos na mesma tabela, e nenhum dos dois se vê olhando só os números.

## 3. O que a medição disse

| linha da tabela | rótulo prometia | código contava | diferença |
|---|---:|---:|---|
| Markdown | `docs/*.md` no topo: **86** arquivos, **54.813** linhas | `docs/**/*.md`: **353** / **85.013** | rótulo descreve 64% a menos do que é publicado |
| Python dos geradores | `docs/dossie/`: 17 / **5.582** | — | **11.747** linhas existem em `docs/**/*.py`: **52%** do ferramental invisível |

Depois do conserto, as duas linhas dizem a conta que rodam: Markdown
`docs/`, **recursivo**, 353 / 85.021; Python `docs/`, **recursivo**,
29 arquivos / 11.747 linhas.

## 4. A regra

**Rótulo de número é parte do número, e envelhece junto.** Um número certo com
a receita errada ao lado é pior que um número errado: o errado alguém
re-mede e acha; este **audita limpo**, e manda quem confere procurar o defeito
no lugar onde ele não está. Quando mexer num gerador, confira que a frase ao
lado descreve a chamada que roda — inclusive quando o valor não mudou.

## 5. Como está guardado hoje

- As duas linhas estão corrigidas em
  `docs/tecnologias/extrair.py::bloco_outras_linguagens()`, cada uma com o
  comentário do que se media antes e o número medido da diferença.
- **Onde o buraco ficou:** o portão dos geradores compara o `TECNOLOGIAS.md`
  por byte e teria pegado o número mudando — mas **não tem como pegar um
  rótulo que mente**, porque rótulo e número saem do mesmo `f-string` e mudam
  juntos ou não mudam. Este achado saiu de alguém ler a frase e conferir a
  chamada. Não há conferidor para isso, e esta linha diz que não há.

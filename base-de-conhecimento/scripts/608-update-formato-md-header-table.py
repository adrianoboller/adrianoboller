# Update FORMATO.md header table
# 28/08 18:02

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()

velho='''Uma tabela de dados do PhxSql é composta por cinco arquivos físicos que
compartilham o mesmo nome-base — mais um sexto, opcional:

```
cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  +  .log  =  cadastroClientes
                     ( +  .bkp, o espelho, quando ligado )
```

| Arquivo | Papel | Assinatura | Pagina? |
|---|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` | sim |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\\0\\0` | **não** |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\\0\\0` | sim |
| `.memo` | Textos longos | `PHXMEMO\\0` | sim |
| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\\0\\0` | sim |

E um **sexto arquivo opcional**, que só existe quando `espelho` está ligado no
`config.json`:'''

novo='''Uma tabela de dados do PhxSql é composta por sete arquivos físicos que
compartilham o mesmo nome-base — mais um oitavo, opcional:

```
cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
                     ( +  .bkp, o espelho, quando ligado )
```

| Arquivo | Papel | Assinatura | Pagina? | Quem lê |
|---|---|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` | sim | quem tem `ler` |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\\0\\0` | **não** | quem tem `ler` |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\\0\\0` | sim | quem tem `ler` |
| `.memo` | Textos longos | `PHXMEMO\\0` | sim | quem tem `ler` |
| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\\0\\0` | sim | quem tem `diario` |
| `.trash` | Linhas que saíram do `.reg`, inteiras | `PHXTRH\\0\\0` | sim | **só `administrar`** |
| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\\0\\0` | sim | **só `administrar`** |

Os três últimos são **os arquivos do administrador**, e a razão está no que
cada um guarda. O `.trash` guarda o dado que alguém mandou apagar — quem só
tem `ler` perdeu o direito àquela linha no instante em que ela foi excluída, e
a lixeira devolveria o direito por outra porta. O `.reason` costuma ser ainda
mais revelador que o registro: *fraude*, *pedido de remoção do titular*,
*duplicidade com o contrato X*. O `.log` tem permissão própria (`diario`), que
só um administrador concede.

E um **oitavo arquivo opcional**, que só existe quando `espelho` está ligado no
`config.json`:'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

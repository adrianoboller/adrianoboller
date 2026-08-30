# Rerun the concurrent measurement
# 28/08 13:53

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/carga2.mjs')
s = p.read_text()
# O motor numera: sem chance de dois clientes escolherem o mesmo id.
s = s.replace("""    const id = k * 1000000 + i;
    await api('inserir', { database:'loja', tabela:tab, valores:[id, `C${id}`, cid[i%4]] });""",
"""    await api('inserir', { database:'loja', tabela:tab,
      valores:[null, `C${k}-${i}`, cid[i%4]] });""")
s = s.replace("""  colunas:[{nome:'id',tipo:'Int8',obrigatoria:true},{nome:'nome',tipo:'Str(60)'},""",
"""  colunas:[{nome:'id',tipo:'Sequence',obrigatoria:true},{nome:'nome',tipo:'Str(60)'},""")
p.write_text(s)

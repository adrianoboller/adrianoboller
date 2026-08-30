# Wire the checkbox into the create request
# 28/08 17:47

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    r.particionada = $("#nt_particionada").checked;''',
'''    r.particionada = $("#nt_particionada").checked;
    r.motivo_obrigatorio = $("#nt_motivo_obrig").checked;''',1)
s=s.replace('''      indices: r.indices.filter(x => x.nome && x.colunas).map(x => ({
        nome: x.nome, unico: x.unico, primario: x.primario,
        colunas: x.colunas.split(",").map(s => s.trim()).filter(Boolean),
      })),
    };''','''      indices: r.indices.filter(x => x.nome && x.colunas).map(x => ({
        nome: x.nome, unico: x.unico, primario: x.primario,
        colunas: x.colunas.split(",").map(s => s.trim()).filter(Boolean),
      })),
      motivo_obrigatorio: !!r.motivo_obrigatorio,
    };''',1)
io.open(p,'w',encoding='utf-8').write(s)

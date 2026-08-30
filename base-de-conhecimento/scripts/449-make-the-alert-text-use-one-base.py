# Make the alert text use one base
# 28/08 15:04

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''        for e in discos {
            t.push_str(&format!(
                "  {}\\n    montagem  {} ({})\\n    livre     {} MB de {} MB ({:.1}%)\\n\\n",
                e.caminho,
                e.montagem,
                e.dispositivo,
                e.livre_kb / 1_024,
                e.total_kb / 1_024,
                e.livre_percentual()
            ));
        }'''
b='''        for e in discos {
            // O "de" e o ALCANCAVEL, e nao o tamanho do disco: o percentual ao
            // lado ja e sobre ele, e misturar as duas bases daria uma conta que
            // nao fecha para quem le ("45% de 258 GB nao dao 17 GB"). A reserva
            // do sistema de arquivos aparece a parte, quando existe.
            t.push_str(&format!(
                "  {}\\n    montagem  {} ({})\\n    livre     {} MB de {} MB ({:.1}%)\\n",
                e.caminho,
                e.montagem,
                e.dispositivo,
                e.livre_kb / 1_024,
                e.utilizavel_kb() / 1_024,
                e.livre_percentual()
            ));
            if e.reservado_kb() > 0 {
                t.push_str(&format!(
                    "    reserva   {} MB do sistema de arquivos, fora do alcance\\n",
                    e.reservado_kb() / 1_024
                ));
            }
            t.push('\\n');
        }'''
assert a in s; s=s.replace(a,b,1)
a='''                "DISCO APERTADO: {} ({}) -- {:.1}% livre, {} MB",'''
b='''                "DISCO APERTADO: {} ({}) -- {:.1}% livre, {} MB de espaco",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

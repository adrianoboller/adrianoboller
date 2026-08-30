# Tokenize the entry gradient and audit literal colors
# 27/08 20:53

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
s=s.replace('  --sombra-fenix:0 6px 22px rgba(255,77,16,.28);',
            '  --sombra-fenix:0 6px 22px rgba(255,77,16,.28);\n  --brilho-entrada:#0d1830;')
s=s.replace('  --sombra-fenix:0 6px 18px rgba(198,60,10,.18);',
            '  --sombra-fenix:0 6px 18px rgba(198,60,10,.18);\n  --brilho-entrada:#efe7dd;')
s=s.replace('background:radial-gradient(1200px 600px at 50% -10%,#0d1830 0%,var(--fundo) 62%);',
            'background:radial-gradient(1200px 600px at 50% -10%,var(--brilho-entrada) 0%,var(--fundo) 62%);')
open(p,'w').write(s)

# Satisfy clippy and re-verify
# 27/08 21:22

p='crates/phxsql-core/src/zip.rs'
s=open(p).read()
s=s.replace('''            let fim = (i + melhor_tam).min(dados.len());
            for k in (i + 1)..fim {
                if k + CASAMENTO_MIN <= dados.len() {
                    let h = dispersar(dados, k);
                    let ligacao = cabeca[h];
                    anterior[k] = ligacao;
                    cabeca[h] = k;
                }
            }''','''            let fim = (i + melhor_tam).min(dados.len());
            for (n, ligar) in anterior[i + 1..fim].iter_mut().enumerate() {
                let k = i + 1 + n;
                if k + CASAMENTO_MIN <= dados.len() {
                    let h = dispersar(dados, k);
                    *ligar = cabeca[h];
                    cabeca[h] = k;
                }
            }''')
open(p,'w').write(s)

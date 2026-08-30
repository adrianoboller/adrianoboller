# Fix clippy warning and run all tests
# 27/08 21:21

p='crates/phxsql-core/src/zip.rs'
s=open(p).read()
s=s.replace('''            // Registra as posicoes puladas, senao o casamento seguinte nao acha.
            for k in (i + 1)..(i + melhor_tam).min(dados.len()) {
                if k + CASAMENTO_MIN <= dados.len() {
                    let h = dispersar(dados, k);
                    anterior[k] = cabeca[h];
                    cabeca[h] = k;
                }
            }''','''            // Registra as posicoes puladas, senao o casamento seguinte nao acha.
            let fim = (i + melhor_tam).min(dados.len());
            for k in (i + 1)..fim {
                if k + CASAMENTO_MIN <= dados.len() {
                    let h = dispersar(dados, k);
                    let ligacao = cabeca[h];
                    anterior[k] = ligacao;
                    cabeca[h] = k;
                }
            }''')
open(p,'w').write(s)

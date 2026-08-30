# Extract and test the retention rule
# 27/08 21:17

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho = s[s.index('        let mut nossos: Vec<std::path::PathBuf> = dir'):s.index('        apagados\n    }')]
novo = '''        let nomes: Vec<String> = dir
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();
        let mut apagados = 0;
        for nome in phxsql_store::backup::escolher_para_apagar(&nomes, b.manter) {
            if std::fs::remove_file(b.destino.join(&nome)).is_ok() {
                apagados += 1;
            }
        }
'''
s = s.replace(velho, novo)
open(p,'w').write(s)

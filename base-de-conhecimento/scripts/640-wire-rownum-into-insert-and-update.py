# Wire rownum into insert and update
# 28/08 18:27

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

# insercao: numera depois de completar
velho='''        let completos;
        let valores = match self.completar(valores, None) {
            Some(v) => {
                completos = v;
                &completos[..]
            }
            None => valores,
        };

        // A sequencia entra ANTES das chaves'''
novo='''        // Numerar ANTES das chaves, pela mesma razao da sequencia: se a coluna
        // estiver num indice, a chave tem de ser a do numero gravado.
        let mut completos = match self.completar(valores, None) {
            Some(v) => v,
            None => valores.to_vec(),
        };
        self.numerar_linha(&mut completos, None);
        let valores = &completos[..];

        // A sequencia entra ANTES das chaves'''
assert velho in s
s=s.replace(velho,novo,1)

# alteracao: mantem o numero
velho2='''        let completos;
        let valores = match self.completar(valores, Some(&valores_antigos)) {
            Some(v) => {
                completos = v;
                &completos[..]
            }
            None => valores,
        };

        // Nulo na coluna de sequencia guarda o numero que a linha ja tinha.'''
novo2='''        let mut completos = match self.completar(valores, Some(&valores_antigos)) {
            Some(v) => v,
            None => valores.to_vec(),
        };
        self.numerar_linha(&mut completos, Some(&valores_antigos));
        let valores = &completos[..];

        // Nulo na coluna de sequencia guarda o numero que a linha ja tinha.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)

# Fix fixture and guard the filter
# 28/08 17:35

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()

# 1. As linhas do cenario passam a ter a coluna de sistema, como o esquema diz.
velho='''        let la = vec![
            vec![Value::Int(1), txt("Adriano")],
            vec![Value::Int(2), txt("Maria")],
            vec![Value::Int(3), txt("João")],
        ];'''
novo='''        // O `false` no fim é a coluna de sistema: `Schema::new` a acrescenta,
        // e a linha tem de bater com o esquema.
        let la = vec![
            vec![Value::Int(1), txt("Adriano"), Value::Bool(false)],
            vec![Value::Int(2), txt("Maria"), Value::Bool(false)],
            vec![Value::Int(3), txt("João"), Value::Bool(false)],
        ];'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        let lb = vec![
            vec![Value::Int(1), Value::Int(100)],
            vec![Value::Int(1), Value::Int(200)],
            vec![Value::Int(2), Value::Int(300)],
            vec![Value::Int(9), Value::Int(400)],
        ];'''
novo2='''        let lb = vec![
            vec![Value::Int(1), Value::Int(100), Value::Bool(false)],
            vec![Value::Int(1), Value::Int(200), Value::Bool(false)],
            vec![Value::Int(2), Value::Int(300), Value::Bool(false)],
            vec![Value::Int(9), Value::Int(400), Value::Bool(false)],
        ];'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# 2. O filtro so mexe na linha que bate com o cabecalho.
velho3='''    if manter.iter().all(|m| *m) {
        return;
    }
    for linha in &mut r.linhas {
        let mut i = 0;
        linha.retain(|_| {
            let fica = manter.get(i).copied().unwrap_or(true);
            i += 1;
            fica
        });
    }'''
novo3='''    if manter.iter().all(|m| *m) {
        return;
    }
    for linha in &mut r.linhas {
        // Linha que nao bate com o cabecalho fica intacta: cortar por posicao
        // numa linha de outro tamanho e como se tira a coluna errada, e sem
        // aviso nenhum. Se acontecer, o defeito e antes daqui.
        if linha.len() != manter.len() {
            continue;
        }
        let mut i = 0;
        linha.retain(|_| {
            let fica = manter[i];
            i += 1;
            fica
        });
    }'''
assert velho3 in s
s=s.replace(velho3,novo3,1)
io.open(p,'w',encoding='utf-8').write(s)

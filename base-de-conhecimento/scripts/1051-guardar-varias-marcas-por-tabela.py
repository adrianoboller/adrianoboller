# Guardar varias marcas por tabela
# 29/08 03:50

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''    /// E so uma DICA: perde-la custa uma varredura, e uma errada faz o CRC do
    /// evento recusar. Por isso ela nao vai a disco e nao precisa de limpeza --
    /// e um mapa pequeno, uma entrada por tabela replicada.
    marcas_do_diario: Mutex<HashMap<String, phxsql_store::log::MarcaDoDiario>>,'''
novo = '''    /// E so uma DICA: perde-la custa uma varredura, e uma errada faz o CRC do
    /// evento recusar. Por isso ela nao vai a disco.
    ///
    /// # Por que uma LISTA, e nao uma marca por tabela
    ///
    /// Um source serve varias replicas, e elas nao estao na mesma posicao --
    /// uma que ficou fora do ar volta atras das outras. Com uma marca so, a
    /// que estivesse mais adiantada a moveria para frente e as outras nunca a
    /// aproveitariam: a marca so serve para uma posicao DEPOIS dela. Guardar
    /// algumas e escolher a maior que ainda cabe atende todas.
    marcas_do_diario: Mutex<HashMap<String, Vec<phxsql_store::log::MarcaDoDiario>>>,'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2 = '''        if let Ok(m) = self.marcas_do_diario.lock() {
            t.definir_marca_do_diario(m.get(&chave).copied());
        }
        let eventos = t.diario_com_imagem(desde, max)?;
        if let (Ok(mut m), Some(nova)) = (self.marcas_do_diario.lock(), t.marca_do_diario()) {
            m.insert(chave, nova);
        }'''
novo2 = '''        if let Ok(m) = self.marcas_do_diario.lock() {
            // A maior que ainda cabe: a marca so serve para uma posicao depois
            // dela.
            t.definir_marca_do_diario(
                m.get(&chave)
                    .and_then(|v| v.iter().filter(|k| k.evento <= desde).max_by_key(|k| k.evento))
                    .copied(),
            );
        }
        let eventos = t.diario_com_imagem(desde, max)?;
        if let (Ok(mut m), Some(nova)) = (self.marcas_do_diario.lock(), t.marca_do_diario()) {
            let v = m.entry(chave).or_default();
            // A que esta replica acabou de usar sai: ela nao volta atras.
            v.retain(|k| k.evento != desde && k.evento != nova.evento);
            v.push(nova);
            // Teto pequeno: sao dicas, e a mais antiga e a menos util.
            if v.len() > MARCAS_POR_TABELA {
                v.sort_unstable_by_key(|k| k.evento);
                v.remove(0);
            }
        }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# a constante, junto das outras do modulo
import re
m=re.search(r'\nstruct Servidor \{|\npub struct Servidor \{', s)
assert m
s = s[:m.start()] + '''
/// Quantas dicas de posicao do diario guardar por tabela.
///
/// Uma por replica que puxa dela, mais folga. Oito cobre a topologia que a
/// bancada monta (tres) com sobra, e o custo de cada uma e 20 bytes.
const MARCAS_POR_TABELA: usize = 8;
''' + s[m.start():]
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

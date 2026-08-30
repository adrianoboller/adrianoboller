# Fix the patch anchor and apply
# 28/08 13:47

import pathlib, re
p = pathlib.Path('/tmp/recursos.py'); s = p.read_text()
v = '''v = \'\'\'    /// Conexoes simultaneas aceitas.
    pub conexoes_max: usize,\'\'\''''
n = '''v = \'\'\'    /// Conexoes simultaneas aceitas.
    pub conexoes_max: usize,
    /// Segundos de espera por um pedido antes de encerrar a conexao.
    pub timeout_s: u64,\'\'\''''
assert s.count(v) == 1, 'marca'
s = s.replace(v, n)
v2 = '''n = \'\'\'    /// Conexoes simultaneas aceitas.
    ///
    /// Espelha `recursos.conexoes_max`; fica aqui porque `config.json` antigo
    /// traz o campo no topo e nao pode parar de subir.
    pub conexoes_max: usize,
    /// O que o servidor pode consumir da maquina, e quando grava de verdade.
    pub recursos: Recursos,\'\'\''''
n2 = '''n = \'\'\'    /// Conexoes simultaneas aceitas.
    ///
    /// Espelha `recursos.conexoes_max`; fica aqui porque `config.json` antigo
    /// traz o campo no topo e nao pode parar de subir.
    pub conexoes_max: usize,
    /// O que o servidor pode consumir da maquina, e quando grava de verdade.
    pub recursos: Recursos,
    /// Segundos de espera por um pedido antes de encerrar a conexao.
    pub timeout_s: u64,\'\'\''''
assert s.count(v2) == 1, 'marca2'
p.write_text(s.replace(v2, n2))
print('ok')

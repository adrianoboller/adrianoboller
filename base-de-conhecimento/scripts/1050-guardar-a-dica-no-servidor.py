# Guardar a dica no servidor
# 29/08 03:46

import io,re
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

# 1. campo
velho = '''    /// Tabelas reservadas para carga (`BULKINSERT`).
    cargas: Mutex<crate::carga::Cargas>,'''
novo = '''    /// Tabelas reservadas para carga (`BULKINSERT`).
    cargas: Mutex<crate::carga::Cargas>,
    /// Onde a ultima leitura do diario de cada tabela parou, por
    /// `database/tabela`.
    ///
    /// # Por que aqui, e nao na tabela
    ///
    /// A tabela e aberta e fechada a cada pedido, entao a marca morreria entre
    /// um `replicar` e o seguinte -- que sao exatamente os dois pedidos em que
    /// ela vale. Sem ela, servir «500 eventos a partir de P» caminha pelos P
    /// anteriores lendo o cabecalho de cada um, e alcancar N eventos custa
    /// N^2/2 leituras.
    ///
    /// Medido em `--example custo-do-desde`, num diario de 100.000 eventos:
    /// ler 500 a partir de 0 custa 1,11 us por evento; a partir de 90.000,
    /// 72,65. Alcancar os 100.000 de 500 em 500 gastava 4,07 s so aqui.
    ///
    /// E so uma DICA: perde-la custa uma varredura, e uma errada faz o CRC do
    /// evento recusar. Por isso ela nao vai a disco e nao precisa de limpeza --
    /// e um mapa pequeno, uma entrada por tabela replicada.
    marcas_do_diario: Mutex<HashMap<String, phxsql_store::log::MarcaDoDiario>>,'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 2. construtor
m = re.search(r'(\n            cargas: Mutex::new\([^\n]*\),\n)', s)
assert m, 'construtor'
s = s[:m.end(1)] + '            marcas_do_diario: Mutex::new(HashMap::new()),\n' + s[m.end(1):]

# 3. op_replicar usa a dica
velho3 = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let total = t.eventos()?;
        let eventos = t.diario_com_imagem(desde, max)?;
        let lidos = eventos.len() as u64;'''
novo3 = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let total = t.eventos()?;

        // A dica de onde a leitura anterior desta tabela parou. Sem ela, o
        // `desde` faz o diario ser varrido desde o comeco a cada lote -- ver
        // `marcas_do_diario`.
        let chave = format!(
            "{}/{}",
            p.texto_ou("database", "").to_lowercase(),
            p.texto_ou("tabela", "").to_lowercase()
        );
        if let Ok(m) = self.marcas_do_diario.lock() {
            t.definir_marca_do_diario(m.get(&chave).copied());
        }
        let eventos = t.diario_com_imagem(desde, max)?;
        if let (Ok(mut m), Some(nova)) = (self.marcas_do_diario.lock(), t.marca_do_diario()) {
            m.insert(chave, nova);
        }
        let lidos = eventos.len() as u64;'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

# Ler so o cabecalho e o esquema
# 29/08 04:25

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''        let primeiro = achar_primeiro_volume(diretorio.as_ref(), nome, EXT_REG)?;
        let nome_arq = primeiro.display().to_string();
        let bruto = std::fs::read(&primeiro)?;
        if bruto.len() < CAB_LEN {
            return Err(PhxError::Corrompido(format!("{nome_arq} truncado")));
        }
        let mut cab = [0u8; CAB_LEN];
        cab.copy_from_slice(&bruto[..CAB_LEN]);
        conferir_magic(&nome_arq, MAGIC_REG, &cab[0..8])?;'''
novo = '''        let primeiro = achar_primeiro_volume(diretorio.as_ref(), nome, EXT_REG)?;
        let nome_arq = primeiro.display().to_string();
        // Duas leituras curtas, e NAO o arquivo inteiro.
        //
        // Aqui havia um `std::fs::read`, que trazia o volume inteiro para a
        // RAM para tirar dele 128 bytes de cabecalho e o bloco de esquema. Numa
        // tabela sem paginacao esse volume e a tabela toda: abrir custava
        // **69 ms por milhao de linhas** (`--example abrir-cresce`), e o
        // servidor abre a tabela a cada pedido.
        let mut arquivo = std::fs::File::open(&primeiro)?;
        let tamanho = arquivo.metadata()?.len();
        if tamanho < CAB_LEN as u64 {
            return Err(PhxError::Corrompido(format!("{nome_arq} truncado")));
        }
        let mut cab = [0u8; CAB_LEN];
        ler_exato(&mut arquivo, 0, &mut cab)?;
        conferir_magic(&nome_arq, MAGIC_REG, &cab[0..8])?;'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2 = '''        if bruto.len() < CAB_LEN + schema_len {
            return Err(PhxError::Corrompido(format!(
                "{nome_arq} nao contem o esquema inteiro"
            )));
        }
        let bytes_esquema = bruto[CAB_LEN..CAB_LEN + schema_len].to_vec();'''
novo2 = '''        if tamanho < (CAB_LEN + schema_len) as u64 {
            return Err(PhxError::Corrompido(format!(
                "{nome_arq} nao contem o esquema inteiro"
            )));
        }
        let mut bytes_esquema = vec![0u8; schema_len];
        ler_exato(&mut arquivo, CAB_LEN as u64, &mut bytes_esquema)?;'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

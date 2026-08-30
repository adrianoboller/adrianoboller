# Acrescentar a marca ao LogFile
# 29/08 03:45

import io
p='crates/phxsql-store/src/log.rs'
s=io.open(p,encoding='utf-8').read()

# 1. o tipo da marca, junto do LogFile
velho = '''pub struct LogFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Usuario aplicado aos eventos gravados daqui em diante.
    pub usuario: u32,
}'''
novo = '''/// Onde um evento comeca no arquivo.
///
/// # Por que ela existe
///
/// Desde que o evento deixou de ter largura fixa, chegar ao evento N e caminhar
/// pelos N-1 anteriores lendo o cabecalho de cada um. Para quem le UMA vez isso
/// e o preco justo. Para quem le em lotes seguidos -- que e exatamente o que a
/// replicacao faz, «me de 500 a partir de P», com P andando de 500 em 500 --
/// custa N^2/2 leituras de cabecalho no total.
///
/// Medido em `--example custo-do-desde`, num diario de 100.000: ler 500 a
/// partir de 0 custa 1,11 us por evento, e a partir de 90.000 custa **72,65**.
/// Alcancar os 100.000 de 500 em 500 gastava **4,07 s so do lado de quem
/// serve** -- e era isso, e nao o que a replica aplica, que fazia a replicacao
/// parecer lenta.
///
/// A marca e uma **dica**, e nao uma verdade: uma errada faz ler menos, nunca
/// ler lixo, porque o evento continua sendo conferido pelo CRC dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcaDoDiario {
    /// Numero do evento que comeca aqui, contando de zero.
    pub evento: u64,
    pub volume: u32,
    pub offset: u64,
}

pub struct LogFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Ate onde a ultima varredura chegou, para a proxima nao recomecar.
    marca: Option<MarcaDoDiario>,
    /// Usuario aplicado aos eventos gravados daqui em diante.
    pub usuario: u32,
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 2. os dois construtores ganham o campo
s=s.replace('''            volumes: Volumes::novo(diretorio, nome, EXT_LOG, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            usuario: 0,''','''            volumes: Volumes::novo(diretorio, nome, EXT_LOG, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            marca: None,
            usuario: 0,''')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

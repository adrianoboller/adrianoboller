# Fix borrow and retest
# 28/08 17:29

import io
p='crates/phxsql-store/src/lixeira.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''        por_u32(&mut buf, OFF_CRC, crc32_do(&buf));
        Ok(buf)''','''        let crc = crc32_do(&buf);
        por_u32(&mut buf, OFF_CRC, crc);
        Ok(buf)''',1)
io.open(p,'w',encoding='utf-8').write(s)

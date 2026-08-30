# Add path import and permission
# 28/08 14:24

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='use std::net::{SocketAddr, TcpListener, TcpStream};'
b='use std::net::{SocketAddr, TcpListener, TcpStream};\nuse std::path::{Path, PathBuf};'
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

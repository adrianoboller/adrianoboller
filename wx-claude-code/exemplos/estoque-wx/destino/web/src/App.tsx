// WIN_Venda (estoque-interfaces.pdf p.1) convertida para React 19: os mesmos controles, na mesma
// ordem, com as mesmas regras de tela — e as regras de NEGOCIO ficam no servidor (estoque-rs),
// que e quem grava: a tela so as espelha para o usuario ver antes de fechar.
import { useEffect, useState } from 'react'
import './App.css'

const API = (import.meta as any).env?.VITE_API ?? 'http://localhost:8081'
type Cliente = { idcliente: number; nome: string; tipo: string; limite_credito: number }
type Produto = { idproduto: number; codigo: string; descricao: string; preco_venda: number }
type Deposito = { iddeposito: number; nome: string }
type Item = { idproduto: number; codigo: string; descricao: string; quantidade: number; preco: number; total: number }
const moeda = (v: number) => v.toLocaleString('pt-BR', { minimumFractionDigits: 2, maximumFractionDigits: 2 })

export default function App() {
  const [clientes, setClientes] = useState<Cliente[]>([])
  const [produtos, setProdutos] = useState<Produto[]>([])
  const [depositos, setDepositos] = useState<Deposito[]>([])
  const [idcliente, setIdcliente] = useState(0)
  const [iddeposito, setIddeposito] = useState(1)          // COMBO_Deposito: padrao deposito 1
  const [data] = useState(new Date().toISOString().slice(0, 10)) // EDT_Data: padrao hoje
  const [idproduto, setIdproduto] = useState(0)
  const [quantidade, setQuantidade] = useState('')
  const [itens, setItens] = useState<Item[]>([])
  const [perc, setPerc] = useState('0')
  const [desconto, setDesconto] = useState(0)
  const [parcelas, setParcelas] = useState(1)
  const [erro, setErro] = useState('')
  const [aviso, setAviso] = useState('')
  const [fechada, setFechada] = useState('')
  const [pedeConfirmacao, setPedeConfirmacao] = useState(false)

  useEffect(() => {
    fetch(`${API}/clientes`).then(r => r.json()).then(setClientes)
    fetch(`${API}/produtos`).then(r => r.json()).then(setProdutos)
    fetch(`${API}/depositos`).then(r => r.json()).then(setDepositos)
  }, [])

  const cliente = clientes.find(c => c.idcliente === idcliente)
  const subtotal = Math.round(itens.reduce((s, i) => s + i.total, 0) * 100) / 100
  const total = Math.round((subtotal - desconto) * 100) / 100

  // RecalculaTotais (estoque-codigo.pdf p.3): o desconto e calculado pelo servidor (BR-001);
  // acima do teto o legado zera o percentual e avisa — o mesmo aqui.
  async function recalcula(sub: number, p: string) {
    if (!cliente || sub === 0) { setDesconto(0); return }
    const r = await fetch(`${API}/desconto`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ subtotal: sub, percentual: Number(p) || 0, tipo: cliente.tipo }) })
    const j = await r.json()
    if (!r.ok) { setAviso(j.erro); setPerc('0'); setDesconto(0); return }
    setAviso(''); setDesconto(j.desconto)
  }

  // Clique em BTN_AdicionarItem (p.3): valida estoque ANTES de incluir a linha
  async function adicionar() {
    setErro(''); setFechada('')
    const q = Number(quantidade)
    if (!idproduto) { setErro('Escolha um produto.'); return }
    if (!(q > 0)) { setErro('Quantidade deve ser maior que zero.'); return }
    const r = await fetch(`${API}/estoque/valida`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ idproduto, iddeposito, quantidade: q }) })
    if (!r.ok) { setErro((await r.json()).erro); document.getElementById('qtd')?.focus(); return }  // ReturnToCapture(EDT_Quantidade)
    const p = produtos.find(x => x.idproduto === idproduto)!
    const novo: Item = { idproduto, codigo: p.codigo, descricao: p.descricao, quantidade: q, preco: p.preco_venda, total: Math.round(q * p.preco_venda * 100) / 100 }
    const lista = [...itens, novo]
    setItens(lista); setQuantidade('')
    await recalcula(Math.round(lista.reduce((s, i) => s + i.total, 0) * 100) / 100, perc)
  }

  // Duplo clique remove a linha (interfaces p.1)
  async function remover(i: number) {
    const lista = itens.filter((_, k) => k !== i)
    setItens(lista)
    await recalcula(Math.round(lista.reduce((s, x) => s + x.total, 0) * 100) / 100, perc)
  }

  // Clique em BTN_Fechar (p.3-4); BR-007: acima do limite pergunta Sim/Nao so para cliente comum
  async function fechar(confirmou = false) {
    setErro('')
    const r = await fetch(`${API}/vendas`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ idcliente, iddeposito, vendedor: 'carlos.m', itens: itens.map(i => ({ idproduto: i.idproduto, quantidade: i.quantidade })), perc_desconto: Number(perc) || 0, parcelas, confirmou_limite: confirmou }) })
    const j = await r.json()
    if (r.status === 409) { setPedeConfirmacao(true); return }
    if (!r.ok) { setErro(j.erro); return }
    setPedeConfirmacao(false); setFechada(j.mensagem + ' Parcelas: ' + j.parcelas.map(moeda).join(', '))
    setItens([]); setDesconto(0); setPerc('0')
  }

  return (
    <main>
      <header><h1>Nova venda</h1><span className="sub">WIN_Venda · convertida do WINDEV 2025</span></header>
      <div className="bloco cabecalho">
        <label>Cliente
          <select value={idcliente} onChange={e => { setIdcliente(Number(e.target.value)); setDesconto(0) }}>
            <option value={0}>— escolha —</option>
            {clientes.map(c => <option key={c.idcliente} value={c.idcliente}>{c.nome}</option>)}
          </select></label>
        <label>Depósito
          <select value={iddeposito} onChange={e => setIddeposito(Number(e.target.value))}>
            {depositos.map(d => <option key={d.iddeposito} value={d.iddeposito}>{d.nome}</option>)}
          </select></label>
        <label>Data<input type="date" value={data} readOnly /></label>
        {cliente && <p className="info" id="stc-cliente">Tipo: {cliente.tipo === 'E' ? 'Especial' : 'Comum'} · Limite de crédito: {moeda(cliente.limite_credito)}</p>}
      </div>
      <div className="bloco item">
        <label>Produto
          <select value={idproduto} onChange={e => setIdproduto(Number(e.target.value))}>
            <option value={0}>— escolha —</option>
            {produtos.map(p => <option key={p.idproduto} value={p.idproduto}>{p.codigo} — {p.descricao}</option>)}
          </select></label>
        <label>Quantidade<input id="qtd" inputMode="decimal" value={quantidade} onChange={e => setQuantidade(e.target.value)} /></label>
        <button className="incluir" onClick={adicionar}>Adicionar</button>
        {erro && <p className="erro">{erro}</p>}
      </div>
      <table>
        <thead><tr><th>Código</th><th>Descrição</th><th className="num">Quantidade</th><th className="num">Preço</th><th className="num">Total</th></tr></thead>
        <tbody>
          {itens.map((i, k) => <tr key={k} onDoubleClick={() => remover(k)} title="duplo clique remove">
            <td>{i.codigo}</td><td>{i.descricao}</td><td className="num">{i.quantidade.toLocaleString('pt-BR', { minimumFractionDigits: 3 })}</td><td className="num">{moeda(i.preco)}</td><td className="num">{moeda(i.total)}</td></tr>)}
          {itens.length === 0 && <tr><td colSpan={5} className="vazio">Nenhum item.</td></tr>}
        </tbody>
      </table>
      <div className="bloco totais">
        <label>Desconto %<input id="perc" inputMode="decimal" value={perc} onChange={e => setPerc(e.target.value)} onBlur={e => recalcula(subtotal, e.target.value)} /></label>
        <label>Subtotal<input readOnly value={moeda(subtotal)} /></label>
        <label>Desconto<input readOnly value={moeda(desconto)} /></label>
        <label>Total<input id="total" readOnly value={moeda(total)} /></label>
        <label>Parcelas<select value={parcelas} onChange={e => setParcelas(Number(e.target.value))}>{[1, 2, 3, 4, 5, 6].map(n => <option key={n} value={n}>{n}</option>)}</select></label>
        {aviso && <p className="aviso">{aviso}</p>}
        {pedeConfirmacao && <p className="aviso" id="confirma">Total acima do limite de crédito do cliente. Fechar mesmo assim?
          <button className="fechar" onClick={() => fechar(true)} style={{ marginLeft: 8 }}>Sim</button>
          <button className="cancelar" onClick={() => setPedeConfirmacao(false)} style={{ marginLeft: 6 }}>Não</button></p>}
        {fechada && <p className="ok">{fechada}</p>}
      </div>
      <div className="rodape">
        <button className="cancelar" onClick={() => { if (itens.length === 0 || confirm('Descartar os itens?')) { setItens([]); setDesconto(0) } }}>Cancelar</button>
        {/* Estado "vazia": botao Fechar desabilitado (interfaces p.1) */}
        <button className="fechar" disabled={itens.length === 0 || !idcliente} onClick={() => fechar()}>Fechar venda</button>
      </div>
    </main>
  )
}

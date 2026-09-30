-- ERP de vendas
CREATE TABLE cliente (
  id SERIAL PRIMARY KEY,
  razao_social VARCHAR(120) NOT NULL,
  cnpj VARCHAR(18) NOT NULL UNIQUE,
  email VARCHAR(120),
  telefone VARCHAR(20),
  ativo BOOLEAN NOT NULL DEFAULT true,
  observacao TEXT,
  criado_em TIMESTAMP NOT NULL DEFAULT now()
);
CREATE TABLE produto (
  id SERIAL PRIMARY KEY,
  descricao VARCHAR(200) NOT NULL,
  preco NUMERIC(12,2) NOT NULL,
  peso_kg NUMERIC(10,3)
);
CREATE TABLE pedido (
  id SERIAL PRIMARY KEY,
  cliente_id INTEGER NOT NULL REFERENCES cliente(id),
  dt_emissao DATE NOT NULL,
  situacao VARCHAR(12) NOT NULL CHECK (situacao IN ('aberto','aprovado','faturado','cancelado')),
  vl_frete NUMERIC(12,2),
  obs TEXT
);
CREATE TABLE pedido_item (
  id SERIAL,
  pedido_id INTEGER NOT NULL,
  produto_id INTEGER NOT NULL,
  quantidade NUMERIC(12,3) NOT NULL,
  preco_unitario NUMERIC(12,2) NOT NULL,
  vl_total NUMERIC(12,2) NOT NULL,
  PRIMARY KEY (id),
  CONSTRAINT fk_ped FOREIGN KEY (pedido_id) REFERENCES pedido (id),
  FOREIGN KEY (produto_id) REFERENCES produto(id)
);
CREATE INDEX ix_pedido_cliente ON pedido(cliente_id);

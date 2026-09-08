-- Dados de amostra, sinteticos. Nenhum dado real de pessoa.
INSERT INTO deposito (iddeposito, nome) VALUES (1, 'Matriz'), (2, 'Filial Norte');
INSERT INTO cliente (idcliente, nome, cpf, email, cidade, uf, tipo, limite_credito, ativo, data_cadastro) VALUES
 (17, 'Maria Aparecida Souza', '52998224725', 'maria.souza@exemplo.com.br', 'Blumenau', 'SC', 'C', 5000.00, TRUE, '2024-02-14'),
 (18, 'Metalurgica Vale Ltda', '11144477735', 'compras@exemplo.com.br', 'Joinville', 'SC', 'E', 50000.00, TRUE, '2023-11-02');
INSERT INTO produto (idproduto, codigo, descricao, unidade, preco_venda, estoque_minimo, ativo) VALUES
 (21, 'PAR-0021', 'Parafuso sextavado 3/8"', 'UN', 0.85, 500.000, TRUE),
 (7,  'CHA-0007', 'Chapa galvanizada 2 mm', 'UN', 148.90, 10.000, TRUE),
 (130,'TIN-0130', 'Tinta esmalte 3,6 l branco', 'UN', 89.50, 12.000, TRUE);
INSERT INTO saldo (idproduto, iddeposito, quantidade) VALUES
 (21, 1, 1200.000), (7, 1, 40.000), (130, 1, 30.000), (7, 2, 12.000), (130, 2, 4.000);
INSERT INTO venda (idvenda, idcliente, iddeposito, data, vendedor, subtotal, desconto, total, status) VALUES
 (1001, 17, 1, '2026-08-05', 'carlos.m', 2493.80, 249.38, 2244.42, 'F'),
 (1002, 18, 1, '2026-08-12', 'carlos.m', 1000.00, 0.00, 1000.00, 'F'),
 (1003, 17, 2, '2026-08-20', 'carlos.m', 537.00, 0.00, 537.00, 'C');
INSERT INTO titulo (idtitulo, idvenda, vencimento, valor, pago_em, valor_pago) VALUES
 (5001, 1001, '2026-09-04', 748.14, NULL, NULL),
 (5002, 1001, '2026-10-04', 748.14, NULL, NULL),
 (5003, 1001, '2026-11-03', 748.14, NULL, NULL),
 (5004, 1002, '2026-09-11', 1000.00, NULL, NULL);

SELECT setval(pg_get_serial_sequence('cliente','idcliente'),(SELECT MAX(idcliente) FROM cliente));
SELECT setval(pg_get_serial_sequence('produto','idproduto'),(SELECT MAX(idproduto) FROM produto));
SELECT setval(pg_get_serial_sequence('deposito','iddeposito'),(SELECT MAX(iddeposito) FROM deposito));
SELECT setval(pg_get_serial_sequence('venda','idvenda'),(SELECT MAX(idvenda) FROM venda));
SELECT setval(pg_get_serial_sequence('itemvenda','iditem'),(SELECT COALESCE(MAX(iditem),1) FROM itemvenda));
SELECT setval(pg_get_serial_sequence('titulo','idtitulo'),(SELECT MAX(idtitulo) FROM titulo));

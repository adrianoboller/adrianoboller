-- Dados sinteticos. Nenhuma pessoa ou empresa real.
INSERT INTO cliente (ID, RAZAO_SOCIAL, CNPJ, LIMITE, FORMA_PADRAO, ATIVO) VALUES
 (1, 'Comercial Aurora Ltda',  '11.222.333/0001-81', 50000.00, 'PZ', 1),
 (2, 'Metalurgica Serra ME',   '11.444.777/0001-61', 20000.00, 'AV', 1),
 (3, 'Distribuidora Vale S.A.','04.252.011/0001-10', 90000.00, 'PZ', 1);

INSERT INTO pedido (ID, ID_CLIENTE, EMISSAO, SITUACAO) VALUES
 (1, 1, '2026-08-10', 'A_FATURAR'),
 (2, 2, '2026-08-12', 'A_FATURAR'),
 (3, 3, '2026-07-02', 'FATURADO');

INSERT INTO pedido_item (ID_PEDIDO, DESCRICAO, QUANTIDADE, VALOR_UNITARIO) VALUES
 (1, 'Chapa aco 2mm',      10.000, 320.50),
 (1, 'Perfil U 3m',         4.000, 187.25),
 (2, 'Tubo galvanizado',   25.000,  96.40),
 (3, 'Barra redonda 1/2',  12.000, 143.00);

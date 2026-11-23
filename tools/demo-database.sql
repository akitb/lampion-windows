-- lampion_demo: the small webshop database the macOS screenshots show, for trying the Database Manager.
-- 8 tables, 1 view, 8 foreign keys (one self-reference), a table without a primary key (settings),
-- Hungarian accents, NULLs, enums, decimals, dates and text.
-- Import: Database Manager › Import SQL…, or
--   C:\Lampion\bin\mariadb\bin\mariadb.exe -h 127.0.0.1 -u root --default-character-set=utf8mb4 < tools\demo-database.sql

CREATE DATABASE IF NOT EXISTS lampion_demo CHARACTER SET utf8mb4 COLLATE utf8mb4_hungarian_ci;
USE lampion_demo;

CREATE TABLE categories (
  id int unsigned NOT NULL AUTO_INCREMENT,
  name varchar(80) NOT NULL,
  parent_id int unsigned DEFAULT NULL,
  PRIMARY KEY (id),
  KEY parent_id (parent_id),
  CONSTRAINT fk_categories_parent FOREIGN KEY (parent_id) REFERENCES categories (id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE suppliers (
  id int unsigned NOT NULL AUTO_INCREMENT,
  name varchar(120) NOT NULL,
  email varchar(160) DEFAULT NULL,
  country char(2) NOT NULL DEFAULT 'HU',
  PRIMARY KEY (id)
) ENGINE=InnoDB;

CREATE TABLE customers (
  id int unsigned NOT NULL AUTO_INCREMENT,
  name varchar(120) NOT NULL,
  email varchar(160) NOT NULL,
  city varchar(80) DEFAULT NULL,
  phone varchar(30) DEFAULT NULL,
  registered_at date NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY email (email)
) ENGINE=InnoDB;

CREATE TABLE products (
  id int unsigned NOT NULL AUTO_INCREMENT,
  sku varchar(20) NOT NULL,
  name varchar(160) NOT NULL,
  category_id int unsigned DEFAULT NULL,
  supplier_id int unsigned DEFAULT NULL,
  price decimal(10,2) NOT NULL,
  stock int NOT NULL DEFAULT 0,
  active tinyint(1) NOT NULL DEFAULT 1,
  created_at datetime NOT NULL DEFAULT current_timestamp(),
  PRIMARY KEY (id),
  UNIQUE KEY sku (sku),
  KEY category_id (category_id),
  KEY supplier_id (supplier_id),
  CONSTRAINT fk_products_category FOREIGN KEY (category_id) REFERENCES categories (id) ON DELETE SET NULL,
  CONSTRAINT fk_products_supplier FOREIGN KEY (supplier_id) REFERENCES suppliers (id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE orders (
  id int unsigned NOT NULL AUTO_INCREMENT,
  customer_id int unsigned NOT NULL,
  status enum('new','paid','shipped','delivered','cancelled') NOT NULL DEFAULT 'new',
  total decimal(10,2) NOT NULL DEFAULT 0.00,
  ordered_at datetime NOT NULL,
  note text DEFAULT NULL,
  PRIMARY KEY (id),
  KEY customer_id (customer_id),
  CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers (id) ON UPDATE CASCADE
) ENGINE=InnoDB;

CREATE TABLE order_items (
  order_id int unsigned NOT NULL,
  product_id int unsigned NOT NULL,
  quantity smallint unsigned NOT NULL DEFAULT 1,
  unit_price decimal(10,2) NOT NULL,
  PRIMARY KEY (order_id, product_id),
  KEY product_id (product_id),
  CONSTRAINT fk_order_items_order FOREIGN KEY (order_id) REFERENCES orders (id) ON DELETE CASCADE,
  CONSTRAINT fk_order_items_product FOREIGN KEY (product_id) REFERENCES products (id)
) ENGINE=InnoDB;

CREATE TABLE reviews (
  id int unsigned NOT NULL AUTO_INCREMENT,
  product_id int unsigned NOT NULL,
  customer_id int unsigned NOT NULL,
  rating tinyint unsigned NOT NULL,
  body text DEFAULT NULL,
  created_at datetime NOT NULL,
  PRIMARY KEY (id),
  KEY product_id (product_id),
  KEY customer_id (customer_id),
  CONSTRAINT fk_reviews_product FOREIGN KEY (product_id) REFERENCES products (id) ON DELETE CASCADE,
  CONSTRAINT fk_reviews_customer FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
) ENGINE=InnoDB;

-- No primary key on purpose: shows how row edits match a key-less table.
CREATE TABLE settings (
  name varchar(60) NOT NULL,
  value varchar(255) DEFAULT NULL
) ENGINE=InnoDB;

INSERT INTO categories (id, name, parent_id) VALUES
  (1, 'Elektronika', NULL), (2, 'Laptopok', 1), (3, 'Telefonok', 1), (4, 'Kiegészítők', 1),
  (5, 'Otthon', NULL), (6, 'Konyha', 5), (7, 'Kert', 5);

INSERT INTO suppliers (id, name, email, country) VALUES
  (1, 'Tech Nagyker Kft.', 'rendeles@technagyker.hu', 'HU'),
  (2, 'Nordic Home AB', 'orders@nordichome.se', 'SE'),
  (3, 'Bavaria Elektro GmbH', 'vertrieb@bavaria-elektro.de', 'DE'),
  (4, 'Kertész és Társa Bt.', NULL, 'HU');

INSERT INTO customers (id, name, email, city, phone, registered_at) VALUES
  (1, 'Kovács Anna', 'anna.kovacs@example.com', 'Budapest', '+36 30 123 4567', '2025-02-14'),
  (2, 'Nagy Péter', 'peter.nagy@example.com', 'Debrecen', NULL, '2025-03-02'),
  (3, 'Szabó Éva', 'eva.szabo@example.com', 'Szeged', '+36 20 555 0101', '2025-04-19'),
  (4, 'Tóth Gábor', 'gabor.toth@example.com', 'Pécs', '+36 70 222 3344', '2025-06-08'),
  (5, 'Horváth Zsófia', 'zsofia.horvath@example.com', 'Győr', NULL, '2025-07-21'),
  (6, 'Varga Bálint', 'balint.varga@example.com', NULL, '+36 30 987 6543', '2025-09-30'),
  (7, 'Kiss Réka', 'reka.kiss@example.com', 'Miskolc', '+36 20 444 1212', '2025-11-11'),
  (8, 'Molnár Ödön', 'odon.molnar@example.com', 'Eger', NULL, '2026-01-05');

INSERT INTO products (id, sku, name, category_id, supplier_id, price, stock, active, created_at) VALUES
  (1, 'LAP-14-PRO', 'Ultrabook 14" Pro', 2, 1, 489990.00, 12, 1, '2025-01-10 09:00:00'),
  (2, 'LAP-16-GAME', 'Gamer laptop 16"', 2, 3, 649990.00, 4, 1, '2025-01-12 10:30:00'),
  (3, 'TEL-A55', 'Okostelefon A55', 3, 1, 159990.00, 30, 1, '2025-02-01 08:15:00'),
  (4, 'TEL-X1-MINI', 'Okostelefon X1 mini', 3, 3, 249990.00, 0, 0, '2025-02-03 14:00:00'),
  (5, 'KIE-USBC-2M', 'USB-C kábel 2 m', 4, 1, 3490.00, 250, 1, '2025-02-20 11:45:00'),
  (6, 'KIE-TOLTO-65', 'Gyorstöltő 65 W', 4, 3, 12990.00, 80, 1, '2025-03-05 16:20:00'),
  (7, 'KON-KAVE-01', 'Kávéfőző, eszpresszó', 6, 2, 89990.00, 15, 1, '2025-03-18 09:40:00'),
  (8, 'KON-KES-SET', 'Késkészlet, 6 darabos', 6, 2, 24990.00, 40, 1, '2025-04-02 13:10:00'),
  (9, 'KON-SERP-28', 'Serpenyő 28 cm', 6, NULL, 9990.00, 60, 1, '2025-04-15 10:05:00'),
  (10, 'KER-LOCS-15', 'Locsolótömlő 15 m', 7, 4, 7490.00, 35, 1, '2025-05-01 07:30:00'),
  (11, 'KER-FUNYIRO', 'Akkus fűnyíró', 7, 4, 119990.00, 6, 1, '2025-05-09 15:55:00'),
  (12, 'OTT-LAMPA-01', 'Papírlámpás, narancs', 5, 2, 5990.00, 120, 1, '2025-06-21 18:00:00');

INSERT INTO orders (id, customer_id, status, total, ordered_at, note) VALUES
  (1, 1, 'delivered', 493480.00, '2025-12-01 10:12:00', NULL),
  (2, 2, 'shipped', 172980.00, '2025-12-03 18:45:00', 'Kérem, csengessenek kétszer.'),
  (3, 3, 'paid', 114980.00, '2025-12-10 08:05:00', NULL),
  (4, 1, 'new', 11980.00, '2025-12-14 21:30:00', 'Ajándékba, számla nélkül.'),
  (5, 4, 'cancelled', 649990.00, '2025-12-18 12:00:00', 'Vevő lemondta.'),
  (6, 5, 'delivered', 127480.00, '2026-01-07 09:20:00', NULL),
  (7, 7, 'paid', 24990.00, '2026-01-15 17:10:00', NULL),
  (8, 8, 'new', 5990.00, '2026-01-20 11:11:00', 'Első rendelés 🎉');

INSERT INTO order_items (order_id, product_id, quantity, unit_price) VALUES
  (1, 1, 1, 489990.00), (1, 5, 1, 3490.00),
  (2, 3, 1, 159990.00), (2, 6, 1, 12990.00),
  (3, 7, 1, 89990.00), (3, 8, 1, 24990.00),
  (4, 12, 2, 5990.00),
  (5, 2, 1, 649990.00),
  (6, 11, 1, 119990.00), (6, 10, 1, 7490.00),
  (7, 8, 1, 24990.00),
  (8, 12, 1, 5990.00);

INSERT INTO reviews (id, product_id, customer_id, rating, body, created_at) VALUES
  (1, 1, 1, 5, 'Gyors, csendes, a kijelző gyönyörű.', '2025-12-09 19:00:00'),
  (2, 3, 2, 4, 'Jó ár-érték arány, az akkumulátor lehetne erősebb.', '2025-12-12 08:30:00'),
  (3, 7, 3, 5, 'Minden reggel ezzel indul a nap. ☕', '2025-12-20 07:45:00'),
  (4, 11, 5, 3, NULL, '2026-01-14 16:00:00'),
  (5, 12, 8, 5, 'Pont olyan, mint a Lampion ikonja!', '2026-01-25 20:20:00');

INSERT INTO settings (name, value) VALUES
  ('shop_name', 'Lampion Demo Bolt'),
  ('currency', 'HUF'),
  ('free_shipping_over', '20000'),
  ('maintenance', NULL);

CREATE VIEW order_totals AS
  SELECT o.id AS order_id,
         c.name AS customer,
         o.status,
         SUM(i.quantity * i.unit_price) AS items_total,
         COUNT(i.product_id) AS line_count
  FROM orders o
  JOIN customers c ON c.id = o.customer_id
  LEFT JOIN order_items i ON i.order_id = o.id
  GROUP BY o.id, c.name, o.status;

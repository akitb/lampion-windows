-- lampion_import_test: a test file for the Database Manager's Import SQL…, and for what comes after it:
-- paging (1500 rows = 3 pages of 500), sorting, WHERE filters, every common column type, a key-less table
-- with near-duplicates, a foreign key for the Designer, a view and a stored procedure (DELIMITER).
-- It only touches the lampion_import_test database; importing it again rebuilds that database.
--
-- Import: Database Manager › Import SQL… (no database selected, or confirm "Import Anyway"), or
--   C:\Lampion\bin\mariadb\bin\mariadb.exe -h 127.0.0.1 -u root --default-character-set=utf8mb4 < tools\import-test.sql

DROP DATABASE IF EXISTS lampion_import_test;
CREATE DATABASE lampion_import_test CHARACTER SET utf8mb4 COLLATE utf8mb4_hungarian_ci;
USE lampion_import_test;

-- Departments and 1500 people: paging, sorting, filtering, a foreign key.
CREATE TABLE departments (
  id int unsigned NOT NULL AUTO_INCREMENT,
  name varchar(60) NOT NULL,
  PRIMARY KEY (id)
) ENGINE=InnoDB;

INSERT INTO departments (name) VALUES ('Fejlesztés'), ('Értékesítés'), ('Pénzügy'), ('Ügyfélszolgálat'), ('Raktár');

CREATE TABLE people (
  id int unsigned NOT NULL AUTO_INCREMENT,
  name varchar(80) NOT NULL,
  email varchar(120) NOT NULL,
  department_id int unsigned DEFAULT NULL,
  born date NOT NULL,
  salary decimal(10,2) NOT NULL,
  active tinyint(1) NOT NULL DEFAULT 1,
  note text DEFAULT NULL,
  created_at datetime NOT NULL DEFAULT current_timestamp(),
  PRIMARY KEY (id),
  UNIQUE KEY email (email),
  KEY department_id (department_id),
  CONSTRAINT fk_people_department FOREIGN KEY (department_id) REFERENCES departments (id) ON DELETE SET NULL
) ENGINE=InnoDB;

-- seq_1_to_1500 is MariaDB's built-in Sequence engine: one row per number.
INSERT INTO people (name, email, department_id, born, salary, active, note)
SELECT CONCAT(ELT(1 + seq % 10, 'Kovács', 'Nagy', 'Szabó', 'Tóth', 'Horváth', 'Varga', 'Kiss', 'Molnár', 'Németh', 'Farkas'), ' ',
              ELT(1 + (seq DIV 10) % 10, 'Anna', 'Péter', 'Éva', 'Gábor', 'Zsófia', 'Bálint', 'Réka', 'Ödön', 'Lili', 'Ákos')),
       CONCAT('user', seq, '@example.com'),
       IF(seq % 17 = 0, NULL, 1 + seq % 5),
       DATE_ADD('1960-01-01', INTERVAL (seq * 9973) % 16000 DAY),
       ROUND(300000 + (seq * 7919) % 900000, 2),
       seq % 9 <> 0,
       CASE WHEN seq % 50 = 0 THEN CONCAT('Többsoros megjegyzés', CHAR(10), 'második sor #', seq)
            WHEN seq % 7 = 0 THEN ''
            WHEN seq % 3 = 0 THEN NULL
            ELSE CONCAT('Megjegyzés #', seq) END
FROM seq_1_to_1500;

-- One row per common column type, plus a NULL row and an "empty" row.
CREATE TABLE types_showcase (
  id int unsigned NOT NULL AUTO_INCREMENT,
  label varchar(40) NOT NULL,
  t_tinyint tinyint DEFAULT NULL,
  t_bigint bigint DEFAULT NULL,
  t_decimal decimal(20,6) DEFAULT NULL,
  t_float float DEFAULT NULL,
  t_double double DEFAULT NULL,
  t_bit bit(8) DEFAULT NULL,
  t_date date DEFAULT NULL,
  t_time time DEFAULT NULL,
  t_datetime datetime(3) DEFAULT NULL,
  t_timestamp timestamp NULL DEFAULT NULL,
  t_year year DEFAULT NULL,
  t_char char(5) DEFAULT NULL,
  t_varchar varchar(100) DEFAULT NULL,
  t_text text DEFAULT NULL,
  t_json longtext CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT NULL CHECK (json_valid(t_json)),
  t_enum enum('small','medium','large') DEFAULT NULL,
  t_set set('red','green','blue') DEFAULT NULL,
  t_blob blob DEFAULT NULL,
  t_varbinary varbinary(16) DEFAULT NULL,
  PRIMARY KEY (id)
) ENGINE=InnoDB;

INSERT INTO types_showcase (label, t_tinyint, t_bigint, t_decimal, t_float, t_double, t_bit, t_date, t_time, t_datetime,
                            t_timestamp, t_year, t_char, t_varchar, t_text, t_json, t_enum, t_set, t_blob, t_varbinary) VALUES
  ('values', -128, 9007199254740993, 12345678901234.123456, 3.14159, 2.718281828459045, b'10100101', '2026-11-23',
   '838:59:59', '2026-11-23 23:45:12.345', '2026-11-23 23:45:12', 2026, 'abc', 'Árvíztűrő tükörfúrógép',
   'Hosszabb szöveg\nkét sorban', '{"name": "Lampion", "tags": ["php", "mariadb"], "ok": true}', 'medium', 'red,blue',
   x'89504E470D0A1A0A0000000D', x'00FF10'),
  ('nulls', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
  ('empty', 0, 0, 0, 0, 0, b'0', '1000-01-01', '00:00:00', '1000-01-01 00:00:00.000', NULL, 1901, '', '', '', '{}', 'small', '', '', '');

-- No primary key, and rows that differ only in case, accents or a trailing space: editing one row must change
-- exactly that row (the Database Manager compares them byte by byte).
CREATE TABLE no_key_dupes (
  word varchar(20) NOT NULL,
  weight float DEFAULT NULL
) ENGINE=InnoDB;

INSERT INTO no_key_dupes (word, weight) VALUES
  ('PHP', 1.1), ('php', 1.1), ('kor', 2.5), ('kör', 2.5), ('a', 0.1), ('a ', 0.1), ('same', 9.9), ('same', 9.9);

CREATE VIEW people_by_department AS
  SELECT d.name AS department, COUNT(p.id) AS people, ROUND(AVG(p.salary), 2) AS avg_salary
  FROM departments d LEFT JOIN people p ON p.department_id = d.id
  GROUP BY d.id, d.name;

DELIMITER //
CREATE PROCEDURE raise_salaries(IN dept int unsigned, IN percent decimal(5,2))
BEGIN
  UPDATE people SET salary = ROUND(salary * (1 + percent / 100), 2) WHERE department_id = dept;
  SELECT ROW_COUNT() AS raised;
END //
DELIMITER ;

SELECT 'Import OK' AS result,
       (SELECT COUNT(*) FROM departments) AS departments,
       (SELECT COUNT(*) FROM people) AS people,
       (SELECT COUNT(*) FROM types_showcase) AS types_rows,
       (SELECT COUNT(*) FROM no_key_dupes) AS no_key_rows;

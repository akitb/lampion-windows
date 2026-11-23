//! root without a password from this computer only (TCP 127.0.0.1 / ::1 / localhost), and the
//! ready-made connection settings of the "Connecting from apps" card. Never creates root@'%'.

use crate::core::{activity, settings};
use crate::database::client;
use mysql_async::prelude::Queryable;
use serde::Serialize;
use std::time::Duration;

/// Turning it on (SPEC "root jelszó nélkül"; the ALTERs also clear a password set earlier by Lampion).
const ENABLE: &[&str] = &[
    "CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY ''",
    "ALTER USER 'root'@'127.0.0.1' IDENTIFIED BY ''",
    "GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION",
    "CREATE USER IF NOT EXISTS 'root'@'::1' IDENTIFIED BY ''",
    "ALTER USER 'root'@'::1' IDENTIFIED BY ''",
    "GRANT ALL PRIVILEGES ON *.* TO 'root'@'::1' WITH GRANT OPTION",
    "ALTER USER 'root'@'localhost' IDENTIFIED BY ''",
    "FLUSH PRIVILEGES",
];

/// Turning it off: every local root account gets the password.
fn disable(password: &str) -> Vec<String> {
    let pw = client::literal(password);
    let mut sql: Vec<String> = ["localhost", "127.0.0.1", "::1"]
        .iter()
        .map(|host| format!("ALTER USER IF EXISTS 'root'@'{host}' IDENTIFIED BY {pw}"))
        .collect();
    sql.push("FLUSH PRIVILEGES".into());
    sql
}

/// Some(true) = root logs in with an empty password over TCP; Some(false) = refused (1045/1698);
/// None = unknown (MariaDB not running).
pub async fn root_no_password_state() -> Option<bool> {
    let opts = mysql_async::OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(client::port())
        .user(Some("root"))
        .pass(Some(""))
        .prefer_socket(false);
    match tokio::time::timeout(Duration::from_secs(5), mysql_async::Conn::new(opts)).await {
        Ok(Ok(conn)) => {
            let _ = conn.disconnect().await;
            Some(true)
        }
        Ok(Err(mysql_async::Error::Server(e))) if e.code == 1045 || e.code == 1698 => Some(false),
        _ => None,
    }
}

fn refused(e: String) -> String {
    format!("MariaDB did not accept the change. Is it running?\n\n{e}")
}

/// Turns password-less root on, or off with `password` as the new root password. When turning it on
/// while root has a password the app does not know, `password` is the current one.
/// Never creates root@'%'.
pub async fn set_root_no_password(enabled: bool, password: Option<String>) -> Result<(), String> {
    let port = client::port();
    let password = password.filter(|p| !p.is_empty());
    if enabled {
        if password.is_some() {
            client::configure(port, password);
        }
        for sql in ENABLE {
            client::query(sql).await.map_err(refused)?;
        }
        client::configure(port, None);
        activity::ok("MySQL", "root can now log in without a password, from this computer only");
    } else {
        let pw = password.ok_or("Enter a password for root.")?;
        // Straight on the connection, not through client::query: the SQL console must not show the password.
        let mut conn = client::conn().await.map_err(refused)?;
        for sql in disable(&pw) {
            conn.query_drop(sql).await.map_err(|e| refused(client::err(e)))?;
        }
        drop(conn);
        client::configure(port, Some(pw));
        activity::ok("MySQL", "root without a password is turned off; root now needs its password");
    }
    // The user's choice: first-launch setup never overrides it again.
    settings::update(|s| s.root_no_password = Some(enabled));
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Snippet {
    pub id: &'static str,
    pub title: &'static str,
    pub text: String,
}

/// Ready-made settings for apps (`your_database` when no name is given).
pub fn snippets(database: &str, port: u16) -> Vec<Snippet> {
    let d = match database.trim() {
        "" => "your_database",
        name => name,
    };
    let s = |id, title, text: String| Snippet { id, title, text };
    vec![
        s("pdo", "PHP (PDO)", format!("$pdo = new PDO('mysql:host=127.0.0.1;port={port};dbname={d};charset=utf8mb4', 'root', '');")),
        s("mysqli", "PHP (mysqli)", format!("$db = new mysqli('127.0.0.1', 'root', '', '{d}', {port});")),
        s("laravel", "Laravel (.env)", format!("DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT={port}\nDB_DATABASE={d}\nDB_USERNAME=root\nDB_PASSWORD=")),
        s("prisma", "Prisma (.env)", format!("DATABASE_URL=\"mysql://root@127.0.0.1:{port}/{d}\"")),
        s("node", "Node.js (mysql2)", format!("const db = await mysql.createConnection({{ host: '127.0.0.1', port: {port}, user: 'root', password: '', database: '{d}' }});")),
        s("python", "Python", format!("db = mysql.connector.connect(host=\"127.0.0.1\", port={port}, user=\"root\", password=\"\", database=\"{d}\")")),
        s("jdbc", "Java (JDBC)", format!("jdbc:mariadb://127.0.0.1:{port}/{d}?user=root&password=")),
        s("url", "Connection URL", format!("mysql://root@127.0.0.1:{port}/{d}")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_root_at_any_host() {
        for sql in ENABLE.iter().map(|s| s.to_string()).chain(disable("pw")) {
            assert!(!sql.contains("'%'"), "{sql}");
        }
        assert_eq!(disable("it's"), [
            r"ALTER USER IF EXISTS 'root'@'localhost' IDENTIFIED BY 'it\'s'",
            r"ALTER USER IF EXISTS 'root'@'127.0.0.1' IDENTIFIED BY 'it\'s'",
            r"ALTER USER IF EXISTS 'root'@'::1' IDENTIFIED BY 'it\'s'",
            "FLUSH PRIVILEGES",
        ]);
    }

    #[test]
    fn snippet_texts() {
        let s = snippets("shop", 3307);
        let text = |id: &str| s.iter().find(|x| x.id == id).unwrap().text.clone();
        assert_eq!(text("pdo"), "$pdo = new PDO('mysql:host=127.0.0.1;port=3307;dbname=shop;charset=utf8mb4', 'root', '');");
        assert_eq!(text("mysqli"), "$db = new mysqli('127.0.0.1', 'root', '', 'shop', 3307);");
        assert_eq!(text("laravel"), "DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=3307\nDB_DATABASE=shop\nDB_USERNAME=root\nDB_PASSWORD=");
        assert_eq!(text("prisma"), "DATABASE_URL=\"mysql://root@127.0.0.1:3307/shop\"");
        assert_eq!(text("node"), "const db = await mysql.createConnection({ host: '127.0.0.1', port: 3307, user: 'root', password: '', database: 'shop' });");
        assert_eq!(text("python"), "db = mysql.connector.connect(host=\"127.0.0.1\", port=3307, user=\"root\", password=\"\", database=\"shop\")");
        assert_eq!(text("jdbc"), "jdbc:mariadb://127.0.0.1:3307/shop?user=root&password=");
        assert_eq!(text("url"), "mysql://root@127.0.0.1:3307/shop");
        assert_eq!(s.len(), 8);
        assert!(snippets("  ", 3306)[7].text.ends_with("/your_database"));
    }
}

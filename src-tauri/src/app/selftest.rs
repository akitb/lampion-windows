//! The built-in self-tests (docs/SPEC.md "Elfogadási tesztek"; Panel+SelfTest.swift, DBModel+SelfTest.swift).
//! `--selftest`: updater and log parsing, setup, unchanged and changed config saves (on temp copies,
//! checked by the servers), Stop All, a taken port, Start All, the website, phpMyAdmin, MariaDB, and a
//! 404 and a PHP warning reaching their logs. `--dbtest`: database, table, row, index and foreign-key
//! work in a throwaway database `_lampion_teszt`, and an export. Results go to Lampion Activity
//! (module Panel) and to `%LOCALAPPDATA%\Lampion\selftest.log`; then Lampion quits with exit code 0
//! (passed) or 1 (failed).

use super::updater;
use crate::config::{apache, file, locations, mariadb, php};
use crate::core::activity::{self, Level};
use crate::core::{paths, shell};
use crate::database::client::{self, literal, qualified, ResultSet};
use crate::logs::{parse, tail::Tail, Source};
use crate::servers::{self, Server, State};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

pub fn start(args: &[String]) {
    let database = args.iter().any(|a| a == "--dbtest");
    if !database && !args.iter().any(|a| a == "--selftest") {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await; // the window and the server monitor start first
        let mut t = Report::new(if database { "Database self-test" } else { "Self-test" });
        if database {
            database_checks(&mut t).await;
        } else {
            panel_checks(&mut t).await;
        }
        let code = t.finish();
        if let Some(app) = crate::core::app() {
            app.exit(code);
        }
    });
}

struct Report {
    title: &'static str,
    failures: usize,
    log: Option<std::fs::File>,
}

impl Report {
    fn new(title: &'static str) -> Report {
        let mut r = Report { title, failures: 0, log: std::fs::File::create(paths::local_data().join("selftest.log")).ok() };
        r.write(&format!("Lampion {} {title}, {}", updater::VERSION, chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
        activity::info("Panel", &format!("{title} starting"));
        r
    }

    fn write(&mut self, line: &str) {
        if let Some(f) = &mut self.log {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }

    fn check(&mut self, ok: bool, what: &str) {
        activity::add("Panel", &format!("{} {what}", if ok { "✓" } else { "✗" }), if ok { Level::Ok } else { Level::Error });
        self.write(&format!("{} {what}", if ok { "OK  " } else { "FAIL" }));
        if !ok {
            self.failures += 1;
        }
    }

    /// Why the check before failed.
    fn note(&mut self, text: &str) {
        activity::info("Panel", text);
        self.write(&format!("     {text}"));
    }

    fn finish(&mut self) -> i32 {
        let passed = self.failures == 0;
        let title = self.title.to_uppercase();
        let text = if passed { format!("{title} PASSED") } else { format!("{title} FAILED: {} failures", self.failures) };
        activity::add("Panel", &text, if passed { Level::Ok } else { Level::Error });
        self.write(&text);
        i32::from(!passed)
    }
}

// ---- --selftest ----

async fn panel_checks(t: &mut Report) {
    t.check(
        updater::is_newer("1.10", "1.9") && updater::is_newer("v2.0", "1.9.9") && !updater::is_newer("1.3", "1.3")
            && !updater::is_newer("1.2", "1.3") && updater::is_newer("1.3.1", "1.3"),
        "Updater compares version numbers correctly",
    );
    let sample = r#"{"tag_name":"v9.9","html_url":"https://example.com/r","assets":[{"name":"notes.txt","browser_download_url":"https://example.com/n"},
        {"name":"Lampion-9.9-setup.exe","browser_download_url":"https://example.com/a.exe","digest":"sha256:abc123"}]}"#;
    let release = updater::parse_release(sample);
    t.check(
        release.as_ref().is_some_and(|r| r.version == "9.9" && r.url == "https://example.com/a.exe" && r.sha256 == ["abc123"])
            && updater::parse_release("{}").is_none(),
        "Updater reads the release and its installer from GitHub's answer",
    );
    t.check(updater::verify_digest("aa", &["bb".into()]).is_err(), "Updater refuses a download with the wrong checksum");
    t.check(updater::verify_digest("aa", &[]).is_err(), "Updater refuses an installer the release gives no checksum for");
    t.check(parse::clock("23/Nov/2026:12:06:52 +0100") == Some("12:06:52"), "Logs read the time of an access log line");

    t.check(servers::setup::ready(), "No setup banner when everything is installed");
    let pma = std::fs::read_to_string(paths::pma_config()).unwrap_or_default();
    t.check(has_secret(&pma), "Setup gave phpMyAdmin a 32-byte secret");

    for (ok, what) in blocking(config_checks).await.unwrap_or_default() {
        t.check(ok, &what);
    }

    let _ = servers::stop_all().await;
    let st = status().await;
    t.check(st.as_ref().is_some_and(|s| s.apache.state == State::Stopped && s.mariadb.state == State::Stopped), "Stop All: both servers stopped");

    let port = locations::current().apache_port;
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(taken) => {
            let started = servers::start(Server::Apache).await;
            let running = status().await.is_some_and(|s| s.apache.state == State::Running);
            let named = started.as_ref().err().is_some_and(|e| e.contains("already in use"));
            t.check(!running && named, &format!("With port {port} taken, Apache does not start and says what holds it"));
            drop(taken);
        }
        Err(e) => t.check(false, &format!("Reserving port {port} for the test ({e})")),
    }

    if let Err(e) = servers::start_all().await {
        t.note(&e);
    }
    let st = status().await;
    t.check(st.as_ref().is_some_and(|s| s.apache.state == State::Running), "Apache started");
    t.check(st.as_ref().is_some_and(|s| s.mariadb.state == State::Running), "MariaDB started");

    let base = format!("http://localhost:{port}");
    let (code, _) = http(&format!("{base}/")).await;
    t.check(code == Some(200), &format!("Website responds ({base}/)"));
    let (code, page) = http(&format!("{base}/phpmyadmin/")).await;
    t.check(code == Some(200), "phpMyAdmin responds");
    t.check(code == Some(200) && !page.contains("pma_username"), "phpMyAdmin opens without a login page");
    let one = client::query("SELECT 1").await;
    t.check(cell(&one, 0, 0).as_deref() == Some("1"), "MariaDB answers root without a password");

    log_checks(t, &base).await;
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tauri::async_runtime::spawn_blocking(f).await.ok()
}

async fn status() -> Option<servers::Statuses> {
    blocking(servers::status).await
}

async fn http(url: &str) -> (Option<u16>, String) {
    let Ok(c) = reqwest::Client::builder().timeout(Duration::from_secs(5)).no_proxy().build() else {
        return (None, String::new());
    };
    match c.get(url).send().await {
        Ok(r) => (Some(r.status().as_u16()), r.text().await.unwrap_or_default()),
        Err(_) => (None, String::new()),
    }
}

/// `$cfg['blowfish_secret'] = '<32 bytes>';`
fn has_secret(config: &str) -> bool {
    config
        .split("['blowfish_secret']")
        .nth(1)
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .and_then(|rest| rest.trim_start().strip_prefix('\''))
        .and_then(|rest| rest.split('\'').next())
        .is_some_and(|secret| secret.len() == 32)
}

/// Unchanged saves leave the files byte for byte; a change on a temp copy is checked by the server,
/// written, backed up and restored; an invalid value is refused and the copy stays as it was.
fn config_checks() -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let read = |p: &Path| file::read(p).ok();
    let logs = paths::logs();
    let php_log = logs.join("php").join("php_errors.log");
    let (slow, general) = (logs.join("mariadb").join("slow.log"), logs.join("mariadb").join("general.log"));

    match read(&paths::httpd_conf()) {
        Some(t) => {
            let s = apache::parse(&t);
            out.push((apache::apply(&t, &s, &s) == t, "Saving unchanged Apache settings leaves httpd.conf byte for byte".into()));
        }
        None => out.push((false, "httpd.conf can be read".into())),
    }
    match read(&paths::php_ini()) {
        Some(t) => {
            let s = php::parse(&t);
            out.push((php::apply(&t, &s, &s, &paths::fwd(&php_log)) == t, "Saving unchanged PHP settings leaves php.ini byte for byte".into()));
        }
        None => out.push((false, "php.ini can be read".into())),
    }
    match read(&paths::my_ini()) {
        Some(t) => {
            let s = mariadb::parse(&t);
            let same = mariadb::apply(&t, &s, &s, &paths::fwd(&slow), &paths::fwd(&general)) == t;
            out.push((same, "Saving unchanged MariaDB settings leaves my.ini byte for byte".into()));
        }
        None => out.push((false, "my.ini can be read".into())),
    }

    let dir = std::env::temp_dir().join(format!("lampion-selftest-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let copy = |from: &Path, name: &str| {
        let to = dir.join(name);
        std::fs::copy(from, &to).ok().map(|_| to)
    };
    // A changed value must land in the copy, a backup must exist, and restoring must give the original back.
    let round_trip = |path: &Path, original: &str, saved: Result<bool, String>, applied: bool| {
        saved == Ok(true) && applied && file::has_backup(path) && file::restore_backup(path).is_ok() && read(path).as_deref() == Some(original)
    };

    if let Some(conf) = copy(&paths::httpd_conf(), "httpd.conf") {
        let original = read(&conf).unwrap_or_default();
        let base = apache::parse(&original);
        let mut draft = base.clone();
        let port: u16 = base.port.parse().unwrap_or(8080);
        draft.port = (if port < 65000 { port + 1 } else { port - 1 }).to_string();
        let saved = apache::save(&conf, &base, &draft, &file::run_real);
        let applied = read(&conf).is_some_and(|t| apache::parse(&t).port == draft.port);
        out.push((round_trip(&conf, &original, saved, applied), "Apache accepts a port change on a test copy; the original comes back".into()));

        let mut wrong = base.clone();
        wrong.log_level = "verbose".into();
        let refused = apache::save(&conf, &base, &wrong, &file::run_real).is_err() && read(&conf).as_deref() == Some(original.as_str());
        out.push((refused, "Apache refuses an invalid setting and the file stays as it was".into()));
    }
    if let Some(ini) = copy(&paths::php_ini(), "php.ini") {
        let original = read(&ini).unwrap_or_default();
        let base = php::parse(&original);
        let mut draft = base.clone();
        draft.memory_limit = if base.memory_limit.trim().eq_ignore_ascii_case("256M") { "384M" } else { "256M" }.into();
        let saved = php::save(&ini, &base, &draft, &file::run_real, &php_log);
        let applied = read(&ini).is_some_and(|t| php::parse(&t).memory_limit == draft.memory_limit);
        out.push((round_trip(&ini, &original, saved, applied), "PHP accepts a memory limit change on a test copy; the original comes back".into()));
    }
    if let Some(my) = copy(&paths::my_ini(), "my.ini") {
        let original = read(&my).unwrap_or_default();
        let base = mariadb::parse(&original);
        let mut draft = base.clone();
        draft.max_connections = if base.max_connections.trim() == "200" { "201" } else { "200" }.into();
        let saved = mariadb::save(&my, &base, &draft, &file::run_real, &slow, &general);
        let applied = read(&my).is_some_and(|t| mariadb::parse(&t).max_connections == draft.max_connections);
        out.push((
            round_trip(&my, &original, saved, applied),
            "MariaDB accepts a connection limit change on a test copy; the original comes back".into(),
        ));
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// A 404 and a PHP warning reach the logs the Logs page follows, in time and with the right marks.
async fn log_checks(t: &mut Report, base: &str) {
    let l = locations::current();
    let access = Source::ApacheAccess.path(&l);
    let mut tail = Tail::default();
    tail.poll(&access); // from the end on
    let missing = "/lampion-selftest-404";
    let (code, _) = http(&format!("{base}{missing}")).await;
    let asked = chrono::Local::now();
    let line = wait_for(&mut tail, &access, Source::ApacheAccess, missing).await;
    let ok = code == Some(404)
        && line.as_ref().is_some_and(|l| {
            l.level == Level::Warn && l.tag.as_deref() == Some("404") && l.time.as_deref().is_some_and(|tm| near(tm, asked))
        });
    t.check(ok, "A 404 shows up in Apache Requests within a second, in orange, with the right time");

    let source = if l.php_error_log.is_some() { Source::Php } else { Source::ApacheError };
    let log = source.path(&l);
    let script = l.document_root.join("_lampion_selftest.php");
    let marker = "Lampion self-test warning";
    let mut tail = Tail::default();
    tail.poll(&log);
    let wrote = std::fs::write(&script, format!("<?php trigger_error('{marker}', E_USER_WARNING); echo 'ok';")).is_ok();
    let (code, _) = http(&format!("{base}/_lampion_selftest.php")).await;
    let line = wait_for(&mut tail, &log, source, marker).await;
    let _ = std::fs::remove_file(&script);
    let place = if source == Source::Php { "PHP Errors" } else { "Apache Errors" };
    t.check(wrote && code == Some(200) && line.is_some_and(|l| l.level == Level::Warn), &format!("A PHP warning shows up in {place}"));
}

/// Waits up to 1.5 s for a line containing `text` (the Logs page looks once a second).
async fn wait_for(tail: &mut Tail, path: &Path, source: Source, text: &str) -> Option<parse::Line> {
    for _ in 0..15 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let lines = tail.poll(path).map(|r| r.lines).unwrap_or_default();
        if let Some(found) = lines.into_iter().find(|l| l.contains(text)) {
            return Some(parse::parse(source, found, 0));
        }
    }
    None
}

/// `hh:mm:ss` within 3 seconds of `at` (across midnight too).
fn near(time: &str, at: chrono::DateTime<chrono::Local>) -> bool {
    use chrono::Timelike;
    let secs = |h: u32, m: u32, s: u32| i64::from(h * 3600 + m * 60 + s);
    let mut p = time.split(':').map(|x| x.parse::<u32>().unwrap_or(99));
    let (h, m, s) = (p.next().unwrap_or(99), p.next().unwrap_or(99), p.next().unwrap_or(99));
    let d = (secs(h, m, s) - secs(at.hour(), at.minute(), at.second())).rem_euclid(86_400);
    d <= 3 || d >= 86_400 - 3
}

// ---- --dbtest ----

fn cell(r: &Result<ResultSet, String>, row: usize, col: usize) -> Option<String> {
    r.as_ref().ok()?.rows.get(row)?.get(col)?.clone()
}

async fn first(sql: &str) -> Option<String> {
    cell(&client::query(sql).await, 0, 0)
}

/// The first column of every row.
async fn list(sql: &str) -> Vec<String> {
    client::query(sql).await.map(|r| r.rows.into_iter().filter_map(|row| row.into_iter().next().flatten()).collect()).unwrap_or_default()
}

const DB: &str = "_lampion_teszt";

async fn columns(table: &str) -> Vec<String> {
    list(&format!(
        "SELECT COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} ORDER BY ORDINAL_POSITION",
        literal(DB),
        literal(table)
    ))
    .await
}

async fn tables() -> Vec<String> {
    list(&format!("SHOW TABLES FROM {}", client::ident(DB))).await
}

/// `SHOW FULL COLUMNS` of a table: Field, Type, Collation, Null, Key, Default, Extra, Privileges, Comment.
async fn full_columns(table: &str) -> Vec<Vec<String>> {
    client::query(&format!("SHOW FULL COLUMNS FROM {}", qualified(DB, table)))
        .await
        .map(|r| r.rows.into_iter().map(|row| row.into_iter().map(Option::unwrap_or_default).collect()).collect())
        .unwrap_or_default()
}

async fn database_checks(t: &mut Report) {
    let q = |sql: String| async move { client::query(&sql).await };
    let tricky = "Árvíztűrő 'tükörfúrógép' \\ <b>&\"x\"";

    let dbs = client::query("SHOW DATABASES").await;
    let connected = dbs.as_ref().is_ok_and(|r| r.rows.iter().any(|row| row.first().cloned().flatten().as_deref() == Some("mysql")));
    t.check(connected, "Connect and list databases");
    if !connected {
        t.note(&format!("MariaDB did not answer, so the database checks were skipped. {}", dbs.err().unwrap_or_default()));
        return;
    }

    let _ = q(format!("DROP DATABASE IF EXISTS {}", client::ident(DB))).await;
    let made = q(format!("CREATE DATABASE {} COLLATE utf8mb4_hungarian_ci", client::ident(DB))).await;
    t.check(made.is_ok() && list("SHOW DATABASES").await.iter().any(|d| d == DB), "Create database");

    let termekek = qualified(DB, "termekek");
    let made = q(format!("CREATE TABLE {termekek} (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY)")).await;
    t.check(made.is_ok() && columns("termekek").await == ["id"], "Create table");
    let _ = q(format!("ALTER TABLE {termekek} ADD COLUMN nev VARCHAR(100) NULL DEFAULT ''")).await;
    let _ = q(format!("ALTER TABLE {termekek} ADD COLUMN ar DECIMAL(10,2) NOT NULL DEFAULT 0")).await;
    t.check(columns("termekek").await == ["id", "nev", "ar"], "Add columns");

    let insert = format!("INSERT INTO {termekek} (nev, ar) VALUES (?, ?)");
    let a = client::exec(&insert, vec![Some(tricky.into()), Some("1999.50".into())]).await;
    let b = client::exec(&insert, vec![Some("Második".into()), Some("500".into())]).await;
    let count = format!("SELECT COUNT(*) FROM {termekek}");
    t.check(a.is_ok() && b.is_ok() && first(&count).await.as_deref() == Some("2"), "Insert rows");
    t.check(first(&format!("SELECT nev FROM {termekek} WHERE id = 1")).await.as_deref() == Some(tricky), "Accented and special characters survive intact");

    let update = format!("UPDATE {termekek} SET nev = ? WHERE id = 2 LIMIT 1");
    let second = format!("SELECT nev FROM {termekek} WHERE id = 2");
    let _ = client::exec(&update, vec![Some("Módosítva".into())]).await;
    t.check(first(&second).await.as_deref() == Some("Módosítva"), "Edit cell");
    let _ = client::exec(&update, vec![None]).await;
    let null = client::query(&second).await;
    t.check(null.as_ref().is_ok_and(|r| r.rows.len() == 1 && r.rows[0][0].is_none()), "Set cell to NULL");

    let filtered = q(format!("SELECT * FROM {termekek} WHERE ar > 1000")).await;
    t.check(filtered.is_ok_and(|r| r.rows.len() == 1), "Filter (WHERE)");
    let wrong = q(format!("SELECT * FROM {termekek} WHERE nincs_ilyen = 1")).await;
    t.check(wrong.is_err_and(|e| e.contains("1054")), "Invalid filter reports an error");
    t.check(first(&format!("SELECT ar FROM {termekek} ORDER BY ar ASC")).await.as_deref() == Some("500.00"), "Sort ascending");
    t.check(first(&format!("SELECT ar FROM {termekek} ORDER BY ar DESC")).await.as_deref() == Some("1999.50"), "Sort descending");
    let changed = q(format!("UPDATE {termekek} SET ar = ar + 1")).await;
    t.check(changed.is_ok_and(|r| r.affected_rows == 2), "Modifying query reports rows affected");
    t.check(client::query("SELEC hiba").await.is_err_and(|e| e.contains("1064")), "Invalid SQL reports an error");

    let dump = export(DB).await;
    t.check(dump.contains("CREATE TABLE `termekek`") && dump.contains("tükörfúrógép"), "Export to an SQL file");

    let _ = q(format!("DELETE FROM {termekek} WHERE id = 1")).await;
    t.check(first(&count).await.as_deref() == Some("1"), "Delete row");
    let _ = q(format!("ALTER TABLE {termekek} DROP COLUMN ar")).await;
    t.check(columns("termekek").await == ["id", "nev"], "Delete column");
    let _ = q(format!("TRUNCATE TABLE {termekek}")).await;
    t.check(first(&count).await.as_deref() == Some("0"), "Empty table");
    let _ = q(format!("DROP TABLE {termekek}")).await;
    t.check(tables().await.is_empty(), "Delete table");

    // Structure: columns, indexes, foreign keys, table options.
    let vevok = qualified(DB, "vevok");
    let made = q(format!(
        "CREATE TABLE {vevok} (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, nev VARCHAR(50) NOT NULL, \
         email VARCHAR(100) NULL, letrehozva DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP) ENGINE=InnoDB COLLATE utf8mb4_hungarian_ci"
    ))
    .await;
    let cols = full_columns("vevok").await;
    let collation = first(&format!(
        "SELECT TABLE_COLLATION FROM information_schema.TABLES WHERE TABLE_SCHEMA = {} AND TABLE_NAME = 'vevok'",
        literal(DB)
    ))
    .await;
    let field = |cols: &Vec<Vec<String>>, name: &str, i: usize| cols.iter().find(|c| c[0] == name).map(|c| c[i].clone()).unwrap_or_default();
    t.check(
        made.is_ok()
            && cols.iter().map(|c| c[0].as_str()).collect::<Vec<_>>() == ["id", "nev", "email", "letrehozva"]
            && field(&cols, "id", 4) == "PRI"
            && field(&cols, "id", 6).contains("auto_increment")
            && field(&cols, "letrehozva", 5).to_lowercase().starts_with("current_timestamp")
            && collation.as_deref() == Some("utf8mb4_hungarian_ci"),
        "Create a table with all its columns",
    );

    let _ = q(format!(
        "ALTER TABLE {vevok} CHANGE COLUMN nev teljes_nev VARCHAR(120) NOT NULL DEFAULT 'ismeretlen' COMMENT 'Customer''s name' AFTER email"
    ))
    .await;
    let cols = full_columns("vevok").await;
    t.check(
        cols.iter().map(|c| c[0].as_str()).collect::<Vec<_>>() == ["id", "email", "teljes_nev", "letrehozva"]
            && field(&cols, "teljes_nev", 1) == "varchar(120)"
            && field(&cols, "teljes_nev", 5) == "ismeretlen"
            && field(&cols, "teljes_nev", 3) == "NO"
            && field(&cols, "teljes_nev", 8) == "Customer's name",
        "Edit column: rename, type, default, comment, position",
    );
    let _ = q(format!("ALTER TABLE {vevok} ADD COLUMN aktiv TINYINT(1) NOT NULL DEFAULT 1 FIRST")).await;
    let cols = full_columns("vevok").await;
    t.check(cols.first().is_some_and(|c| c[0] == "aktiv" && c[5] == "1"), "Add column at a chosen position");

    let _ = q(format!("ALTER TABLE {vevok} ADD UNIQUE INDEX uq_email (email)")).await;
    let _ = q(format!("ALTER TABLE {vevok} ADD INDEX (teljes_nev, email)")).await;
    let indexes = |r: Result<ResultSet, String>| -> Vec<(String, bool, Vec<String>)> {
        let mut out: Vec<(String, bool, Vec<String>)> = Vec::new();
        for row in r.map(|r| r.rows).unwrap_or_default() {
            let get = |i: usize| row.get(i).cloned().flatten().unwrap_or_default();
            let (name, unique, column) = (get(2), get(1) == "0", get(4));
            match out.iter_mut().find(|(n, ..)| *n == name) {
                Some(entry) => entry.2.push(column),
                None => out.push((name, unique, vec![column])),
            }
        }
        out
    };
    let show_index = format!("SHOW INDEX FROM {vevok}");
    let found = indexes(client::query(&show_index).await);
    t.check(
        found.iter().any(|(n, u, c)| n == "uq_email" && *u && *c == ["email"])
            && found.iter().any(|(_, u, c)| !*u && *c == ["teljes_nev", "email"])
            && found.iter().any(|(n, _, c)| n == "PRIMARY" && *c == ["id"]),
        "Add unique and multi-column indexes",
    );
    let _ = q(format!("DROP INDEX uq_email ON {vevok}")).await;
    let found = indexes(client::query(&show_index).await);
    t.check(!found.iter().any(|(n, ..)| n == "uq_email") && found.len() == 2, "Delete index");

    let rendelesek = qualified(DB, "rendelesek");
    let _ = q(format!(
        "CREATE TABLE {rendelesek} (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, vevo_id INT UNSIGNED NOT NULL) ENGINE=InnoDB"
    ))
    .await;
    let _ = q(format!(
        "ALTER TABLE {rendelesek} ADD CONSTRAINT fk_vevo FOREIGN KEY (vevo_id) REFERENCES {vevok} (id) ON DELETE CASCADE ON UPDATE RESTRICT"
    ))
    .await;
    let rules = format!(
        "SELECT CONCAT(REFERENCED_TABLE_NAME, ' ', UPDATE_RULE, ' ', DELETE_RULE) FROM information_schema.REFERENTIAL_CONSTRAINTS \
         WHERE CONSTRAINT_SCHEMA = {} AND CONSTRAINT_NAME = 'fk_vevo'",
        literal(DB)
    );
    t.check(list(&rules).await == ["vevok RESTRICT CASCADE"], "Add foreign key");
    let orphan = q(format!("INSERT INTO {rendelesek} (vevo_id) VALUES (999)")).await;
    t.check(orphan.is_err_and(|e| e.contains("foreign key constraint fails")), "The foreign key is enforced by the server");
    let _ = q(format!("ALTER TABLE {rendelesek} DROP FOREIGN KEY fk_vevo")).await;
    t.check(list(&rules).await.is_empty(), "Delete foreign key");

    let _ = q(format!("ALTER TABLE {rendelesek} ENGINE=MyISAM")).await;
    let _ = q(format!("ALTER TABLE {rendelesek} CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_hungarian_ci")).await;
    let options = first(&format!(
        "SELECT CONCAT(ENGINE, ' ', TABLE_COLLATION) FROM information_schema.TABLES WHERE TABLE_SCHEMA = {} AND TABLE_NAME = 'rendelesek'",
        literal(DB)
    ))
    .await;
    t.check(options.as_deref() == Some("MyISAM utf8mb4_hungarian_ci"), "Change engine and collation");
    let optimized = q(format!("OPTIMIZE TABLE {rendelesek}")).await;
    t.check(
        optimized.is_ok_and(|r| r.rows.iter().any(|row| row.get(3).cloned().flatten().is_some_and(|m| m.contains("OK") || m.contains("up to date")))),
        "Maintenance reports the server's answer",
    );

    let _ = q(format!("INSERT INTO {rendelesek} (vevo_id) VALUES (1), (2)")).await;
    let masolat = qualified(DB, "rendelesek_masolat");
    let _ = q(format!("CREATE TABLE {masolat} LIKE {rendelesek}")).await;
    let _ = q(format!("INSERT INTO {masolat} SELECT * FROM {rendelesek}")).await;
    t.check(first(&format!("SELECT COUNT(*) FROM {masolat}")).await.as_deref() == Some("2"), "Copy table with data");
    let _ = q(format!("RENAME TABLE {rendelesek} TO {}", qualified(DB, "megrendelesek"))).await;
    let names = tables().await;
    t.check(names.iter().any(|n| n == "megrendelesek") && !names.iter().any(|n| n == "rendelesek"), "Rename table");

    let _ = q(format!("DROP DATABASE {}", client::ident(DB))).await;
    t.check(!list("SHOW DATABASES").await.iter().any(|d| d == DB), "Delete database");
}

/// `mariadb-dump` of the database, as text.
async fn export(db: &str) -> String {
    let port = format!("--port={}", client::port());
    let db = db.to_string();
    blocking(move || {
        let args = ["--host=127.0.0.1", port.as_str(), "--user=root", "--skip-ssl", "--default-character-set=utf8mb4", "--databases", db.as_str()];
        shell::run(paths::mariadb_dump_exe(), &args).out
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn finds_the_phpmyadmin_secret() {
        assert!(has_secret("<?php\n$cfg['blowfish_secret'] = 'abcdefghijABCDEFGHIJ0123456789xy'; /* Lampion */\n"));
        assert!(!has_secret("$cfg['blowfish_secret'] = '';"));
        assert!(!has_secret("$cfg['blowfish_secret'] = 'short';"));
        assert!(!has_secret(""));
    }

    #[test]
    fn times_near_each_other() {
        let at = chrono::Local.with_ymd_and_hms(2026, 11, 23, 19, 27, 18).unwrap();
        assert!(near("19:27:18", at));
        assert!(near("19:27:20", at));
        assert!(!near("19:27:30", at));
        assert!(!near("26:12:06", at));
        let midnight = chrono::Local.with_ymd_and_hms(2026, 11, 23, 23, 59, 59).unwrap();
        assert!(near("00:00:01", midnight));
    }
}

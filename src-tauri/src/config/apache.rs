//! httpd.conf: the Apache settings page. Apache-style text: one directive per line, `#` comments,
//! nested `<Section>` blocks, `Define` variables. Saving writes only the settings that changed.

use super::file::{self, indent, unquote, Check, Doc, Run};
use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ApacheSettings {
    /// `Listen` port (text, as typed).
    pub port: String,
    /// `DocumentRoot`, as a Windows path.
    pub document_root: String,
    /// `ServerAdmin`; empty comments the directive out.
    pub server_admin: String,
    /// `.htaccess` files: `AllowOverride` in the web folder's `<Directory>` block is not `None`.
    pub allow_override: bool,
    /// `Options Indexes` in the web folder's block.
    pub directory_listing: bool,
    /// The main `LogLevel` (per-module levels such as `rewrite:trace3` are kept).
    pub log_level: String,
    /// Feature id → on.
    pub modules: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Feature {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    /// Switched on and off together; the feature is on when all of them are.
    pub modules: &'static [&'static str],
    /// Also switched on (never off: something else may use them).
    pub needs: &'static [&'static str],
}

pub const FEATURES: &[Feature] = &[
    Feature { id: "rewrite", title: "URL rewriting", detail: "mod_rewrite. Needed by Laravel, WordPress permalinks and most .htaccess rules.", modules: &["rewrite"], needs: &[] },
    Feature { id: "headers", title: "Custom headers", detail: "mod_headers. Lets .htaccess set CORS and caching headers.", modules: &["headers"], needs: &[] },
    Feature { id: "deflate", title: "Gzip compression", detail: "mod_deflate. Compresses responses, as most live servers do.", modules: &["deflate"], needs: &["filter"] },
    Feature { id: "expires", title: "Cache expiry", detail: "mod_expires. Honours ExpiresActive and ExpiresByType rules.", modules: &["expires"], needs: &[] },
    Feature { id: "proxy", title: "Reverse proxy", detail: "mod_proxy, mod_proxy_http and mod_proxy_wstunnel. Forward requests to Node, Vite and similar dev servers.", modules: &["proxy", "proxy_http", "proxy_wstunnel"], needs: &[] },
    Feature { id: "vhost_alias", title: "Dynamic virtual hosts", detail: "mod_vhost_alias. Map host names to folders with VirtualDocumentRoot.", modules: &["vhost_alias"], needs: &[] },
    Feature { id: "info", title: "Server info page", detail: "mod_info. Shows the loaded configuration at a /server-info handler you configure.", modules: &["info"], needs: &[] },
];

pub const LOG_LEVELS: &[&str] = &["emerg", "alert", "crit", "error", "warn", "notice", "info", "debug"];

/// The text of httpd.conf with helpers to find and change directives.
pub struct Conf {
    doc: Doc,
}

fn first_word_is(t: &str, directive: &str) -> bool {
    t.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case(directive))
}

/// Everything after the directive name.
fn args(t: &str) -> &str {
    t.split_once(char::is_whitespace).map(|(_, r)| r.trim()).unwrap_or("")
}

/// The first argument: `"quoted path" common` → `quoted path`.
fn first_arg(a: &str) -> &str {
    match a.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(""),
        None => a.split_whitespace().next().unwrap_or(""),
    }
}

/// A path for comparing: forward slashes, no trailing slash, case-insensitive (Windows).
fn same_path(a: &str, b: &str) -> bool {
    let n = |p: &str| p.trim().replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(b)
}

impl Conf {
    pub fn new(text: &str) -> Conf {
        Conf { doc: Doc::new(text) }
    }

    pub fn text(&self) -> String {
        self.doc.text()
    }

    fn trimmed(&self, i: usize) -> &str {
        self.doc.line(i).trim()
    }

    fn active(&self, i: usize) -> bool {
        let t = self.trimmed(i);
        !t.is_empty() && !t.starts_with('#')
    }

    /// For each line: nesting depth (0 = top level) and whether it is inside a `<VirtualHost>`.
    fn layout(&self) -> Vec<(usize, bool)> {
        let mut stack: Vec<String> = Vec::new();
        let vhost = |s: &Vec<String>| s.iter().any(|n| n == "virtualhost");
        (0..self.doc.len())
            .map(|i| {
                let t = self.trimmed(i);
                if t.starts_with("</") {
                    stack.pop();
                    return (stack.len(), vhost(&stack));
                }
                let here = (stack.len(), vhost(&stack));
                if t.starts_with('<') && !t.starts_with("<!") {
                    let name = t[1..].split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("");
                    stack.push(name.to_ascii_lowercase());
                }
                here
            })
            .collect()
    }

    /// The first active top-level line of a directive.
    fn find_top(&self, directive: &str) -> Option<usize> {
        let layout = self.layout();
        (0..self.doc.len()).find(|&i| layout[i].0 == 0 && self.active(i) && first_word_is(self.trimmed(i), directive))
    }

    /// The arguments of a top-level directive, without quotes.
    pub fn value(&self, directive: &str) -> Option<String> {
        self.find_top(directive).map(|i| unquote(args(self.trimmed(i))).to_string())
    }

    /// The arguments of the first active occurrence at any depth outside `<VirtualHost>` blocks
    /// (`CustomLog` usually sits in `<IfModule log_config_module>`).
    fn value_anywhere(&self, directive: &str) -> Option<String> {
        let layout = self.layout();
        (0..self.doc.len())
            .find(|&i| !layout[i].1 && self.active(i) && first_word_is(self.trimmed(i), directive))
            .map(|i| args(self.trimmed(i)).to_string())
    }

    /// The file of the main `ErrorLog` / `CustomLog` (resolved, forward slashes); `None` when the
    /// directive is missing or logs to a pipe or syslog.
    pub fn log_file(&self, directive: &str) -> Option<String> {
        let a = self.value_anywhere(directive)?;
        let target = first_arg(&a);
        (!target.is_empty() && !target.starts_with('|') && !target.starts_with("syslog")).then(|| self.resolve(target))
    }

    /// Sets a top-level directive: replaces the active line, else the first commented-out one, else appends.
    pub fn set(&mut self, directive: &str, value: &str) {
        let layout = self.layout();
        let n = self.doc.len();
        let commented = |i: usize| {
            let t = self.trimmed(i);
            t.starts_with('#') && first_word_is(t.trim_start_matches('#'), directive)
        };
        let at = self.find_top(directive).or_else(|| (0..n).find(|&i| layout[i].0 == 0 && commented(i)));
        let line = format!("{directive} {value}");
        match at {
            Some(i) => {
                let new = format!("{}{line}", indent(self.doc.line(i)));
                self.doc.replace(i, new);
            }
            None => self.doc.push(line),
        }
    }

    /// Comments out a top-level directive, so Apache's built-in default applies.
    pub fn unset(&mut self, directive: &str) {
        let layout = self.layout();
        for i in 0..self.doc.len() {
            if layout[i].0 == 0 && self.active(i) && first_word_is(self.trimmed(i), directive) {
                let line = self.doc.line(i);
                let ind = indent(line);
                let new = format!("{ind}#{}", &line[ind.len()..]);
                self.doc.replace(i, new);
            }
        }
    }

    // ---- Variables and paths ----

    /// `${NAME}` replaced by top-level `Define NAME value`.
    pub fn expand(&self, s: &str) -> String {
        let mut out = s.to_string();
        if !out.contains("${") {
            return out;
        }
        for i in 0..self.doc.len() {
            let t = self.trimmed(i);
            if self.active(i) && first_word_is(t, "Define") {
                let a = args(t);
                let (name, value) = a.split_once(char::is_whitespace).unwrap_or((a, ""));
                out = out.replace(&format!("${{{name}}}"), unquote(value));
            }
        }
        out
    }

    /// `ServerRoot` (forward slashes), else Lampion's Apache folder.
    pub fn server_root(&self) -> String {
        self.value("ServerRoot")
            .map(|v| self.expand(&v).replace('\\', "/"))
            .unwrap_or_else(|| paths::fwd(&paths::apache_dir()))
    }

    /// A path from the config: variables expanded, relative paths resolved against `ServerRoot`,
    /// forward slashes.
    pub fn resolve(&self, raw: &str) -> String {
        let p = self.expand(unquote(raw)).replace('\\', "/");
        if Path::new(&p).is_absolute() || p.starts_with("//") {
            p
        } else {
            format!("{}/{}", self.server_root().trim_end_matches('/'), p)
        }
    }

    // ---- LoadModule ----

    /// The module's LoadModule line, preferring an active one over a commented-out one.
    fn module_line(&self, name: &str) -> Option<usize> {
        let id = format!("{name}_module");
        let is_it = |i: usize| {
            let mut w = self.trimmed(i).trim_start_matches('#').split_whitespace();
            w.next().is_some_and(|d| d.eq_ignore_ascii_case("LoadModule")) && w.next() == Some(id.as_str())
        };
        let lines: Vec<usize> = (0..self.doc.len()).filter(|&i| is_it(i)).collect();
        lines.iter().copied().find(|&i| self.active(i)).or(lines.first().copied())
    }

    pub fn module_enabled(&self, name: &str) -> bool {
        self.module_line(name).is_some_and(|i| self.active(i))
    }

    /// Comments a LoadModule line in or out (adds it after the last LoadModule line when missing).
    pub fn set_module(&mut self, name: &str, on: bool) {
        match self.module_line(name) {
            Some(i) if self.active(i) == on => {}
            Some(i) => {
                let line = self.doc.line(i);
                let ind = indent(line).to_string();
                let body = line.trim().trim_start_matches('#').trim_start().to_string();
                self.doc.replace(i, format!("{ind}{}{body}", if on { "" } else { "#" }));
            }
            None if on => {
                let line = format!("LoadModule {name}_module modules/mod_{name}.so");
                let last = (0..self.doc.len()).rev().find(|&i| first_word_is(self.trimmed(i).trim_start_matches('#'), "LoadModule"));
                match last {
                    Some(i) => self.doc.insert(i + 1, line),
                    None => self.doc.push(line),
                }
            }
            None => {}
        }
    }

    // ---- <Directory "…"> blocks ----

    /// The top-level `<Directory>` block for a path: (opening line, closing line).
    fn directory_block(&self, path: &str) -> Option<(usize, usize)> {
        let layout = self.layout();
        let n = self.doc.len();
        let open = (0..n).find(|&i| {
            let t = self.trimmed(i);
            layout[i].0 == 0
                && t.get(..11).is_some_and(|p| p.eq_ignore_ascii_case("<directory "))
                && same_path(&self.resolve(t[11..].trim_end_matches('>').trim()), path)
        })?;
        let close = (open + 1..n).find(|&i| self.trimmed(i).to_ascii_lowercase().starts_with("</directory"))?;
        Some((open, close))
    }

    /// A directive directly inside the block (not in a nested section).
    fn block_line(&self, (open, close): (usize, usize), directive: &str) -> Option<usize> {
        let layout = self.layout();
        (open + 1..close).find(|&i| layout[i].0 == 1 && self.active(i) && first_word_is(self.trimmed(i), directive))
    }

    pub fn directory_value(&self, path: &str, directive: &str) -> Option<String> {
        let block = self.directory_block(path)?;
        self.block_line(block, directive).map(|i| args(self.trimmed(i)).to_string())
    }

    fn set_directory_value(&mut self, path: &str, directive: &str, value: &str) {
        let Some(block) = self.directory_block(path) else { return };
        let line = format!("{directive} {value}");
        match self.block_line(block, directive) {
            Some(i) => {
                let new = format!("{}{line}", indent(self.doc.line(i)));
                self.doc.replace(i, new);
            }
            None => {
                let ind = if block.1 > block.0 + 1 { indent(self.doc.line(block.0 + 1)).to_string() } else { "    ".into() };
                self.doc.insert(block.1, format!("{ind}{line}"));
            }
        }
    }

    /// Points the block of `old` at `new`. False when there is no such block.
    fn rename_directory(&mut self, old: &str, new: &str) -> bool {
        let Some((open, _)) = self.directory_block(old) else { return false };
        let line = format!("{}<Directory \"{new}\">", indent(self.doc.line(open)));
        self.doc.replace(open, line);
        true
    }

    /// Adds `<Directory "path">` (readable by everyone) after the DocumentRoot line.
    fn add_directory_block(&mut self, path: &str) {
        let lines = [format!("<Directory \"{path}\">"), "    Require all granted".into(), "</Directory>".into()];
        match self.find_top("DocumentRoot") {
            Some(i) => {
                for (k, l) in lines.into_iter().enumerate() {
                    self.doc.insert(i + 1 + k, l);
                }
            }
            None => lines.into_iter().for_each(|l| self.doc.push(l)),
        }
    }
}

/// `Listen [addr:]port [protocol]` → (address part incl. its colon, port, protocol part incl. space).
/// The port is cut after the last `:` so `[::1]:8080` keeps its address.
pub fn split_listen(v: &str) -> (String, Option<u16>, String) {
    let v = v.trim();
    let (addr, proto) = match v.split_once(char::is_whitespace) {
        Some((a, rest)) => (a, format!(" {}", rest.trim())),
        None => (v, String::new()),
    };
    match addr.rfind(':') {
        Some(c) => (addr[..=c].to_string(), addr[c + 1..].parse().ok(), proto),
        None => (String::new(), addr.parse().ok(), proto),
    }
}

/// The main level of a `LogLevel` value: the first word without a `module:` prefix.
fn main_level(v: &str) -> Option<&str> {
    v.split_whitespace().find(|w| !w.contains(':'))
}

fn has_indexes(options: &str) -> bool {
    options.split_whitespace().any(|o| o.eq_ignore_ascii_case("Indexes") || o.eq_ignore_ascii_case("+Indexes"))
}

/// The `Options` value with directory listing turned on or off. Options are either all `+`/`-`
/// or all plain: a mixed line is an invalid config, so it is never written.
fn options_with_listing(current: Option<&str>, on: bool) -> String {
    let all: Vec<&str> = current.unwrap_or("FollowSymLinks").split_whitespace().collect();
    let relative = !all.is_empty() && all.iter().all(|o| o.starts_with('+') || o.starts_with('-'));
    let mut rest: Vec<String> = all
        .iter()
        .filter(|o| !["indexes", "+indexes", "-indexes", "none"].contains(&o.to_ascii_lowercase().as_str()))
        .map(|o| o.to_string())
        .collect();
    if on {
        rest.insert(0, if relative { "+Indexes" } else { "Indexes" }.into());
    } else if relative {
        rest.push("-Indexes".into());
    }
    if rest.is_empty() {
        rest.push("None".into());
    }
    rest.join(" ")
}

pub fn parse(text: &str) -> ApacheSettings {
    let conf = Conf::new(text);
    let port = conf.value("Listen").and_then(|l| split_listen(&l).1).unwrap_or(8080);
    let root = conf.value("DocumentRoot").map(|r| conf.resolve(&r)).unwrap_or_else(|| paths::fwd(&paths::www()));
    let allow_override = conf.directory_value(&root, "AllowOverride").is_some_and(|o| !o.eq_ignore_ascii_case("none"));
    let directory_listing = conf.directory_value(&root, "Options").is_some_and(|o| has_indexes(&o));
    let log_level = conf.value("LogLevel").and_then(|l| main_level(&l).map(str::to_ascii_lowercase)).unwrap_or_else(|| "warn".into());
    let modules = FEATURES.iter().map(|f| (f.id.to_string(), f.modules.iter().all(|m| conf.module_enabled(m)))).collect();
    ApacheSettings {
        port: port.to_string(),
        document_root: root.replace('/', "\\"),
        server_admin: conf.value("ServerAdmin").unwrap_or_default(),
        allow_override,
        directory_listing,
        log_level,
        modules,
    }
}

/// Applies the settings the user changed (`draft` differs from `base`, the values the page loaded)
/// and that the file does not already hold. Everything else in the file stays exactly as it was.
pub fn apply(text: &str, base: &ApacheSettings, draft: &ApacheSettings) -> String {
    let cur = parse(text);
    let mut conf = Conf::new(text);
    fn changed<T: PartialEq>(d: T, b: T, c: T) -> bool {
        d != b && d != c
    }
    let port = |s: &ApacheSettings| s.port.trim().parse::<u16>().ok();

    if let Some(p) = port(draft).filter(|_| changed(port(draft), port(base), port(&cur))) {
        let (prefix, old, proto) = split_listen(&conf.value("Listen").unwrap_or_default());
        conf.set("Listen", &format!("{prefix}{p}{proto}"));
        let old = old.unwrap_or(8080);
        if let Some(name) = conf.value("ServerName") {
            if let Some(host) = name.strip_suffix(&format!(":{old}")) {
                conf.set("ServerName", &format!("{host}:{p}"));
            }
        }
    }

    let root_now = paths::fwd(Path::new(&cur.document_root));
    let new_root = paths::fwd(Path::new(draft.document_root.trim()));
    let root_moved = !same_path(&draft.document_root, &base.document_root) && !same_path(&draft.document_root, &cur.document_root);
    let mut created = false;
    if root_moved {
        conf.set("DocumentRoot", &format!("\"{new_root}\""));
        if !conf.rename_directory(&root_now, &new_root) {
            conf.add_directory_block(&new_root);
            created = true;
        }
    }
    let root = if root_moved { new_root } else { root_now };
    if conf.directory_block(&root).is_none() && (changed(draft.allow_override, base.allow_override, cur.allow_override)
        || changed(draft.directory_listing, base.directory_listing, cur.directory_listing))
    {
        conf.add_directory_block(&root);
        created = true;
    }
    // A new block gets both values, so the page shows afterwards what the user saw before saving.
    if created || changed(draft.allow_override, base.allow_override, cur.allow_override) {
        conf.set_directory_value(&root, "AllowOverride", if draft.allow_override { "All" } else { "None" });
    }
    if created || changed(draft.directory_listing, base.directory_listing, cur.directory_listing) {
        let options = options_with_listing(conf.directory_value(&root, "Options").as_deref(), draft.directory_listing);
        conf.set_directory_value(&root, "Options", &options);
    }

    let level = draft.log_level.trim().to_ascii_lowercase();
    if changed(&level, &base.log_level, &cur.log_level) {
        let raw = conf.value("LogLevel").unwrap_or_default();
        let mut words: Vec<String> = raw.split_whitespace().map(String::from).collect();
        match words.iter_mut().find(|w| !w.contains(':')) {
            Some(w) => *w = level,
            None => words.insert(0, level),
        }
        conf.set("LogLevel", &words.join(" "));
    }

    let admin = draft.server_admin.trim();
    if changed(admin, base.server_admin.trim(), cur.server_admin.trim()) {
        if admin.is_empty() {
            conf.unset("ServerAdmin");
        } else {
            conf.set("ServerAdmin", admin);
        }
    }

    for f in FEATURES {
        let on = |s: &ApacheSettings| s.modules.get(f.id).copied().unwrap_or(false);
        if changed(on(draft), on(base), on(&cur)) {
            for m in f.modules {
                conf.set_module(m, on(draft));
            }
            if on(draft) {
                f.needs.iter().for_each(|m| conf.set_module(m, true));
            }
        }
    }
    conf.text()
}

pub fn is_log_level(l: &str) -> bool {
    let l = l.trim().to_ascii_lowercase();
    LOG_LEVELS.contains(&l.as_str()) || l.strip_prefix("trace").is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8"))
}

pub fn validate(s: &ApacheSettings) -> Check {
    let mut c = Check::default();
    if !s.port.trim().parse::<u16>().is_ok_and(|p| p >= 1024) {
        c.problem("port", "The port must be between 1024 and 65535. Lower ports need administrator rights.");
    }
    let root = Path::new(s.document_root.trim());
    if !root.is_absolute() || root.parent().is_none() {
        c.problem("documentRoot", "The web folder must be a full path to a folder, not a whole drive.");
    } else if !root.is_dir() {
        c.problem("documentRoot", format!("The web folder does not exist: {}", root.display()));
    } else if paths::is_unsafe_location(root) {
        c.warning("documentRoot", "This folder is synced by OneDrive or is inside Program Files, which can break sites (sync conflicts, no write access). A folder such as C:\\Lampion\\www is safer.");
    }
    if s.server_admin.trim().contains(char::is_whitespace) {
        c.problem("serverAdmin", "The administrator email must be a single address, without spaces.");
    }
    if !is_log_level(&s.log_level) {
        c.problem("logLevel", format!("Unknown log level: {}", s.log_level));
    }
    c
}

/// Writes httpd.conf after Apache accepted a test copy (`httpd.exe -t -f <copy>`).
pub fn save(path: &Path, base: &ApacheSettings, draft: &ApacheSettings, run: Run) -> Result<bool, String> {
    let current = file::read(path)?;
    let updated = apply(&current, base, draft);
    file::check_and_write(path, &current, &updated, "conf", |tmp| {
        let tmp = tmp.to_string_lossy();
        let r = run(&paths::httpd_exe(), &["-t", "-f", &tmp]);
        if r.ok() {
            Ok(())
        } else {
            Err(format!("Apache rejected the new configuration, so nothing was changed.\n\n{}", file::tail(&r.out, 4)))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shell::Output;

    const CONF: &str = "Define SRVROOT \"C:/Lampion/bin/apache\"\r\n\
ServerRoot \"${SRVROOT}\"\r\n\
#Listen 12.34.56.78:80\r\n\
Listen 127.0.0.1:8080\r\n\
LoadModule rewrite_module modules/mod_rewrite.so\r\n\
#LoadModule headers_module modules/mod_headers.so\r\n\
LoadModule proxy_module modules/mod_proxy.so\r\n\
LoadModule proxy_http_module modules/mod_proxy_http.so\r\n\
#LoadModule proxy_wstunnel_module modules/mod_proxy_wstunnel.so\r\n\
ServerAdmin admin@example.com\r\n\
ServerName localhost:8080\r\n\
DocumentRoot \"C:/Lampion/www\"\r\n\
<Directory \"C:/Lampion/www\">\r\n\
\x20   Options Indexes FollowSymLinks\r\n\
\x20   AllowOverride FileInfo AuthConfig\r\n\
\x20   Require all granted\r\n\
</Directory>\r\n\
ErrorLog \"logs/error.log\"\r\n\
LogLevel warn rewrite:trace3\r\n\
<IfModule log_config_module>\r\n\
\x20   CustomLog \"C:/Lampion/logs/apache/access.log\" common\r\n\
</IfModule>\r\n";

    fn with(f: impl FnOnce(&mut ApacheSettings)) -> (ApacheSettings, ApacheSettings) {
        let base = parse(CONF);
        let mut draft = base.clone();
        f(&mut draft);
        (base, draft)
    }

    fn changed_lines(a: &str, b: &str) -> Vec<String> {
        let (a, b): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
        b.iter().filter(|l| !a.contains(l)).map(|l| l.trim_end_matches('\r').to_string()).collect()
    }

    #[test]
    fn reads_the_settings() {
        let s = parse(CONF);
        assert_eq!(s.port, "8080");
        assert_eq!(s.document_root, r"C:\Lampion\www");
        assert!(s.allow_override, "FileInfo AuthConfig is not None");
        assert!(s.directory_listing);
        assert_eq!(s.log_level, "warn");
        assert_eq!(s.server_admin, "admin@example.com");
        assert_eq!(s.modules["rewrite"], true);
        assert_eq!(s.modules["headers"], false);
        assert_eq!(s.modules["proxy"], false, "proxy_wstunnel is off");
    }

    #[test]
    fn unchanged_save_keeps_every_byte() {
        // Pitfall: an unchanged save rewrote AllowOverride FileInfo → All, cut `rewrite:trace3`
        // and commented out the proxy modules.
        let (base, draft) = with(|_| {});
        assert_eq!(apply(CONF, &base, &draft), CONF);
    }

    #[test]
    fn saves_never_touch_phpmyadmins_block() {
        // Its <RequireAll> (Require local + the Host check) is what keeps phpMyAdmin to this computer.
        let conf = crate::servers::configs::httpd_conf(Path::new(r"C:\Lampion"), 8080);
        let block = |t: &str| t[t.find("Alias /phpmyadmin").unwrap()..].split("</Directory>").next().unwrap().to_string();
        let base = parse(&conf);
        let dir = tempfile::tempdir().unwrap();
        for draft in [
            ApacheSettings { port: "9090".into(), allow_override: false, directory_listing: false, log_level: "debug".into(), ..base.clone() },
            ApacheSettings { document_root: dir.path().display().to_string(), server_admin: "me@example.com".into(), ..base.clone() },
            ApacheSettings { document_root: r"C:\Lampion\bin\phpmyadmin".into(), directory_listing: false, ..base.clone() },
            ApacheSettings { modules: base.modules.keys().map(|k| (k.clone(), true)).collect(), ..base.clone() },
        ] {
            let out = apply(&conf, &base, &draft);
            assert_ne!(out, conf);
            assert_eq!(block(&out), block(&conf), "{draft:?}");
        }
    }

    #[test]
    fn port_change_keeps_address_and_server_name_follows() {
        let (base, draft) = with(|s| s.port = "9090".into());
        let out = apply(CONF, &base, &draft);
        assert_eq!(changed_lines(CONF, &out), vec!["Listen 127.0.0.1:9090", "ServerName localhost:9090"]);
        assert!(out.contains("\r\nListen 127.0.0.1:9090\r\n"), "CRLF kept");
        assert!(out.contains("#Listen 12.34.56.78:80"), "the commented example stays");
    }

    #[test]
    fn listen_forms() {
        // Pitfall: `[::1]:8080` became `[:1]:9090` and `8080 http` lost its protocol.
        for (listen, want) in [("[::1]:8080", "[::1]:9090"), ("8080 http", "9090 http"), ("127.0.0.1:8080", "127.0.0.1:9090"), ("[::]:8080 https", "[::]:9090 https")] {
            let text = format!("Listen {listen}\nDocumentRoot \"C:/w\"\n");
            let base = parse(&text);
            assert_eq!(base.port, "8080", "{listen}");
            let mut draft = base.clone();
            draft.port = "9090".into();
            assert_eq!(apply(&text, &base, &draft), format!("Listen {want}\nDocumentRoot \"C:/w\"\n"));
        }
    }

    #[test]
    fn allow_override_written_only_when_changed() {
        let (base, draft) = with(|s| s.allow_override = false);
        let out = apply(CONF, &base, &draft);
        assert_eq!(changed_lines(CONF, &out), vec!["    AllowOverride None"]);
        let back = parse(&out);
        let mut on = back.clone();
        on.allow_override = true;
        assert!(apply(&out, &back, &on).contains("    AllowOverride All\r\n"));
    }

    #[test]
    fn options_never_mixed() {
        // Pitfall: `Indexes` added next to `+FollowSymLinks -Indexes` made the config invalid.
        assert_eq!(options_with_listing(Some("+FollowSymLinks -Indexes"), true), "+Indexes +FollowSymLinks");
        assert_eq!(options_with_listing(Some("+FollowSymLinks +Indexes"), false), "+FollowSymLinks -Indexes");
        assert_eq!(options_with_listing(Some("-Indexes"), true), "+Indexes");
        assert_eq!(options_with_listing(Some("Indexes FollowSymLinks"), false), "FollowSymLinks");
        assert_eq!(options_with_listing(Some("Indexes"), false), "None");
        assert_eq!(options_with_listing(Some("None"), true), "Indexes");
        assert_eq!(options_with_listing(None, true), "Indexes FollowSymLinks");
        let (base, draft) = with(|s| s.directory_listing = false);
        assert_eq!(changed_lines(CONF, &apply(CONF, &base, &draft)), vec!["    Options FollowSymLinks"]);
    }

    #[test]
    fn log_level_keeps_module_levels() {
        let (base, draft) = with(|s| s.log_level = "debug".into());
        assert_eq!(changed_lines(CONF, &apply(CONF, &base, &draft)), vec!["LogLevel debug rewrite:trace3"]);
        let text = "LogLevel rewrite:trace3 warn\n";
        let base = parse(text);
        assert_eq!(base.log_level, "warn");
        let mut draft = base.clone();
        draft.log_level = "info".into();
        assert_eq!(apply(text, &base, &draft), "LogLevel rewrite:trace3 info\n");
    }

    #[test]
    fn modules_only_changed_features() {
        let (base, draft) = with(|s| {
            s.modules.insert("headers".into(), true);
            s.modules.insert("rewrite".into(), false);
        });
        let out = apply(CONF, &base, &draft);
        assert_eq!(
            changed_lines(CONF, &out),
            vec!["#LoadModule rewrite_module modules/mod_rewrite.so", "LoadModule headers_module modules/mod_headers.so"]
        );
        assert!(out.contains("\r\nLoadModule proxy_module modules/mod_proxy.so\r\nLoadModule proxy_http_module"), "proxy untouched");
        let (base, draft) = with(|s| {
            s.modules.insert("proxy".into(), true);
        });
        assert_eq!(
            changed_lines(CONF, &apply(CONF, &base, &draft)),
            vec!["LoadModule proxy_wstunnel_module modules/mod_proxy_wstunnel.so"]
        );
        let (base, draft) = with(|s| {
            s.modules.insert("info".into(), true);
        });
        let out = apply(CONF, &base, &draft);
        assert!(out.contains("#LoadModule proxy_wstunnel_module modules/mod_proxy_wstunnel.so\r\nLoadModule info_module modules/mod_info.so\r\n"));
    }

    #[test]
    fn gzip_also_switches_on_mod_filter_but_never_off() {
        let text = "#LoadModule deflate_module modules/mod_deflate.so\n#LoadModule filter_module modules/mod_filter.so\n";
        let base = parse(text);
        let mut on = base.clone();
        on.modules.insert("deflate".into(), true);
        let out = apply(text, &base, &on);
        assert_eq!(out, "LoadModule deflate_module modules/mod_deflate.so\nLoadModule filter_module modules/mod_filter.so\n");
        let back = parse(&out);
        let mut off = back.clone();
        off.modules.insert("deflate".into(), false);
        assert_eq!(apply(&out, &back, &off), "#LoadModule deflate_module modules/mod_deflate.so\nLoadModule filter_module modules/mod_filter.so\n");
    }

    #[test]
    fn server_admin_empty_comments_it_out() {
        let (base, draft) = with(|s| s.server_admin = "  ".into());
        let out = apply(CONF, &base, &draft);
        assert_eq!(changed_lines(CONF, &out), vec!["#ServerAdmin admin@example.com"]);
        let back = parse(&out);
        assert_eq!(back.server_admin, "");
        let mut again = back.clone();
        again.server_admin = "me@site.test".into();
        assert_eq!(changed_lines(&out, &apply(&out, &back, &again)), vec!["ServerAdmin me@site.test"]);
    }

    #[test]
    fn web_folder_moves_its_directory_block() {
        let (base, draft) = with(|s| s.document_root = r"D:\Sites\my site".into());
        let out = apply(CONF, &base, &draft);
        assert_eq!(changed_lines(CONF, &out), vec!["DocumentRoot \"D:/Sites/my site\"", "<Directory \"D:/Sites/my site\">"]);
        let s = parse(&out);
        assert_eq!(s.document_root, r"D:\Sites\my site");
        assert!(s.allow_override && s.directory_listing, "the block came along");
    }

    #[test]
    fn missing_block_is_created_with_the_page_values() {
        let text = "DocumentRoot \"C:/old\"\nListen 8080\n";
        let base = parse(text);
        let mut draft = base.clone();
        draft.document_root = r"C:\new".into();
        draft.allow_override = true;
        let out = apply(text, &base, &draft);
        assert_eq!(
            out,
            "DocumentRoot \"C:/new\"\n<Directory \"C:/new\">\n    Require all granted\n    AllowOverride All\n    Options FollowSymLinks\n</Directory>\nListen 8080\n"
        );
    }

    #[test]
    fn hand_edits_are_not_undone() {
        // The page loaded port 8080; meanwhile the file was edited to 9000; the user changed only the log level.
        let base = parse(CONF);
        let edited = CONF.replace("Listen 127.0.0.1:8080", "Listen 127.0.0.1:9000");
        let mut draft = base.clone();
        draft.log_level = "info".into();
        let out = apply(&edited, &base, &draft);
        assert!(out.contains("Listen 127.0.0.1:9000"));
        assert!(out.contains("LogLevel info rewrite:trace3"));
    }

    #[test]
    fn validation() {
        let mut s = parse(CONF);
        s.document_root = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(validate(&s).problems.is_empty(), "{:?}", validate(&s).problems);
        for bad in ["80", "1023", "65536", "x", ""] {
            s.port = bad.into();
            assert_eq!(validate(&s).problems[0].field, "port", "{bad}");
        }
        s.port = "8080".into();
        for bad in [r"C:\", "relative\\www", "", r"C:\does-not-exist-lampion-test"] {
            s.document_root = bad.into();
            assert_eq!(validate(&s).problems[0].field, "documentRoot", "{bad}");
        }
        s.document_root = std::env::temp_dir().to_string_lossy().into_owned();
        s.log_level = "trace3".into();
        assert!(validate(&s).problems.is_empty());
        s.log_level = "loud".into();
        assert_eq!(validate(&s).problems[0].field, "logLevel");
    }

    #[test]
    fn rejected_config_leaves_the_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("httpd.conf");
        std::fs::write(&path, CONF).unwrap();
        let (base, draft) = with(|s| s.port = "9090".into());
        let calls = std::cell::RefCell::new(Vec::<String>::new());
        let reject = |p: &Path, a: &[&str]| {
            calls.borrow_mut().push(format!("{} {}", p.file_name().unwrap().to_string_lossy(), a[..2].join(" ")));
            Output { status: 1, out: "AH00526: Syntax error on line 4\nInvalid command 'Listen'".into() }
        };
        let err = save(&path, &base, &draft, &reject).unwrap_err();
        assert!(err.starts_with("Apache rejected") && err.contains("AH00526"), "{err}");
        assert_eq!(calls.borrow().as_slice(), ["httpd.exe -t -f"]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), CONF);
        assert!(!file::has_backup(&path));

        let accept = |_: &Path, _: &[&str]| Output { status: 0, out: "Syntax OK".into() };
        assert_eq!(save(&path, &base, &draft, &accept), Ok(true));
        assert!(std::fs::read_to_string(&path).unwrap().contains("Listen 127.0.0.1:9090\r\n"));
        assert_eq!(std::fs::read_to_string(file::backup_path(&path)).unwrap(), CONF);
        // Same values again: nothing to write, the server is not even asked.
        let base2 = parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(save(&path, &base2, &base2.clone(), &|_: &Path, _: &[&str]| panic!("no check")), Ok(false));
    }
}

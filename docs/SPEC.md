
# Lampion Windowsra, Rustban — specifikáció

 · 

Ez a dokumentum a macOS-es Lampion (SwiftUI, github.com/akitb/lampion, 1.1) teljes működését írja le úgy, hogy abból egy Rust-fejlesztő vagy AI megépíthesse a Windowsos változatot.

## Áttekintés

A Lampion egy helyi webfejlesztő vezérlőpult (XAMPP-alternatíva): egy ablakból indítja és figyeli az Apache, PHP és MariaDB szervereket, szerkeszti a beállításaikat, élőben mutatja a naplókat, és saját adatbázis-kezelője van (TablePlus/phpMyAdmin-szerű, ER-diagrammal).

A Windows-verziónak ugyanezt kell tudnia, natív Windows-appként, Rustban:
- 

**Szerverek:** Apache, PHP (Apache-modulként), MariaDB és phpMyAdmin letöltése, beállítása, indítása, leállítása, újraindítása, állapot (PID, port, verzió) figyelése.
- 

**Minden egy helyen:** weboldalak, adatbázisok, naplók, mentések egy mappában (`C:\Lampion`), mint az XAMPP-nál.
- 

**Beállítások:** Apache, PHP, MariaDB és az app saját beállításai űrlapokon; mentés előtt maga a szerver ellenőrzi a változást.
- 

**Naplók:** az app eseményei és a szerverek naplói élőben, szűrhetően, kereshetően.
- 

**Csatlakozás programokból:** `root` jelszó nélkül, csak erről a gépről; phpMyAdmin bejelentkezés nélkül.
- 

**Adatbázis-kezelő:** adatok, szerkezet, lekérdezés, Designer (táblák kapcsolatai ábrán), import/export.
- 

**Frissítés:** az app GitHub-kiadásokból frissíti magát.

**Nem cél:** távoli szerverek kezelése, PostgreSQL/SQLite, több PHP-verzió párhuzamosan, Docker. A felület nyelve angol (mint a macOS-verzióé).

A macOS-verzió forrása referenciaként: github.com/akitb/lampion, branch `redesign-config-logs-database`. Ahol ez a dokumentum nem egyértelmű, az a forrás a mérvadó.

**Letölthető macOS-build referenciának** (a v1.1 kiadásban): Lampion-1.1.dmg (telepítő, 2,4 MB) és Lampion-1.1-build.zip (a teljes `build` mappa: `Lampion.app`, ikonkészlet, DMG; 4,6 MB).

## Architektúra

Javaslat: **Tauri 2** (Rust háttér + webes felület, pl. Svelte vagy React) — a gazdag felületi elemekhez (virtualizált adatrács, SQL-szerkesztő színezéssel és javaslatokkal, ER-diagram vászon) itt a legtöbb kész építőelem van (pl. CodeMirror 6, AG Grid/TanStack). Tiszta Rust alternatíva a Slint vagy az egui, ezekben a rácsot, a szerkesztőt és a diagramot magunknak kell megírni.

A felület csak parancsokat küld és eseményeket fogad; minden lassú munka (folyamatindítás, fájlírás, lekérdezés, letöltés) a magban, háttérszálon fut, a szerverműveletek egy soros sorban.

**Javasolt crate-ek:** `tauri` (ablakok, IPC, tálca), `tokio` (aszinkron futás), `mysql_async` vagy `sqlx` (adatbázis), `sysinfo` + `windows` (folyamatok, portok `GetExtendedTcpTable`-lel, szolgáltatások, registry), `reqwest` + `sha2` + `zip` (letöltés, ellenőrzés, kicsomagolás), `notify` vagy saját lekérdezés (naplók), `regex`, `serde`/`serde_json` (manifest, beállítások), `iana-time-zone` (időzóna).

**Modulok** (a macOS-forrás mappái szerint): `servers` (vezérlés, állapot, első telepítés, költöztetés), `config` (Apache/ini/MariaDB szerkesztők és ellenőrzés), `logs` (források, követő, sorbontó), `database` (kliens, modell, séma, Designer-adat), `app` (beállítások, frissítő, öntesztek, naplózó). A beállítások `%APPDATA%\Lampion\settings.json`-ba kerülnek.

## Szerverek Windows-on

macOS-en a Homebrew telepíti és a `brew services` (launchd) futtatja a szervereket; Windows-on ezt a Lampionnak magának kell megoldania hordozható (zip) csomagokkal, mindent egy mappába téve.

**Csomagok** (az első indításkor letöltve, SHA-256-tal ellenőrizve; a pontos URL-ek és kivonatok egy, a GitHub-kiadásban tárolt `manifest.json`-ból jöjjenek, hogy app-frissítés nélkül is cserélhetők legyenek):
| 

Csomag | 

Forrás | 

Megjegyzés |
| 

Apache 2.4 x64 | 

Apache Lounge (VS17 build) | 

`httpd.exe`, `bin\ApacheMonitor` nem kell |
| 

PHP 8.x **Thread Safe** x64 | 

windows.php.net | 

csak a TS-verzióban van `php8apache2_4.dll` |
| 

MariaDB 11+ x64 (zip) | 

mariadb.org | 

`mariadbd.exe`, `mariadb.exe`, `mariadb-dump.exe`, `mariadb-install-db.exe` |
| 

phpMyAdmin 5.2 | 

phpmyadmin.net (all-languages zip) | 

PHP-alkalmazás, az Apache szolgálja ki |
| 

VC++ Redistributable x64 | 

Microsoft | 

az Apache-nak és a PHP-nak kell; a telepítő tegye fel |

**Mappaszerkezet** (alapértelmezetten `C:\Lampion`, az első indításkor választható; soha ne `Program Files` (írási jog) és ne OneDrive-szinkronizált mappa (a macOS-verzión az iCloud ugyanígy rontotta a fájlokat)):
```
C:\Lampion\
├── bin\apache, bin\php, bin\mariadb, bin\phpmyadmin   ← a csomagok
├── config\        ← httpd.conf, php.ini, my.ini, config.inc.php (Lampion írja)
├── www\           ← weboldalak (DocumentRoot)
├── databases\     ← MariaDB datadir
├── logs\apache, logs\mariadb, logs\php
├── backups\       ← exportok alapmappája
├── tmp\           ← PHP session és feltöltések
└── README.txt     ← mi hol van
```

**Első telepítés** (a főablak tetején egy sáv mutatja a haladást, a részletek a naplóba mennek):
- 

Csomagok letöltése, kivonat ellenőrzése, kicsomagolás a `bin\` alá.
- 

MariaDB adatkönyvtár létrehozása: `mariadb-install-db.exe --datadir=C:\Lampion\databases`.
- 

Konfigurációk megírása a `config\` mappába (lásd lent), PHP-időzóna a Windows időzónájából IANA-névre fordítva (pl. `Europe/Budapest`).
- 

Üdvözlő `index.php` a `www\`-be, ha üres.
- 

Szerverek indítása, majd a `root` jelszó nélküli elérése és a phpMyAdmin automatikus belépése (lásd „Csatlakozás programokból”).

**Generált konfiguráció lényege** (Windows-on is perjeles `/` útvonalakkal):
- 

`httpd.conf`: `Define SRVROOT`, `Listen 127.0.0.1:8080`, `ServerName localhost:8080`, `LoadModule php_module "C:/Lampion/bin/php/php8apache2_4.dll"`, `PHPIniDir "C:/Lampion/config"`, `<FilesMatch \.php$> SetHandler application/x-httpd-php`, `DirectoryIndex index.php index.html`, `mod_rewrite` be, a webmappán `AllowOverride All`, `Alias /phpmyadmin` + `Require local`, `ErrorLog`/`CustomLog` a `logs\apache` alá.
- 

`php.ini` (a `php.ini-development` alapján): `extension_dir`, bekapcsolt `mysqli`, `pdo_mysql`, `mbstring`, `openssl`, `curl`, `gd`, `intl`, `zip`, `fileinfo`, `sodium`, `exif`; `date.timezone`; `error_log`; `session.save_path` és `upload_tmp_dir` a `tmp\`-be.
- 

`my.ini`: `[mysqld]` `datadir`, `port=3306`, `bind-address=127.0.0.1`, `log-error`; a Lampion saját beállításai külön blokkban (lásd Beállítások).
- 

Alapból minden csak `127.0.0.1`-en figyel: így a Windows tűzfal nem kérdez rá; hálózati elérés külön kapcsolóval.

**Folyamatkezelés.** A macOS-verzión a szerverek az app bezárása után is futnak. Windows-on két mód legyen:
- 

**Alap:** háttérfolyamatként indítva (`CREATE_NO_WINDOW`, leválasztva, PID-fájllal), adminjog nélkül. Az app újraindításkor PID és port alapján megtalálja őket.
- 

**Opcionális:** Windows-szolgáltatásként (`httpd.exe -k install -n LampionApache`, `mariadbd.exe --install LampionMariaDB --defaults-file=…`), egyszeri UAC-engedéllyel, a gép indításakor indulva.

**Indítás** (szerverenként, egy soros műveleti sorban, hogy két művelet ne fusson egyszerre):
- 

Apache-nál `httpd.exe -t -f <conf>`: ha hibás, nem indul, a hiba a naplóba megy.
- 

A port szabad-e? Ha nem, a hibaüzenet nevezze meg a foglaló programot (Windows-on lekérdezhető: `GetExtendedTcpTable` → PID → folyamatnév, pl. „port 8080 is used by skype.exe”).
- 

Indítás, majd 0,5 másodpercenként ellenőrzés, legfeljebb 12 másodpercig, hogy fut-e és figyel-e a porton.
- 

Stabilitás: 2,5 másodperc múlva ugyanazzal a PID-del kell futnia. Ha nem, leállítja (hogy ne induljon újra végtelenül), és kiírja a naplófájl utolsó 3 hibasorát.

**Leállítás:** MariaDB-nél `mariadb-admin shutdown` (rendezett leállás, különben sérülhet az InnoDB), Apache-nál a szülőfolyamat és a gyermek-folyamat leállítása; végső esetben kényszerített leállítás. **Újraindítás:** konfiguráció-teszt, az esetleg megváltozott port szabad-e, majd leállítás + indítás a fenti ellenőrzésekkel.

**Állapotfigyelés** 2 másodpercenként: fut-e, PID-ek, figyelt portok; ha egy szerver magától leáll, a naplóba kerül „Stopped unexpectedly” és a hibasorok. A felületet csak tényleges változáskor szabad frissíteni (a macOS-verzión a feleslegesen újraküldött állapot akadást okozott).

## Főablak és Áttekintés

A főablak bal oldali sávval navigál (min. 900×580, alap 1080×720), sötét és világos témával; a sáv elemei és billentyűparancsai:
| 

Elem | 

Billentyű | 

Tartalom |
| 

Overview | 

Ctrl+1 | 

szerverkártyák |
| 

Logs | 

Ctrl+2 | 

naplók (hibák száma jelvényként) |
| 

Apache | 

Ctrl+3 | 

beállítások, mellette futás-pötty |
| 

PHP | 

Ctrl+4 | 

beállítások |
| 

MariaDB | 

Ctrl+5 | 

beállítások, mellette futás-pötty |
| 

General | 

Ctrl+6 és Ctrl+, | 

az app beállításai |

A sáv alján mindig látszik: Apache és MariaDB állapota (zöld pötty + port, vagy „Stopped”). Ha egy beállításoldalon mentetlen változás van, a sávban narancs ceruza-ikon jelzi.

**Eszköztár** (jobbra fent): Database Manager, Web Folder (Intézőben), Terminal (Windows Terminal / PowerShell a webmappában), Open Website (böngészőben `http://localhost:<port>/`).

**Overview oldal:**
- 

Fejléc: „Local Server”, alatta összegzés („Everything is running at localhost:8080”, „All servers are stopped”, „1 of 2 servers running”), jobbra Start All / Stop All.
- 

Két kártya egymás mellett: **Apache** („Web server”, földgömb ikon, narancs) és **MariaDB** („Database server”, henger ikon, kékes-türkiz). Mindkettőn: állapotcímke (Running / Stopped / Working… / Not installed, színes pötty + szöveg), PORT, PID, VERSION, Start vagy Stop + Restart gomb, „Open Site” / „phpMyAdmin” gomb, „…” menü (Settings, naplók, konfigurációs fájl szerkesztése).
- 

Alattuk egy széles **PHP** kártya: verzió, „Linked” (be van-e töltve az Apache-ba), memory_limit, upload_max_filesize, napló és Settings gomb.
- 

A felhasználó kérésére **nincs** gyorslink-sáv és „Recent Activity” lista az Overview-n.

**Értesítő sávok** a tartalom tetején (beúsznak, Reduce Motion esetén csak megjelennek): első telepítés haladása / hiányzó csomag „Try Again” gombbal; új verzió (Later, What’s New, Update Now), letöltés haladása, telepítés, hiba.

**Mozgás:** lapváltáskor 0,18 s-os áttűnés csak a tartalomban; rövid, megszakítható rugós animációk (~0,28 s); a rendszer „csökkentett mozgás” beállítását tiszteletben kell tartani.

## Beállítások

Minden szerver beállításoldala űrlap, amely a valódi konfigurációs fájlt szerkeszti; mentéskor előbb maga a szerver ellenőrzi a változást, és csak a megváltozott sorokhoz nyúlunk.

**Közös mentési szabályok (mindhárom szerverre):**
- 

Az űrlap a fájl beolvasásából töltődik; a szerkesztés egy piszkozaton történik, ami lapváltáskor is megmarad (a sávban jelezve).
- 

Csak az értékek alapján változott kulcsok íródnak át; minden más sor, megjegyzés, sorrend változatlan marad. Ha semmi nem változott, a fájlhoz nem nyúlunk.
- 

Az új szöveg egy ideiglenes másolatba kerül, amit a szerver ellenőriz: `httpd.exe -t -f <tmp>`, `php.exe -c <tmp> -r "echo 'ok';"` (figyelmeztetés is hibának számít), `mariadbd.exe --defaults-extra-file=<tmp> --help --verbose` (kilépési kód ≠ 0 → hiba, az `[ERROR]` sorok az üzenetben). Ha elutasítja, a valódi fájl nem változik, a hiba megjelenik.
- 

Az első módosítás előtt az eredeti fájl mentése `<név>.lampion-backup` néven (egyszer); a fájl-menüben „Restore Original File…”.
- 

Ha a szerver fut, a gomb „Save & Restart <szerver>” (van „Save Only” is); mentés után a port- és mappaadatok újraolvasása.
- 

Hibás mező: piros keret + első probléma a mentéssávban, a Save tiltva. Revert gomb; az Esc nem állít vissza.
- 

A fájl kézi szerkesztése is támogatott: 10 másodpercenként újraolvassuk a portot és a webmappát, és az indítás mindig frissen olvassa a portot.

**Apache** (`httpd.conf`):
| 

Mező | 

Kulcs | 

Alap | 

Szabály |
| 

Port | 

`Listen` | 

8080 | 

1024–65535; cím- és protokoll-rész (`127.0.0.1:`, ` http`) megmarad; `ServerName localhost:<régi>` követi |
| 

Web folder | 

`DocumentRoot` + a hozzá tartozó `<Directory "…">` | 

`C:\Lampion\www` | 

létező, abszolút mappa, nem a meghajtó gyökere; mappaválasztóval |
| 

Administrator email | 

`ServerAdmin` | 

— | 

üresre állítva a sor kikommentelődik |
| 

.htaccess files | 

`AllowOverride` a webmappa blokkjában | 

All | 

csak változáskor írjuk (pl. `FileInfo AuthConfig` megmarad) |
| 

List folder contents | 

`Options` `Indexes` | 

be | 

ha az Options +/- formájú, `+Indexes`/`-Indexes` |
| 

Modules | 

`LoadModule` sorok ki/be kommentelése | 

rewrite be | 

rewrite, headers, deflate, expires, proxy (+proxy_http, proxy_wstunnel), vhost_alias, info; csak a megváltozott funkciók |
| 

Error log detail | 

`LogLevel` első szava | 

warn | 

emerg…debug; a modulonkénti szintek (`rewrite:trace3`) megmaradnak |

**PHP** (`php.ini`, ini-formátum: `kulcs = érték`, `;` megjegyzés; kikommentelt kulcsnál az első `;kulcs =` sort írjuk át):
| 

Mező | 

Kulcs | 

Alap | 

Szabály |
| 

Memory limit | 

`memory_limit` | 

128M | 

méret (K/M/G) > 0 vagy -1 |
| 

Maximum upload size | 

`upload_max_filesize` | 

2M | 

méret > 0 |
| 

Maximum POST size | 

`post_max_size` | 

8M | 

méret, 0 vagy -1; nem lehet kisebb a feltöltésinél |
| 

Maximum run time | 

`max_execution_time` | 

30 s | 

≥ 0 |
| 

Maximum input time | 

`max_input_time` | 

60 s | 

≥ -1 |
| 

Maximum input variables | 

`max_input_vars` | 

1000 | 

≥ 1 |
| 

Show errors / startup errors | 

`display_errors`, `display_startup_errors` | 

On | 

Development/Production gyorsgomb |
| 

Report | 

`error_reporting` | 

E_ALL | 

4 előbeállítás + „Custom” megtartva |
| 

Write errors to a log / separate log | 

`log_errors`, `error_log` | 

On / `logs\php\php_errors.log` | 

 |
| 

Time zone | 

`date.timezone` | 

Windows-időzóna | 

IANA-név, kis-nagybetű mindegy; `GMT+2` tilos |
| 

Short open tags, OPcache, Session lifetime | 

`short_open_tag`, `opcache.enable`, `session.gc_maxlifetime` | 

Off, 1, 1440 | 

 |

Az oldalon csak olvasható lista a betöltött bővítményekről (`php -m`); Windows-on ide kapcsolók is kerülhetnek a `php.ini` `extension=` soraihoz.

**MariaDB** (a Lampion saját blokkja/fájlja; üres mező = a szerver alapértéke, szürkével mutatva):
| 

Mező | 

Kulcs | 

Alap | 

Szabály |
| 

Port | 

`port` | 

3306 | 

1024–65535 |
| 

Only this computer can connect | 

`bind-address = 127.0.0.1` | 

be | 

ki: `0.0.0.0` (tűzfal-figyelmeztetéssel) |
| 

Maximum connections | 

`max_connections` | 

151 | 

> 0 |
| 

Maximum packet size | 

`max_allowed_packet` | 

16M | 

méret > 0 |
| 

InnoDB buffer pool | 

`innodb_buffer_pool_size` | 

128M | 

méret > 0 |
| 

Character set / Collation | 

`character-set-server`, `collation-server` | 

utf8mb4 / uca1400 | 

utf8mb4, utf8mb3, latin1, latin2 + rendezések (hungarian is) |
| 

SQL mode | 

`sql_mode` | 

szerver alap | 

Strict / Relaxed (`NO_ENGINE_SUBSTITUTION`) / megtartott egyedi érték |
| 

Slow query log + küszöb | 

`slow_query_log`, `slow_query_log_file`, `long_query_time` | 

ki, 2 s | 

küszöb ≥ 0 |
| 

Log every query | 

`general_log`, `general_log_file` | 

ki | 

a naplók a `logs\mariadb`-be |

Itt van még a „Connecting from apps” és a phpMyAdmin rész (lásd ott).

**General** (az app beállításai, azonnal érvényesek): Open Lampion at login (Windows: `HKCU\…\Run`), Start servers when Lampion opens, Stop servers when Lampion quits (kilépés előtt megvárja a futó műveletet; költöztetés közben a kilépés tiltva), Check for updates automatically + Check Now, Locations (mappák Megnyitás gombbal), Backups (mely konfigurációknak van eredeti mentése).

## Naplók

A Logs oldal hét forrást mutat élőben; a fájlok útvonala mindig a tényleges konfigurációból jön (`ErrorLog`, `CustomLog`, `log-error`, `error_log`), nem fixen beírva.
| 

Forrás | 

Fájl | 

Sorbontás (idő · címke · üzenet) | 

Szint |
| 

Lampion Activity | 

az app saját eseményei (memóriában, max. 2000, + `%LOCALAPPDATA%\Lampion\Lampion.log`) | 

idő, modul (Apache/MySQL/PHP/Setup/Panel/Move), szöveg | 

info / ok / warn / error |
| 

Apache Errors | 

`logs\apache\error_log` | 

`^\[([^\]]+)\] \[([^:\]]*):([a-z0-9]+)\](?: \[pid [^\]]+\])?(?: \[client [^\]]+\])? (?:AH\d+: )?(.*)$` → idő, modul, üzenet | 

`:error]` `:emerg]` `:crit]` `:alert]` = error, `:warn]` = warn |
| 

Apache Requests | 

`logs\apache\access_log` | 

`^(\S+) \S+ \S+ \[([^\]]+)\] "([^"]*)" (\d{3}) (\S+)(.*)$` → idő, státuszkód címkének, `GET /x · 236 B · ::1` | 

5xx = error, 4xx = warn (a parse-olt kódból, combined formátumnál is) |
| 

PHP Errors | 

a `php.ini` `error_log`-ja | 

`^\[([^\]]+)\] (?:PHP )?([A-Za-z ]+?):\s+(.*)$` | 

Fatal/Parse error = error, Warning/Deprecated/Notice = warn |
| 

MariaDB | 

`log-error` (ha nincs: a datadir legújabb `*.err` fájlja) | 

`^\d{4}-\d{2}-\d{2}\s+(\d{1,2}:\d{2}:\d{2})\s+\d+\s+\[([A-Za-z]+)\]\s*(.*)$` | 

`[ERROR]` = error, `[Warning]` = warn |
| 

Slow Queries | 

`slow_query_log_file` | 

nyers sor | 

info |
| 

All Queries | 

`general_log_file` | 

nyers sor | 

info |

Az időből csak az óra:perc:mp kell: `(?<![0-9])[0-9]{2}:[0-9]{2}:[0-9]{2}` (a lookbehind nélkül az access log `2026:12:06:52` dátumából rossz „26:12:06” jönne ki).

**Élő követés (tail -f), forrásonként egy háttérszálon, 1 másodpercenként:**
- 

Megnyitáskor a fájl utolsó 1 MB-ja kerül beolvasásra; ha közből kezdünk, az első (csonka) sort eldobjuk.
- 

Utána csak az új bájtok; ha a fájl kisebb lett (ürítették), kicserélték (Windows-on: fájlazonosító / létrehozási idő változott) vagy 1 MB-nál több az új rész, újrakezdjük az 1. ponttal.
- 

A befejezetlen utolsó sort (és a félbevágott UTF-8 karaktert) bájtként tartjuk meg a következő körre; CRLF és LF egyaránt sorvég.
- 

Legfeljebb 5000 sor marad meg; 2000 karakter fölött a sor levágva „…”-vel.
- 

Hiányzó fájl: üres állapot magyarázattal (pl. „Turn on Separate PHP error log in PHP settings”) és gombbal a megfelelő beállításoldalra. A fájl későbbi megjelenését is észre kell venni.
- 

A felületnek csak valódi változáskor szólunk (azonos méret → nincs újrarajzolás).

**Felület:** forrás-csipek (a kiválasztott mindig látható szűk ablakban is), szintszűrő All / Warnings N / Errors N, keresőmező (találatok kiemelve), Follow kapcsoló (az új soroknál alulra görget), menü: Copy Shown Lines, Show in Explorer, Reload, Clear (a Lampion listájánál csak a lista, fájlnál megerősítés után csonkolás). A sor: idő · színes címke · üzenet monospace betűvel; hibánál piros, figyelmeztetésnél narancs háttér és bal oldali csík. Állapotsor: sorok száma, hibák, figyelmeztetések, fájlútvonal, méret, „Live”. A szűrést/számolást csak a sorok, a szűrő vagy a keresés változásakor szabad újraszámolni (5000 sornál ez számít).

## Csatlakozás programokból

Az XAMPP-hoz hasonlóan a `root` felhasználó jelszó nélkül léphet be, de csak erről a gépről; a phpMyAdmin belépési oldal nélkül nyílik. Mindkettő kikapcsolható, és az első indításkor egyszer alapból bekapcsolódik; ha a felhasználó utána máshogy dönt, azt többé nem írjuk felül.

**root jelszó nélkül** (Windows-on nincs unix socket, így a TCP a fő út; a `mariadb-install-db` után a root általában már üres jelszavú, ezt ellenőrizni kell):
```
CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '';
GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;
CREATE USER IF NOT EXISTS 'root'@'::1' IDENTIFIED BY '';
GRANT ALL PRIVILEGES ON *.* TO 'root'@'::1' WITH GRANT OPTION;
ALTER USER 'root'@'localhost' IDENTIFIED BY '';
FLUSH PRIVILEGES;
```

Kikapcsoláskor a felhasználó jelszót ad meg (a macOS-verzió itt a socket-belépésre esett vissza; Windows-on jelszó kell). Soha ne legyen `root@'%'`. Állapot-ellenőrzés: TCP-belépés próba `root` + üres jelszóval; `1045`/`1698` = nincs engedélyezve, nem futó szerver = ismeretlen (a kapcsoló ilyenkor tiltott).

**phpMyAdmin automatikus belépés** (`config.inc.php`, a változás előtt `php -l` szintaxis-ellenőrzéssel):
```
$cfg['Servers'][$i]['auth_type'] = 'config';
$cfg['Servers'][$i]['host'] = '127.0.0.1';
$cfg['Servers'][$i]['user'] = 'root'; // Lampion: automatic sign-in
$cfg['Servers'][$i]['password'] = ''; // Lampion: automatic sign-in
$cfg['Servers'][$i]['AllowNoPassword'] = true;
```

Kikapcsolva: `auth_type = 'cookie'`, `AllowNoPassword = false`, a jelölt sorok törlése. Első telepítéskor egy 32 bájtos `blowfish_secret` generálása. A biztonságot az Apache `Require local` szabálya adja a `/phpmyadmin` alias-on.

**„Connecting from apps” rész a MariaDB oldalon:** Host `127.0.0.1`, Port (a tényleges), User `root`, Password „(empty)”, mindegyik másoló gombbal; egy adatbázisnév-mező és „Copy Settings” menü kész beállításokkal:
| 

Cél | 

Vágólapra kerülő szöveg |
| 

PHP (PDO) | 

`$pdo = new PDO('mysql:host=127.0.0.1;port=3306;dbname=DB;charset=utf8mb4', 'root', '');` |
| 

PHP (mysqli) | 

`$db = new mysqli('127.0.0.1', 'root', '', 'DB', 3306);` |
| 

Laravel (.env) | 

`DB_CONNECTION=mysql`, `DB_HOST=127.0.0.1`, `DB_PORT=3306`, `DB_DATABASE=DB`, `DB_USERNAME=root`, `DB_PASSWORD=` (külön sorokban) |
| 

Prisma (.env) | 

`DATABASE_URL="mysql://root@127.0.0.1:3306/DB"` |
| 

Node.js (mysql2) | 

`const db = await mysql.createConnection({ host: '127.0.0.1', port: 3306, user: 'root', password: '', database: 'DB' });` |
| 

Python | 

`db = mysql.connector.connect(host="127.0.0.1", port=3306, user="root", password="", database="DB")` |
| 

Java (JDBC) | 

`jdbc:mariadb://127.0.0.1:3306/DB?user=root&password=` |
| 

Connection URL | 

`mysql://root@127.0.0.1:3306/DB` |

(`DB` = a beírt adatbázisnév, üresen `your_database`; a port a tényleges port.)

## Adatbázis-kezelő

Külön ablak (alap 1120×720), TablePlus-szerű elrendezéssel: bal oldalt az adatbázisok fája, felül az eszköztárban a fülek, középen a kiválasztott elem fejléce és tartalma, alul az SQL-konzol. Rustban az adatbázist natív driverrel érjük el (`mysql_async` vagy `sqlx`, TCP `127.0.0.1`, root), nem parancssori kliensen át, ahogy a macOS-verzió tette; export/importhoz a `mariadb-dump.exe` / `mariadb.exe` jó.

**Oldalsáv.** „Databases” szakasz a saját adatbázisokkal, alatta összecsukható „System” szakasz (`information_schema`, `mysql`, `performance_schema`, `sys`), alapból csukva, állapota megjegyezve; kereséskor kinyílik. Adatbázis kibontásakor töltjük be a táblákat (csak a betöltés után nyílik, így nem ugrál). Sorok: henger ikon + név + táblaszám; tábla ikon (kék) vagy nézet ikon (lila) + név + becsült sorszám tömören (1.2k). Szűrőmező felül. Lábléc: kapcsolat pötty + „MariaDB 11.x”, „…” menü (Show System Databases, Refresh), „+” új adatbázis. Jobb klikk: táblán Open, Export…, Empty Table…, Table Options…, Delete…; adatbázison Export…, New Table…, Import SQL…, Delete Database… (mindegyik törlés megerősítéssel).

**Fülek a kiválasztástól függően** (szegmentált választó az eszköztár közepén, ikonnal; fülváltáskor 0,2 s áttűnés):
| 

Kiválasztva | 

Fülek |
| 

semmi | 

Query |
| 

adatbázis | 

Tables · Designer · Query |
| 

tábla / nézet | 

Data · Structure · Query |

Táblák között váltva a Structure és a Query fül megmarad, egyébként Data; adatbázisok között a Designer és a Query marad. Eszköztár még: Refresh, New (adatbázis/tábla), Export, Import; a Refresh ikon pulzál, ha egy művelet 0,25 s-nál tovább tart.

**Fejléc** (56 px, a Query fülön rejtve): táblánál ikon + név + adatbázis, jobbra motor · ~sorok · méret · rendezés; adatbázisnál „N tables · N views · méret · rendezés”, a Tables fülön New Table… / Export Database… / Import SQL… gombokkal.

**Tables fül:** rendezhető lista (Name ikonnal, Rows jobbra igazítva, Size arányos sávval, Engine, Collation); nézetnél a sor- és méretoszlopban „—”; dupla kattintás megnyitja; több kijelölt elem törlése.

**Data fül:**
- 

Felső sáv: „WHERE” címkés szűrőmező (példaszöveg halványan: `e.g. id > 10 AND name LIKE 'a%'`), Enter alkalmazza; újratöltés, új sor, kijelölt sorok törlése, sorrészletező ki/be. Nézetnél szerkesztés tiltva.
- 

Rács (gyors, virtualizált táblázat): kétsoros oszlopfejléc (név + típus, pl. `varchar(100)`), elsődleges kulcsnál kulcs-ikon, számoszlopok jobbra igazítva, NULL halvány dőlt „NULL” (az üres sztring üres cella), többsoros érték „⏎” jellel egy sorban; sorag 24 px, váltakozó sorháttér csak a valódi sorok mögött; oszlopszélesség a fejlécből és az első 60 sorból (max. 380 px). Fejlécre kattintva rendezés (ASC/DESC), dupla kattintás a cellán szerkesztés. Jobb klikk: Copy Cell, Copy Row (tabbal tagolt), Copy Row as INSERT, Copy Column Name, Set to NULL; Ctrl+C a kijelölt sorokat másolja. Szerkesztés után a görgetés és a kijelölés megmarad.
- 

Alsó sáv: „1–500 of 1 234 rows · N selected”, rendezés-csip (kattintva törli), előző/következő oldal (500 soros lapok).
- 

Sorrészletező (jobb oldali 300 px-es panel, becsúszó animációval): a kijelölt sor minden mezője névvel, típus-címkével, kulcs-jelvényekkel, NULL jelölőnégyzettel, hosszú szövegnél többsoros szerkesztővel; módosított mező kiemelve; Save / Revert. Bináris oszlop és nézet csak olvasható.
- 

Új sor ablak: oszloponként mező, „automatic” / alapérték / típus helykitöltővel, kötelező mezők jelölve, ENUM/SET választóval.

**Structure fül** (egy görgethető oldal kártyákkal, max. 1000 px széles): fejlécben motor, rendezés, sorok, méret + Table Options…; **Columns** (sorszám, név, megjegyzés/rendezés alatta, típus-címke, jelvények PK / UNIQUE / INDEX / FK / AUTO / NOT NULL, alapérték; rámutatva szerkesztés/törlés ikon, dupla kattintás szerkeszt); **Indexes** (név, oszlopok csipekként sorrendben, PRIMARY/UNIQUE, típus); **Foreign Keys** (`oszlop → tábla.oszlop`, ON UPDATE/ON DELETE csipek, a hivatkozott táblára kattintva megnyílik); **CREATE statement** (alapból csukva, színezett SQL, Copy). Nézetnél: „Views are read-only here”. Ablakok: Add/Edit Column (név, megjegyzés, típus menüből + hossz, Unsigned, NULL, alapérték, ON UPDATE CURRENT_TIMESTAMP, pozíció; az SQL előnézete látszik), Add Index (típus, név, oszlopok sorrenddel), Add Foreign Key, Table Options (átnevezés/áthelyezés, másolás adattal vagy anélkül, motor, rendezés, OPTIMIZE / REPAIR / ANALYZE / CHECK), New Table (oszlopsorok PK/AUTO/NULL kapcsolókkal), New Database (név ellenőrzéssel, rendezés magyarázattal).

**Query fül:** felső sáv Run (Ctrl+R / Ctrl+Enter), adatbázis-választó („Selected in sidebar” vagy egy konkrét), History (utolsó 30 lekérdezés, mentésre kerül), Clear; jobbra az utolsó futás állapota (✓ „1 result, 10 rows · 0,035 s” vagy piros „Failed”). Szerkesztő: sorszámok, SQL-színezés, javaslatok gépelés közben (kulcsszavak, függvények, az aktuális adatbázis táblái és oszlopai, `t.` után az alias oszlopai; Tab fogadja el), hibánál a hibás rész aláhúzva az első szerkesztésig. Eredmény: „Result” sáv összegzéssel és Hide/Show gombbal (teljesen összecsukható, az állapot megjegyezve), több eredményhalmaznál választható csipek, hibánál piros kártya másoló gombbal, sorokat nem adó utasításnál „Query Finished” + érintett sorok száma.

**SQL-konzol** (az ablak alján): összecsukva 28 px-es sáv (utolsó utasítás egy sorban, hibánál pirosan, darabszám, nyíl); kinyitva +180 px lista (minden lefuttatott utasítás, hibák piros ikonnal), Copy All, Clear. A konzol és a töltésjelző változása ne rajzolja újra az egész ablakot.

**Designer fül** (phpMyAdmin Designer-szerű ER-diagram): pöttyözott, görgethető, nagyítható vászon (egérgörgő görget, Ctrl+görgő / csípés nagyít a mutató körül, Ctrl+0 Fit). Tábla = kártya (fejléc ikon + név, nézet lila „VIEW” címkével; oszlopsorok kulcs/kapcsolat ikonnal, név, tömör típus; 24 oszlop felett az első 16 + kulcsok + „+N more”). Idegen kulcs = ívelt lila vonal a FK-oszlop sorától a hivatkozott oszlop soráig, nyílhegy a hivatkozott oldalon, önhivatkozás hurokként. Rámutatásra a kapcsolódók kiemelve, a többi elhalványul, vonalon tooltip: `orders.customer_id → customers.id · ON DELETE CASCADE`. Automatikus elrendezés (a hivatkozott táblák balra, összefüggő csoportok blokkokban, kapcsolat nélküliek jobbra), húzható kártyák, pozíciók adatbázisonként mentve, Auto Arrange, „Relations only” szűrő, számláló („3 tables · 1 view · 1 relation”). Új kapcsolat: az oszlop melletti karikát egy másik tábla oszlopára húzva megerősítő ablak nyílik (Swap, ON DELETE/ON UPDATE, javasolt név `fk_<tábla>_<oszlop>`, az SQL látszik; figyelmeztet típuseltérésre, index nélküli célra, SET NULL + NOT NULL ütközésre, nem InnoDB táblára); a MariaDB hibája az ablakban jelenik meg. Törlés: jobb klikk a vonalon → Delete Relation…. Dupla kattintás a táblán megnyitja a szerkezetét. A sémát adatbázisonként gyorsítótárazzuk: visszaváltáskor azonnal látszik, a háttérben frissül; az új diagram áttűnik, a töltésjelző csak 0,45 s után jelenik meg; ~100 táblánál is folyamatos maradjon.

**Használt SQL** (azonosítók backtickkel, a backtick duplázva; értékek `'`-ben, `'` és `\` escape-elve — vagy inkább paraméterezett lekérdezés):
| 

Mire | 

Lekérdezés |
| 

Adatbázisok | 

`SHOW DATABASES` |
| 

Táblák | 

`SELECT TABLE_NAME, TABLE_TYPE, IFNULL(ENGINE,''), IFNULL(TABLE_ROWS,''), IFNULL(DATA_LENGTH+INDEX_LENGTH,0), IFNULL(TABLE_COLLATION,'') FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME` |
| 

Tábla részletei | 

`SHOW FULL COLUMNS FROM db.t; SHOW INDEX FROM db.t; SHOW CREATE TABLE db.t;` + idegen kulcsok: `information_schema.KEY_COLUMN_USAGE` ⋈ `REFERENTIAL_CONSTRAINTS` (név, oszlop, hivatkozott séma/tábla/oszlop, UPDATE_RULE, DELETE_RULE) |
| 

Adatok | 

`SELECT * FROM db.t [WHERE <szűrő>] [ORDER BY col ASC/DESC] LIMIT 500 OFFSET n; SELECT COUNT(*) FROM db.t [WHERE …]` |
| 

Sor módosítása | 

`SELECT COUNT(*) FROM db.t WHERE <egyezés>; UPDATE db.t SET … WHERE <egyezés> LIMIT 1` — egyezés: elsődleges kulcs, különben egy UNIQUE NOT NULL oszlop, különben minden oszlop, `<=>` (NULL-biztos) összehasonlítással; ha a COUNT 0, figyelmeztetés, nem csendes kudarc |
| 

Designer séma | 

`information_schema.COLUMNS` (név, típus, kulcs, nullable, sorrend) + `KEY_COLUMN_USAGE` ⋈ `REFERENTIAL_CONSTRAINTS` az egész adatbázisra, egy körben |
| 

Kapcsolat fel / le | 

`ALTER TABLE db.t ADD CONSTRAINT fk FOREIGN KEY (col) REFERENCES db.r (rcol) ON DELETE … ON UPDATE …` · `ALTER TABLE db.t DROP FOREIGN KEY fk` |
| 

Lekérdezés futása | 

a felhasználó szövege több utasítással, minden eredményhalmaz külön; érintett sorok: `ROW_COUNT()` |
| 

Export / Import | 

`mariadb-dump.exe --databases db [tábla]` fájlba (alapmappa `backups\`) · SQL-fájl lefuttatása a kiválasztott adatbázisban |

## Frissítés és terjesztés

Az app GitHub-kiadásokból frissíti magát; a telepítő és a frissítés ugyanazt a csomagot használja.

**Kiadás formátuma** (mint a macOS-verzióé): tag `vX.Y`, egy telepítő-asset (`Lampion-X.Y-setup.exe` vagy `.msi`), a leírásban „## What's new”, „## Install”, „## Requirements” és „SHA-256 of the installer: `…`”. A frissítő a leírásból csak a „What's new” részt mutatja.

**Frissítő:**
- 

Indításkor (ha engedélyezett) `GET https://api.github.com/repos/<owner>/lampion/releases/latest`, `Accept: application/vnd.github+json`, 15 s időtúllépés.
- 

Verzió-összehasonlítás számrészenként (`1.10` > `1.9`, `v` előtag elhagyva); kihagyott verzió megjegyezve („Skip This Version”), kézi ellenőrzésnél újra felajánlva.
- 

Letöltés haladássávval (megszakítható), SHA-256 ellenőrzése az asset GitHub által adott `digest` mezőjével.
- 

Aláírás ellenőrzése (Authenticode, ha aláírt), majd a telepítő csendes futtatása az app bezárása után, és az új verzió indítása. A szerverek közben futhatnak tovább (frissítéskor a „Stop servers on quit” nem érvényes).
- 

Hibánál: érthető üzenet + „Download Manually” (a kiadások oldala) + „Try Again”.

A repo-cím (`owner/repo`) fordítási beállítás legyen, ne fixen a kódban, hogy egy fork a saját kiadásaiból frissíthessen.

**Telepítő:** WiX (MSI) vagy Inno Setup/NSIS: a program a `%LOCALAPPDATA%\Programs\Lampion` alá (adminjog nélkül), Start menü parancsikon, VC++ Redistributable ellenőrzése. A szervereket és az adatokat nem a telepítő, hanem az első indítás teszi a `C:\Lampion` mappába; eltávolításkor a `C:\Lampion` marad (adatvédelem), erre a telepítő figyelmeztessen.

**Aláírás:** aláírás nélkül a SmartScreen figyelmeztet („More info → Run anyway”) — a macOS-verzió „Open Anyway” lépésének megfelelője, ezt a kiadás leírása magyarázza el. Hosszú távon kódaláíró tanúsítvány (pl. Azure Trusted Signing).

**Ikon:** lapos, csak formákból álló papírlámpás fekete csempén: sötét csempe (`#2A2A30` → `#111114` függőleges színátmenet, lekerekített négyzet), narancs kör (`#FF7A33`) a lámpás teste, középen világosabb ellipszis (`#FFC15E`), bordák vonalai (`#D9561F`), fehér fedőlapok és fül (`#F2F2F5`), narancs kapszula bojt. A pontos geometria a macOS-repo `Tools/make-icon.swift` fájljában; Windowsra `.ico` (16, 24, 32, 48, 64, 128, 256 px).

## Buktatók

Ezeket a hibákat a macOS-verzió fejlesztése közben találtuk és javítottuk; a Windows-verzióban eleve el kell kerülni őket.
| 

Terület | 

Mi volt a hiba | 

Teendő Windows-on |
| 

Szinkronizált mappa | 

iCloud-mappában a build és az adatfájlok sérültek | 

soha ne OneDrive-ba (Dokumentumok/Asztal) és ne `Program Files`-ba tegyünk adatot |
| 

Indító felülírja a beállítást | 

a Homebrew parancssorból adta meg a régi `--datadir`-t, a beállítás hatástalan volt | 

a szerver indítási paramétereit a Lampion maga állítsa össze; költöztetés után `SELECT @@datadir`-rel ellenőrizni, hogy tényleg az új helyről fut |
| 

Gyorsítótárazott port | 

kézi `httpd.conf`-szerkesztés után a régi portot ellenőrizte | 

indításkor mindig frissen olvasni a portot |
| 

Változatlan mentés is átírt | 

`AllowOverride FileInfo` → `All`, `LogLevel … rewrite:trace3` csonkolva, proxy-modulok kikommentelve | 

csak az érték szerint megváltozott kulcsokat írni |
| 

`Listen` formák | 

`Listen [::1]:8080` → `[:1]:9090`; `Listen 8080 http` protokoll elveszett | 

a portot az utolsó `:` után vágni, cím- és protokoll-részt megtartani |
| 

`Options` +/- | 

`Options +FollowSymLinks -Indexes` mellé `Indexes` → érvénytelen konfig | 

vegyes formát nem írni |
| 

Idézőjeles ini-érték | 

`"a;b" ; megjegyzés` rosszul olvasva | 

záró idézőjelig olvasni |
| 

Túl nagy méret | 

`9000000000G` túlcsordulás, összeomlás | 

ellenőrzött szorzás (Rustban `checked_mul`) |
| 

Időzóna-ellenőrzés | 

`CET`, `US/Pacific` elutasítva, `GMT+2` elfogadva (a PHP fordítva) | 

a PHP saját listájához igazítani |
| 

Kulcs nélküli tábla | 

FLOAT oszlop miatt az `UPDATE … WHERE` 0 sort talált, a módosítás csendben elveszett | 

előtte `COUNT(*)`, 0 esetén figyelmeztetés; UNIQUE NOT NULL oszlop előnyben |
| 

Napló idő | 

`2026:12:06:52`-ből „26:12:06” | 

lookbehind a regexben |
| 

Napló követése | 

félbevágott UTF-8 karakter, cserélt fájl, nagy ugrás | 

bájtszintű maradék, fájlazonosító figyelése, 1 MB-os plafon |
| 

Naplófájl neve | 

a MariaDB-napló a gépnévből készül, gépnév-váltás után a régit mutatta | 

`log-error` fix útvonalra állítva |
| 

Mentetlen szerkesztés | 

lapváltáskor és Esc-re elveszett | 

piszkozat megtartása, Esc ne állítson vissza |
| 

Kilépés | 

a leállítás a főszálon fagyott, a közben induló szervert kihagyta | 

háttérben, a futó művelet után; költöztetés közben nincs kilépés |
| 

Költöztetés visszagörgetése | 

félkész másolat blokkolta az újrapróbálást; egy ponton adatmappa nélkül maradhatott | 

minden lépést azonnal nyilvántartani; félkész másolatot csak akkor törölni, ha az eredeti biztosan a helyén van |
| 

Akadó felület | 

2 s-onkénti változatlan állapot, lekérdezésenkénti naplósor az egész ablakot újrarajzolta; a Designer adatbázis-váltáskor eltűnt, kétszer töltött | 

csak valódi változást küldeni a felületnek; gyorsítótár, egy betöltés, áttűnés |
| 

phpMyAdmin belépés | 

`cookie` mód + tiltott üres jelszó → belépési oldal | 

`config` mód root-tal (lásd Csatlakozás) |
| 

Tűzfal / port | 

(Windows-specifikus) `0.0.0.0`-n figyelés tűzfal-ablakot dob; a 80-as portot gyakran más foglalja | 

alapból `127.0.0.1` és 8080; a foglaló programot néven nevezni |
| 

Sorvégek | 

(Windows-specifikus) a konfigurációkban CRLF lehet | 

olvasáskor CRLF-et kezelni, íráskor az eredeti sorvéget megtartani |

## Elfogadási tesztek

A kész app akkor jó, ha az alábbiak mind teljesülnek; a macOS-verzióhoz hasonlóan legyen beépített önteszt (`--selftest`, `--dbtest` kapcsolóval, eredmény a naplóba), amely ezek nagy részét gépi úton ellenőrzi.

**Szerverek**
- 

Tiszta Windows 11-en az első indítás letölti és beállítja a csomagokat, majd mindkét szerver elindul.
- 

`http://localhost:8080/` és `/phpmyadmin/` 200-as választ ad; a phpMyAdmin belépés nélkül nyílik.
- 

Stop All után egyik szerver sem fut; Start All után mindkettő fut, PID és port látszik.
- 

Foglalt 8080-as portnál az Apache nem indul, és a hiba megnevezi a foglaló programot.
- 

Hibás `httpd.conf`-nál az Apache nem indul, a hiba a naplóban.
- 

Az app bezárása és újranyitása után a futó szervereket újra észleli.

**Beállítások**
- 

Változatlan mentés nem módosítja a fájlokat (bájtra azonosak).
- 

Port-, webmappa-, modul-, PHP-limit- és MariaDB-változás mentve és újraindítás után érvényes; az eredeti fájl `.lampion-backup` néven megvan, és visszaállítható.
- 

Érvénytelen értéket a szerver elutasít, a fájl változatlan marad, a hiba látszik.
- 

Mentetlen szerkesztés lapváltás után is megvan.

**Naplók**
- 

Egy 404-es kérés 1 másodpercen belül megjelenik az Apache Requests-ben narancs jelöléssel, helyes idővel.
- 

Egy PHP-hiba megjelenik a PHP Errors-ban; a naplófájl ürítése után az új sorok újra jönnek.

**Csatlakozás**
- 

`mysql -h 127.0.0.1 -u root` jelszó nélkül belép; másik gépről nem.
- 

A „Copy Settings” PDO-sora egy PHP-fájlba másolva működik.

**Adatbázis-kezelő**
- 

Adatbázis és tábla létrehozása, oszlop hozzáadása/módosítása/törlése, index és idegen kulcs hozzáadása/törlése, átnevezés, másolás, ürítés, törlés működik.
- 

Sor beszúrása, cella és egész sor szerkesztése, NULL beállítása, sorok törlése működik; kulcs nélküli, FLOAT-os táblánál figyelmeztet, nem csendesen bukik el.
- 

Szűrés, rendezés, lapozás 500 soros lapokkal; hibás szűrőnél hibaüzenet.
- 

Lekérdezés több utasítással, több eredménnyel; hibánál a hely jelölve; előzmények újraindítás után is megvannak.
- 

Export és visszaimport után az adatbázis azonos.
- 

Designer: a kapcsolatok vonalként látszanak; húzással új idegen kulcs jön létre, típuseltérésnél a MariaDB hibája az ablakban; törlés működik; 100 táblánál is folyamatos.
- 

Adatbázisok és fülek közötti váltáskor nincs üres képkocka vagy ugrás (képernyőfelvételen ellenőrizve).

**Frissítés**
- 

Egy magasabb verziójú teszt-kiadást felajánl, letölt, ellenőriz és telepít; rossz kivonatnál megtagadja.

## Ütemterv

Javasolt sorrend: minden lépés végén legyen egy használható, kipróbálható állapot.
- 

**Váz:** Rust-projekt a választott UI-val, főablak oldalsávval, sötét/világos téma, naplózó réteg (memória + fájl), beállítások tárolása.
- 

**Szerverek:** csomag-manifest, letöltés + kivonat + kicsomagolás, konfiguráció-generálás, MariaDB inicializálás, indítás/leállítás/újraindítás ellenőrzésekkel, állapotfigyelés, Overview oldal. Elfogadás: a „Szerverek” tesztek.
- 

**Csatlakozás:** root jelszó nélkül, phpMyAdmin automatikus belépés, csatlakozási adatok és kész beállítások.
- 

**Beállítások:** konfiguráció-szerkesztő réteg (Apache-direktívák, ini, MariaDB-blokk) egységtesztekkel a „Buktatók” eseteire, majd az négy beállításoldal.
- 

**Naplók:** fájlkövető + sorbontó egységtesztekkel, Logs oldal.
- 

**Adatbázis-kezelő alapok:** kapcsolat, oldalsáv, Tables, Data (rács, szűrés, rendezés, lapozás, szerkesztés, sorrészletező), Query, konzol, export/import.
- 

**Szerkezet-szerkesztés:** Structure fül és az összes ablak (oszlop, index, idegen kulcs, tábla-beállítások, új tábla/adatbázis).
- 

**Designer:** séma-betöltés, automatikus elrendezés, vászon (görgetés, nagyítás), kapcsolatok rajzolása, húzással új kapcsolat, gyorsítótár.
- 

**Terjesztés:** ikon, telepítő, frissítő, GitHub-kiadás, öntesztek (`--selftest`, `--dbtest`).
- 

**Finomítás:** animációk, teljesítmény (100 tábla, 5000 naplósor), képernyőfelvételes ellenőrzés, a teljes elfogadási lista végigvitele.
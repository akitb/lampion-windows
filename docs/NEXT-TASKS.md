# Következő feladatok

A tulajdonos kérései, amiket még nem csináltunk meg. Csak a tulajdonos szólására kezdjük el őket.

## 1. A telepítő tegyen fel mindent (felvéve: 2026-11-23)

**Most:** a `Lampion-X.Y-setup.exe` csak magát az appot telepíti. Az Apache-ot, a PHP-t, a MariaDB-t és a
phpMyAdmint az app **első indításakor** tölti le és állítja be (a főablak tetején a „setup” sáv).

**Kérés:** a telepítő futtatásakor kerüljön fel minden, hogy az app első indításakor már ne kelljen semmit
letölteni vagy beállítani; a szerverek azonnal indíthatók legyenek.

**Megoldási lehetőségek (dönteni kell):**
- **a) A telepítő tölti le:** a NSIS-telepítő a `manifest.json` alapján letölti és SHA-256-tal ellenőrzi a
  csomagokat, kicsomagolja őket a `C:\Lampion\bin` alá, és lefuttatja az első beállítást (konfigurációk,
  `mariadb-install-db`, root jelszó nélkül, phpMyAdmin automatikus belépés). Kicsi marad a telepítő (~4 MB),
  de telepítéskor internet kell. Az app saját setup-logikáját érdemes újrahasználni: a telepítő a végén
  csendben futtatja a `lampion.exe --setup` kapcsolót (ezt kell megírni), és megvárja a végét, haladásjelzővel.
- **b) Minden benne van a telepítőben:** a csomagokat a kiadáskor a GitHub Actions tölti le és teszi be a
  telepítőbe. Offline is települ, de a telepítő ~150–200 MB lesz, és a szerverek frissítéséhez új kiadás kell.

**Figyelni:**
- A mappaválasztás (alapból `C:\Lampion`, soha nem Program Files vagy OneDrive) a telepítőben legyen.
- A Visual C++ runtime ellenőrzése már most is a telepítőben van; maradjon előtte.
- Frissítéskor (`/UPDATE`) a telepítő ne futtassa újra a szerverek beállítását, és ne írja felül a meglévő
  konfigurációkat és adatbázisokat.
- Eltávolításkor a `C:\Lampion` maradjon meg (adatvédelem), ahogy most.
- Ha a telepítő nem tudja befejezni (nincs internet, hibás letöltés), az app első indításkor pótolja, ahogy most.

## 2. Első indítás friss Windowson: a szerverek nem indulnak (felvéve: 2026-11-23)

Egy friss Windows VM-en kiderült hibák (a tulajdonos gépén minden működött):
- **VC++ runtime:** ha a telepítése újraindítást kér, a Lampion mégis elindítja a szervereket; az Apache
  hibaüzenet nélkül leáll. Teendő: ilyenkor „Restart Windows…” állapot, a szerverek csak újraindítás után induljanak.
- **`httpd -t` mint „futó Apache”:** lassú gépen az állapotfigyelő a konfigurációtesztet futó Apache-nak nézi,
  ezért az Apache indítása kimarad („Started (externally)”, majd „Stopped unexpectedly”). Teendő: a valódi
  szervert a parancssora alapján felismerni.
- **MariaDB 11.8 RSA-kulcs:** azon a gépen a kulcsgenerálás nem sikerül, ezért a `caching_sha2_password`
  modul és az automatikus SSL miatt a MariaDB nem indul. Kézi megoldás, ami működött: a `my.ini` `[mysqld]`
  részébe `caching_sha2_password=OFF` és `skip-ssl`. Teendő: új telepítésnél `caching_sha2_password=OFF`
  alapból; ha az indítás ezzel a hibával bukik, a Lampion maga írja be a hiányzó sort/sorokat (mentéssel),
  és újrapróbálja. A Lampion saját MariaDB-eszközei (mariadb-admin, mariadb-dump, mariadb.exe) SSL nélkül is
  működjenek.

**Félkész munka:** egy agent már elkezdte, a változtatásai a `git stash` első elemében vannak
(„WIP: first-run fix on fresh Windows …”). Folytatás: `git stash pop`, átnézni, befejezni, tesztelni.
Utána kiadás `v1.0.2` néven.

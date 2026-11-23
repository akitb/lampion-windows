# Lampion for Windows

A local web development control panel, in the spirit of XAMPP: one window that sets up, starts and
watches **Apache**, **PHP**, **MariaDB** and **phpMyAdmin**, edits their settings, shows their logs live,
and has its own database manager. The Windows version of [Lampion for macOS](https://github.com/akitb/lampion),
built with Rust (Tauri 2) and Svelte.

## Features

- **Servers:** first launch downloads Apache, PHP (thread safe), MariaDB and phpMyAdmin, checks their
  SHA-256 checksums and configures them. Start, stop and restart with checks (busy ports name the program
  that holds them, a broken `httpd.conf` is caught before Apache starts), status every 2 seconds, and
  optionally Windows services that start with Windows.
- **Everything in one folder:** sites, databases, logs and backups live in `C:\Lampion` (or a folder you
  choose), never in Program Files or OneDrive.
- **Settings:** Apache, PHP and MariaDB settings as forms that edit the real configuration files. Only the
  values you change are written, the server checks every change on a copy first, and the original file is
  kept as `<name>.lampion-backup`.
- **Logs:** Lampion's own activity and the Apache, PHP and MariaDB logs, followed live, filtered by level and
  searchable.
- **Connecting from apps:** `root` without a password, from this computer only; phpMyAdmin without a login
  page; ready-made connection settings for PDO, mysqli, Laravel, Prisma, Node.js, Python and JDBC.
- **Database manager:** data, structure, queries, a Designer with the relations of your tables, import
  and export.
- **Updates:** Lampion finds new releases on GitHub and updates itself in a few seconds; the servers keep
  running.

## Install

1. Download `Lampion-X.Y-setup.exe` from the [latest release](https://github.com/akitb/lampion-windows/releases/latest)
   and open it. It installs Lampion for your Windows account only, without administrator rights, into
   `%LOCALAPPDATA%\Programs\Lampion`, and adds it to the Start menu.
2. **SmartScreen:** the installer is not code-signed yet, so Windows may say "Windows protected your PC".
   Click **More info**, then **Run anyway**. Each release lists the installer's SHA-256 checksum, which you can
   compare with `Get-FileHash Lampion-X.Y-setup.exe` in PowerShell.
3. Apache and PHP need the Microsoft Visual C++ runtime. When it is missing, the installer offers it and
   Windows asks for permission once.
4. On first launch, choose where the Lampion folder goes (`C:\Lampion` by default). Lampion downloads the
   servers, sets them up and starts them. Your site is at <http://localhost:8080/>, phpMyAdmin at
   <http://localhost:8080/phpmyadmin/>, MariaDB at `127.0.0.1:3306` (user `root`, no password).

Requirements: Windows 10 version 1809 or later, or Windows 11, 64-bit; about 1 GB of free disk space; an
internet connection for the first setup and for updates.

## Folder layout

```
C:\Lampion\
├── bin\apache, bin\php, bin\mariadb, bin\phpmyadmin   the servers
├── config\        httpd.conf, php.ini, my.ini (Lampion writes them; edit them by hand too)
├── www\           your websites (the web folder)
├── databases\     MariaDB's data
├── logs\apache, logs\mariadb, logs\php
├── backups\       database exports
├── tmp\           PHP sessions and uploads
└── README.txt     what is where
```

Lampion's own files: settings in `%APPDATA%\Lampion\settings.json`; its activity log, downloads and
self-test results in `%LOCALAPPDATA%\Lampion`.

## Updates

Lampion looks for a new release each time it opens (General › Check for updates automatically; Check Now
looks at once). The update banner shows what is new; **Update Now** downloads the installer, checks it
against the SHA-256 checksum GitHub lists for it (and its signature, once releases are signed), installs it
and opens Lampion again. A version you skip is offered again when you check by hand.

## Uninstall

Remove Lampion in Settings › Apps. The Lampion folder (`C:\Lampion`) with your servers, websites and
databases is kept; delete it yourself when you no longer need it. Servers that are still running keep
running until you restart Windows, so stop them in Lampion first.

## Command-line options

| Option | What it does |
| --- | --- |
| `--selftest` | Checks setup, configuration saves (on copies), stopping and starting, a taken port, the website, phpMyAdmin, MariaDB and the logs, then quits with exit code 0 (passed) or 1. Results: Logs › Lampion Activity and `%LOCALAPPDATA%\Lampion\selftest.log`. Stops and starts the servers. |
| `--dbtest` | Checks database, table, row, index and foreign key work in a throwaway database `_lampion_teszt`, and an export; same results and exit code. MariaDB must be running. |
| `--preview-update <state>` | Shows the update banner in one state without asking GitHub: `available`, `downloading`, `installing`, `uptodate` or `failed`. |
| `--auto-update` | Checks for a newer release and installs it without asking. |

## Building from source

You need Windows 10 or 11 with the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
(Microsoft C++ Build Tools, WebView2), [Rust](https://rustup.rs/) (stable, MSVC) and Node.js 22.

```powershell
npm ci
npm run tauri dev                       # run from source
npm run tauri build -- --bundles nsis   # the installer, in src-tauri\target\release\bundle\nsis
cd src-tauri; cargo test                # unit tests
npm run check                           # Svelte and TypeScript checks
```

The updater and first setup read the releases of `akitb/lampion-windows`. A fork sets its own repository at
build time: `$env:LAMPION_REPO = "you/lampion-windows"` before building (the release workflow does this
automatically).

## Releasing

Push a tag `vX.Y` (or `vX.Y.Z`). The [release workflow](.github/workflows/release.yml) builds the installer
on GitHub, names it `Lampion-X.Y-setup.exe` and publishes the release with `manifest.json` (the server
package list first setup reads) and notes in the form the updater expects: **What's new** comes from the
tag's message (`git tag -a v1.2 -m "- What changed"`), or from the commit subjects since the previous tag.

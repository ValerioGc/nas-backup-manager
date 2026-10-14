# nas-backup-manager

External application for the NAS launcher: it configures and runs scheduled and real-time backups (cloud backups can be configured, but are not run yet). Every repository stands on its own and produces an archive the launcher can install.

## What a rule does

A rule has a name; it is turned on and off, edited and deleted from the list, where every row shows the schedule (for example "Weekly · 03:00", and "Every Monday at 03:00" on hover; not in the real-time section), the outcome of the last run (an icon: done, failed, interrupted, in progress; on hover, date, time and the reason if any), the next run and the commands to start it at once or stop it. A running rule can be neither edited nor deleted.

- **Folders**: source and destination, chosen with the Explorer picker and shown with the name of the disk (`GET api/volumes`, from the disk registry of the launcher) instead of its identifier. A real-time rule watches its source and may write into the same folder ("Use the source folder as destination"): in that case it must have an operation, and there is no mirror.
- **Filters**: each filter includes or excludes the files of some extensions (`*` or `.jpg, .png`) by one condition: updated or created before or after a day (today, yesterday, last week, last month, this month, measured at the run), size in MB, name with `*` and `?`. A file goes in if it matches at least one including filter (or there is none) and no excluding one.
- **Operations**: rename (a text before the name or after it, before the extension: `report.pdf` becomes `backup_report.pdf` or `report_backup.pdf`; a file that already carries it is not renamed again) and 7z archive, AES-256 encrypted with the password of the rule, names encrypted on request, verified at the end on request. Encryption of single files will come later.
- **Destination**: mirror (default: the destination follows the source) or copy only (never deletes anything); what to do with a file already there (overwrite, leave it, create a numbered copy); byte by byte verification of the copies.

The **mirror** removes from the destination only the files the rule itself copied and whose original is gone: never a file that was there before the rule, never one the filters exclude. The first run only copies. In mirror an archive is a single one, `<name>.7z`, updated at every run; in copy only every run writes a new one, `<name>-<date>-<time>.7z`. If nothing changed since the last time, the archive is left as it is.

**Scheduled** rules start at the chosen time, in the local time of the NAS; a missed time (NAS off) is skipped. **Real-time** rules watch the source (inotify) and act a few seconds after the last change, on the changed files only; at the start of the app they go through the whole source again. Every run shows among the operations of the launcher, where it can be stopped and where the scheduled shutdown waits for it; a failed run leaves a notification. While a scheduled or real-time rule is on, the app stays running with its window closed and starts with the NAS (`run.background`), at low priority: it is stopped with `sudo systemctl stop nas-app@nas-backup-manager` or by turning the rules off. The archive requires the Debian package `7zip`, which the launcher installs with the app.

## Local start

Requires Node.js 24, npm and Rust/Cargo.

```bash
pnpm install
pnpm run build
pnpm run backend:dev
```

In a second terminal:

```bash
pnpm run dev
```

Vite serves the UI on `http://127.0.0.1:1420/` and forwards `api/...` to the backend over a Unix socket. `NAS_APP_SOCKET` changes the path of the socket. `GET api/rules` loads the saved rules with the outcome of the last run, the next run and the state; `POST api/rules` creates a rule, `PUT`, `PATCH` (`{ enabled }`) and `DELETE api/rules/{id}` edit, turn on or delete it, `POST api/rules/{id}/run` and `/stop` start or stop it. Passwords never go back to the browser: a rule saved with an empty password keeps the one it had.

Folders are chosen with the Explorer picker opened by the launcher (`pickNasFolder`): source and destination for scheduled and real-time backups, the source only for real-time ones that write into the watched folder. A cloud backup instead chooses one of the network disks the launcher reads from nas-disk-manager (`requestNetworkDisks`); if there is none, the Cloud page and the first step of the rule say where to add one and open nas-disk-manager on its Network disks section. The backend refuses a rule without the folders of its kind, with a folder its kind does not use, with a path that is not absolute or holds `..`, and with source and destination one inside the other, either way; a mirror cannot have a whole disk as destination. Scheduled and cloud rules have a **Schedule** step, missing in real-time ones that follow every change: daily, weekly (with the day of the week), monthly (with the day of the month) or yearly (with month and day) frequency, always with a time. A day a month does not have (the 31st, February 29th) counts as the last day of the month. The step reads the power rules from the launcher (`requestPowerSchedule`), draws for every day of the week the hours the NAS is off, and does not let the user go on while the chosen time falls in a shutdown: for a weekly rule its day counts, for the others every day, because a day of the month changes its day of the week. Without a power-on rule a shutdown lasts until midnight; if the launcher does not answer, the step says so and does not block. The backend refuses a schedule in a real-time rule, its absence in the others and the fields the frequency does not use; it saves the time as minute of the day, in columns of its own, like the destination options, while operations and options stay an encrypted document, accepted only if every chosen operation has its options and vice versa. The SQLite database uses `NAS_DB_PATH` when set, otherwise `$STATE_DIRECTORY/backup-manager.sqlite3`; in development it is created in `backend/data/`. The passwords of the Archive/Encryption options are encrypted with AES-GCM in the database; the key is generated next to the DB in a `.key` file with `0600` permissions and must be kept together with the database to recover them.

For structure, tests and checks see [DEVELOPMENT.md](DEVELOPMENT.md). The contract for installation, search and communication with the launcher is in [APPS.md](APPS.md); packaging and release are in [NAS.md](NAS.md).

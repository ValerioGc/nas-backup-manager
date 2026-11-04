# nas-backup-manager

External application for the NAS launcher. It configures and runs scheduled and real-time backups. Cloud backups can be configured, but are not run yet. Every repository stands on its own and produces an archive the launcher can install.

## What a rule does

A rule has a name and is turned on and off, edited and deleted from the list. Every row shows the schedule (for example "Weekly · 03:00", and "Every Monday at 03:00" on hover, missing in the real-time section), the outcome of the last run, the next run and the commands to start it at once or stop it. The outcome is an icon for done, failed, interrupted or in progress, and on hover it shows date, time and the reason if any. A running rule can be neither edited nor deleted.

- **Folders**. Source and destination are chosen with the Explorer picker and shown with the name of the disk (`GET api/volumes`, from the disk registry of the launcher) instead of its identifier. A real-time rule watches its source and may write into the same folder ("Use the source folder as destination"). In that case it must have an operation, and there is no mirror.
- **Filters**. Each filter includes or excludes the files of some extensions (`*` or `.jpg, .png`) by one condition. The condition is updated or created before or after a day (today, yesterday, last week, last month, this month, measured at the run), a size in MB, or a name with `*` and `?`. A file goes in if it matches at least one including filter (or there is none) and no excluding one.
- **Operations**. Rename adds a text before the name or after it, before the extension, so `report.pdf` becomes `backup_report.pdf` or `report_backup.pdf`, and a file that already carries it is not renamed again. The 7z archive is AES-256 encrypted with the password of the rule, with names encrypted and a final check on request. Encryption of single files will come later.
- **Destination**. Mirror is the default and the destination follows the source, while copy only never deletes anything. The rule also says what to do with a file already there (overwrite, leave it, create a numbered copy) and whether the copies are verified byte by byte.

The **mirror** removes from the destination only the files the rule itself copied and whose original is gone. It never removes a file that was there before the rule, nor one the filters exclude. The first run only copies. What a run removes or replaces in the destination goes to the trash of its disk (the trash of Files, `.nas-trash`), and a version already there is kept beside it with a number. While that trash is off, or on a network disk, the file is removed. A scheduled or real-time run that would remove more than a quarter of the copies of the rule (at least 100) stops before removing any more and fails. A person then checks the source, and a run started by hand removes them. Each copy is written down as soon as it is in place, and a file of the destination with the same size and time as its original is taken as its copy, so a crash makes no duplicate. The form says when source and destination are on the same disk. In mirror there is a single archive, `<name>.7z`, updated at every run. In copy only every run writes a new one, `<name>-<date>-<time>.7z`. If nothing changed since the last time, the archive is left as it is.

**Scheduled** rules start at the chosen time, in the local time of the NAS. A time missed while the NAS was off or asleep runs once as soon as the app starts or the NAS wakes, with a notification. **Real-time** rules watch the source (inotify) and act a few seconds after the last change, on the changed files only. At the start of the app they go through the whole source again. Every run shows among the operations of the launcher, where it can be stopped and where the scheduled shutdown waits for it, and a failed run leaves a notification. While a scheduled or real-time rule is on, the app stays running with its window closed and starts with the NAS (`run.background`), at low priority. It is stopped with `sudo systemctl stop nas-app@nas-backup-manager` or by turning the rules off. The archive requires the Debian package `7zip`, which the launcher installs with the app.

## Local start

Requires Node.js 24, npm and Rust/Cargo.

```bash
pnpm install
pnpm run build
pnpm run backend:dev
```

In a second terminal run the UI.

```bash
pnpm run dev
```

Vite serves the UI on `http://127.0.0.1:1420/` and forwards `api/...` to the backend over a Unix socket. `NAS_APP_SOCKET` changes the path of the socket. `GET api/rules` loads the saved rules with the outcome of the last run, the next run and the state. `POST api/rules` creates a rule, `PUT`, `PATCH` (`{ enabled }`) and `DELETE api/rules/{id}` edit, turn on or delete it, and `POST api/rules/{id}/run` and `/stop` start or stop it. Passwords never go back to the browser, and a rule saved with an empty password keeps the one it had.

Folders are chosen with the Explorer picker opened by the launcher (`pickNasFolder`). Scheduled and real-time backups choose source and destination, and real-time ones that write into the watched folder choose the source only. A cloud backup instead chooses one of the network disks the launcher reads from nas-disk-manager (`requestNetworkDisks`). If there is none, the Cloud page and the first step of the rule say where to add one and open nas-disk-manager on its Network disks section. The backend refuses a rule without the folders of its kind, with a folder its kind does not use, with a path that is not absolute or holds `..`, and with source and destination one inside the other, either way. A mirror cannot have a whole disk as destination.

Scheduled and cloud rules have a **Schedule** step, missing in real-time ones that follow every change. The frequency is daily, weekly (with the day of the week), monthly (with the day of the month) or yearly (with month and day), always with a time. A day a month does not have (the 31st, February 29th) counts as the last day of the month. The step reads the power rules from the launcher (`requestPowerSchedule`) and draws for every day of the week the hours the NAS is off. It does not let the user go on while the chosen time falls in a shutdown. For a weekly rule its day counts, for the others every day, because a day of the month changes its day of the week. Without a power-on rule a shutdown lasts until midnight. If the launcher does not answer, the step says so and does not block. The backend refuses a schedule in a real-time rule, its absence in the others and the fields the frequency does not use. It saves the time as minute of the day in columns of its own, like the destination options, while operations and options stay an encrypted document, accepted only if every chosen operation has its options and the reverse.

The SQLite database uses `NAS_DB_PATH` when set, otherwise `$STATE_DIRECTORY/backup-manager.sqlite3`, and in development it is created in `backend/data/`. The passwords of the Archive and Encryption options are encrypted with AES-GCM in the database. The key is generated next to the database in a `.key` file with `0600` permissions and must be kept together with the database to recover them.

For structure, tests and checks see [DEVELOPMENT.md](DEVELOPMENT.md). The contract for installation, search and communication with the launcher is in [APPS.md](APPS.md), and packaging and release are in [NAS.md](NAS.md).

# Local SQLite migration recovery

Local structured state is stored in `%LOCALAPPDATA%/minos/lineage.db` on Windows.
Starting with v0.0.9, `lineage-core` owns an append-only migration chain and records
the applied version in `PRAGMA user_version`. FullOS invokes the bundled runner
before the webview can open the database, while Minos and the runner migrate as
part of `Database::open`.

## Automatic backup

Before changing an existing file database, the migration owner creates an online
SQLite backup beside the database:

```text
lineage.db.pre-migration-v0-to-v1-<unix-milliseconds>.bak
```

The backup is created before `journal_mode` or schema changes are applied. Unknown
unversioned schema shapes and databases from a newer application version are
rejected without creating a backup or modifying the source database.

## Recovery procedure

1. Close Lineage Minos, Lineage FullOS, and every Lineage Runner process.
2. Copy `lineage.db`, `lineage.db-wal`, and `lineage.db-shm` to a separate incident
   directory if they exist. Do not edit the only copy.
3. Select the newest `.bak` whose `v<from>-to-v<to>` range matches the failed
   upgrade.
4. Rename the current `lineage.db` out of the way; do not overwrite it.
5. Copy the selected backup to `lineage.db`.
6. Start one Lineage entrypoint. It will validate the restored unversioned shape,
   create a new backup, and retry the versioned migration.
7. Run `agentos --json verify` after startup to verify the lineage hash
   chain before resuming normal work.

If the restored database is rejected as an unknown shape or future version, keep
all copies unchanged and inspect the reported table or version. Do not force
`PRAGMA user_version` or replay `db/schema.sql`; that can make the version marker
disagree with the actual schema.

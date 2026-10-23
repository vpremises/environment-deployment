use crate::{Error, Ledger, Result};
use std::path::Path;

impl Ledger {
    /// Opens the existing ledger under the Linux/Unix single-local-owner boundary.
    /// # Errors
    /// Refuses shared, linked, aliased or foreign state; never repairs permissions.
    pub fn open_private(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        admit(path)?;
        let connection = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL; PRAGMA busy_timeout=1000;
          CREATE TABLE IF NOT EXISTS maintenance_clock(id INTEGER PRIMARY KEY CHECK(id=1), observed_at INTEGER NOT NULL);
          INSERT OR IGNORE INTO maintenance_clock VALUES(1,0);
          CREATE TABLE IF NOT EXISTS maintenance_runs(
            id TEXT PRIMARY KEY, environment_id TEXT NOT NULL, host TEXT NOT NULL,
            plan_sha256 TEXT NOT NULL, plan_json TEXT NOT NULL CHECK(length(plan_json)<=16384),
            phase TEXT NOT NULL CHECK(phase IN ('approved','unknown','completed','failed','stopped')),
            next_step INTEGER NOT NULL DEFAULT 0 CHECK(next_step BETWEEN 0 AND 16),
            revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)), stopped INTEGER NOT NULL DEFAULT 0 CHECK(stopped IN (0,1)),
            deadline_at INTEGER, active_json TEXT CHECK(length(active_json)<=2048),
            observation_json TEXT CHECK(length(observation_json)<=2048));
          CREATE INDEX IF NOT EXISTS maintenance_unknown_scope ON maintenance_runs(environment_id,host,phase);
          CREATE TABLE IF NOT EXISTS maintenance_receipts(
            plan_id TEXT NOT NULL REFERENCES maintenance_runs(id),step_id TEXT NOT NULL,
            reserved_at INTEGER NOT NULL,state_sha256 TEXT NOT NULL,evidence_sha256 TEXT NOT NULL,
            observed_at INTEGER NOT NULL,outcome TEXT NOT NULL CHECK(outcome IN ('after-state-observed','before-state-observed')),
            PRIMARY KEY(plan_id,step_id))")?;
        Ok(Self {
            connection,
            private: true,
        })
    }
}

#[cfg(unix)]
// Keep the standard octal Unix permission boundary explicit for review.
#[allow(clippy::verbose_bit_mask)]
fn admit(path: &Path) -> Result<()> {
    use std::{
        fs,
        fs::OpenOptions,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let owner = rustix::process::geteuid().as_raw();
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("missing private ledger parent"))?;
    if !path.is_absolute() || parent.canonicalize()? != parent {
        return Err(Error::new("private ledger parent alias refused"));
    }
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.uid() != owner || meta.mode() & 0o077 != 0 {
        return Err(Error::new(
            "ledger parent must be private and locally owned",
        ));
    }
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = path.as_os_str().to_os_string();
        candidate.push(suffix);
        match fs::symlink_metadata(&candidate) {
            Ok(meta)
                if meta.is_file()
                    && meta.nlink() == 1
                    && meta.uid() == owner
                    && meta.mode() & 0o077 == 0 => {}
            Ok(_) => return Err(Error::new("private ledger file boundary refused")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if !path.exists() {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?
            .sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn admit(_path: &Path) -> Result<()> {
    Err(Error::new(
        "private maintenance ledger is not accepted on this platform",
    ))
}

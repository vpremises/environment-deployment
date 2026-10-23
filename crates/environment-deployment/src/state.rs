use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    Error, Plan, PlanAction, Receipt, ResolvedEnvironment, Result,
    digest::{bytes, resolved},
};

pub struct Ledger {
    pub(crate) connection: Connection,
    pub(crate) private: bool,
}

impl Ledger {
    /// Opens and migrates the private reconciliation ledger.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open or migrate the file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        initialize_reconciliation(&connection)?;
        Ok(Self {
            connection,
            private: false,
        })
    }

    /// Compares the desired digest with the latest accepted receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when encoding or reading the ledger fails.
    pub fn plan(&self, desired: &ResolvedEnvironment) -> Result<Plan> {
        let desired_digest = resolved(desired)?;
        let previous_digest = self
            .connection
            .query_row(
                "SELECT desired_digest FROM reconciliations
                 WHERE environment_id = ?1 AND status IN ('applied','no-op')
                 ORDER BY rowid DESC LIMIT 1",
                [&desired.environment.environment_id],
                |row| row.get(0),
            )
            .optional()?;
        let action = if previous_digest.as_deref() == Some(desired_digest.as_str()) {
            PlanAction::NoOp
        } else {
            PlanAction::Apply
        };
        Ok(Plan {
            schema: "vpremises://infrastructure/plan/v1".into(),
            environment_id: desired.environment.environment_id.clone(),
            desired_digest,
            previous_digest,
            action,
        })
    }

    /// Records an exact driver receipt and treats an identical replay as a no-op.
    ///
    /// # Errors
    ///
    /// Rejects mismatched digests, conflicting operation IDs, and `SQLite` failures.
    pub fn record(&self, desired: &ResolvedEnvironment, receipt: &Receipt) -> Result<bool> {
        validate_receipt(desired, receipt)?;
        let encoded = serde_json::to_vec(receipt)?;
        let receipt_digest = bytes(&encoded);
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT receipt_digest FROM reconciliations WHERE operation_id = ?1",
                [&receipt.operation_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing_digest) = existing {
            if existing_digest != receipt_digest {
                return Err(Error::new("operation_id already has a different receipt"));
            }
            return Ok(false);
        }
        self.connection.execute(
            "INSERT INTO reconciliations VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                receipt.operation_id,
                receipt.environment_id,
                receipt.desired_digest,
                receipt.driver,
                receipt.status,
                receipt.changed,
                receipt.observed_at,
                receipt_digest
            ],
        )?;
        Ok(true)
    }
}

fn validate_receipt(desired: &ResolvedEnvironment, receipt: &Receipt) -> Result<()> {
    if receipt.schema != "vpremises://infrastructure/apply-receipt/v1"
        || !matches!(receipt.status.as_str(), "applied" | "no-op")
        || receipt.environment_id != desired.environment.environment_id
        || receipt.desired_digest != resolved(desired)?
    {
        return Err(Error::new(
            "receipt does not attest the resolved environment",
        ));
    }
    let driver = match desired.environment.driver {
        crate::model::Driver::Native => "native",
        crate::model::Driver::Incus => "incus",
    };
    if receipt.driver != driver
        || receipt.operation_id.is_empty()
        || receipt.operation_id.len() > 128
        || receipt
            .operation_id
            .bytes()
            .any(|byte| !byte.is_ascii_graphic())
    {
        return Err(Error::new(
            "receipt driver or operation identity is invalid",
        ));
    }
    let observed = time::OffsetDateTime::parse(
        &receipt.observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| Error::new("receipt observation must be RFC3339"))?;
    if observed > time::OffsetDateTime::now_utc() {
        return Err(Error::new("receipt observation cannot be in the future"));
    }
    if receipt.status == "no-op" && receipt.changed {
        return Err(Error::new("no-op receipt cannot report a change"));
    }
    Ok(())
}

pub(crate) fn initialize_reconciliation(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations(
               version INTEGER PRIMARY KEY
             );
             CREATE TABLE IF NOT EXISTS reconciliations(
               operation_id TEXT PRIMARY KEY,
               environment_id TEXT NOT NULL,
               desired_digest TEXT NOT NULL,
               driver TEXT NOT NULL,
               status TEXT NOT NULL CHECK(status IN ('applied','no-op')),
               changed INTEGER NOT NULL CHECK(changed IN (0,1)),
               observed_at TEXT NOT NULL,
               receipt_digest TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS reconciliations_environment
               ON reconciliations(environment_id);",
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO schema_migrations(version) VALUES(1)",
        [],
    )?;
    Ok(())
}

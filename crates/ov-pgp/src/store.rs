//! SQLite backed keyring storage.
//!
//! The database lives inside the application sandbox (`filesDir/pgp`); it
//! holds the public certificates in the clear and the transferable secret
//! keys only in their wrapped (encrypted) form.

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::SystemTime,
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use sequoia_openpgp::Fingerprint;

/// Fingerprint of a certificate as stored in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCertificate {
    /// The certificate fingerprint.
    pub fingerprint: Fingerprint,
    /// The ASCII armored public certificate.
    pub armored: String,
    /// Whether a (wrapped) secret key is stored for this certificate.
    pub has_secret: bool,
    /// Creation timestamp (seconds since the epoch).
    pub created_at: i64,
}

/// The keyring database.
///
/// The connection is guarded by a mutex: `rusqlite::Connection` is not
/// `Sync`, but the keyring is shared between the CEF threads and the JNI
/// entry points.
pub struct KeyringStore {
    conn: Mutex<Connection>,
}

impl KeyringStore {
    /// Open (and create if needed) the keyring database.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open the keyring database {}", path.display()))?;
        // The database is private to the application.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory database (used by the tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// The database connection.
    fn conn(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(err) => panic!("the keyring database is poisoned: {err}"),
        }
    }

    /// Create the schema.
    fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS certificates (
                 fingerprint BLOB PRIMARY KEY NOT NULL,
                 armored      TEXT NOT NULL,
                 created_at   INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS secret_keys (
                 fingerprint BLOB PRIMARY KEY NOT NULL,
                 wrapped     BLOB NOT NULL,
                 created_at  INTEGER NOT NULL,
                 FOREIGN KEY (fingerprint) REFERENCES certificates (fingerprint)
                     ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS certificates_created_at
                 ON certificates (created_at);",
        )?;
        Ok(())
    }

    /// Store (or replace) a public certificate.
    pub fn put_certificate(&self, fingerprint: &Fingerprint, armored: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO certificates (fingerprint, armored, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (fingerprint) DO UPDATE SET armored = excluded.armored",
            params![fingerprint.as_bytes(), armored, now_seconds(),],
        )?;
        Ok(())
    }

    /// Store the wrapped secret key of a certificate.
    pub fn put_secret(&self, fingerprint: &Fingerprint, wrapped: &[u8]) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO secret_keys (fingerprint, wrapped, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (fingerprint) DO UPDATE SET wrapped = excluded.wrapped",
            params![fingerprint.as_bytes(), wrapped, now_seconds()],
        )?;
        Ok(())
    }

    /// The armored public certificate of `fingerprint`.
    pub fn certificate(&self, fingerprint: &Fingerprint) -> Result<Option<String>> {
        let conn = self.conn();
        let armored = conn
            .query_row(
                "SELECT armored FROM certificates WHERE fingerprint = ?1",
                params![fingerprint.as_bytes()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(armored)
    }

    /// The wrapped secret key of `fingerprint`.
    pub fn secret(&self, fingerprint: &Fingerprint) -> Result<Option<Vec<u8>>> {
        let conn = self.conn();
        let wrapped = conn
            .query_row(
                "SELECT wrapped FROM secret_keys WHERE fingerprint = ?1",
                params![fingerprint.as_bytes()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        Ok(wrapped)
    }

    /// `true` when a secret key is stored for `fingerprint`.
    pub fn has_secret(&self, fingerprint: &Fingerprint) -> Result<bool> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM secret_keys WHERE fingerprint = ?1",
            params![fingerprint.as_bytes()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// All certificates in the keyring.
    pub fn certificates(&self) -> Result<Vec<StoredCertificate>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT c.fingerprint, c.armored, c.created_at,
                    EXISTS (SELECT 1 FROM secret_keys s WHERE s.fingerprint = c.fingerprint)
             FROM certificates c
             ORDER BY c.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let fingerprint: Vec<u8> = row.get(0)?;
            let armored: String = row.get(1)?;
            let created_at: i64 = row.get(2)?;
            let has_secret: bool = row.get(3)?;
            Ok((fingerprint, armored, created_at, has_secret))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (fingerprint, armored, created_at, has_secret) = row?;
            if fingerprint.len() != 20 && fingerprint.len() != 32 {
                anyhow::bail!("the stored fingerprint has an invalid length");
            }
            let fingerprint = Fingerprint::from_bytes(&fingerprint);
            out.push(StoredCertificate {
                fingerprint,
                armored,
                has_secret,
                created_at,
            });
        }
        Ok(out)
    }

    /// Remove the secret key of a certificate (the public part is kept).
    pub fn remove_secret(&self, fingerprint: &Fingerprint) -> Result<bool> {
        let conn = self.conn();
        let removed = conn.execute(
            "DELETE FROM secret_keys WHERE fingerprint = ?1",
            params![fingerprint.as_bytes()],
        )?;
        Ok(removed > 0)
    }

    /// Remove a certificate and its secret key.
    pub fn remove_certificate(&self, fingerprint: &Fingerprint) -> Result<bool> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM secret_keys WHERE fingerprint = ?1",
            params![fingerprint.as_bytes()],
        )?;
        let removed = conn.execute(
            "DELETE FROM certificates WHERE fingerprint = ?1",
            params![fingerprint.as_bytes()],
        )?;
        Ok(removed > 0)
    }
}

/// Seconds since the Unix epoch.
fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

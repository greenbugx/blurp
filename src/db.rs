use crate::contract::{Draft, DraftBatch};
use crate::draft::{DraftStatus, Tone};
use chrono::SecondsFormat;
use directories::ProjectDirs;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const DATABASE_FILE: &str = "blurp.sqlite3";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("could not locate the user data directory")]
    NoDataDirectory,
    #[error("could not create the data directory at {path}: {source}")]
    Directory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("no stored draft with id {0}")]
    MissingDraft(String),
    #[error("stored draft has an invalid id '{0}'")]
    InvalidId(String),
    #[error("stored value '{value}' is not a valid {field}")]
    InvalidEnum { field: &'static str, value: String },
    #[error("local storage error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("could not read draft details: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn database_path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("", "", "blurp")?;
    Some(dirs.data_dir().join(DATABASE_FILE))
}

pub struct Store {
    connection: Connection,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Store").finish_non_exhaustive()
    }
}

#[allow(dead_code)]
impl Store {
    pub fn open() -> Result<Self, StoreError> {
        let path = database_path().ok_or(StoreError::NoDataDirectory)?;
        Self::open_at(&path)
    }

    pub fn open_at(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::Directory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(connection: Connection) -> Result<Self, StoreError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS draft_batches (
                 id TEXT PRIMARY KEY,
                 dump TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS drafts (
                 id TEXT PRIMARY KEY,
                 batch_id TEXT NOT NULL REFERENCES draft_batches(id) ON DELETE CASCADE,
                 position INTEGER NOT NULL,
                 title TEXT NOT NULL,
                 recipient TEXT,
                 body TEXT NOT NULL,
                 tone TEXT NOT NULL,
                 status TEXT NOT NULL,
                 missing_details TEXT NOT NULL
             );",
        )?;

        Ok(Self { connection })
    }

    pub fn save_batch(&self, batch: &DraftBatch) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO draft_batches (id, dump, created_at) VALUES (?1, ?2, ?3)",
            params![
                batch.id.to_string(),
                batch.dump,
                batch.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
            ],
        )?;

        for (position, draft) in batch.drafts.iter().enumerate() {
            transaction.execute(
                "INSERT INTO drafts
                     (id, batch_id, position, title, recipient, body, tone, status, missing_details)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    draft.id.to_string(),
                    batch.id.to_string(),
                    position as i64,
                    draft.title,
                    draft.recipient,
                    draft.body,
                    draft.tone.label(),
                    draft.status.label(),
                    serde_json::to_string(&draft.missing_details)?,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn drafts(&self) -> Result<Vec<Draft>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT d.id, d.title, d.recipient, d.body, d.tone, d.status, d.missing_details
             FROM drafts d
             JOIN draft_batches b ON d.batch_id = b.id
             ORDER BY b.created_at DESC, d.position ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(RawDraft {
                id: row.get(0)?,
                title: row.get(1)?,
                recipient: row.get(2)?,
                body: row.get(3)?,
                tone: row.get(4)?,
                status: row.get(5)?,
                missing_details: row.get(6)?,
            })
        })?;

        let mut drafts = Vec::new();
        for row in rows {
            drafts.push(draft_from_raw(row?)?);
        }
        Ok(drafts)
    }

    pub fn update_draft(&self, draft: &Draft) -> Result<(), StoreError> {
        let changed = self.connection.execute(
            "UPDATE drafts
             SET title = ?1, recipient = ?2, body = ?3, tone = ?4, status = ?5, missing_details = ?6
             WHERE id = ?7",
            params![
                draft.title,
                draft.recipient,
                draft.body,
                draft.tone.label(),
                draft.status.label(),
                serde_json::to_string(&draft.missing_details)?,
                draft.id.to_string(),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::MissingDraft(draft.id.to_string()));
        }

        Ok(())
    }

    pub fn delete_draft(&self, id: Uuid) -> Result<(), StoreError> {
        let changed = self
            .connection
            .execute("DELETE FROM drafts WHERE id = ?1", params![id.to_string()])?;
        if changed == 0 {
            return Err(StoreError::MissingDraft(id.to_string()));
        }

        Ok(())
    }
}

struct RawDraft {
    id: String,
    title: String,
    recipient: Option<String>,
    body: String,
    tone: String,
    status: String,
    missing_details: String,
}

fn draft_from_raw(raw: RawDraft) -> Result<Draft, StoreError> {
    Ok(Draft {
        id: Uuid::parse_str(&raw.id).map_err(|_| StoreError::InvalidId(raw.id.clone()))?,
        title: raw.title,
        recipient: raw.recipient,
        body: raw.body,
        tone: Tone::from_label(&raw.tone).ok_or_else(|| StoreError::InvalidEnum {
            field: "tone",
            value: raw.tone.clone(),
        })?,
        status: DraftStatus::from_label(&raw.status).ok_or_else(|| StoreError::InvalidEnum {
            field: "status",
            value: raw.status.clone(),
        })?,
        missing_details: serde_json::from_str(&raw.missing_details)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_batch() -> DraftBatch {
        DraftBatch {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            dump: "tell sam i cant make it tonight, ask dev about the invoice".to_string(),
            drafts: vec![
                Draft {
                    id: Uuid::new_v4(),
                    title: "Cancel tonight".to_string(),
                    recipient: Some("Sam".to_string()),
                    body: "Hey Sam, I can't make it tonight.".to_string(),
                    tone: Tone::Casual,
                    status: DraftStatus::Review,
                    missing_details: vec!["Which plans?".to_string()],
                },
                Draft {
                    id: Uuid::new_v4(),
                    title: "Invoice status".to_string(),
                    recipient: None,
                    body: "Has the invoice been paid?".to_string(),
                    tone: Tone::Direct,
                    status: DraftStatus::Draft,
                    missing_details: Vec::new(),
                },
            ],
        }
    }

    fn temp_database_path() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "blurp-tests-{}/nested/blurp-{unique}.sqlite3",
            std::process::id()
        ))
    }

    #[test]
    fn the_database_lives_in_the_user_data_directory() {
        let path = database_path().unwrap();
        assert_eq!(path.file_name().unwrap(), DATABASE_FILE);
        assert!(path.is_absolute());
    }

    #[test]
    fn a_fresh_store_has_no_drafts() {
        let store = Store::open_in_memory().unwrap();

        assert!(store.drafts().unwrap().is_empty());
    }

    #[test]
    fn drafts_round_trip_with_every_field() {
        let store = Store::open_in_memory().unwrap();
        let batch = sample_batch();

        store.save_batch(&batch).unwrap();

        assert_eq!(store.drafts().unwrap(), batch.drafts);
    }

    #[test]
    fn batches_reload_newest_first_with_drafts_in_model_order() {
        let store = Store::open_in_memory().unwrap();
        let mut older = sample_batch();
        older.created_at -= chrono::Duration::hours(2);
        let newer = sample_batch();

        store.save_batch(&older).unwrap();
        store.save_batch(&newer).unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].id, newer.drafts[0].id);
        assert_eq!(drafts[1].id, newer.drafts[1].id);
        assert_eq!(drafts[2].id, older.drafts[0].id);
        assert_eq!(drafts[3].id, older.drafts[1].id);
    }

    #[test]
    fn updating_a_draft_persists_every_edit() {
        let store = Store::open_in_memory().unwrap();
        let batch = sample_batch();
        store.save_batch(&batch).unwrap();

        let mut draft = batch.drafts[0].clone();
        draft.title = "Edited title".to_string();
        draft.body = "Edited body".to_string();
        draft.recipient = None;
        draft.tone = Tone::Firm;
        draft.status = DraftStatus::Snoozed;
        draft.missing_details = vec!["New question".to_string()];
        store.update_draft(&draft).unwrap();

        assert_eq!(store.drafts().unwrap()[0], draft);
    }

    #[test]
    fn deleting_a_draft_leaves_the_rest() {
        let store = Store::open_in_memory().unwrap();
        let batch = sample_batch();
        store.save_batch(&batch).unwrap();

        store.delete_draft(batch.drafts[0].id).unwrap();

        assert_eq!(store.drafts().unwrap(), vec![batch.drafts[1].clone()]);
        assert!(matches!(
            store.delete_draft(batch.drafts[0].id),
            Err(StoreError::MissingDraft(_))
        ));
    }

    #[test]
    fn updating_or_deleting_an_unknown_draft_fails() {
        let store = Store::open_in_memory().unwrap();
        let draft = sample_batch().drafts.remove(0);

        assert!(matches!(
            store.update_draft(&draft),
            Err(StoreError::MissingDraft(_))
        ));
        assert!(matches!(
            store.delete_draft(draft.id),
            Err(StoreError::MissingDraft(_))
        ));
    }

    #[test]
    fn a_failing_batch_save_leaves_no_partial_rows() {
        let store = Store::open_in_memory().unwrap();
        let batch = sample_batch();
        store.save_batch(&batch).unwrap();

        let mut clash = sample_batch();
        let fresh_draft = clash.drafts.remove(0);
        clash.drafts.push(batch.drafts[0].clone());
        clash.created_at += chrono::Duration::hours(1);

        assert!(store.save_batch(&clash).is_err());

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].id, batch.drafts[0].id);
        assert_ne!(drafts[0].id, fresh_draft.id);
    }

    #[test]
    fn a_new_database_file_initializes_and_survives_reopening() {
        let path = temp_database_path();
        let batch = sample_batch();

        {
            let store = Store::open_at(&path).unwrap();
            store.save_batch(&batch).unwrap();
        }

        let reopened = Store::open_at(&path).unwrap();
        assert_eq!(reopened.drafts().unwrap(), batch.drafts);

        if let Some(parent) = path.ancestors().nth(2) {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

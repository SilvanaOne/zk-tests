use rocksdb::{
    ColumnFamilyDescriptor, DB, DBCompressionType, Env, Options,
    backup::{BackupEngine, BackupEngineOptions},
    checkpoint::Checkpoint,
};

use std::{sync::Arc, thread, time::Duration};

use crate::db::{Key, Value};

#[derive(Clone)]
pub struct DBStore {
    db: Arc<DB>,
}

impl DBStore {
    const DB_PATH: &'static str = "./db";
    const BACKUP_PATH: &'static str = "./backup";
    const CF_KV: &'static str = "kv";
    #[allow(dead_code)]
    const CF_NONCE: &'static str = "nonce"; // timestamp is used as nonce

    // ---- constructor ------------------------------------------------------

    /// Opens (or creates) the database and starts the background backup task.
    pub fn open() -> anyhow::Result<Self> {
        // Global options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_compression_type(DBCompressionType::Lz4);

        // Column‑family specific opts (inherit defaults for now)
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(Self::CF_KV, Options::default()),
            ColumnFamilyDescriptor::new(Self::CF_NONCE, Options::default()),
        ];

        // Open the DB directory (created if absent)
        let db = DB::open_cf_descriptors(&opts, Self::DB_PATH, cf_descriptors)?;
        let store = Self { db: Arc::new(db) };

        // Fire‑and‑forget backup thread
        store.spawn_backup_thread();

        Ok(store)
    }

    // ---- High‑level API: KV column family ---------------------------------

    pub fn put_kv(&self, key: &Key, value: &Value) -> anyhow::Result<()> {
        let cf = self
            .db
            .cf_handle(Self::CF_KV)
            .expect("kv column family missing");
        let k = bincode::serialize(key)?;
        let v = bincode::serialize(value)?;
        self.db.put_cf(&cf, k, v)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_kv(&self, key: &Key) -> anyhow::Result<()> {
        let cf = self
            .db
            .cf_handle(Self::CF_KV)
            .expect("kv column family missing");
        let k = bincode::serialize(key)?;
        self.db.delete_cf(&cf, k)?;
        Ok(())
    }

    /// Convenience wrapper: returns `None` if the key is missing.
    pub fn get_kv(&self, key: &Key) -> anyhow::Result<Option<Value>> {
        let cf = self
            .db
            .cf_handle(Self::CF_KV)
            .expect("kv column family missing");
        let k = bincode::serialize(key)?;
        Ok(self
            .db
            .get_cf(&cf, k)?
            .map(|bytes| bincode::deserialize(&bytes).expect("failed to decode Value")))
    }

    // ---- High‑level API: NUM column family --------------------------------

    #[allow(dead_code)]
    pub fn put_nonce(&self, key: &Key, num: u64) -> anyhow::Result<()> {
        let cf = self
            .db
            .cf_handle(Self::CF_NONCE)
            .expect("num column family missing");
        let k = bincode::serialize(key)?;
        let v = num.to_be_bytes();
        self.db.put_cf(&cf, k, v)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_nonce(&self, key: &Key) -> anyhow::Result<Option<u64>> {
        let cf = self
            .db
            .cf_handle(Self::CF_NONCE)
            .expect("num column family missing");
        let k = bincode::serialize(key)?;
        Ok(self.db.get_cf(&cf, k)?.map(|bytes| {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes);
            u64::from_be_bytes(arr)
        }))
    }

    // ---- backups ----------------------------------------------------------

    /// Spawn a detached thread that performs an incremental flush+backup
    /// every 10 minutes. Failures are logged but do **not** crash the process.
    fn spawn_backup_thread(&self) {
        let db = Arc::clone(&self.db);
        thread::spawn(move || {
            let be_opts = BackupEngineOptions::new(std::path::Path::new(Self::BACKUP_PATH))
                .expect("create BackupEngineOptions");
            // Ensure the backup directory exists
            let _ = std::fs::create_dir_all(Self::BACKUP_PATH);
            let env = Env::new().expect("create default Env");
            let mut be = BackupEngine::open(&be_opts, &env).expect("open BackupEngine failed");

            loop {
                if let Err(e) = be.create_new_backup_flush(&db, true) {
                    eprintln!("[rocksdb‑backup] backup failed: {e:#}");
                }
                thread::sleep(Duration::from_secs(600)); // 10 min
            }
        });
    }

    /// Creates a *manual* on‑demand snapshot under `./backup/snapshot-<ts>`
    /// using RocksDB Checkpoint (hard‑linked, instant).
    #[allow(dead_code)]
    pub fn snapshot(&self) -> anyhow::Result<()> {
        let ts = chrono::Utc::now().format("snapshot-%Y%m%d%H%M%S");
        let target = std::path::PathBuf::from(Self::BACKUP_PATH).join(ts.to_string());
        Checkpoint::new(&self.db)?.create_checkpoint(&target)?;
        Ok(())
    }
}

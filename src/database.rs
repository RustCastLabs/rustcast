use crate::clipboard::{ClipBoardContentType, ImageData};
use rusqlite::{Connection, Result, params};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;
use sha2::{Sha256, Digest};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or("/".to_string());
        let db_dir = format!("{}/Library/Application Support/rustcast", home);
        std::fs::create_dir_all(&db_dir).ok();
        let db_path = format!("{}/rustcast.db", db_dir);

        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS rankings (
                name TEXT PRIMARY KEY,
                rank INTEGER NOT NULL
            )",
            [],
        )?;

        let user_version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap_or(0);
        
        if user_version < 1 {
            let table_exists: bool = conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='clipboard'",
                [],
                |row| {
                    let count: i32 = row.get(0)?;
                    Ok(count > 0)
                },
            ).unwrap_or(false);

            if table_exists {
                // Ignore failure if column somehow exists
                let _ = conn.execute("ALTER TABLE clipboard ADD COLUMN image_hash TEXT", []);
            }
            conn.execute("PRAGMA user_version = 1", [])?;
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                content TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_bytes BLOB,
                image_hash TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Ensure indices for speed
        conn.execute("CREATE INDEX IF NOT EXISTS idx_clipboard_hash ON clipboard(image_hash)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_clipboard_content ON clipboard(content)", [])?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_ranking(&self, name: &str, rank: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO rankings (name, rank) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET rank = excluded.rank",
            params![name, rank],
        )?;
        Ok(())
    }

    pub fn get_rankings(&self) -> Result<HashMap<String, i32>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT name, rank FROM rankings")?;
        let ranking_iter = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut map = HashMap::new();
        for (name, rank) in ranking_iter.flatten() {
            map.insert(name, rank);
        }
        Ok(map)
    }

    pub fn save_clipboard_item(&self, item: &ClipBoardContentType) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        match item {
            ClipBoardContentType::Text(text) => {
                conn.execute(
                    "INSERT OR REPLACE INTO clipboard (id, type, content) VALUES ((SELECT id FROM clipboard WHERE type = 'Text' AND content = ?1), 'Text', ?1)",
                    params![text],
                )?;
            }
            ClipBoardContentType::Image(img) => {
                let mut hasher = Sha256::new();
                hasher.update(&img.bytes);
                let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                
                conn.execute(
                    "INSERT OR REPLACE INTO clipboard (id, type, image_width, image_height, image_bytes, image_hash) VALUES ((SELECT id FROM clipboard WHERE type = 'Image' AND image_hash = ?4), 'Image', ?1, ?2, ?3, ?4)",
                    params![img.width as i64, img.height as i64, img.bytes.as_ref(), hash],
                )?;
            }
            ClipBoardContentType::Files(files, img_opt) => {
                if let Some(img) = img_opt {
                    let mut hasher = Sha256::new();
                    hasher.update(&img.bytes);
                    let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                    
                    conn.execute(
                        "INSERT OR REPLACE INTO clipboard (id, type, content, image_width, image_height, image_bytes, image_hash) VALUES ((SELECT id FROM clipboard WHERE type = 'Files' AND content = ?1 AND image_hash = ?5), 'Files', ?1, ?2, ?3, ?4, ?5)",
                        params![files.join("\n"), img.width as i64, img.height as i64, img.bytes.as_ref(), hash],
                    )?;
                } else {
                    conn.execute(
                        "INSERT OR REPLACE INTO clipboard (id, type, content) VALUES ((SELECT id FROM clipboard WHERE type = 'Files' AND content = ?1 AND image_bytes IS NULL), 'Files', ?1)",
                        params![files.join("\n")],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn delete_clipboard_item(&self, item: &ClipBoardContentType) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        match item {
            ClipBoardContentType::Text(text) => {
                conn.execute(
                    "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Text' AND content = ?1 ORDER BY created_at DESC LIMIT 1)",
                    params![text],
                )?;
            }
            ClipBoardContentType::Image(img) => {
                let mut hasher = Sha256::new();
                hasher.update(&img.bytes);
                let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                
                conn.execute(
                    "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Image' AND image_hash = ?1 ORDER BY created_at DESC LIMIT 1)",
                    params![hash],
                )?;
            }
            ClipBoardContentType::Files(files, img_opt) => {
                if let Some(img) = img_opt {
                    let mut hasher = Sha256::new();
                    hasher.update(&img.bytes);
                    let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                    
                    conn.execute(
                        "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Files' AND content = ?1 AND image_hash = ?2 ORDER BY created_at DESC LIMIT 1)",
                        params![files.join("\n"), hash],
                    )?;
                } else {
                    conn.execute(
                        "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Files' AND content = ?1 AND image_bytes IS NULL ORDER BY created_at DESC LIMIT 1)",
                        params![files.join("\n")],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn clear_clipboard(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM clipboard", [])?;
        Ok(())
    }

    pub fn get_clipboard_history(&self, limit: u32) -> Result<Vec<ClipBoardContentType>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT type, content, image_width, image_height, image_bytes FROM clipboard ORDER BY created_at DESC LIMIT ?1"
        )?;

        let history_iter = stmt.query_map([limit], |row| {
            let typ: String = row.get(0)?;
            if typ == "Text" {
                let content: String = row.get(1)?;
                Ok(ClipBoardContentType::Text(content))
            } else if typ == "Files" {
                let content: String = row.get(1)?;
                let files: Vec<String> = content
                    .split('\n')
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                let bytes: Option<Vec<u8>> = row.get(4)?;
                let img_opt = if let Some(b) = bytes {
                    let width: i64 = row.get(2)?;
                    let height: i64 = row.get(3)?;
                    Some(ImageData {
                        width: width as usize,
                        height: height as usize,
                        bytes: Cow::Owned(b),
                    })
                } else {
                    None
                };

                Ok(ClipBoardContentType::Files(files, img_opt))
            } else {
                let width: i64 = row.get(2)?;
                let height: i64 = row.get(3)?;
                let bytes: Vec<u8> = row.get(4)?;
                Ok(ClipBoardContentType::Image(ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: Cow::Owned(bytes),
                }))
            }
        })?;

        let mut items = Vec::new();
        for item in history_iter.flatten() {
            items.push(item);
        }

        Ok(items)
    }
}

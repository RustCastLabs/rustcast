use crate::clipboard::ClipBoardContentType;
use arboard::ImageData;
use rusqlite::{Connection, Result, params};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or("/".to_string());
        let db_path = format!("{}/.config/rustcast/rustcast.db", home);

        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS rankings (
                name TEXT PRIMARY KEY,
                rank INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                content TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_bytes BLOB,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_ranking(&self, name: &str, rank: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO rankings (name, rank) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET rank = excluded.rank",
            params![name, rank],
        )?;
        Ok(())
    }

    pub fn get_rankings(&self) -> Result<HashMap<String, i32>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        match item {
            ClipBoardContentType::Text(text) => {
                conn.execute(
                    "INSERT INTO clipboard (type, content) VALUES ('Text', ?1)",
                    params![text],
                )?;
            }
            ClipBoardContentType::Image(img) => {
                conn.execute(
                    "INSERT INTO clipboard (type, image_width, image_height, image_bytes) VALUES ('Image', ?1, ?2, ?3)",
                    params![img.width as i64, img.height as i64, img.bytes.as_ref()],
                )?;
            }
            ClipBoardContentType::Files(files, img_opt) => {
                if let Some(img) = img_opt {
                    conn.execute(
                        "INSERT INTO clipboard (type, content, image_width, image_height, image_bytes) VALUES ('Files', ?1, ?2, ?3, ?4)",
                        params![files.join("\n"), img.width as i64, img.height as i64, img.bytes.as_ref()],
                    )?;
                } else {
                    conn.execute(
                        "INSERT INTO clipboard (type, content) VALUES ('Files', ?1)",
                        params![files.join("\n")],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn delete_clipboard_item(&self, item: &ClipBoardContentType) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        match item {
            ClipBoardContentType::Text(text) => {
                conn.execute(
                    "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Text' AND content = ?1 ORDER BY created_at DESC LIMIT 1)",
                    params![text],
                )?;
            }
            ClipBoardContentType::Image(img) => {
                conn.execute(
                    "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Image' AND image_width = ?1 AND image_height = ?2 AND image_bytes = ?3 ORDER BY created_at DESC LIMIT 1)",
                    params![img.width as i64, img.height as i64, img.bytes.as_ref()],
                )?;
            }
            ClipBoardContentType::Files(files, img_opt) => {
                if let Some(img) = img_opt {
                    conn.execute(
                        "DELETE FROM clipboard WHERE id = (SELECT id FROM clipboard WHERE type = 'Files' AND content = ?1 AND image_width = ?2 AND image_height = ?3 AND image_bytes = ?4 ORDER BY created_at DESC LIMIT 1)",
                        params![files.join("\n"), img.width as i64, img.height as i64, img.bytes.as_ref()],
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard", [])?;
        Ok(())
    }

    pub fn get_clipboard_history(&self, limit: u32) -> Result<Vec<ClipBoardContentType>> {
        let conn = self.conn.lock().unwrap();
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

//! World saves manager: persistent metadata and directory operations.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const SAVES_DIR: &str = "saves";

/// Persistent metadata for a world save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldMeta {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub created_at: u64,
    pub last_played: u64,
    pub player_pos: [f32; 3],
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub player_flying: bool,
}

impl WorldMeta {
    pub fn new(id: String, name: String, seed: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            id,
            name,
            seed,
            created_at: now,
            last_played: now,
            player_pos: [0.0, 0.0, 0.0],
            player_yaw: -135.0,
            player_pitch: -20.0,
            player_flying: false,
        }
    }
}

pub fn sanitize_folder_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "world".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn parse_or_random_seed(seed_input: &str) -> u64 {
    let trimmed = seed_input.trim();
    if trimmed.is_empty() {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(42);
        // Mix with FNV-1a hash
        (now as u64) ^ 0xcbf2_9ce4_8422_2325
    } else if let Ok(num) = trimmed.parse::<i64>() {
        num as u64
    } else if let Ok(num) = trimmed.parse::<u64>() {
        num
    } else {
        trimmed.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3)
        })
    }
}

pub fn saves_dir() -> PathBuf {
    PathBuf::from(SAVES_DIR)
}

pub fn list_worlds_in(dir: &Path) -> Vec<WorldMeta> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut worlds = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join("world.json");
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(meta) = serde_json::from_str::<WorldMeta>(&content) {
                worlds.push(meta);
            }
        }
    }

    worlds.sort_by_key(|b| std::cmp::Reverse(b.last_played));
    worlds
}

pub fn list_worlds() -> Vec<WorldMeta> {
    list_worlds_in(&saves_dir())
}

pub fn create_world_in(dir: &Path, name: &str, seed_input: &str) -> std::io::Result<WorldMeta> {
    fs::create_dir_all(dir)?;
    let base_id = sanitize_folder_name(name);
    let mut id = base_id.clone();
    let mut counter = 1;

    while dir.join(&id).exists() {
        id = format!("{base_id}_{counter}");
        counter += 1;
    }

    let world_dir = dir.join(&id);
    fs::create_dir_all(&world_dir)?;

    let seed = parse_or_random_seed(seed_input);
    let meta = WorldMeta::new(id, name.trim().to_string(), seed);
    let json = serde_json::to_string_pretty(&meta)?;
    fs::write(world_dir.join("world.json"), json)?;

    Ok(meta)
}

pub fn create_world(name: &str, seed_input: &str) -> std::io::Result<WorldMeta> {
    create_world_in(&saves_dir(), name, seed_input)
}

pub fn save_world_in(dir: &Path, meta: &WorldMeta) -> std::io::Result<()> {
    let world_dir = dir.join(&meta.id);
    fs::create_dir_all(&world_dir)?;
    let json = serde_json::to_string_pretty(meta)?;
    fs::write(world_dir.join("world.json"), json)
}

pub fn save_world(meta: &WorldMeta) -> std::io::Result<()> {
    save_world_in(&saves_dir(), meta)
}

pub fn delete_world_in(dir: &Path, id: &str) -> std::io::Result<()> {
    let world_dir = dir.join(id);
    if world_dir.exists() {
        fs::remove_dir_all(world_dir)?;
    }
    Ok(())
}

pub fn delete_world(id: &str) -> std::io::Result<()> {
    delete_world_in(&saves_dir(), id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_save_lifecycle() {
        let temp = std::env::temp_dir().join(format!(
            "goblock_test_{}",
            SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        let _ = fs::remove_dir_all(&temp);

        // 1. Create world
        let mut meta1 = create_world_in(&temp, "My First World", "12345").expect("create world 1");
        assert_eq!(meta1.name, "My First World");
        assert_eq!(meta1.seed, 12345);

        // 2. Create another world with blank seed (random)
        let meta2 = create_world_in(&temp, "Another World", "").expect("create world 2");
        assert_eq!(meta2.name, "Another World");

        // 3. List worlds
        let list = list_worlds_in(&temp);
        assert_eq!(list.len(), 2);

        // 4. Update coordinates & save
        meta1.player_pos = [10.0, 75.0, -20.0];
        meta1.player_yaw = 45.0;
        meta1.last_played += 100;
        save_world_in(&temp, &meta1).expect("save world");

        let updated_list = list_worlds_in(&temp);
        assert_eq!(updated_list[0].id, meta1.id);
        assert_eq!(updated_list[0].player_pos, [10.0, 75.0, -20.0]);

        // 5. Delete world
        delete_world_in(&temp, &meta1.id).expect("delete world 1");
        let after_delete = list_worlds_in(&temp);
        assert_eq!(after_delete.len(), 1);
        assert_eq!(after_delete[0].id, meta2.id);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn seed_parsing_matches_rules() {
        assert_eq!(parse_or_random_seed("42"), 42);
        assert_eq!(parse_or_random_seed("-1"), u64::MAX);
        assert_ne!(parse_or_random_seed("hello"), parse_or_random_seed("world"));
    }
}

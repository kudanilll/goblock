//! World saves manager: persistent metadata and directory operations.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

pub const SAVE_FORMAT_VERSION: u32 = 1;
pub const AUTOSAVE_SECONDS: u64 = 60;

pub fn autosave_due(last: Instant, now: Instant) -> bool {
    now.checked_duration_since(last)
        .is_some_and(|d| d.as_secs() >= AUTOSAVE_SECONDS)
}

const SAVES_DIR: &str = "saves";

/// Persistent metadata for a world save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldMeta {
    #[serde(default)]
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub created_at: u64,
    pub last_played: u64,
    pub player_pos: [f64; 3],
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
            format_version: SAVE_FORMAT_VERSION,
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
            match serde_json::from_str::<WorldMeta>(&content) {
                Ok(meta) => worlds.push(meta),
                Err(e) => eprintln!("goblock: failed to parse {}: {e}", path.display()),
            }
        }
    }

    worlds.sort_by_key(|b| std::cmp::Reverse(b.last_played));
    worlds
}

pub fn list_worlds() -> Vec<WorldMeta> {
    list_worlds_in(&saves_dir())
}

fn write_meta(world_dir: &Path, meta: &WorldMeta) -> std::io::Result<()> {
    let tmp_path = world_dir.join("world.json.tmp");
    let target_path = world_dir.join("world.json");
    let json = serde_json::to_string_pretty(meta)?;
    {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(tmp_path, target_path)?;
    Ok(())
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
    write_meta(&world_dir, &meta)?;

    Ok(meta)
}

pub fn create_world(name: &str, seed_input: &str) -> std::io::Result<WorldMeta> {
    create_world_in(&saves_dir(), name, seed_input)
}

pub fn save_world_in(dir: &Path, meta: &WorldMeta) -> std::io::Result<()> {
    let world_dir = dir.join(&meta.id);
    fs::create_dir_all(&world_dir)?;
    write_meta(&world_dir, meta)
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

    fn temp_test_dir(name: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("goblock_{name}_{}_{count}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn world_save_lifecycle() {
        let temp = temp_test_dir("lifecycle");

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
    fn failed_save_keeps_the_old_world_json() {
        let temp = temp_test_dir("failed_save");
        let mut meta = create_world_in(&temp, "Resilient World", "123").expect("create world");
        let world_dir = temp.join(&meta.id);

        // Create a directory named world.json.tmp so writing to it fails.
        fs::create_dir(world_dir.join("world.json.tmp")).expect("create blocking directory");

        meta.name = "Corrupted Name".to_string();
        let result = save_world_in(&temp, &meta);
        assert!(result.is_err(), "save must fail when temp write fails");

        let loaded = list_worlds_in(&temp);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Resilient World");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn leftover_tmp_file_does_not_hide_a_world() {
        let temp = temp_test_dir("leftover_tmp");
        let mut meta = create_world_in(&temp, "Stable World", "456").expect("create world");
        let world_dir = temp.join(&meta.id);

        // Leave a stray world.json.tmp file.
        fs::write(world_dir.join("world.json.tmp"), "broken garbage json")
            .expect("write stray tmp");

        let loaded = list_worlds_in(&temp);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Stable World");

        // Later save should overwrite stray tmp cleanly and succeed.
        meta.player_yaw = 90.0;
        save_world_in(&temp, &meta).expect("save should succeed despite stray tmp");

        let loaded_after = list_worlds_in(&temp);
        assert_eq!(loaded_after[0].player_yaw, 90.0);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn corrupt_world_json_is_skipped() {
        let temp = temp_test_dir("corrupt");
        let meta = create_world_in(&temp, "Corrupt World", "789").expect("create world");
        let world_dir = temp.join(&meta.id);

        fs::write(world_dir.join("world.json"), "invalid json content")
            .expect("write corrupt world.json");

        let loaded = list_worlds_in(&temp);
        assert!(loaded.is_empty());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn old_world_json_still_loads() {
        let json = r#"{"id":"old","name":"Old","seed":7,"created_at":1,"last_played":2,"player_pos":[10.5,75.0,-20.25],"player_yaw":45.0,"player_pitch":-20.0,"player_flying":true}"#;
        let meta: WorldMeta = serde_json::from_str(json).expect("parse old world");
        assert_eq!(meta.format_version, 0);
        assert_eq!(meta.player_pos, [10.5, 75.0, -20.25]);
    }

    #[test]
    fn position_round_trips_without_rounding() {
        let temp = temp_test_dir("precise_pos");
        let mut meta = create_world_in(&temp, "Precise World", "999").expect("create world");
        meta.player_pos = [20000000.5, 80.0, -20000000.5];
        save_world_in(&temp, &meta).expect("save world");

        let loaded = list_worlds_in(&temp);
        assert_eq!(loaded[0].player_pos, [20000000.5, 80.0, -20000000.5]);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn autosave_due_after_sixty_seconds() {
        let start = std::time::Instant::now();
        assert!(!autosave_due(
            start,
            start + std::time::Duration::from_secs(59)
        ));
        assert!(autosave_due(
            start,
            start + std::time::Duration::from_secs(60)
        ));
        assert!(autosave_due(
            start,
            start + std::time::Duration::from_secs(61)
        ));
    }

    #[test]
    fn seed_parsing_matches_rules() {
        assert_eq!(parse_or_random_seed("42"), 42);
        assert_eq!(parse_or_random_seed("-1"), u64::MAX);
        assert_ne!(parse_or_random_seed("hello"), parse_or_random_seed("world"));
    }
}

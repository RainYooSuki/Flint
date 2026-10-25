use base64::{engine::general_purpose::STANDARD, Engine};
use flint_core::{
    net,
    paths::{normalize_relative_metadata_path, resolve_game_dir, safe_join},
    ports::{ArchiveEntry, ArchiveLimits, PathPort, SafeWorkspace},
};
use serde::{Deserialize, Serialize};
use flate2::read::GzDecoder;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const MAX_LEVEL_NBT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NBT_DEPTH: usize = 64;
const MAX_NBT_TAGS: usize = 1_000_000;
const MAX_NBT_ARRAY_ITEMS: usize = 16_000_000;
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IMPORT_ENTRIES: usize = 100_000;
const MAX_IMPORT_PATH_BYTES: usize = 4_096;
const MAX_IMPORT_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_IMPORT_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveInfo {
    pub folder_name: String,
    pub display_name: String,
    pub path: String,
    pub icon_data_url: Option<String>,
    pub last_played: Option<i64>,
    pub size_bytes: u64,
    pub game_mode: Option<String>,
    pub hardcore: bool,
    pub version_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveTransferResult {
    pub path: String,
    pub save: Option<SaveInfo>,
}

pub fn list_saves_blocking(game_dir: &str) -> Result<Vec<SaveInfo>, String> {
    let saves_dir = resolve_game_dir(game_dir).join("saves");
    if !saves_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&saves_dir).map_err(|error| {
        format!(
            "Cannot read saves directory ({}): {error}",
            saves_dir.display()
        )
    })?;
    let mut saves = Vec::new();
    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        if validate_save_folder_name(&folder_name).is_err() {
            continue;
        }
        let path = entry.path();
        if !is_regular_file(&path.join("level.dat")) {
            continue;
        }
        saves.push(build_save_info(&path, &folder_name));
    }
    saves.sort_by(|left, right| {
        right
            .last_played
            .unwrap_or_default()
            .cmp(&left.last_played.unwrap_or_default())
            .then_with(|| {
                left.display_name
                    .to_ascii_lowercase()
                    .cmp(&right.display_name.to_ascii_lowercase())
            })
    });
    Ok(saves)
}

pub fn backup_save_blocking(
    game_dir: &str,
    folder_name: &str,
) -> Result<SaveTransferResult, String> {
    let game_root = resolve_game_dir(game_dir);
    let save_path = validated_save_path(&game_root, folder_name)?;
    let backup_dir = game_root.join("backups").join("flint-saves");
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "Cannot create save backup directory ({}): {error}",
            backup_dir.display()
        )
    })?;
    let file_name = format!(
        "flint-save-{}-{}.zip",
        sanitize_file_component(folder_name),
        net::nonce()
    );
    let destination = backup_dir.join(file_name);
    write_save_zip(&save_path, &destination, Some(folder_name))?;
    Ok(SaveTransferResult {
        path: destination.display().to_string(),
        save: Some(build_save_info(&save_path, folder_name)),
    })
}

pub fn export_save_blocking(
    game_dir: &str,
    folder_name: &str,
    destination: &str,
) -> Result<SaveTransferResult, String> {
    let game_root = resolve_game_dir(game_dir);
    let save_path = validated_save_path(&game_root, folder_name)?;
    if destination.trim().is_empty() {
        return Err("Choose a destination ZIP file for the exported save.".to_string());
    }
    let destination = resolve_game_dir(destination);
    if destination.is_dir() {
        return Err("The save export destination is a directory, not a ZIP file.".to_string());
    }
    if destination.starts_with(&save_path) {
        return Err("The save export destination cannot be inside the save folder.".to_string());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create save export directory ({}): {error}",
                parent.display()
            )
        })?;
    }
    write_save_zip(&save_path, &destination, Some(folder_name))?;
    Ok(SaveTransferResult {
        path: destination.display().to_string(),
        save: Some(build_save_info(&save_path, folder_name)),
    })
}

pub fn import_save_blocking(game_dir: &str, archive_path: &str) -> Result<SaveInfo, String> {
    import_save_blocking_with_limits(game_dir, archive_path, import_archive_limits())
}

fn import_archive_limits() -> ArchiveLimits {
    ArchiveLimits {
        max_entries: MAX_IMPORT_ENTRIES,
        max_path_bytes: MAX_IMPORT_PATH_BYTES,
        max_uncompressed_bytes: MAX_IMPORT_TOTAL_BYTES,
    }
}

fn import_save_blocking_with_limits(
    game_dir: &str,
    archive_path: &str,
    limits: ArchiveLimits,
) -> Result<SaveInfo, String> {
    if archive_path.trim().is_empty() {
        return Err("Choose a save ZIP archive to import.".to_string());
    }
    let archive_path = resolve_game_dir(archive_path);
    ensure_regular_file(&archive_path)?;
    let saves_dir = resolve_game_dir(game_dir).join("saves");
    fs::create_dir_all(&saves_dir).map_err(|error| {
        format!(
            "Cannot create saves directory ({}): {error}",
            saves_dir.display()
        )
    })?;

    let file = fs::File::open(&archive_path).map_err(|error| {
        format!(
            "Cannot open save archive ({}): {error}",
            archive_path.display()
        )
    })?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Save archive is not a valid ZIP file: {error}"))?;
    let entries = collect_import_entries(&mut archive, &limits)?;
    let layout = determine_import_layout(&entries, &archive_path)?;
    let desired_name = unique_save_folder_name(&saves_dir, &layout.folder_name)?;
    let temp_name = format!(".{desired_name}.import-{}", net::nonce());
    let temp_path = saves_dir.join(&temp_name);
    if temp_path.exists() {
        return Err("A temporary import directory already exists. Try again.".to_string());
    }
    fs::create_dir(&temp_path).map_err(|error| {
        format!(
            "Cannot create temporary save import directory ({}): {error}",
            temp_path.display()
        )
    })?;

    let extraction_result = (|| -> Result<(), String> {
        let mut total_written = 0u64;
        let max_file_bytes = MAX_IMPORT_FILE_BYTES.min(limits.max_uncompressed_bytes);
        for entry in &entries {
            if entry.is_dir || entry.ignored {
                continue;
            }
            let relative = layout
                .relative_path(&entry.normalized_name)
                .ok_or_else(|| "Save archive contains an unexpected path.".to_string())?;
            let relative = normalize_relative_metadata_path(&relative)
                .ok_or_else(|| "Save archive contains an unsafe path.".to_string())?;
            let Some(target) = safe_join(&temp_path, &relative) else {
                return Err("Save archive contains an unsafe extraction path.".to_string());
            };
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Cannot create save directory ({}): {error}",
                        parent.display()
                    )
                })?;
            }
            let mut zip_file = archive
                .by_index(entry.index)
                .map_err(|error| format!("Cannot read save archive entry: {error}"))?;
            let declared_size = zip_file.size();
            let remaining_total = limits
                .max_uncompressed_bytes
                .saturating_sub(total_written);
            let read_limit = max_file_bytes.min(remaining_total).saturating_add(1);
            let mut output = fs::File::create(&target).map_err(|error| {
                format!("Cannot create save file ({}): {error}", target.display())
            })?;
            let written = io::copy(
                &mut zip_file.by_ref().take(read_limit),
                &mut output,
            )
            .map_err(|error| format!("Cannot extract save file ({}): {error}", target.display()))?;
            if written > max_file_bytes {
                return Err("A save archive entry exceeds the maximum file size.".to_string());
            }
            if written > remaining_total {
                return Err("Save archive exceeds the extraction size limit.".to_string());
            }
            if written != declared_size {
                return Err(format!(
                    "Save archive entry size changed while extracting: {relative}"
                ));
            }
            total_written = total_written
                .checked_add(written)
                .ok_or_else(|| "Save archive is too large.".to_string())?;
        }
        if !is_regular_file(&temp_path.join("level.dat")) {
            return Err("The imported archive does not contain a level.dat file.".to_string());
        }
        Ok(())
    })();
    if let Err(error) = extraction_result {
        let _ = fs::remove_dir_all(&temp_path);
        return Err(error);
    }

    let final_path = saves_dir.join(&desired_name);
    if final_path.exists() {
        let _ = fs::remove_dir_all(&temp_path);
        return Err("The imported save destination already exists.".to_string());
    }
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_dir_all(&temp_path);
        format!(
            "Cannot finalize imported save ({}): {error}",
            final_path.display()
        )
    })?;
    Ok(build_save_info(&final_path, &desired_name))
}

pub fn rename_save_blocking(
    game_dir: &str,
    folder_name: &str,
    new_name: &str,
) -> Result<SaveInfo, String> {
    validate_save_folder_name(new_name)?;
    let game_root = resolve_game_dir(game_dir);
    let source = validated_save_path(&game_root, folder_name)?;
    ensure_no_symlinks(&source)?;
    let saves_dir = game_root.join("saves");
    let destination = saves_dir.join(new_name);
    if destination.exists() {
        return Err("A save folder with the new name already exists.".to_string());
    }
    fs::rename(&source, &destination).map_err(|error| {
        format!(
            "Cannot rename save ({} -> {}): {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(build_save_info(&destination, new_name))
}

pub fn delete_save_blocking(game_dir: &str, folder_name: &str) -> Result<(), String> {
    let game_root = resolve_game_dir(game_dir);
    let save_path = validated_save_path(&game_root, folder_name)?;
    ensure_no_symlinks(&save_path)?;
    fs::remove_dir_all(&save_path).map_err(|error| {
        format!(
            "Cannot delete save folder ({}): {error}",
            save_path.display()
        )
    })
}

fn validated_save_path(game_root: &Path, folder_name: &str) -> Result<PathBuf, String> {
    validate_save_folder_name(folder_name)?;
    let saves_dir = game_root.join("saves");
    let path = saves_dir.join(folder_name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Cannot access save ({}): {error}", path.display()))?;
    if !metadata.file_type().is_dir() {
        return Err("Refusing to operate on a symlink or non-directory save.".to_string());
    }
    ensure_regular_file(&path.join("level.dat"))?;
    let canonical_saves = fs::canonicalize(&saves_dir)
        .map_err(|error| format!("Cannot resolve saves directory: {error}"))?;
    let canonical_save = fs::canonicalize(&path)
        .map_err(|error| format!("Cannot resolve save directory: {error}"))?;
    if !canonical_save.starts_with(&canonical_saves) {
        return Err("The save path escapes the saves directory.".to_string());
    }
    Ok(canonical_save)
}

fn validate_save_folder_name(folder_name: &str) -> Result<(), String> {
    if folder_name.is_empty()
        || folder_name.len() > 160
        || folder_name == "."
        || folder_name == ".."
        || folder_name.ends_with('.')
        || folder_name.ends_with(' ')
        || folder_name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
    {
        return Err("The save folder name is unsafe.".to_string());
    }
    Ok(())
}

fn build_save_info(path: &Path, folder_name: &str) -> SaveInfo {
    let level_data = parse_level_data(&path.join("level.dat")).unwrap_or_default();
    let icon_path = path.join("icon.png");
    let icon_data_url = if is_regular_file(&icon_path) {
        fs::metadata(&icon_path)
            .ok()
            .filter(|metadata| metadata.len() <= MAX_ICON_BYTES)
            .and_then(|_| fs::read(&icon_path).ok())
            .map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    } else {
        None
    };
    SaveInfo {
        folder_name: folder_name.to_string(),
        display_name: level_data
            .level_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| folder_name.to_string()),
        path: path.display().to_string(),
        icon_data_url,
        last_played: level_data.last_played.filter(|value| *value > 0),
        size_bytes: directory_size(path),
        game_mode: level_data.game_mode,
        hardcore: level_data.hardcore,
        version_name: level_data.version_name,
    }
}

fn directory_size(path: &Path) -> u64 {
    let entries = match fs::read_dir(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            total = total.saturating_add(directory_size(&path));
        }
    }
    total
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot access file ({}): {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err("Refusing to operate on a symlink or non-file path.".to_string());
    }
    Ok(())
}

fn sanitize_file_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    if sanitized.is_empty() {
        "save".to_string()
    } else {
        sanitized
    }
}

#[derive(Debug, Clone, Default)]
struct LevelData {
    level_name: Option<String>,
    last_played: Option<i64>,
    game_mode: Option<String>,
    hardcore: bool,
    version_name: Option<String>,
}

fn parse_level_data(path: &Path) -> Result<LevelData, String> {
    ensure_regular_file(path)?;
    let file = fs::File::open(path)
        .map_err(|error| format!("Cannot open level.dat ({}): {error}", path.display()))?;
    let mut decoder = GzDecoder::new(file);
    let mut bytes = Vec::new();
    Read::by_ref(&mut decoder)
        .take(MAX_LEVEL_NBT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot decompress level.dat: {error}"))?;
    if bytes.len() as u64 > MAX_LEVEL_NBT_BYTES {
        return Err("level.dat exceeds the decompressed size limit.".to_string());
    }
    let root = parse_nbt_document(&bytes)?;
    let Some(data) = root
        .as_compound()
        .and_then(|root| root.get("Data"))
        .and_then(NbtTag::as_compound)
    else {
        return Err("level.dat has no Data compound.".to_string());
    };
    let level_name = data
        .get("LevelName")
        .and_then(NbtTag::as_str)
        .map(str::to_string);
    let last_played = data.get("LastPlayed").and_then(NbtTag::as_i64);
    let game_mode = data
        .get("GameType")
        .and_then(NbtTag::as_i64)
        .map(game_mode_name);
    let hardcore = data
        .get("hardcore")
        .or_else(|| data.get("Hardcore"))
        .map(NbtTag::as_bool)
        .unwrap_or(false);
    let version_name = data
        .get("Version")
        .and_then(NbtTag::as_compound)
        .and_then(|version| version.get("Name"))
        .and_then(NbtTag::as_str)
        .map(str::to_string);
    Ok(LevelData {
        level_name,
        last_played,
        game_mode,
        hardcore,
        version_name,
    })
}

fn game_mode_name(value: i64) -> String {
    match value {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => "unknown",
    }
    .to_string()
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    fn as_compound(&self) -> Option<&HashMap<String, NbtTag>> {
        match self {
            Self::Compound(value) => Some(value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Byte(value) => Some(*value as i64),
            Self::Short(value) => Some(*value as i64),
            Self::Int(value) => Some(*value as i64),
            Self::Long(value) => Some(*value),
            _ => None,
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            Self::Byte(value) => *value != 0,
            Self::Short(value) => *value != 0,
            Self::Int(value) => *value != 0,
            Self::Long(value) => *value != 0,
            _ => false,
        }
    }
}

struct NbtCursor<'a> {
    data: &'a [u8],
    position: usize,
    tags: usize,
}

fn parse_nbt_document(data: &[u8]) -> Result<NbtTag, String> {
    let mut cursor = NbtCursor {
        data,
        position: 0,
        tags: 0,
    };
    let tag_type = cursor.read_u8()?;
    if tag_type == 0 {
        return Err("NBT root tag is missing.".to_string());
    }
    let _root_name = cursor.read_string()?;
    cursor.read_payload(tag_type, 0)
}

impl<'a> NbtCursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| "NBT offset overflow.".to_string())?;
        if end > self.data.len() {
            return Err("NBT data ended unexpectedly.".to_string());
        }
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn read_i8(&mut self) -> Result<i8, String> {
        Ok(self.read_u8()? as i8)
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| "Invalid NBT short.".to_string())?,
        ))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| "Invalid NBT int.".to_string())?,
        ))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| "Invalid NBT long.".to_string())?,
        ))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| "Invalid NBT float.".to_string())?,
        ))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| "Invalid NBT double.".to_string())?,
        ))
    }

    fn read_string(&mut self) -> Result<String, String> {
        let length = u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| "Invalid NBT string length.".to_string())?,
        ) as usize;
        Ok(String::from_utf8_lossy(self.take(length)?).to_string())
    }

    fn read_length(&mut self) -> Result<usize, String> {
        let length = self.read_i32()?;
        if length < 0 {
            return Err("NBT array/list has a negative length.".to_string());
        }
        let length = length as usize;
        if length > MAX_NBT_ARRAY_ITEMS {
            return Err("NBT array/list exceeds the safety limit.".to_string());
        }
        Ok(length)
    }

    fn read_payload(&mut self, tag_type: u8, depth: usize) -> Result<NbtTag, String> {
        if depth > MAX_NBT_DEPTH {
            return Err("NBT nesting exceeds the safety limit.".to_string());
        }
        self.tags = self
            .tags
            .checked_add(1)
            .ok_or_else(|| "NBT tag count overflow.".to_string())?;
        if self.tags > MAX_NBT_TAGS {
            return Err("NBT contains too many tags.".to_string());
        }
        match tag_type {
            1 => Ok(NbtTag::Byte(self.read_i8()?)),
            2 => Ok(NbtTag::Short(self.read_i16()?)),
            3 => Ok(NbtTag::Int(self.read_i32()?)),
            4 => Ok(NbtTag::Long(self.read_i64()?)),
            5 => Ok(NbtTag::Float(f32::from_bits(self.read_u32()?))),
            6 => Ok(NbtTag::Double(f64::from_bits(self.read_u64()?))),
            7 => {
                let length = self.read_length()?;
                Ok(NbtTag::ByteArray(self.take(length)?.to_vec()))
            }
            8 => Ok(NbtTag::String(self.read_string()?)),
            9 => {
                let element_type = self.read_u8()?;
                let length = self.read_length()?;
                if length > 0 && element_type == 0 {
                    return Err("NBT list has elements but no element type.".to_string());
                }
                let mut values = Vec::with_capacity(length.min(4096));
                for _ in 0..length {
                    values.push(self.read_payload(element_type, depth + 1)?);
                }
                Ok(NbtTag::List(values))
            }
            10 => {
                let mut values = HashMap::new();
                loop {
                    let child_type = self.read_u8()?;
                    if child_type == 0 {
                        break;
                    }
                    let name = self.read_string()?;
                    let value = self.read_payload(child_type, depth + 1)?;
                    values.insert(name, value);
                }
                Ok(NbtTag::Compound(values))
            }
            11 => {
                let length = self.read_length()?;
                if length > self.data.len().saturating_sub(self.position) / 4 {
                    return Err("NBT int array length exceeds available data.".to_string());
                }
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.read_i32()?);
                }
                Ok(NbtTag::IntArray(values))
            }
            12 => {
                let length = self.read_length()?;
                if length > self.data.len().saturating_sub(self.position) / 8 {
                    return Err("NBT long array length exceeds available data.".to_string());
                }
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.read_i64()?);
                }
                Ok(NbtTag::LongArray(values))
            }
            _ => Err(format!("Unsupported NBT tag type {tag_type}.")),
        }
    }
}
#[derive(Debug)]
struct ImportEntry {
    index: usize,
    normalized_name: String,
    is_dir: bool,
    ignored: bool,
}

#[derive(Debug)]
struct ImportLayout {
    strip_prefix: Option<String>,
    folder_name: String,
}

impl ImportLayout {
    fn relative_path(&self, name: &str) -> Option<String> {
        let relative = match self.strip_prefix.as_deref() {
            Some(prefix) => name.strip_prefix(prefix)?.trim_start_matches('/'),
            None => name,
        };
        if relative.is_empty() {
            None
        } else {
            Some(relative.to_string())
        }
    }
}

fn collect_import_entries(
    archive: &mut ZipArchive<fs::File>,
    limits: &ArchiveLimits,
) -> Result<Vec<ImportEntry>, String> {
    struct RawImportEntry {
        index: usize,
        name: String,
        is_dir: bool,
        uncompressed_size: u64,
        is_symlink: bool,
    }

    if archive.len() > limits.max_entries {
        return Err(format!(
            "Archive contains {} entries, limit {}.",
            archive.len(),
            limits.max_entries
        ));
    }
    let mut raw_entries = Vec::new();
    let mut archive_entries = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Cannot inspect save archive entry: {error}"))?;
        let raw_name = entry.name().replace('\\', "/");
        let is_dir = entry.is_dir() || raw_name.ends_with('/');
        let trimmed = raw_name.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        let is_symlink = entry.unix_mode().map(is_zip_symlink_mode).unwrap_or(false);
        let uncompressed_size = entry.size();
        archive_entries.push(ArchiveEntry {
            relative_path: trimmed.to_string(),
            uncompressed_size,
            is_directory: is_dir,
            is_symlink,
        });
        raw_entries.push(RawImportEntry {
            index,
            name: trimmed.to_string(),
            is_dir,
            uncompressed_size,
            is_symlink,
        });
    }
    if raw_entries.is_empty() {
        return Err("Save archive is empty.".to_string());
    }

    SafeWorkspace.validate_archive_entries(&archive_entries, limits)?;

    let mut entries = Vec::new();
    for raw in raw_entries {
        if raw.is_symlink {
            return Err(format!(
                "Save archives containing symbolic links are not supported: {}",
                raw.name
            ));
        }
        let normalized = normalize_relative_metadata_path(&raw.name)
            .ok_or_else(|| "Save archive contains an unsafe path.".to_string())?;
        if raw.uncompressed_size > MAX_IMPORT_FILE_BYTES {
            return Err(format!("Save archive entry is too large: {normalized}"));
        }
        let ignored = normalized == "__MACOSX"
            || normalized.starts_with("__MACOSX/")
            || normalized == ".DS_Store"
            || normalized.ends_with("/.DS_Store");
        entries.push(ImportEntry {
            index: raw.index,
            normalized_name: normalized,
            is_dir: raw.is_dir,
            ignored,
        });
    }
    Ok(entries)
}

fn is_zip_symlink_mode(mode: u32) -> bool {
    mode & 0o170000 == 0o120000
}

fn determine_import_layout(
    entries: &[ImportEntry],
    archive_path: &Path,
) -> Result<ImportLayout, String> {
    let names: HashSet<&str> = entries
        .iter()
        .filter(|entry| !entry.ignored)
        .map(|entry| entry.normalized_name.as_str())
        .collect();
    if names.contains("level.dat") {
        if names
            .iter()
            .any(|name| *name != "level.dat" && name.ends_with("/level.dat"))
        {
            return Err("Save archive contains more than one save.".to_string());
        }
        let archive_stem = archive_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported Save");
        return Ok(ImportLayout {
            strip_prefix: None,
            folder_name: sanitize_save_folder_name(archive_stem),
        });
    }

    let mut top_levels = HashSet::new();
    for entry in entries.iter().filter(|entry| !entry.ignored) {
        let top = entry.normalized_name.split('/').next().unwrap_or_default();
        if !top.is_empty() {
            top_levels.insert(top.to_string());
        }
    }
    if top_levels.len() != 1 {
        return Err(
            "Save archive must contain exactly one save folder or a top-level level.dat."
                .to_string(),
        );
    }
    let top = top_levels.into_iter().next().unwrap_or_default();
    validate_save_folder_name(&top)
        .map_err(|_| "Save archive contains an unsafe top-level folder name.".to_string())?;
    let level_path = format!("{top}/level.dat");
    if !names.contains(level_path.as_str()) {
        return Err("The save archive folder does not contain level.dat.".to_string());
    }
    Ok(ImportLayout {
        strip_prefix: Some(top.clone()),
        folder_name: top,
    })
}

fn sanitize_save_folder_name(value: &str) -> String {
    let mut sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(96)
        .collect();
    sanitized = sanitized.trim().trim_matches('.').trim().to_string();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "Imported Save".to_string()
    } else {
        sanitized
    }
}

fn unique_save_folder_name(saves_dir: &Path, base_name: &str) -> Result<String, String> {
    let base = sanitize_save_folder_name(base_name);
    for suffix in 1..=9999u32 {
        let candidate = if suffix == 1 {
            base.clone()
        } else {
            let suffix = format!("-{suffix}");
            let max_base = 159usize.saturating_sub(suffix.len());
            format!("{}{}", truncate_chars(&base, max_base), suffix)
        };
        if validate_save_folder_name(&candidate).is_err() {
            continue;
        }
        if !saves_dir.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err("Cannot choose a unique save folder name.".to_string())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn ensure_no_symlinks(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("Cannot inspect save path ({}): {error}", root.display()))?;
    if metadata.file_type().is_symlink() {
        return Err("Refusing to operate on a save tree containing symbolic links.".to_string());
    }
    if metadata.is_dir() {
        let entries = fs::read_dir(root).map_err(|error| {
            format!(
                "Cannot inspect save directory ({}): {error}",
                root.display()
            )
        })?;
        for entry in entries.flatten() {
            ensure_no_symlinks(&entry.path())?;
        }
    } else if !metadata.is_file() {
        return Err("Save tree contains an unsupported filesystem entry.".to_string());
    }
    Ok(())
}

#[derive(Debug)]
struct ArchiveFileEntry {
    absolute: PathBuf,
    relative: String,
    size: u64,
}

fn collect_save_archive_files(save_path: &Path) -> Result<Vec<ArchiveFileEntry>, String> {
    let mut result = Vec::new();
    let mut total_bytes = 0u64;
    collect_save_archive_files_recursive(save_path, save_path, &mut result, &mut total_bytes)?;
    if result.is_empty() {
        return Err("The save folder has no files to archive.".to_string());
    }
    result.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(result)
}

fn collect_save_archive_files_recursive(
    root: &Path,
    current: &Path,
    result: &mut Vec<ArchiveFileEntry>,
    total_bytes: &mut u64,
) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|error| {
        format!(
            "Cannot read save directory ({}): {error}",
            current.display()
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Cannot inspect save file ({}): {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(
                "Save contains a symbolic link; backup was refused for safety.".to_string(),
            );
        }
        if metadata.is_dir() {
            collect_save_archive_files_recursive(root, &path, result, total_bytes)?;
        } else if metadata.is_file() {
            if result.len() >= MAX_ARCHIVE_FILES {
                return Err("Save contains too many files to archive.".to_string());
            }
            *total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "Save archive size overflow.".to_string())?;
            if *total_bytes > MAX_IMPORT_TOTAL_BYTES {
                return Err("Save is too large to archive.".to_string());
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "Cannot determine a safe save archive path.".to_string())?;
            let components: Vec<String> = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect();
            if components.is_empty()
                || components.iter().any(|component| {
                    component.is_empty()
                        || component == "."
                        || component == ".."
                        || component.contains('/')
                        || component.contains('\\')
                })
            {
                return Err("Cannot determine a safe save archive path.".to_string());
            }
            result.push(ArchiveFileEntry {
                absolute: path,
                relative: components.join("/"),
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn write_save_zip(
    save_path: &Path,
    destination: &Path,
    archive_root: Option<&str>,
) -> Result<(), String> {
    ensure_no_symlinks(save_path)?;
    let files = collect_save_archive_files(save_path)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create save ZIP directory ({}): {error}",
                parent.display()
            )
        })?;
    }
    let temp = net::temp_path_near(destination, "tmp");
    let result = (|| -> Result<(), String> {
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|error| format!("Cannot create temporary save ZIP: {error}"))?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for entry in files {
            let archive_name = match archive_root {
                Some(root) => format!("{root}/{}", entry.relative),
                None => entry.relative.clone(),
            };
            writer
                .start_file(&archive_name, options)
                .map_err(|error| format!("Cannot add save file to ZIP: {error}"))?;
            let mut source = fs::File::open(&entry.absolute).map_err(|error| {
                format!(
                    "Cannot read save file ({}): {error}",
                    entry.absolute.display()
                )
            })?;
            let written = io::copy(&mut source, &mut writer)
                .map_err(|error| format!("Cannot write save file to ZIP: {error}"))?;
            if written != entry.size {
                return Err("Save file changed while it was being archived.".to_string());
            }
        }
        let file = writer
            .finish()
            .map_err(|error| format!("Cannot finalize save ZIP: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Cannot flush save ZIP: {error}"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    match fs::rename(&temp, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if !destination.exists() {
                let _ = fs::remove_file(&temp);
                return Err(format!("Cannot finalize save ZIP: {first_error}"));
            }
            let backup = destination.with_file_name(format!(
                ".{}.{}.{}.bak",
                destination
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("save.zip"),
                std::process::id(),
                net::nonce()
            ));
            fs::rename(destination, &backup)
                .map_err(|error| format!("Cannot replace existing save ZIP: {error}"))?;
            match fs::rename(&temp, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, destination);
                    let _ = fs::remove_file(&temp);
                    Err(format!("Cannot finalize save ZIP: {error}"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn push_nbt_string(output: &mut Vec<u8>, value: &str) {
        output.extend_from_slice(&(value.len() as u16).to_be_bytes());
        output.extend_from_slice(value.as_bytes());
    }

    fn sample_level_nbt() -> Vec<u8> {
        let mut output = Vec::new();
        output.push(10);
        push_nbt_string(&mut output, "");
        output.push(10);
        push_nbt_string(&mut output, "Data");
        output.push(8);
        push_nbt_string(&mut output, "LevelName");
        push_nbt_string(&mut output, "Test World");
        output.push(4);
        push_nbt_string(&mut output, "LastPlayed");
        output.extend_from_slice(&1_700_000_000_000i64.to_be_bytes());
        output.push(3);
        push_nbt_string(&mut output, "GameType");
        output.extend_from_slice(&1i32.to_be_bytes());
        output.push(1);
        push_nbt_string(&mut output, "hardcore");
        output.push(1);
        output.push(10);
        push_nbt_string(&mut output, "Version");
        output.push(8);
        push_nbt_string(&mut output, "Name");
        push_nbt_string(&mut output, "1.20.1");
        output.push(0);
        output.push(0);
        output.push(0);
        output
    }

    fn write_level_dat(path: &Path) {
        let nbt = sample_level_nbt();
        let file = fs::File::create(path).expect("level.dat");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(&nbt).expect("compress level.dat");
        encoder.finish().expect("finish level.dat");
    }

    fn level_dat_bytes() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&sample_level_nbt())
            .expect("compress sample level.dat");
        encoder.finish().expect("finish sample level.dat")
    }

    fn write_import_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("import zip file");
        let mut writer = ZipWriter::new(file);
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("zip entry");
            writer.write_all(data).expect("zip data");
        }
        writer.finish().expect("finish import zip");
    }

    fn write_symlink_import_zip(path: &Path, name: &str) {
        let file = fs::File::create(path).expect("symlink zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(
                name,
                SimpleFileOptions::default().unix_permissions(0o120777),
            )
            .expect("symlink zip entry");
        writer.write_all(b"../outside").expect("symlink target");
        writer.finish().expect("finish symlink zip");
    }

    fn patch_zip_uncompressed_size(path: &Path, uncompressed_size: u32) {
        let mut bytes = fs::read(path).expect("read zip for patch");
        let signatures: [([u8; 4], usize); 2] = [
            ([0x50, 0x4b, 0x03, 0x04], 22usize),
            ([0x50, 0x4b, 0x01, 0x02], 24usize),
        ];
        let mut patched = 0;
        for (signature, offset) in signatures {
            let position = bytes
                .windows(signature.len())
                .position(|window| window == &signature[..])
                .expect("zip signature");
            let field = position + offset;
            bytes[field..field + 4]
                .copy_from_slice(&uncompressed_size.to_le_bytes());
            patched += 1;
        }
        assert_eq!(patched, 2, "one local and one central header");
        fs::write(path, bytes).expect("write patched zip");
    }

    fn import_temp_dirs(game_dir: &Path) -> Vec<PathBuf> {
        let saves_dir = game_dir.join("saves");
        let Ok(entries) = fs::read_dir(saves_dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".import-")
            })
            .map(|entry| entry.path())
            .collect()
    }
    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "flint-{label}-{}-{}",
            std::process::id(),
            net::nonce()
        ))
    }

    #[test]
    fn parses_level_dat_nbt_fields() {
        let root = parse_nbt_document(&sample_level_nbt()).expect("NBT root");
        let data = root
            .as_compound()
            .and_then(|value| value.get("Data"))
            .and_then(NbtTag::as_compound)
            .expect("Data compound");
        assert_eq!(
            data.get("LevelName").and_then(NbtTag::as_str),
            Some("Test World")
        );
        assert_eq!(
            data.get("LastPlayed").and_then(NbtTag::as_i64),
            Some(1_700_000_000_000)
        );
        assert!(data.get("hardcore").map(NbtTag::as_bool).unwrap_or(false));
        assert_eq!(game_mode_name(1), "creative");
        assert_eq!(game_mode_name(3), "spectator");
    }

    #[test]
    fn builds_save_info_from_gzip_level_dat_and_icon() {
        let root = temp_root("save-info");
        let save = root.join("saves").join("Test World");
        fs::create_dir_all(&save).expect("save dir");
        write_level_dat(&save.join("level.dat"));
        fs::write(save.join("icon.png"), b"png-data").expect("icon");
        let info = build_save_info(&save, "Test World");
        assert_eq!(info.display_name, "Test World");
        assert_eq!(info.game_mode.as_deref(), Some("creative"));
        assert!(info.hardcore);
        assert_eq!(info.version_name.as_deref(), Some("1.20.1"));
        assert_eq!(info.last_played, Some(1_700_000_000_000));
        assert!(info
            .icon_data_url
            .as_deref()
            .unwrap_or_default()
            .starts_with("data:image/png;base64,"));
        assert!(info.size_bytes > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backs_up_and_imports_a_save_zip() {
        let root = temp_root("save-backup");
        let game_dir = root.join("game");
        let save = game_dir.join("saves").join("World");
        fs::create_dir_all(save.join("region")).expect("save dirs");
        write_level_dat(&save.join("level.dat"));
        fs::write(save.join("region").join("r.0.0.mca"), b"region-data").expect("region");
        let game_dir_text = game_dir.display().to_string();

        let backup = backup_save_blocking(&game_dir_text, "World").expect("backup");
        assert!(Path::new(&backup.path).is_file());
        let archive_file = fs::File::open(&backup.path).expect("backup zip");
        let mut archive = ZipArchive::new(archive_file).expect("open backup zip");
        assert!(archive.by_name("World/level.dat").is_ok());
        assert!(archive.by_name("World/region/r.0.0.mca").is_ok());

        let imported = import_save_blocking(&game_dir_text, &backup.path).expect("import");
        assert_eq!(imported.folder_name, "World-2");
        assert_eq!(imported.display_name, "Test World");
        assert!(game_dir
            .join("saves")
            .join("World-2")
            .join("level.dat")
            .is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_zip_path_traversal() {
        let root = temp_root("save-traversal");
        let game_dir = root.join("game");
        fs::create_dir_all(&game_dir).expect("game dir");
        let archive_path = root.join("malicious.zip");
        let file = fs::File::create(&archive_path).expect("zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("../escape/level.dat", SimpleFileOptions::default())
            .expect("zip entry");
        writer.write_all(b"bad").expect("zip data");
        writer.finish().expect("finish zip");

        let error = import_save_blocking(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
        )
        .expect_err("traversal must be rejected by the core archive policy");
        assert!(error.contains("Unsafe archive entry path"));
        assert!(!root.join("escape").join("level.dat").exists());
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_symlink_entries_before_extraction() {
        let root = temp_root("save-symlink");
        let game_dir = root.join("game");
        let archive_path = root.join("symlink.zip");
        write_symlink_import_zip(&archive_path, "World/link");
        let error = import_save_blocking(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
        )
        .expect_err("symlink entry must fail closed");
        assert!(error.to_ascii_lowercase().contains("symlink"));
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_and_case_alias_entries_before_extraction() {
        let root = temp_root("save-duplicate");
        let game_dir = root.join("game");
        let archive_path = root.join("duplicate.zip");
        let level = level_dat_bytes();
        write_import_zip(
            &archive_path,
            &[
                ("World/level.dat", level.as_slice()),
                ("World/Data.bin", b"one"),
                ("World/data.BIN", b"two"),
            ],
        );
        let error = import_save_blocking(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
        )
        .expect_err("case alias duplicate must fail closed");
        assert!(error.contains("duplicate entry path"));
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn injected_archive_limits_reject_entry_and_size_overflows() {
        let root = temp_root("save-limits");
        let game_dir = root.join("game");
        let archive_path = root.join("limits.zip");
        let level = level_dat_bytes();
        write_import_zip(
            &archive_path,
            &[
                ("World/level.dat", level.as_slice()),
                ("World/region.bin", b"region"),
            ],
        );

        let entry_error = import_save_blocking_with_limits(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
            ArchiveLimits {
                max_entries: 1,
                max_path_bytes: MAX_IMPORT_PATH_BYTES,
                max_uncompressed_bytes: MAX_IMPORT_TOTAL_BYTES,
            },
        )
        .expect_err("entry-count limit must fail closed");
        assert_eq!(entry_error, "Archive contains 2 entries, limit 1.");
        assert!(!game_dir.join("saves").join("World").exists());
        assert!(import_temp_dirs(&game_dir).is_empty());

        let size_error = import_save_blocking_with_limits(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
            ArchiveLimits {
                max_entries: MAX_IMPORT_ENTRIES,
                max_path_bytes: MAX_IMPORT_PATH_BYTES,
                max_uncompressed_bytes: 1,
            },
        )
        .expect_err("declared-size limit must fail closed");
        assert!(size_error.contains("uncompressed data exceeds"));
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn entry_limit_rejects_before_path_validation_or_temp_creation() {
        let root = temp_root("save-early-entry-limit");
        let game_dir = root.join("game");
        let archive_path = root.join("early-limit.zip");
        let level = level_dat_bytes();
        write_import_zip(
            &archive_path,
            &[
                ("../escape/level.dat", b"bad"),
                ("World/level.dat", level.as_slice()),
            ],
        );
        let error = import_save_blocking_with_limits(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
            ArchiveLimits {
                max_entries: 1,
                max_path_bytes: MAX_IMPORT_PATH_BYTES,
                max_uncompressed_bytes: MAX_IMPORT_TOTAL_BYTES,
            },
        )
        .expect_err("entry-count limit must reject before collection/path validation");
        assert_eq!(error, "Archive contains 2 entries, limit 1.");
        assert!(!root.join("escape").exists());
        assert!(!game_dir.join("saves").join("World").exists());
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn declared_smaller_than_actual_expansion_fails_and_cleans_temporary_directory() {
        let root = temp_root("save-expansion");
        let game_dir = root.join("game");
        let archive_path = root.join("expanded.zip");
        let level = level_dat_bytes();
        write_import_zip(
            &archive_path,
            &[("World/level.dat", level.as_slice())],
        );
        patch_zip_uncompressed_size(&archive_path, 1);
        let error = import_save_blocking(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
        )
        .expect_err("declared/actual size mismatch must fail closed");
        assert!(
            error.contains("size changed")
                || error.contains("Cannot extract save file")
                || error.contains("level.dat")
        );
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extraction_failure_cleans_temporary_import_directory() {
        let root = temp_root("save-cleanup");
        let game_dir = root.join("game");
        let archive_path = root.join("cleanup.zip");
        let level = level_dat_bytes();
        write_import_zip(
            &archive_path,
            &[
                ("World/level.dat", level.as_slice()),
                ("World/blocked", b"file"),
                ("World/blocked/child.bin", b"child"),
            ],
        );
        let error = import_save_blocking(
            &game_dir.display().to_string(),
            &archive_path.display().to_string(),
        )
        .expect_err("file/directory collision must fail after partial extraction");
        assert!(error.contains("Cannot create save directory"));
        assert!(import_temp_dirs(&game_dir).is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn validates_save_folder_names() {
        assert!(validate_save_folder_name("My World").is_ok());
        assert!(validate_save_folder_name("world_1").is_ok());
        assert!(validate_save_folder_name("../escape").is_err());
        assert!(validate_save_folder_name("a/b").is_err());
        assert!(validate_save_folder_name("CON?").is_err());
    }

    #[test]
    fn parses_direct_and_folder_import_layouts() {
        let direct = vec![
            ImportEntry {
                index: 0,
                normalized_name: "level.dat".to_string(),
                is_dir: false,
                ignored: false,
            },
            ImportEntry {
                index: 1,
                normalized_name: "region/r.0.0.mca".to_string(),
                is_dir: false,
                ignored: false,
            },
        ];
        let layout = determine_import_layout(&direct, Path::new("MySave.zip")).expect("direct");
        assert_eq!(layout.folder_name, "MySave");
        assert_eq!(
            layout.relative_path("level.dat").as_deref(),
            Some("level.dat")
        );

        let folder = vec![
            ImportEntry {
                index: 0,
                normalized_name: "World/level.dat".to_string(),
                is_dir: false,
                ignored: false,
            },
            ImportEntry {
                index: 1,
                normalized_name: "World/region/r.0.0.mca".to_string(),
                is_dir: false,
                ignored: false,
            },
        ];
        let layout = determine_import_layout(&folder, Path::new("anything.zip")).expect("folder");
        assert_eq!(layout.folder_name, "World");
        assert_eq!(
            layout.relative_path("World/region/r.0.0.mca").as_deref(),
            Some("region/r.0.0.mca")
        );
    }
}

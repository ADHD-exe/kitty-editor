use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    String,
    Number,
    NumberWithUnit,
    Boolean,
    Enum,
    Color,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingMetadata {
    pub key: String,
    pub category: String,
    pub description: String,
    pub default_value: Option<String>,
    pub examples: Vec<String>,
    pub enum_choices: Vec<String>,
    pub repeatable: bool,
    pub value_type: ValueType,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingValue {
    pub key: String,
    pub value: String,
    pub source_file: PathBuf,
    pub line_no: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEntry {
    pub mode: String,
    pub shortcut: String,
    pub action: String,
    pub option_prefix: String,
    pub source_file: PathBuf,
    pub line_no: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeLine {
    pub raw: String,
    pub path: String,
    pub source_file: PathBuf,
    pub line_no: usize,
}

#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub values: HashMap<String, Vec<SettingValue>>,
    pub keymaps: Vec<MapEntry>,
    pub includes: Vec<IncludeLine>,
    pub leading_block: String,
    pub main_file: PathBuf,
}

impl EffectiveConfig {
    pub fn last_value(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key).and_then(|items| items.last())
    }
}

#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: String,
    pub path: PathBuf,
    pub preview: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationState {
    Valid,
    Invalid(String),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Mode {
    Browse,
    Search,
    Edit,
    EnumPicker,
    Diff,
    Themes,
    Keybindings,
    Confirm,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Categories,
    Settings,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    Settings,
    Shortcuts,
    Themes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMode {
    Full,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapField {
    Shortcut,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutView {
    Custom,
    Effective,
    Defaults,
}

impl ShortcutView {
    pub fn title(self) -> &'static str {
        match self {
            Self::Custom => "Custom",
            Self::Effective => "Effective",
            Self::Defaults => "Defaults",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutStatus {
    Default,
    Added,
    Changed,
    Removed,
}

impl ShortcutStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRow {
    pub mode: String,
    pub shortcut: String,
    pub raw_shortcut: String,
    pub action: String,
    pub status: ShortcutStatus,
    pub detail: String,
    pub source_file: PathBuf,
    pub line_no: usize,
    pub option_prefix: String,
    pub edited_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditTarget {
    Setting(String),
    KeymapShortcut(usize),
    ShortcutOverride(ShortcutRow, KeymapField),
    ThemeSaveAs,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub metadata: Vec<SettingMetadata>,
    pub metadata_by_key: HashMap<String, SettingMetadata>,
    pub effective: EffectiveConfig,
    pub default_keymaps: Vec<MapEntry>,
    pub edited_values: HashMap<String, String>,
    pub edited_keymaps: Vec<MapEntry>,
    pub selected_category: usize,
    pub selected_setting: usize,
    pub categories: Vec<String>,
    pub search_query: String,
    pub search_target: SearchTarget,
    pub search_results: Vec<usize>,
    pub theme_query: String,
    pub mode: Mode,
    pub focus: Focus,
    pub edit_buffer: String,
    pub edit_target: Option<EditTarget>,
    pub enum_index: usize,
    pub keymap_field: KeymapField,
    pub detail_scroll: u16,
    pub diff_scroll: u16,
    pub status: String,
    pub diff_lines: Vec<DiffLine>,
    pub themes: Vec<ThemeEntry>,
    pub themes_dir: Option<PathBuf>,
    pub current_theme_artifact: PathBuf,
    pub selected_theme: usize,
    pub shortcut_view: ShortcutView,
    pub shortcut_query: String,
    pub selected_shortcut: usize,
    #[allow(dead_code)]
    pub pending_quit: bool,
    pub current_theme_include: Option<String>,
    pub pending_theme_path: Option<PathBuf>,
    pub theme_edit_save_path: Option<PathBuf>,
    pub theme_preview_active: bool,
    pub live_theme_edit: bool,
    pub theme_edit_keys: Vec<String>,
    pub theme_edit_dirty: bool,
}

impl AppConfig {
    pub fn current_setting_index(&self) -> Option<usize> {
        self.search_results.get(self.selected_setting).copied()
    }

    pub fn current_setting(&self) -> Option<&SettingMetadata> {
        self.current_setting_index()
            .and_then(|idx| self.metadata.get(idx))
    }

    pub fn current_value_for(&self, key: &str) -> Option<String> {
        if let Some(v) = self.edited_values.get(key) {
            return Some(v.clone());
        }
        self.effective.last_value(key).map(|v| v.value.clone())
    }

    pub fn is_changed(&self, key: &str) -> bool {
        match self.edited_values.get(key) {
            Some(new_val) => self.effective.last_value(key).map(|v| &v.value) != Some(new_val),
            None => false,
        }
    }

    pub fn category_counts(&self) -> BTreeMap<String, usize> {
        let mut map = BTreeMap::new();
        if self.search_query.trim().is_empty() {
            for meta in &self.metadata {
                *map.entry(meta.category.clone()).or_insert(0) += 1;
            }
        } else {
            for idx in &self.search_results {
                if let Some(meta) = self.metadata.get(*idx) {
                    *map.entry(meta.category.clone()).or_insert(0) += 1;
                }
            }
        }
        map
    }
}

#[derive(Debug, Clone)]
pub enum DiffKind {
    Same,
    Add,
    Remove,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

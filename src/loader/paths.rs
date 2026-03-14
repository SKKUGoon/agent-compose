use std::path::{Path, PathBuf};

pub(super) fn resolve_schema_path(config_path: &Path, schema_file: &str) -> PathBuf {
    let schema_path = PathBuf::from(schema_file);
    if schema_path.is_absolute() {
        return schema_path;
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(schema_path)
}

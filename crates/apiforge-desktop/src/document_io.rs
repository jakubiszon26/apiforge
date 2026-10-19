//! Bounded file reads; validation happens off the UI thread.
use apiforge_core::{ImportLimits, ValidatedDocument, import_document};
use std::{fs::File, io::Read, path::Path};

pub fn read_document(path: &Path, limits: &ImportLimits) -> Result<ValidatedDocument, String> {
    let file = File::open(path)
        .map_err(|_| "Nie można otworzyć pliku. Sprawdź ścieżkę i uprawnienia.".to_string())?;
    let mut bytes = Vec::new();
    // Metadata alone is insufficient: the file can grow between stat() and read().
    file.take((limits.max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "Nie można odczytać całego pliku.".to_string())?;
    import_document(&bytes, limits).map_err(|error| error.to_string())
}

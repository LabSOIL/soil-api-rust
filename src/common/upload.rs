use base64::{Engine as _, engine::general_purpose};

pub fn decode_base64(value: &str) -> Result<(Vec<u8>, String), String> {
    // Helper function: decode base64 and return (raw_bytes, file_type)
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() < 2 {
        return Err("Invalid base64 format".into());
    }
    let meta = parts[0];
    let data_part = parts[1];
    let file_type = if meta.contains("text/csv") {
        "csv".to_string()
    } else if meta.contains("gpx") {
        "gpx".to_string()
    } else if meta.contains("text/plain") {
        "txt".to_string()
    } else {
        return Err("Only CSV, TXT, and GPX files are supported".into());
    };
    let decoded = general_purpose::STANDARD
        .decode(data_part)
        .map_err(|e| e.to_string())?;
    Ok((decoded, file_type))
}

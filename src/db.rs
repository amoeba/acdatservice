use acprotocol::dat::{DatDatabaseType, DatFileSubtype, DatFileType};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Deserialize, Serialize)]
pub struct File {
    pub id: i64,
    pub database_type: i64,
    pub file_type: i64,
    pub file_subtype: i64,
    pub file_offset: i64,
    pub file_size: i64,
}

impl File {
    pub fn resolved_file_type(&self) -> DatFileType {
        let file_type = DatFileType::from_object_id(self.id as u32);
        if file_type != DatFileType::Unknown {
            file_type
        } else {
            DatFileType::from_u32(self.file_type as u32).unwrap_or(DatFileType::Unknown)
        }
    }
}

/// Response struct with string representations for enum fields
#[derive(Serialize)]
pub struct FileResponse {
    pub id: i64,
    pub database_type: String,
    pub file_type: String,
    pub file_subtype: String,
    pub file_offset: i64,
    pub file_size: i64,
}

impl From<&File> for FileResponse {
    fn from(file: &File) -> Self {
        FileResponse {
            id: file.id,
            database_type: DatDatabaseType::from_u32(file.database_type as u32)
                .map(|v| v.to_string())
                .unwrap_or_else(|| format!("Unknown({})", file.database_type)),
            file_type: file.resolved_file_type().to_string(),
            file_subtype: DatFileSubtype::from_u32(file.file_subtype as u32)
                .map(|v| v.to_string())
                .unwrap_or_else(|| format!("Unknown({})", file.file_subtype)),
            file_offset: file.file_offset,
            file_size: file.file_size,
        }
    }
}

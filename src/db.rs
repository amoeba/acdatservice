use serde::Deserialize;

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct File {
    pub id: i64,
    pub database_type: i64,
    pub file_type: i64,
    pub file_subtype: i64,
    pub file_offset: i64,
    pub file_size: i64,
}

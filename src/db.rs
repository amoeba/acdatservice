use serde::Deserialize;

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct File {
    pub id: u64,
    pub id_short: u64,
    pub offset: u64,
    pub size: u32,
    pub dat_type: u32,
}

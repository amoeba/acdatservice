#![cfg(feature = "index")]

use libac_rs::dat::{
    enums::dat_file_type::{DatFileSubtype, DatFileType},
    file_types::{dat_file::DatFile, texture::Texture},
    reader::{
        file_reader::FileRangeReader, sync_dat_file_reader::SyncDatFileReader,
        sync_file_reader::SyncFileRangeReader, types::dat_database::DatDatabase,
    },
};
use sqlite::{self, Connection};
use std::{
    env,
    fs::{self, File},
    io::Cursor,
    path::Path,
};
use strum::IntoEnumIterator;

fn setup() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("./data")?;

    Ok(())
}

fn migrate(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    connection.execute("DROP TABLE IF EXISTS file_types;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS file_types (
            id INTEGER NOT NULL,
            name TEXT NOT NULL
        )",
    )?;

    connection.execute("DROP TABLE IF EXISTS file_subtypes;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS file_subtypes (
            id INTEGER NOT NULL,
            file_type_id INTEGER,
            name TEXT NOT NULL
        )",
    )?;

    connection.execute("DROP TABLE IF EXISTS files;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER NOT NULL,
            type INTEGER NOT NULL,
            subtype INTEGER,
            offset INTEGER NOT NULL,
            extra_info JSON
        )",
    )?;

    Ok(())
}

fn seed(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    // file_types
    for file_type in DatFileType::iter() {
        let mut statement = connection.prepare("INSERT INTO file_types VALUES(?, ?);")?;
        statement.bind((1, file_type.as_u32() as i64))?;
        statement.bind((2, file_type.to_string().as_str()))?;
        statement.next()?; // Is this really how we execute a prepared statement?
    }

    // file_subtype
    // Handle subtypes manually for now until I come up with something fancier
    let mut statement = connection.prepare("INSERT INTO file_subtypes VALUES(?, ?, ?);")?;
    statement.bind((1, DatFileSubtype::Icon.as_u32() as i64))?;
    statement.bind((2, DatFileType::Texture.as_u32() as i64))?;
    statement.bind((3, DatFileType::Texture.to_string().as_str()))?;
    statement.next()?; // Is this really how we execute a prepared statement?

    Ok(())
}

fn show_data(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare("SELECT count(1) FROM files;")?;

    while let sqlite::State::Row = statement.next()? {
        let count: i64 = statement.read(0)?;
        println!("Count: {}", count);
    }

    Ok(())
}

fn create_index(connection: &Connection, dat_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut db_file = File::open(dat_path)?;
    let db: DatDatabase = DatDatabase::read(&mut db_file)?;
    let mut db_file_reader = SyncFileRangeReader::new(db_file);

    let files = db.list_files(true)?;

    for file in files {
        println!("Processing file: {:?}", file);

        let dat_file_type = file.file_type();

        let mut statement = connection
            .prepare("INSERT INTO files (id, type, subtype, offset) VALUES (?, ?, ?, ?)")?;

        statement.bind((1, file.object_id as i64))?;
        statement.bind((2, dat_file_type.as_u32() as i64))?;

        // Read the entire file so we can find out its subtype, if anye
        let mut reader =
            SyncDatFileReader::new(file.file_size as usize, db.header.block_size as usize)?;
        let buf = reader.read_file(&mut db_file_reader, file.file_offset)?;
        let mut buf_reader = Cursor::new(buf);

        match file.file_type() {
            DatFileType::Texture => {
                let outer_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
                let icon = outer_file.inner;
                if icon.width == 32 && icon.height == 32 {
                    statement.bind((3, DatFileSubtype::Icon.as_u32() as i64))?;
                } else {
                    statement.bind((3, DatFileSubtype::Unknown.as_u32() as i64))?;
                }
            }
            DatFileType::Unknown => {
                statement.bind((3, DatFileSubtype::Unknown.as_u32() as i64))?;
            }
        }

        statement.bind((4, file.file_offset as i64))?;

        statement.next()?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return Err(Box::from("Must specify path to dat file to index."));
    }

    let dat_path = &args[1];

    if !Path::new(dat_path).exists() {
        return Err(Box::from(format!(
            "Provided dat file path doesn't exist: {}",
            dat_path
        )));
    }

    let db_path = "./data/index.sqlite";
    let connection = sqlite::open(db_path)?;

    setup()?;
    migrate(&connection)?;
    seed(&connection)?;
    create_index(&connection, dat_path)?;
    show_data(&connection)?;

    Ok(())
}

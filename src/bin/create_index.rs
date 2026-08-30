#![cfg(feature = "index")]

use acdatservice::DatDatabaseType;
use acprotocol::dat::{
    file_types::{dat_file::DatFile, texture::Texture},
    reader::{
        sync_dat_file_reader::SyncDatFileReader, sync_file_reader::SyncFileRangeReader,
        types::dat_database::DatDatabase,
    },
    DatFileSubtype, DatFileType,
};
use sha2::{Digest, Sha256};
use sqlite::{self, Connection};
use std::{
    env,
    fs::{self, File},
    io::{Cursor, Read},
    path::Path,
};
use strum::IntoEnumIterator;
type FileType = DatFileType;

fn setup() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("./data")?;

    Ok(())
}

fn migrate(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    connection.execute("DROP TABLE IF EXISTS database_types;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS database_types (
            id INTEGER NOT NULL,
            name TEXT NOT NULL
        )",
    )?;

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
            database_type INTEGER NOT NULL,
            file_type INTEGER NOT NULL,
            file_subtype INTEGER,
            file_offset INTEGER NOT NULL,
            file_size INTEGER NOT NULL,
            extra_info JSON
        )",
    )?;

    connection.execute("DROP TABLE IF EXISTS dats;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS dats (
            database_type INTEGER NOT NULL PRIMARY KEY,
            object_key TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            sha256 TEXT NOT NULL
        )",
    )?;

    Ok(())
}

fn seed(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    // database_types
    for db_type in DatDatabaseType::ALL {
        let mut statement = connection.prepare("INSERT INTO database_types VALUES(?, ?);")?;
        statement.bind((1, db_type.as_u32() as i64))?;
        statement.bind((2, db_type.name()))?;
        statement.next()?; // Is this really how we execute a prepared statement?
    }

    // file_types
    for file_type in DatFileType::iter() {
        let mut statement = connection.prepare("INSERT INTO file_types VALUES(?, ?);")?;
        let ft: FileType = file_type;
        statement.bind((1, ft.as_u32() as i64))?;
        statement.bind((2, ft.to_string().as_str()))?;
        statement.next()?; // Is this really how we execute a prepared statement?
    }

    // file_subtype
    // Handle subtypes manually for now until I come up with something fancier
    let mut statement = connection.prepare("INSERT INTO file_subtypes VALUES(?, ?, ?);")?;
    statement.bind((1, DatFileSubtype::Icon.as_u32() as i64))?;
    statement.bind((2, DatFileType::Texture.as_u32() as i64))?;
    statement.bind((3, DatFileSubtype::Icon.to_string().as_str()))?;
    statement.next()?; // Is this really how we execute a prepared statement?

    Ok(())
}

fn show_data(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare("SELECT count(1) FROM files;")?;

    while let sqlite::State::Row = statement.next()? {
        let count: i64 = statement.read(0)?;
        println!("Count: {}", count);
    }

    let mut statement =
        connection.prepare("SELECT database_type, object_key, size_bytes, sha256 FROM dats;")?;

    while let sqlite::State::Row = statement.next()? {
        let database_type: i64 = statement.read(0)?;
        let object_key: String = statement.read(1)?;
        let size_bytes: i64 = statement.read(2)?;
        let sha256: String = statement.read(3)?;
        println!(
            "DAT: database_type={}, object_key={}, size_bytes={}, sha256={}",
            database_type, object_key, size_bytes, sha256
        );
    }

    Ok(())
}

fn dat_type_from_path(dat_path: &str) -> DatDatabaseType {
    let file_name = Path::new(dat_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(dat_path);
    acdatservice::parse_dat_param(file_name)
        .map(|(database_type, _)| database_type)
        .unwrap_or_else(|error| panic!("Unsupported DAT filename {}: {}", file_name, error))
}

fn object_key_from_path(dat_path: &str) -> String {
    Path::new(dat_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(dat_path)
        .to_string()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

fn sha256_file(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(bytes_to_hex(&result))
}

fn record_dat_metadata(
    connection: &Connection,
    dat_path: &str,
    database_type: DatDatabaseType,
) -> Result<(), Box<dyn std::error::Error>> {
    let object_key = object_key_from_path(dat_path);
    let size_bytes = fs::metadata(dat_path)?.len();
    let sha256 = sha256_file(dat_path)?;

    let mut statement = connection.prepare(
        "INSERT OR REPLACE INTO dats (database_type, object_key, size_bytes, sha256) VALUES (?, ?, ?, ?)",
    )?;
    statement.bind((1, database_type.as_u32() as i64))?;
    statement.bind((2, object_key.as_str()))?;
    statement.bind((3, size_bytes as i64))?;
    statement.bind((4, sha256.as_str()))?;
    statement.next()?;

    println!(
        "Recorded metadata for {}: size={} bytes, sha256={}",
        object_key, size_bytes, sha256
    );

    Ok(())
}

fn create_index(
    connection: &Connection,
    dat_path: &str,
    database_type: DatDatabaseType,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db_file = File::open(dat_path)?;
    let db: DatDatabase = DatDatabase::read(&mut db_file)?;
    let mut db_file_reader = SyncFileRangeReader::new(db_file);

    let files = db.list_files(true)?;

    for file in files {
        println!("Processing file: {:?}", file);

        let dat_file_type = DatFileType::from_object_id(file.object_id);

        let mut statement = connection.prepare(
            "INSERT INTO files (id, database_type, file_type, file_subtype, file_offset, file_size) VALUES (?, ?, ?, ?, ?, ?)",
        )?;

        statement.bind((1, file.object_id as i64))?;
        statement.bind((2, database_type.as_u32() as i64))?;
        statement.bind((3, dat_file_type.as_u32() as i64))?;

        // Read the entire file so we can find out its subtype, if anye
        let subtype_col_index = 4;
        let mut reader =
            SyncDatFileReader::new(file.file_size as usize, db.header.block_size as usize)?;
        let buf = reader.read_file(&mut db_file_reader, file.file_offset)?;
        let mut buf_reader = Cursor::new(buf);

        match dat_file_type {
            DatFileType::Texture => {
                // Some textures (especially in cell DATs) use unsupported pixel formats.
                // If we can't parse it, treat it as a non-icon texture rather than
                // failing the entire index.
                match DatFile::<Texture>::read(&mut buf_reader) {
                    Ok(outer_file) => {
                        let icon = outer_file.inner;
                        if icon.width == 32 && icon.height == 32 {
                            statement
                                .bind((subtype_col_index, DatFileSubtype::Icon.as_u32() as i64))?;
                        } else {
                            statement
                                .bind((subtype_col_index, DatFileSubtype::None.as_u32() as i64))?;
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Warning: failed to parse texture {} (0x{:X}): {}. Treating as non-icon.",
                            file.object_id, file.object_id, err
                        );
                        statement
                            .bind((subtype_col_index, DatFileSubtype::None.as_u32() as i64))?;
                    }
                }
            }
            _ => {
                statement.bind((subtype_col_index, DatFileSubtype::None.as_u32() as i64))?;
            }
        }

        statement.bind((5, file.file_offset as i64))?;
        statement.bind((6, file.file_size as i64))?;
        statement.next()?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(Box::from(
            "Must specify at least one path to a dat file to index.",
        ));
    }

    let dat_paths = &args[1..];

    for dat_path in dat_paths {
        if !Path::new(dat_path).exists() {
            return Err(Box::from(format!(
                "Provided dat file path doesn't exist: {}",
                dat_path
            )));
        }
    }

    let db_path = "./data/index.sqlite";
    let connection = sqlite::open(db_path)?;

    setup()?;
    migrate(&connection)?;
    seed(&connection)?;

    for dat_path in dat_paths {
        let database_type = dat_type_from_path(dat_path);
        record_dat_metadata(&connection, dat_path, database_type.clone())?;
        create_index(&connection, dat_path, database_type)?;
    }

    show_data(&connection)?;

    Ok(())
}

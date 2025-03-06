const fs = require('fs');

const DAT_HEADER_OFFSET = 0x140;

// internal static readonly uint ObjectSize = ((sizeof(uint) * 0x3E) + sizeof(uint) + (DatFile.ObjectSize * 0x3D));
const DAT_DIRECTORY_HEADER_OBJECT_SIZE = 0x35A0; // 32 * 62 + 32 + 192 * 61 == 11728 == 0x35A0
// internal static readonly uint ObjectSize = (sizeof(uint) * 6);
const DAT_FILE_OBJECT_SIZE = 0xC0; // 32 * 6 == 192 == 0xC0


class SeekableFileReader {
  filePath: string;
  fd: any;
  position: number

  constructor(filePath: string) {
    this.filePath = filePath;
    this.fd = null;
    this.position = 0;
  }

  open() {
    if (this.fd === null) {
      this.fd = fs.openSync(this.filePath, 'r');
    }

    return this;
  }

  seek(position: number) {
    if (this.fd === null) {
      this.open();
    }
    this.position = position;
    return this;
  }

  read(count: number) {
    if (this.fd === null) {
      this.open();
    }

    const buffer = Buffer.alloc(count);
    const bytesRead = fs.readSync(this.fd, buffer, 0, count, this.position);

    this.position += bytesRead;

    // Return only the bytes that were actually read
    return buffer.subarray(0, bytesRead);
  }

  close() {
    if (this.fd !== null) {
      fs.closeSync(this.fd);
      this.fd = null;
    }
  }

  ReadInt8(): number {
    const buf = this.read(1);
    const view = new DataView(buf.buffer);

    return view.getInt8(0);
  }

  ReadInt16(): number {
    const buf = this.read(2);
    const view = new DataView(buf.buffer);

    return view.getInt16(0, true);
  }

  ReadInt32(): number {
    const buf = this.read(4);
    const view = new DataView(buf.buffer);

    return view.getInt32(0, true);
  }

  ReadUInt8(): number {
    const buf = this.read(1);
    const view = new DataView(buf.buffer);

    return view.getUint8(0);
  }

  ReadUInt16(): number {
    const buf = this.read(2);
    const view = new DataView(buf.buffer);

    return view.getUint16(0, true);
  }

  ReadUInt32(): number {
    const buf = this.read(4);
    const view = new DataView(buf.buffer);

    return view.getUint32(0, true);
  }

  ReadUint8Array(count: number): Uint8Array {
    const buf = this.read(count);
    const out = new Uint8Array(buf);

    return out;
  }

  ReadUint16Array(count: number): Uint16Array {
    const buf = this.read(count * 2);
    const out = new Uint16Array(buf);

    return out;
  }

  ReadUint32Array(count: number): Uint32Array {
    const buf = this.read(count * 4);
    const out = new Uint32Array(buf);

    return out;
  }

  ReadString(): string {
    return "";
  }
}



class DatDatabaseHeader {
  reader: SeekableFileReader

  FileType: number | undefined;
  BlockSize: number | undefined;
  FileSize: number | undefined;
  DataSet: any | undefined;
  DataSubset: number | undefined;
  FreeHead: number | undefined;
  FreeTail: number | undefined;
  FreeCount: number | undefined;
  BTree: number | undefined;
  NewLRU: number | undefined;
  OldLRU: number | undefined;
  UseLRU: boolean | undefined;
  MasterMapID: number | undefined;
  EnginePackVersion: number | undefined;
  GamePackVersion: number | undefined;
  VersionMajor: Uint8Array | undefined;
  VersionMinor: number | undefined;

  constructor(reader: SeekableFileReader) {
    this.reader = reader;
  }

  read(reader: SeekableFileReader) {
    // TODO: Figure out why the other impl skips this data
    // ACE has...
    //   private static readonly uint DAT_HEADER_OFFSET = 0x140; => 320
    reader.seek(DAT_HEADER_OFFSET);

    this.FileType = reader.ReadUInt32();
    this.BlockSize = reader.ReadUInt32();
    this.FileSize = reader.ReadUInt32();

    this.DataSet = reader.ReadUInt32();
    this.DataSubset = reader.ReadUInt32();

    this.FreeHead = reader.ReadUInt32();
    this.FreeTail = reader.ReadUInt32();
    this.FreeCount = reader.ReadUInt32();
    this.BTree = reader.ReadUInt32();

    this.NewLRU = reader.ReadUInt32();
    this.OldLRU = reader.ReadUInt32();
    this.UseLRU = (reader.ReadUInt32() == 1);

    this.MasterMapID = reader.ReadUInt32();

    this.EnginePackVersion = reader.ReadUInt32();
    this.GamePackVersion = reader.ReadUInt32();
    this.VersionMajor = reader.ReadUint8Array(16);
    this.VersionMinor = reader.ReadUInt32();
  }

  debug() {
    console.log("DatFileHeader: " + JSON.stringify(this));
  }
}

class DatDirectoryHeader {
  reader: SeekableFileReader

  constructor(reader: SeekableFileReader) {
    this.reader = reader;
  }

  read(offset: number, objectSize: number, blockSize: number) {
    // TODO
  }
}

class DatDirectory {
  reader: SeekableFileReader

  RootSectorOffset: number
  BlockSize: number

  constructor(reader: SeekableFileReader, RootSectorOffset: number, BlockSize: number) {
    this.reader = reader;
    this.RootSectorOffset = RootSectorOffset;
    this.BlockSize = BlockSize;
  }

  read() {
    const header = new DatDirectoryHeader(this.reader);
    // TODO: Implement this
    // header.read(this.reader);
  }
}

class DatDatabase {
  reader: SeekableFileReader
  header: DatDatabaseHeader | undefined

  constructor(path: string) {
    this.reader = new SeekableFileReader(path);
  }

  close() {
    this.reader.close();
  }

  read_header() {
    this.header = new DatDatabaseHeader(this.reader);
    this.header.read(this.reader);

    console.log(this.header.debug());
  }

  get_iteration() {
    // In each data file, the iteration file has ID '0xFFFF0001'
  }

  debug() {
    console.log("DatFile: " + JSON.stringify(this));
  }

}

const main = function () {
  const portal_path = "/Users/bryce/src/ACEmulator/ACE/Dats/client_portal.dat";
  const cell_path = "/Users/bryce/src/ACEmulator/ACE/Dats/client_cell_1.dat";

  if (!fs.existsSync(portal_path)) {
    console.log(`portal doesn't exist at ${portal_path}, exiting`);

    return;
  };

  if (!fs.existsSync(cell_path)) {
    console.log(`cell doesn't exist at ${cell_path}, exiting`);

    return;
  };

  const file = new DatDatabase(portal_path);
  file.read_header();

  // Debugging
  file.debug();
  file.header?.debug();

  file.close();
}

main();

const fs = require('fs');

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

    return view.getInt16(0);
  }

  ReadInt32(): number {
    const buf = this.read(4);
    const view = new DataView(buf.buffer);

    return view.getInt32(0);
  }

  ReadUInt8(): number {
    const buf = this.read(1);
    const view = new DataView(buf.buffer);

    return view.getUint8(0);
  }

  ReadUInt16(): number {
    const buf = this.read(2);
    const view = new DataView(buf.buffer);

    return view.getUint16(0);
  }

  ReadUInt32(): number {
    const buf = this.read(4);
    const view = new DataView(buf.buffer);

    return view.getUint32(0);
  }

  ReadUint8Array(count: number): Uint8Array {
    const buf = this.read(count);
    const out = new Uint8Array(buf);

    return out;
  }

  ReadString(): string {
    return "";
  }
}

class DatDatabaseHeader {
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

  constructor() {
    // TODO
  }

  read(reader: SeekableFileReader) {
    // TODO: Figure out why the other impl skips this data
    var offset = 256;
    offset += 64;
    reader.seek(offset);

    this.FileType = reader.ReadUInt32();
    this.BlockSize = reader.ReadUInt32();
    this.FileSize = reader.ReadUInt32();

    // TODO: Remember I can try casting this. For now, just read and store
    // directly
    //
    // this.DataSet = (DatDatabaseType)reader.ReadUInt32();
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

class DatFile {
  reader: SeekableFileReader
  header: DatDatabaseHeader | undefined

  constructor(path: string) {
    this.reader = new SeekableFileReader(path);
  }

  read_header() {
    this.header = new DatDatabaseHeader();
    // I probably can read the entire header into a buffer and pass it here
    this.header.read(this.reader);
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

  console.log("main");

  // let reader = fs.createReadStream(portal_path);
  const reader = new SeekableFileReader(portal_path);
  const file = new DatFile(portal_path);
  file.read_header();

  file.debug();
  file.header?.debug();

  // this.magic = reader.getUint32();
  // this.blockSize = reader.getUint32();

  reader.close();
}

main();

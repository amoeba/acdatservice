const fs = require('fs');

const DAT_HEADER_OFFSET = 0x140;

// internal static readonly uint ObjectSize = ((sizeof(uint) * 0x3E) + sizeof(uint) + (DatFile.ObjectSize * 0x3D));
const DAT_DIRECTORY_HEADER_OBJECT_SIZE = 0x6B4; // 4 * 62 + 4 + 24 * 61 == 1716
// internal static readonly uint ObjectSize = (sizeof(uint) * 6);
const DAT_FILE_OBJECT_SIZE = 0xC0; // 4 * 6 == 24 == 0xC0

class BinaryReader {
  buffer: ArrayBufferLike
  position: number

  constructor(buffer: ArrayBufferLike) {
    this.buffer = buffer;
    this.position = 0;
  }

  read(length: number): ArrayBufferLike {
    let view = new DataView(this.buffer, this.position, length);
    this.position += length;

    return view.buffer;
  }

  ReadInt8(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 1;
    return view.getInt8(0);
  }

  ReadInt16(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 2;
    return view.getInt16(0, true);
  }

  ReadInt32(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 4;

    return view.getInt32(0, true);
  }

  ReadUint8(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 1;
    return view.getUint8(0);
  }

  ReadUint16(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 2;
    return view.getUint16(0, true);
  }

  ReadUint32(): number {
    let view = new DataView(this.buffer, this.position);
    this.position += 4;

    return view.getUint32(0, true);
  }

  ReadUint8Array(length: number): Uint32Array {
    const buf = this.read(length);
    const out = new Uint32Array(buf);

    return out;
  }

  ReadUint16Array(length: number): Uint32Array {
    const buf = this.read(length * 2);
    const out = new Uint32Array(buf);

    return out;
  }

  ReadUint32Array(length: number): Uint32Array {
    const buf = this.read(length * 4);
    const out = new Uint32Array(buf);

    return out;
  }
}

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

  // TODO
  ReadString(): string {
    return "TODO";
  }
}

class DatDirectoryHeader {
  buffer: Uint8Array
  reader: BinaryReader

  branches: Uint32Array
  entryCount: number | undefined
  entries: DatFile[]

  constructor(buffer: Uint8Array) {
    this.buffer = buffer;
    this.reader = new BinaryReader(buffer.buffer);

    this.branches = new Uint32Array(62);
    this.entries = [];
  }

  unpack() {
    for (let i = 0; i < this.branches.length; i++) {
      this.branches[i] = this.reader.ReadUint32();
    }

    this.entryCount = this.reader.ReadUint32();

    for (let i = 0; i < this.entryCount; i++) {
      this.entries[i] = new DatFile();
      this.entries[i].unpack(this.reader);
    }
  }
}

class DatFile {
  BitFlags: number | undefined
  ObjectId: number | undefined
  FileOffset: number | undefined
  FileSize: number | undefined
  Date: number | undefined
  Iteration: number | undefined

  unpack(reader: BinaryReader) {
    this.BitFlags = reader.ReadUint32();
    this.ObjectId = reader.ReadUint32();
    this.FileOffset = reader.ReadUint32();
    this.FileSize = reader.ReadUint32();
    this.Date = reader.ReadUint32();
    this.Iteration = reader.ReadUint32();
  }
}

class DatReader {
  reader: SeekableFileReader
  buffer: Buffer | undefined

  constructor(reader: SeekableFileReader) {
    this.reader = reader;
  }

  read(offset: number, size: number, blockSize: number) {
    // init buffer
    let buffer = new Uint8Array(size);
    this.reader.position = offset;

    let nextAddress = this.getNextAdress();
    let bufferOffset = 0;

    while (size > 0) {
      if (size < blockSize) {
        buffer.set(this.reader.ReadUint8Array(size), bufferOffset);

        // We can quit looping now since we've read to the end
        break;
      } else {
        // stream.Read(buffer, bufferOffset, Convert.ToInt32(blockSize) - 4); // Read in our sector into the buffer[]
        buffer.set(this.reader.ReadUint8Array(blockSize - 4), bufferOffset);
        // bufferOffset += Convert.ToInt32(blockSize) - 4; // Adjust this so we know where in our buffer[] the next sector gets appended to
        bufferOffset += blockSize - 4;
        // stream.Seek(nextAddress, SeekOrigin.Begin); // Move the file pointer to the start of the next sector we read above.
        this.reader.position = nextAddress;
        // nextAddress = GetNextAddress(stream, 0); // Get the start location of the next sector.
        nextAddress = this.getNextAdress();
        // size -= (blockSize - 4); // Decrease this by the amount of data we just read into buffer[] so we know how much more to go
        size -= blockSize - 4;
      }
    }

    return buffer;
  }

  getNextAdress(): number {
    return this.reader.ReadUInt32();
  }
}

class DatDirectory {
  reader: SeekableFileReader

  RootSectorOffset: number
  BlockSize: number

  header: DatDirectoryHeader | undefined
  directories: DatDirectory[]

  constructor(reader: SeekableFileReader, offset: number, blockSize: number) {
    this.reader = reader;

    this.RootSectorOffset = offset;
    this.BlockSize = blockSize

    this.directories = []
  }

  read() {
    // Take care of header
    let reader = new DatReader(this.reader).read(this.RootSectorOffset, DAT_DIRECTORY_HEADER_OBJECT_SIZE, this.BlockSize);
    this.header = new DatDirectoryHeader(reader);
    this.header.unpack();

    if (!this.header) {
      return;
    }

    // Stop reading if this is a leaf directory
    if (this.isLeaf()) {
      console.log("[INFO] DatDirectory: isLeaf");
      return;
    }

    if (!this.header || !this.header.entryCount || !this.header.branches) {
      console.log("[WARN] early return, this shouldn't happen");

      return;
    }


    for (let i = 0; i < this.header.entryCount + 1; i++) {
      let dir = new DatDirectory(this.reader, this.header.branches[i], this.BlockSize)
      dir.read();

      this.directories.push(dir);
    }
  }

  // TODO: This is super gross right now
  isLeaf() {
    if (!this.header) {
      return false;
    }

    return !this.header.branches || this.header.branches[0] == 0
  }
}

class FileChunk {
  reader: SeekableFileReader

  flag: number
  id: number
  offset: number
  size: number
  time: number
  version: number

  constructor(reader: SeekableFileReader) {
    this.reader = reader;

    this.flag = 0;
    this.id = 0;
    this.offset = 0;
    this.size = 0;
    this.time = 0;
    this.version = 0;
  }

  read() {
    this.flag = this.reader.ReadUInt32();
    this.id = this.reader.ReadUInt32();
    this.offset = this.reader.ReadUInt32();
    this.size = this.reader.ReadUInt32();
    this.time = this.reader.ReadUInt32();
    this.version = this.reader.ReadUInt32();
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
}

class DatDatabase {
  reader: SeekableFileReader
  header: DatDatabaseHeader | undefined
  rootDir: DatDirectory | undefined

  constructor(path: string) {
    this.reader = new SeekableFileReader(path);
  }

  close() {
    this.reader.close();
  }

  read_header() {
    this.header = new DatDatabaseHeader(this.reader);
    this.header.read(this.reader);
  }

  read() {
    // TODO: Clean this up with better type checking
    if (!this.header || !this.header.BTree || !this.header.BlockSize) {
      console.log("[WARN] Header is null, not finding");

      return;
    }

    var position: number = this.header.BTree
    this.rootDir = new DatDirectory(this.reader, position, this.header.BlockSize)
    this.rootDir.read();
  }

  // TODO
  get_iteration() {
    // In each data file, the iteration file has ID '0xFFFF0001'
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
  file.read();
  file.close();
}

main();

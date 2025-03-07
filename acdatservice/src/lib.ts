import BinaryReader from "./binary_reader";
import SeekableFileReader from "./seekable_file_reader";

const fs = require('fs');

// Constants. TODO: Move elsewhere
const DAT_HEADER_OFFSET = 0x140;
const DAT_DIRECTORY_HEADER_OBJECT_SIZE = 0x6B4; // 4 * 62 + 4 + 24 * 61 == 1716
const DAT_FILE_OBJECT_SIZE = 0xC0; // 4 * 6 == 24 == 0xC0

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

export class DatDatabase {
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

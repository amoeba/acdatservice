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
}

class DatDatabaseHeader {
  constructor() {

  }

  read(reader: SeekableFileReader) {
    var offset = 256;
    // var view = new DataView(this.buffer, 0, 0x400);
    // this.transactions = new Uint8Array(this.buffer, offset, 64);
    offset += 64;

    reader.seek(offset);
    let last_read = reader.ReadUInt32();
    console.log(last_read);

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

  // this.magic = reader.getUint32();
  // this.blockSize = reader.getUint32();

  reader.close();
}

main();

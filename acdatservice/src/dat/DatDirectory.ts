import { DatDirectoryHeader } from "./DatDirectoryHeader"
import { DatFile } from "./DatFile"
import { DatReader } from "./DatReader"
import SeekableFileReader from "../seekable_file_reader"
import BinaryReader from "../binary_reader";

const DAT_DIRECTORY_HEADER_OBJECT_SIZE = 0x6B4; // 4 * 62 + 4 + 24 * 61 == 1716

export class DatDirectory {
  reader: SeekableFileReader

  RootSectorOffset: number
  BlockSize: number

  header: DatDirectoryHeader
  directories: DatDirectory[]

  constructor(reader: SeekableFileReader, offset: number, blockSize: number) {
    this.reader = reader;

    this.RootSectorOffset = offset;
    this.BlockSize = blockSize;

    this.header = new DatDirectoryHeader();
    this.directories = [];
  }

  read() {
    let dat_reader = new DatReader(this.reader).read(this.RootSectorOffset, DAT_DIRECTORY_HEADER_OBJECT_SIZE, this.BlockSize);
    let header_reader = new BinaryReader(dat_reader.buffer);
    this.header.unpack(header_reader);

    // Stop reading if this is a leaf directory
    if (this.isLeaf()) {
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

  files(dest: DatFile[]) {
    if (!this.header || !this.header.entryCount || !this.header.entries) {
      throw new Error("TODO");
    }

    this.directories.forEach(d => d.files(dest));

    for (let i = 0; i < this.header.entryCount; i++) {
      dest.push(this.header.entries[i]);
    }

    return dest;
  }
}

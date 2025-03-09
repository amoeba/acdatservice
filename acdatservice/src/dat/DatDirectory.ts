import { DatDirectoryHeader } from "./DatDirectoryHeader"
import { DatFile } from "./DatFile"
import { DatReader } from "./DatReader"
import SeekableFileReader from "../seekable_file_reader"

const DAT_DIRECTORY_HEADER_OBJECT_SIZE = 0x6B4; // 4 * 62 + 4 + 24 * 61 == 1716

export class DatDirectory {
  reader: SeekableFileReader

  RootSectorOffset: number
  BlockSize: number

  header: DatDirectoryHeader | undefined
  directories: DatDirectory[]

  entries: DatFile[]

  constructor(reader: SeekableFileReader, offset: number, blockSize: number) {
    this.reader = reader;

    this.RootSectorOffset = offset;
    this.BlockSize = blockSize;

    this.directories = [];

    // WIP
    this.entries = [];
  }

  read() {
    // Take care of header
    let reader = new DatReader(this.reader).read(this.RootSectorOffset, DAT_DIRECTORY_HEADER_OBJECT_SIZE, this.BlockSize);
    this.header = new DatDirectoryHeader(reader);
    this.header.unpack();

    if (!this.header) {
      return [];
    }

    // Stop reading if this is a leaf directory
    if (this.isLeaf()) {
      return [];
    }

    if (!this.header || !this.header.entryCount || !this.header.branches) {
      console.log("[WARN] early return, this shouldn't happen");

      return [];
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

  iter() {
    let queue = []
    let files = [];

    // Populate initial set
    queue.push(...this.directories);
    files.push(...this.entries);

    while (queue.length > 0) {
      let d = queue.pop();
      if (!d) { continue }
      files.push(...this.entries);
      queue.push(...d?.directories);
    }

    return files;
  }
}

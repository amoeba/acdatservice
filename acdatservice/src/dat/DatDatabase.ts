import { DatDatabaseHeader } from "./DatDatabaseHeader";
import { DatDirectory } from "./DatDirectory";
import SeekableFileReader from "../seekable_file_reader";

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
    this.read_header();

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

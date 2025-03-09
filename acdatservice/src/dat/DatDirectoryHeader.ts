import BinaryReader from "../binary_reader"
import { DatFile } from "./DatFile"

export class DatDirectoryHeader {
  reader: BinaryReader | undefined

  branches: Uint32Array
  entryCount: number | undefined
  entries: DatFile[] | undefined

  constructor() {
    this.branches = new Uint32Array(62);
  }

  unpack(reader: BinaryReader) {
    for (let i = 0; i < this.branches.length; i++) {
      this.branches[i] = reader.ReadUint32();
    }

    this.entryCount = reader.ReadUint32();

    this.entries = [];

    for (let i = 0; i < this.entryCount; i++) {
      this.entries[i] = new DatFile();
      this.entries[i].unpack(reader);
    }
  }
}

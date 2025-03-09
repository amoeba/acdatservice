import BinaryReader from "./binary_reader"
import { DatFile } from "./DatFile"

export class DatDirectoryHeader {
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

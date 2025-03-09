import BinaryReader from "./binary_reader"
import { DatFileType } from "./DatFileType"

export class DatFile {
  BitFlags: number | undefined
  ObjectId: number | undefined
  FileOffset: number | undefined
  FileSize: number | undefined
  Date: number | undefined
  Iteration: number | undefined

  unpack(reader: BinaryReader) {
    console.log("DatFile.unpack")
    this.BitFlags = reader.ReadUint32();
    this.ObjectId = reader.ReadUint32();
    this.FileOffset = reader.ReadUint32();
    this.FileSize = reader.ReadUint32();
    this.Date = reader.ReadUint32();
    this.Iteration = reader.ReadUint32();
  }

  type(): DatFileType {
    if (!this.ObjectId) {
      return DatFileType.Unknown
    }

    if (this.ObjectId >= 0x06000000 && this.ObjectId <= 0x07FFFFFF) {
      return DatFileType.Texture
    } else {
      return DatFileType.Unknown
    }
  }
}

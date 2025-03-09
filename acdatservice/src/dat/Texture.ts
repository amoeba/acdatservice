import SeekableFileReader from "../seekable_file_reader";

export class Texture {
  width: number | undefined
  height: number | undefined
  format: number | undefined
  length: number | undefined
  buffer: Buffer | undefined

  unpack(reader: SeekableFileReader) {
    // ID?
    reader.ReadUint32();

    // TODO: What is this?
    reader.ReadUint32();

    // Form? What is form?
    let form = reader.ReadUint32();

    if (form == 10 || form == 6) {
      this.width = reader.ReadUint32();
      this.height = reader.ReadUint32();
      this.format = reader.ReadUint32();
      this.length = reader.ReadUint32();
    } else {
      throw new Error("TODO")
    }

    this.buffer = Buffer.alloc(this.length);

    for (let i = 0; i < this.length; i++) {
      this.buffer[i] = reader.ReadUint8()
    }
  }
}

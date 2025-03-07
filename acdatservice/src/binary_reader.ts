export default class BinaryReader {
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

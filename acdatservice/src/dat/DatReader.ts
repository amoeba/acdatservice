import SeekableFileReader from "./seekable_file_reader";

export class DatReader {
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
    return this.reader.ReadUint32();
  }
}

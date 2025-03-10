import SeekableFileReader from "../seekable_file_reader";

const DAT_HEADER_OFFSET = 0x140;

export class DatDatabaseHeader {
  reader: SeekableFileReader

  FileType: number | undefined;
  BlockSize: number | undefined;
  FileSize: number | undefined;
  DataSet: any | undefined;
  DataSubset: number | undefined;
  FreeHead: number | undefined;
  FreeTail: number | undefined;
  FreeCount: number | undefined;
  BTree: number | undefined;
  NewLRU: number | undefined;
  OldLRU: number | undefined;
  UseLRU: boolean | undefined;
  MasterMapID: number | undefined;
  EnginePackVersion: number | undefined;
  GamePackVersion: number | undefined;
  VersionMajor: Uint8Array | undefined;
  VersionMinor: number | undefined;

  constructor(reader: SeekableFileReader) {
    this.reader = reader;
  }

  read(reader: SeekableFileReader) {
    // TODO: Figure out why the other impl skips this data
    // ACE has...
    //   private static readonly uint DAT_HEADER_OFFSET = 0x140; => 320
    reader.seek(DAT_HEADER_OFFSET);

    this.FileType = reader.ReadUint32();
    this.BlockSize = reader.ReadUint32();
    this.FileSize = reader.ReadUint32();

    this.DataSet = reader.ReadUint32();
    this.DataSubset = reader.ReadUint32();

    this.FreeHead = reader.ReadUint32();
    this.FreeTail = reader.ReadUint32();
    this.FreeCount = reader.ReadUint32();
    this.BTree = reader.ReadUint32();

    this.NewLRU = reader.ReadUint32();
    this.OldLRU = reader.ReadUint32();
    this.UseLRU = (reader.ReadUint32() == 1);

    this.MasterMapID = reader.ReadUint32();

    this.EnginePackVersion = reader.ReadUint32();
    this.GamePackVersion = reader.ReadUint32();
    this.VersionMajor = reader.ReadUint8Array(16);
    this.VersionMinor = reader.ReadUint32();
  }
}

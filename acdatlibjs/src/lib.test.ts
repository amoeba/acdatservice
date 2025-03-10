
import BinaryReader from './binary_reader';
import SeekableFileReader from './seekable_file_reader';

const rdr = function (values: number[]): BinaryReader {
  const buf = Buffer.from(values);
  const array = new Uint8Array(buf);

  return new BinaryReader(array.buffer)
}

test('BinaryReader smoke test', () => {
  expect(rdr([0x01, 0x00, 0x00, 0x00]).ReadUint32()).toBe(1);
  expect(rdr([0xFF, 0x00, 0x00, 0x00]).ReadUint32()).toBe(255);
  expect(rdr([0x03, 0x02, 0x01, 0x00]).ReadUint32()).toBe(66051);
});

test('SeekableFileReader smoke test', () => {
  // TODO

  // expect(2).toBe(4);
});

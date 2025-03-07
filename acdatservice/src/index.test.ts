
import BinaryReader from '../src/binary_reader';
import SeekableFileReader from '../src/seekable_file_reader';

test('BinaryReader smoke test', () => {
  // WIP
  let input = new Uint8Array(4);

  input[0] = 0x00;
  input[1] = 0x01;
  input[2] = 0x02;
  input[3] = 0x03;

  let reader = new BinaryReader(input.buffer);
  let result = reader.ReadUint32();

  expect(result).toBe(4);
});


test('SeekableFileReader smoke test', () => {
  // TODO

  expect(2).toBe(4);
});

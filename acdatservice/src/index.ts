const fs = require('fs');

import BinaryReader from "./binary_reader";
import { DatDatabase } from "./lib";
import SeekableFileReader from "./seekable_file_reader";
import sharp from "sharp";

const main = function () {
  const portal_path = "/Users/bryce/src/ACEmulator/ACE/Dats/client_portal.dat";
  const cell_path = "/Users/bryce/src/ACEmulator/ACE/Dats/client_cell_1.dat";

  if (!fs.existsSync(portal_path)) {
    console.log(`portal doesn't exist at ${portal_path}, exiting`);

    return;
  };

  if (!fs.existsSync(cell_path)) {
    console.log(`cell doesn't exist at ${cell_path}, exiting`);

    return;
  };

  const file = new DatDatabase(portal_path);

  file.read_header();
  file.read();

  let files = file.rootDir?.iter();

  if (files) {
    for (let i = 0; i < files.length; i++) {
      let file = files[i];

      if (file.ObjectId && (file.ObjectId & 0x7F000000) == 0x6000000 && file.FileSize == 4120) {
        console.log("yes", { file: file });
        let r = new SeekableFileReader(portal_path, file.FileOffset);

        r.ReadUint32();
        r.ReadUint32();

        let form = r.ReadUint32();
        console.log(form);
        if (form == 10 || form == 6) {
          let width = r.ReadUint32();
          let height = r.ReadUint32();
          let format = r.ReadUint32();
          let bufLen = r.ReadUint32();

          console.log({ width, height, format, bufLen });
          let buf = Buffer.alloc(bufLen);

          for (let i = 0; i < bufLen; i++) {
            buf[i] = r.ReadUint8()
          }

          console.log(buf)
          sharp(buf, {
            raw: {
              width: width,
              height: height,
              channels: 4
            }
          }).png().toFile("out.png");

          console.log("ten");
          break;
        }

        // ByteCursor csr = library.LoadFile(file);
        // int nForm = csr.GetDWORD(true);
        // if (nForm == 10 || nForm == 6) {
        //   images[i] = (nForm == 10) ? csr.GetRGBImage(true) : csr.Get24BitImage(true);
        //   imgIndex = iconList.Images.Count;
        //   iconList.Images.Add(images[i]);
        // }

        break;
      }
    }
  }

  file.close();
}

main();

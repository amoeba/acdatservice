const fs = require('fs');

import sharp from "sharp";
import { DatDatabase } from "./dat/DatDatabase";
import { Texture } from "./dat/Texture";
import SeekableFileReader from "./seekable_file_reader";
import { DatFileType } from "./dat/DatFileType";
import { DatFile } from "./dat/DatFile";

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

  const db = new DatDatabase(portal_path);
  db.read();
  let result: DatFile[] = [];
  db.rootDir?.files(result);
  console.log({ res: result.length });
  // let files = file.rootDir?.iter();

  // if (files) {
  //   for (let i = 0; i < files.length; i++) {
  //     let file = files[i];

  //     if (file.type() == DatFileType.Texture) {
  //       let file_reader = new SeekableFileReader(portal_path, file.FileOffset);
  //       let icon = new Texture();
  //       icon.unpack(file_reader);

  //       // WIP: Export
  //       sharp(icon.buffer, {
  //         raw: {
  //           width: icon.width || 0,
  //           height: icon.height || 0,
  //           channels: 4
  //         }
  //       }).png().toFile("latest.png");

  //       break;

  //     }
  //   }
  // }

  db.close();
}

main();

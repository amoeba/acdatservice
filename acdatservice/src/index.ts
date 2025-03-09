import fs from 'fs';
import path from 'path';

import sharp from "sharp";
import { DatDatabase } from "./dat/DatDatabase";
import { Texture } from "./dat/Texture";
import SeekableFileReader from "./seekable_file_reader";
import { DatFileType } from "./dat/DatFileType";
import { DatFile } from "./dat/DatFile";

const exportIcons = function (portal_path: string, files: DatFile[], path: string) {
  if (!fs.existsSync(`./${path}`)) {
    fs.mkdirSync(`./${path}`);
  }

  for (let i = 0; i < files.length; i++) {
    console.log(i);
    let file = files[i];

    if (file.type() != DatFileType.Texture) {
      continue;
    }

    let file_reader = new SeekableFileReader(portal_path, file.FileOffset);
    let icon = new Texture();
    icon.unpack(file_reader);

    sharp(icon.buffer, {
      raw: {
        width: icon.width || 0,
        height: icon.height || 0,
        channels: 4
      }
    }).png().toFile(`./${path}/${i}.png`);
  }
}

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
  let files: DatFile[] = [];
  db.rootDir?.files(files);
  db.close();

  exportIcons(portal_path, files, "export2");
}

main();

const fs = require('fs');

import { DatDatabase } from "./DatDatabase";

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
  console.log({ l: files?.length });
  // if (files) {
  //   for (let i = 0; i < files.length; i++) {
  //     let file = files[i];

  //     if (file.ObjectId && (file.ObjectId & 0x7F000000) == 0x6000000 && file.FileSize == 4120) {
  //       console.log("yes", { file: file });

  //       let file_reader = new SeekableFileReader(portal_path, file.FileOffset);
  //       let icon = new PortalIcon();
  //       icon.unpack(file_reader);

  //       // WIP: Export
  //       sharp(icon.buffer, {
  //         raw: {
  //           width: icon.width || 0,
  //           height: icon.height || 0,
  //           channels: 4
  //         }
  //       }).png().toFile("latest.png");
  //     }
  //   }
  // }

  file.close();
}

main();

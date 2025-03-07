const fs = require('fs');

import { DatDatabase } from "./lib";

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
  file.close();
}

main();

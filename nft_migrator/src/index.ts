'use strict';

import express from 'express';
import 'dotenv/config';
import https from 'https';
import fs from 'fs';
import toobusy from 'toobusy-js';
import { Address } from './terra_utils';
import {
  LCDClient,
} from '@terra-money/terra.js';

const PORT = 8081;
const HTTPS_PORT = 8444;

// We start the server
const app = express();

app.listen(PORT, () => {
  console.log("Serveur à l'écoute");
});
// Allow any to access this API.
app.use(function (_req: any, res: any, next: any) {
  res.header('Access-Control-Allow-Origin', '*');
  res.header(
    'Access-Control-Allow-Headers',
    'Origin, X-Requested-With, Content-Type, Accept'
  );
  next();
});

app.use(function (_req, res, next) {
  if (toobusy()) {
    res.status(503).send("I'm busy right now, sorry.");
  } else {
    next();
  }
});

async function main() {

  app.get('/migrator/contract_list', async (_req: any, res: any) => {
      let contractList = require('../nft_contracts.json');
      await res.status(200).send(contractList);
  });

  if (process.env.EXECUTION == 'PRODUCTION') {
    const options = {
      cert: fs.readFileSync('/home/illiquidly/identity/fullchain.pem'),
      key: fs.readFileSync('/home/illiquidly/identity/privkey.pem')
    };
    https.createServer(options, app).listen(HTTPS_PORT);
  }
}
main();

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


let global_env = require("../env.json");

const PORT = 8080;
const querier = new Address("");

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


async function try_send_token_2_0(contract_info: any, user_address: string, token_id: string){



}



async function main() {

  app.get('/migrator/contract_list', async (_req: any, res: any) => {
      let contract_list = require('../nft_contracts.json');
      await res.status(200).send(contract_list);
  });


  app.get('/migrator/migrate/:address/:contract/:token_id', async (req: any, res: any) => {
      const address = req.params.address;
      const contract = req.params.contract;
      const token_id = req.params.token_id;

      // We verify the contract is registered with the api
      let contract_list = require('../nft_contracts.json');
      let contract_info = contract_list[contract];
      if(!contract_info){
        await res.status(404).send("Contract was not registered with this api");
        return;
      }

      // We query the Terra 1.0 chain to make sure the designated NFT has been deposited by the address in the escrow contract
      let terra = new LCDClient(global_env["classic"]['chain']);
      let response: any = await terra.wasm.contractQuery(
        contract_info.escrow_contract,
        {
          depositor:{
            token_id
          }
        }
      ).then(async (response: any)=>{
        // If there is a response, we check it matches the info sent
        if(response?.token_id != token_id){
          await res.status(404).send("Token not deposited");
          return

        }else if(response.depositor != address){
          await res.status(404).send("Token not deposited by the indicated address");
          return
        }
        console.log(response);
        await res.status(200).send("This address indeed deposited this token_id");
        // We try to send the token to the depositor on the Terra 2.0 chain
        try_send_token_2_0(contract_info, address, token_id);

      }).catch(async (error) =>{
        console.log(error)
          await res.status(404).send({
            error_text:"Token not deposited", 
            error: error.message
          });
      });
      

  });

  if (process.env.EXECUTION == 'PRODUCTION') {
    const options = {
      cert: fs.readFileSync('/home/illiquidly/identity/fullchain.pem'),
      key: fs.readFileSync('/home/illiquidly/identity/privkey.pem')
    };
    https.createServer(options, app).listen(8443);
  }
}
main();

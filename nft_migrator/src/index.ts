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

let env = require("../env.json")
let global_env = require("../env.json");

const PORT = 8080;

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

let mnemonics = require("../mnemonics.json")

let escrow_handler = new Address(env["classic"],mnemonics.escrow.mnemonic);


async function try_send_token_2_0(contract_info: any, user_address: string, token_id: string){
  // We try to send the token.
  let nft_address_2 = contract_info.contract2;
  let nft_address_1 = contract_info.contract1;
  let terra2Mnemonic = mnemonics[nft_address_1].mnemonic;
  let nft_handler = new Address(env["staging"],terra2Mnemonic);
  let nft_2_contract = nft_handler.getContract(nft_address_2);
  let escrow = escrow_handler.getContract(contract_info.escrow_contract)
  let migrated = false;

  console.log(nft_address_2, {
    recipient: user_address,
    token_id,
  })
  
  // We execute the transfer on Terra 2.0
  await nft_2_contract.execute.transfer_nft({
    recipient: user_address,
    token_id,
  })
  .then(async (_response: any)=>{
    // We indicate the token has been migrated
    await escrow.execute.migrated({
      token_id
    })
    migrated = true;
    
  })
  .catch((error: any) => {
    console.log("Error when transfering to Terra 2.0 or migrating on Terra 1.0", error)
  })
  
  return migrated
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
        }else if(response.migrated){
          await res.status(404).send("Token already migrated");
          return
        }
        // We try to send the token to the depositor on the Terra 2.0 chain
        if(await try_send_token_2_0(contract_info, address, token_id)){
          await res.status(200).send("Your Token was migrated successfuly");
        }else{
          await res.status(503).send("Your token couldn't be migrated for now, it is locked in the escrow contract waiting to be migrated, try again later");
        }

      }).catch(async (error) =>{
        console.log(error)
          await res.status(404).send({
            error_text:"Error occured while migrating the token. Please report the error to the project owner", 
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

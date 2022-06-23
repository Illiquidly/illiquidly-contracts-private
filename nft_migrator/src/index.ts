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

let globalEnv = require("../env.json");

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

let mnemonics = require("../mnemonics.json")

let escrowHandler = new Address(globalEnv["classic"],mnemonics.escrow.mnemonic);


async function trySendToken2_0(contractInfo: any, userAddress: string, tokenId: string){
  // We try to send the token.
  let nftAddress2 = contractInfo.contract2;
  let nftAddress1 = contractInfo.contract1;
  let terra2Mnemonic = mnemonics[nftAddress1].mnemonic;
  let nftHandler = new Address(globalEnv["staging"],terra2Mnemonic);
  let nft2Contract = nftHandler.getContract(nftAddress2);
  let escrow = escrowHandler.getContract(contractInfo.escrow_contract)
  let migrated = false;

  console.log(nftAddress2, {
    recipient: userAddress,
    token_id: tokenId,
  })
  
  // We execute the transfer on Terra 2.0
  await nft2Contract.execute.transfer_nft({
    recipient: userAddress,
    token_id: tokenId
  })
  .then(async (_response: any)=>{
    // We indicate the token has been migrated
    await escrow.execute.migrated({
      token_id: tokenId
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
      let contractList = require('../nft_contracts.json');
      await res.status(200).send(contractList);
  });


  app.get('/migrator/migrate/:address/:contract/:tokenId', async (req: any, res: any) => {
      const address = req.params.address;
      const contract = req.params.contract;
      const tokenId = req.params.tokenId;

      // We verify the contract is registered with the api
      let contractList = require('../nft_contracts.json');
      let contractInfo = contractList[contract];
      if(!contractInfo){
        await res.status(404).send("Contract was not registered with this api");
        return;
      }

      // We query the Terra 1.0 chain to make sure the designated NFT has been deposited by the address in the escrow contract
      let terra = new LCDClient(globalEnv["classic"]['chain']);
      await terra.wasm.contractQuery(
        contractInfo.escrow_contract,
        {
          depositor:{
            token_id: tokenId
          }
        }
      ).then((response: any)=>{
        // If there is a response, we check it matches the info sent
        if(response?.token_id != tokenId){
          throw Error("Token not deposited");
        }else if(response.depositor != address){
          throw Error("Token not deposited by the indicated user");
        }else if(response.migrated){
          throw Error("Token already migrated");
        }
        // We try to send the token to the depositor on the Terra 2.0 chain
        return trySendToken2_0(contractInfo, address, tokenId);
      }).then((migrated)=>{
        if(migrated){
          return res.status(200).send("Your Token was migrated successfuly");
        }else{
          return res.status(503).send("Your token couldn't be migrated for now, it is locked in the escrow contract waiting to be migrated, try again later");
        }
      })
      .catch((error) =>{
          return res.status(404).send({
            error_text:"Error occured while migrating the token", 
            error: error.message
          });
      });
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

const IPFS = require('ipfs');
const OrbitDB = require('orbit-db');
import { getBlockHeight, getNewDatabaseInfo, parseNFTSet, chains } from './index.js';
const express = require('express');

interface NFTsInteracted {
  lastBlock: number;
  interacted_nfts: string[];
  owned_nfts: any;
}

const app = express();

app.listen(8080, () => {
  console.log("Serveur à l'écoute");
});

function default_api_structure(): NFTsInteracted {
  return {
    lastBlock: 0,
    interacted_nfts: [],
    owned_nfts: {}
  };
}

async function updateAddress(
  db: any,
  network:string,
  address: string,
  currentData: NFTsInteracted | undefined = undefined,
  lastBlock: number | undefined = undefined
) {
  let blockHeight = await getBlockHeight(network);

  if (!currentData) {
    let currentData: NFTsInteracted = await db.get(to_key(network,address));
  }
  // In case the address was never scanned (or never interacted with any NFT contract)
  if (!currentData) {
    currentData = default_api_structure();
  }
  if (lastBlock == undefined) {
    lastBlock = currentData.lastBlock;
  }
  let new_nfts = await getNewDatabaseInfo(network, address, lastBlock);
  if (new_nfts.size) {
    let nfts: Set<string> = new Set(currentData.interacted_nfts);
    new_nfts.forEach((nft) => nfts.add(nft));

    currentData.interacted_nfts = [...nfts];
    console.log('Added new nfts !');
  } else {
    console.log('No new nfts');
  }
  console.log('Checking property!');

  currentData.owned_nfts = await parseNFTSet(
    network, 
    currentData.interacted_nfts,
    address
  );
  currentData.lastBlock = blockHeight;
  await db.put(to_key(network,address), currentData);

  return currentData;
}

function to_key(network: string, address:string){
  return `${address}@${network}`;
}
function validate(network: string, res: any): boolean{
  if(chains[network] == undefined){
    res.status(404).send(
      {status:"Network not found"}
    );
    return false
  }else{
    return true
  }

}

async function main() {
  // Create IPFS instance
  const ipfsOptions = { repo: './ipfs' };
  const ipfs = await IPFS.create(ipfsOptions);

  // Create OrbitDB instance
  const orbitdb = await OrbitDB.createInstance(ipfs);

  // Create database instance
  const db = await orbitdb.keyvalue('wallet-nfts');
  console.log('Created database at', db.address);

  app.get('/nfts', (req: any, res: any) => {
    res.status(200).send('Syntax : echo here the syntax you need');
  });

  app.get('/nfts/query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    if(validate(network, res)){
      let currentData = await db.get(to_key(network,address));
      if (!currentData) {
        currentData = default_api_structure();
      }
      res.status(200).send(currentData);
    }
  });

  app.get('/nfts/update-query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    if(validate(network,res)){
      let currentData = await db.get(to_key(network,address));
      res.status(200).send(await updateAddress(db, network, address, currentData));
    }
  });

  app.get('/nfts/force-update/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    if(validate(network, res)){
      res.status(200).send(await updateAddress(db, network, address, default_api_structure(), 0));
    }
  });
}
main();

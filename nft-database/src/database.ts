const IPFS = require('ipfs');
const OrbitDB = require('orbit-db');
import { getBlockHeight, getNewDatabaseInfo } from './index.js';
const express = require('express');

interface NFTsInteracted {
  lastBlock: number;
  nfts: string[];
}

const app = express();

app.listen(8080, () => {
  console.log("Serveur à l'écoute");
});

function default_api_structure() {
  return {
    lastBlock: 0,
    nfts: []
  };
}

async function updateAddress(
  db: any,
  address: string,
  currentData: any = undefined
) {
  let blockHeight = await getBlockHeight();

  if (!currentData) {
    let currentData: NFTsInteracted = await db.get(address);
  }
  // In case the address was never scanned (or never interacted with any NFT contract)
  if (!currentData) {
    currentData = default_api_structure();
  }

  let new_nfts = await getNewDatabaseInfo(address, currentData.lastBlock);
  if (new_nfts.size) {
    let nfts = new Set(currentData.nfts);
    new_nfts.forEach((nft) => nfts.add(nft));

    currentData.nfts = [...nfts];
    currentData.lastBlock = blockHeight;
    await db.put(address, currentData);

    console.log('Added new nfts !');
  } else {
    console.log('No new nfts, perfect');
  }
  return currentData;
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

  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  //await updateAddress(db,address);

  // Update if necessary
  //console.log(await updateAndGetCurrentNFTs(db,address), await db.get(address));

  app.get('/nfts', (req: any, res: any) => {
    res.status(200).send('Syntax : echo here the syntax you need');
  });

  app.get('/nfts/query/:address', async (req: any, res: any) => {
    let currentData = await db.get(address);
    if (!currentData) {
      currentData = default_api_structure();
    }
    res.status(200).send(currentData);
  });

  app.get('/nfts/update-query/:address', async (req: any, res: any) => {
    let currentData = await db.get(address);
    if (currentData) {
      res.status(200).send(currentData);
    } else {
      console.log('No New nfts');
      res.status(200).send(await updateAddress(db, address, currentData));
    }
  });
}
main();

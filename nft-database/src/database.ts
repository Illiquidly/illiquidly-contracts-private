const IPFS = require('ipfs');
const OrbitDB = require('orbit-db');
import { getBlockHeight, getNewDatabaseInfo, parseNFTSet } from './index.js';
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

function default_api_structure() {
  return {
    lastBlock: 0,
    nfts: [],
    owned_nfts: {}
  };
}

async function updateAddress(
  db: any,
  address: string,
  currentData: any = undefined,
  lastBlock: number | undefined = undefined
) {
  let blockHeight = await getBlockHeight();

  if (!currentData) {
    let currentData: NFTsInteracted = await db.get(address);
  }
  // In case the address was never scanned (or never interacted with any NFT contract)
  if (!currentData) {
    currentData = default_api_structure();
  }
  if (lastBlock == undefined) {
    lastBlock = currentData.lastBlock;
  }
  console.log(lastBlock);
  let new_nfts = await getNewDatabaseInfo(address, lastBlock);
  if (new_nfts.size) {
    let nfts: Set<string> = new Set(currentData.nfts);
    new_nfts.forEach((nft) => nfts.add(nft));

    currentData.interacted_nfts = [...nfts];
    console.log('Added new nfts !');
  } else {
    console.log('No new nfts');
  }
  console.log('Checking property!');

  currentData.owned_nfts = await parseNFTSet(
    currentData.interacted_nfts,
    address
  );
  currentData.lastBlock = blockHeight;
  await db.put(address, currentData);

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

  app.get('/nfts', (req: any, res: any) => {
    res.status(200).send('Syntax : echo here the syntax you need');
  });

  app.get('/nfts/query/:address', async (req: any, res: any) => {
    const address = req.params.address;
    let currentData = await db.get(address);
    if (!currentData) {
      currentData = default_api_structure();
    }
    res.status(200).send(currentData);
  });

  app.get('/nfts/update-query/:address', async (req: any, res: any) => {
    const address = req.params.address;
    let currentData = await db.get(address);
    res.status(200).send(await updateAddress(db, address, currentData));
  });

  app.get('/nfts/force-update/:address', async (req: any, res: any) => {
    const address = req.params.address;
    res.status(200).send(await updateAddress(db, address, {}, 0));
  });
}
main();

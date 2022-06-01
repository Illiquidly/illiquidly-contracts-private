'use strict';

import {
  updateInteractedNfts,
  parseNFTSet,
  chains,
  TxInterval
} from './index.js';
import express from 'express';
import 'dotenv/config';
import https from 'https';
import fs from 'fs';
import toobusy from 'toobusy-js';
import Redis from 'ioredis';
import Redlock from 'redlock';
import axios from 'axios';


type Nullable<T> = T | null;


const UPDATE_INTERVAL = 60_000;
const IDLE_UPDATE_INTERVAL = 20_000;
const PORT = 8080;
const QUERY_TIMEOUT = 50_000;

enum NFTState {
  Full,
  Partial,
  isUpdating
}

interface TxQueried {
  external: TxInterval;
  internal: TxInterval;
}
interface NFTsInteracted {
  interacted_nfts: Set<string>;
  owned_nfts: any;
  state: NFTState;
  txs: TxQueried;
  last_update_start_time: number;
}

interface SerializableNFTsInteracted {
  interacted_nfts: string[];
  owned_nfts: any;
  state: NFTState;
  txs: TxQueried;
  last_update_start_time: number;
}

async function initDB() {
  // We start the db
  return new Redis();
}

async function initMutex(db: Redis) {
  const redlock = new Redlock(
    // You should have one client for each independent redis node
    // or cluster.
    [db],
    {
      // The expected clock drift; for more details see:
      // http://redis.io/topics/distlock
      driftFactor: 0.01, // multiplied by lock ttl to determine drift time

      // The max number of times Redlock will attempt to lock a resource
      // before erroring.
      retryCount: 1,

      // the time in ms between attempts
      retryDelay: 200, // time in ms

      // the max time in ms randomly added to retries
      // to improve performance under high contention
      // see https://www.awsarchitectureblog.com/2015/03/backoff.html
      retryJitter: 200, // time in ms

      // The minimum remaining time on a lock before an extension is automatically
      // attempted with the `using` API.
      automaticExtensionThreshold: 500 // time in ms
    }
  );
  return redlock;
}

function fillEmpty(currentData: Nullable<NFTsInteracted>): NFTsInteracted {
  if (!currentData || Object.keys(currentData).length === 0) {
    return default_api_structure();
  } else {
    return currentData;
  }
}

async function acquireUpdateLock(lock: any, key: string) {
  return await lock.acquire([key + 'updateLock'], UPDATE_INTERVAL);
}

async function releaseUpdateLock(lock: any) {
  await lock.release();
}

async function lastUpdateStartTime(db: any, key: string): Promise<number> {
  let updateTime = await db.get(key + '_updateStartTime');
  return parseInt(updateTime);
}

async function setLastUpdateStartTime(db: any, key: string, time: number) {
  await db.set(key + '_updateStartTime', time);
}

function serialise(currentData: NFTsInteracted): SerializableNFTsInteracted {
  const serialised: any = { ...currentData };
  if (serialised.interacted_nfts) {
    serialised.interacted_nfts = Array.from(serialised.interacted_nfts);
  }
  return serialised;
}

function deserialise(
  serialisedData: SerializableNFTsInteracted
): NFTsInteracted | null {
  if (serialisedData) {
    const currentData: any = { ...serialisedData };
    if (currentData.interacted_nfts) {
      currentData.interacted_nfts = new Set(currentData.interacted_nfts);
    }
    return currentData;
  } else {
    return serialisedData;
  }
}

function saveToDb(db: any, key: string, currentData: NFTsInteracted) {
  const serialisedData = serialise(currentData);
  return db.set(key, JSON.stringify(serialisedData));
}

async function getDb(db: any, key: string): Promise<NFTsInteracted> {
  const serialisedData = await db.get(key);
  const currentData = deserialise(JSON.parse(serialisedData));
  return fillEmpty(currentData);
}

function default_api_structure(): NFTsInteracted {
  return {
    interacted_nfts: new Set(),
    owned_nfts: {},
    state: NFTState.Full,
    txs: {
      external: {
        oldest: null,
        newest: null
      },
      internal: {
        oldest: null,
        newest: null
      }
    },
    last_update_start_time: 0
  };
}

async function updateOwnedNfts(
  network: string,
  address: string,
  newNfts: Set<string>,
  currentData: NFTsInteracted
) {
  const ownedNfts: any = await parseNFTSet(network, newNfts, address);
  Object.keys(ownedNfts).forEach((nft) => {
    currentData.owned_nfts[nft] = ownedNfts[nft];
  });

  return currentData;
}

async function updateOwnedAndSave(
  network: string,
  address: string,
  new_nfts: Set<string>,
  currentData: NFTsInteracted,
  new_txs: TxInterval
) {
  if (new_nfts.size) {
    const nfts: Set<string> = new Set(currentData.interacted_nfts);

    // For new nft interactions, we update the owned nfts
    console.log('Querying NFT data from LCD');

    new_nfts.forEach((nft) => nfts.add(nft));
    currentData.interacted_nfts = nfts;
    currentData = await updateOwnedNfts(network, address, new_nfts, {
      ...currentData
    });
  }

  // If there is an interval, we init the interval data
  if (
    new_txs.oldest &&
    currentData.txs.external.newest &&
    new_txs.oldest > currentData.txs.external.newest
  ) {
    currentData.txs.internal.newest = new_txs.oldest;
    currentData.txs.internal.oldest = currentData.txs.external.newest;  
  }

  // We fill the internal hole first
  if(
    currentData.txs.internal.newest && 
    currentData.txs.internal.oldest && 
    new_txs.newest &&
    new_txs.oldest &&
    currentData.txs.internal.newest > new_txs.oldest && new_txs.newest >= currentData.txs.internal.oldest){
    currentData.txs.internal.newest = new_txs.oldest;
  } 

  if (
    currentData.txs.external.newest == null ||
    (new_txs.newest && new_txs.newest > currentData.txs.external.newest)
  ) {
    currentData.txs.external.newest = new_txs.newest;
  }
  if (
    currentData.txs.external.oldest == null ||
    (new_txs.oldest && new_txs.oldest < currentData.txs.external.oldest)
  ) {
    currentData.txs.external.oldest = new_txs.oldest;
  }
  return currentData;
}

async function updateAddress(
  db: any,
  network: Nullable<string>,
  address: Nullable<string>,
  currentData: Nullable<NFTsInteracted>,
  hasTimedOut: any
) {
  console.log("Let's udate");

  currentData = fillEmpty(currentData);
  if (!network || !address) {
    return currentData;
  }
  const willQueryBefore = currentData.state != NFTState.Full;
  // We update currentData to prevent multiple updates
  currentData.state = NFTState.isUpdating;
  currentData.last_update_start_time = Date.now();
  await saveToDb(db, to_key(network, address), currentData);

  const queryCallback = async (newNfts: Set<string>, txSeen: TxInterval) => {
    if (!network || !address || !currentData) {
      return;
    }
    currentData = await updateOwnedAndSave(
      network,
      address,
      newNfts,
      { ...currentData },
      txSeen
    );
    currentData.state = NFTState.isUpdating;
    await saveToDb(db, to_key(network, address), currentData);
  };

  // We start by querying data in the possible interval
  if (
    currentData.txs.internal.newest != null &&
    currentData.txs.internal.oldest != null &&
    currentData.txs.internal.oldest < currentData.txs.internal.newest
  ) {
    //Here we can query interval transactions
    await updateInteractedNfts(
      network,
      address,
      currentData.txs.internal.newest,
      currentData.txs.internal.oldest,
      queryCallback,
      hasTimedOut
    );
  }

  // Then we query new transactions

  await updateInteractedNfts(
    network,
    address,
    null,
    currentData.txs.external.newest,
    queryCallback,
    hasTimedOut
  );
  // We then query old data if not finalized
  if (willQueryBefore) {
    await updateInteractedNfts(
      network,
      address,
      currentData.txs.external.oldest,
      null,
      queryCallback,
      hasTimedOut
    );
  }

  if(hasTimedOut.timeout){
    currentData.state = NFTState.Partial
  }else{
    currentData.state = NFTState.Full;
  }

  return currentData;
}

function to_key(network: string, address: string) {
  return `${address}@${network}`;
}

function validate(network: string, res: any): boolean {
  if (chains[network] == undefined) {
    res.status(404).send({ status: 'Network not found' });
    return false;
  } else {
    return true;
  }
}

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
  let db = await initDB();
  let redlock = await initMutex(db);

  app.get('/nfts', async (_req: any, res: any) => {
    await res.status(404).send('You got the wrong syntax, sorry mate');
  });
  //db.flushdb();

  // Query a list of known NFTS
  app.get('/nfts/query/:network', async (req: any, res: any) =>{
    const network = req.params.network;
    if(validate(network, res)){
      let official_list: any = await axios
        .get(`https://assets.terra.money/cw721/contracts.json`);
      let local_list: any = require('../nft_list.json');
      let nft_list = {...official_list.data[network], ...local_list[network]};
      await res.status(200).send(nft_list);
    }
  })


  // Simple query, just query the current state
  app.get('/nfts/query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    if (validate(network, res)) {
      let currentData: NFTsInteracted = await getDb(
        db,
        to_key(network, address)
      );

      const action = req.query.action;

      // In general, we simply return the current database state

      // If we want to update, we do it in the background
      if (action == 'update' || action == 'force_update') {
        let isLocked = false;
        let lock = await acquireUpdateLock(
          redlock,
          to_key(network, address)
        ).catch((_error) => {
          isLocked = true;
        });
        if (
          currentData &&
          (isLocked ||
            Date.now() <
              (await lastUpdateStartTime(db, to_key(network, address))) +
                IDLE_UPDATE_INTERVAL)
        ) {
          if (!isLocked) {
            await releaseUpdateLock(lock)
            .catch((error) => console.log("Lock couldn't be released : ", error));
          }
          await res.status(200).send(serialise(currentData));
          return;
        }
        // Force update restarts everything from scratch
        if (action == 'force_update') {
          currentData = default_api_structure();
        }
        const returnData = { ...currentData };
        returnData.state = NFTState.isUpdating;
        await res.status(200).send(serialise(returnData));

        await setLastUpdateStartTime(db, to_key(network, address), Date.now());
        let hasTimedOut = { timeout: false };
        let timeout = 
          setTimeout(async () => {
              hasTimedOut.timeout = true;
              console.log("has timeout");
            }, QUERY_TIMEOUT);

        currentData = await updateAddress(db, network, address, { ...currentData }, hasTimedOut);
        clearTimeout(timeout);
        await saveToDb(db, to_key(network, address), currentData);
        await releaseUpdateLock(lock);
        console.log("Released lock");
      } else {
        await res.status(200).send(serialise(currentData));
      }
    }
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

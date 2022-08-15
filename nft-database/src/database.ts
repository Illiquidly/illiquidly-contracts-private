'use strict';

import {
  updateInteractedNfts,
  parseNFTSet,
} from './index.js';

import {
  updateInteractedCW20s,
  parseCW20Set,
} from "./CW20-querier.js";


import {
  chains 
} from "./utils/blockchain/chains.js";
import express from 'express';
import 'dotenv/config';
import https from 'https';
import fs from 'fs';
import toobusy from 'toobusy-js';
import Redis from 'ioredis';
import Redlock from 'redlock';
import axios from 'axios';


type Nullable<T> = T | null;

// We defined some time constants for the api queries
if(process.env.UPDATE_DESPITE_LOCK_TIME == undefined){
  process.env.UPDATE_DESPITE_LOCK_TIME = "120000";
}
const UPDATE_DESPITE_LOCK_TIME = parseInt(process.env.UPDATE_DESPITE_LOCK_TIME);
if(process.env.QUERY_TIMEOUT == undefined){
  process.env.QUERY_TIMEOUT = "100000";
}
const QUERY_TIMEOUT = parseInt(process.env.QUERY_TIMEOUT);
if(process.env.IDLE_UPDATE_INTERVAL == undefined){
  process.env.IDLE_UPDATE_INTERVAL = "20000";
}
const IDLE_UPDATE_INTERVAL = parseInt(process.env.IDLE_UPDATE_INTERVAL);

const PORT = 8080;

enum UpdateState {
  Full,
  Partial,
  isUpdating
}

interface TxInterval {
  oldest: number | null;
  newest: number | null;
}
interface TxQueried {
  external: TxInterval;
  internal: TxInterval;
}

interface ContractsInteracted {
  interactedContracts: Set<string>;
  ownedTokens: any;
  state: UpdateState;
  txs: TxQueried;
}

interface SerializableContractsInteracted {
  interacted_contracts: string[];
  ownedTokens: any;
  state: UpdateState;
  txs: TxQueried;
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

function fillEmpty(currentData: Nullable<ContractsInteracted>): ContractsInteracted {
  if (!currentData || Object.keys(currentData).length === 0) {
    return defaultContractsApiStructure();
  } else {
    return currentData;
  }
}

async function acquireUpdateLock(lock: any, key: string) {
  return await lock.acquire([key + 'updateLock'], UPDATE_DESPITE_LOCK_TIME);
}

async function releaseUpdateLock(lock: any) {
  await lock.release().catch((error:any) => console.log("Couldn't release lock", error));
}

async function lastUpdateStartTime(db: any, key: string): Promise<number> {
  let updateTime = await db.get(key + '_updateStartTime');
  return parseInt(updateTime);
}

async function setLastUpdateStartTime(db: any, key: string, time: number) {
  await db.set(key + '_updateStartTime', time);
}

function serialise(currentData: ContractsInteracted): SerializableContractsInteracted {
  const serialised: any = { ...currentData };
  if (serialised.interactedContracts) {
    serialised.interactedContracts = Array.from(serialised.interactedContracts);
  }
  return serialised;
}

function deserialise(
  serialisedData: SerializableContractsInteracted
): ContractsInteracted | null {
  if (serialisedData) {
    const currentData: any = { ...serialisedData };
    if (currentData.interactedContracts) {
      currentData.interactedContracts = new Set(currentData.interactedContracts);
    }
    return currentData;
  } else {
    return serialisedData;
  }
}

function saveToDb(db: any, key: string, currentData: ContractsInteracted) {
  const serialisedData = serialise(currentData);
  return db.set(key, JSON.stringify(serialisedData));
}

async function getDb(db: any, key: string): Promise<ContractsInteracted> {
  const serialisedData = await db.get(key);
  const currentData = deserialise(JSON.parse(serialisedData));
  return fillEmpty(currentData);
}

function defaultContractsApiStructure(): ContractsInteracted {
  return {
    interactedContracts: new Set(),
    ownedTokens: {},
    state: UpdateState.Full,
    txs: {
      external: {
        oldest: null,
        newest: null
      },
      internal: {
        oldest: null,
        newest: null
      }
    }
  };
}


function updateSeenTransaction(currentData: ContractsInteracted, newTxs: TxInterval){
  // If there is an interval, we init the interval data
  if (
    newTxs.oldest &&
    currentData.txs.external.newest &&
    newTxs.oldest > currentData.txs.external.newest
  ) {
    currentData.txs.internal.newest = newTxs.oldest;
    currentData.txs.internal.oldest = currentData.txs.external.newest;
  }

  // We fill the internal hole first
  if (
    currentData.txs.internal.newest &&
    currentData.txs.internal.oldest &&
    newTxs.newest &&
    newTxs.oldest &&
    currentData.txs.internal.newest > newTxs.oldest &&
    newTxs.newest >= currentData.txs.internal.oldest
  ) {
    currentData.txs.internal.newest = newTxs.oldest;
  }

  if (
    currentData.txs.external.newest == null ||
    (newTxs.newest && newTxs.newest > currentData.txs.external.newest)
  ) {
    currentData.txs.external.newest = newTxs.newest;
  }
  if (
    currentData.txs.external.oldest == null ||
    (newTxs.oldest && newTxs.oldest < currentData.txs.external.oldest)
  ) {
    currentData.txs.external.oldest = newTxs.oldest;
  }
}


async function updateOwnedTokensAndSave(
  network: string,
  address: string,
  newContracts: Set<string>,
  currentData: ContractsInteracted,
  newTxs: TxInterval,
  parseTokenSet: (n:string, c: Set<string>, a: string) => any
) {

  // We start by updating the NFT object
  if (newContracts.size) {
    const contracts: Set<string> = new Set(currentData.interactedContracts);

    // For new nft interactions, we update the owned nfts
    console.log('Querying NFT data from LCD');

    newContracts.forEach((token) => contracts.add(token));
    currentData.interactedContracts = contracts;
    // We query what tokens are actually owned by the address

    const ownedTokens: any = await parseTokenSet(network, newContracts, address);
    Object.keys(ownedTokens).forEach((token) => {
      currentData.ownedTokens[token] = ownedTokens[token];
    });
  }

  // Then we update the transactions we've already seen
  updateSeenTransaction(currentData, newTxs);

  return currentData;
}

async function updateAddress(
  db: any,
  dbKey: string,
  network: Nullable<string>,
  address: Nullable<string>,
  currentData: Nullable<ContractsInteracted>,
  hasTimedOut: any,
  queryNewInteractedContracts: any,
  parseTokenSet: typeof parseNFTSet
) {
  currentData = fillEmpty(currentData);
  if (!network || !address) {
    return currentData;
  }
  const willQueryBefore = currentData.state != UpdateState.Full;
  // We update currentData to prevent multiple updates
  currentData.state = UpdateState.isUpdating;
  await saveToDb(db, dbKey, currentData);

  const queryCallback = async (newContracts: Set<string>, txSeen: TxInterval) => {
    if (!network || !address || !currentData) {
      return;
    }
    currentData = await updateOwnedTokensAndSave(
      network,
      address,
      newContracts,
      { ...currentData },
      txSeen,
      parseTokenSet
    );
    currentData.state = UpdateState.isUpdating;
    await saveToDb(db, dbKey, currentData);
  };

  // We start by querying data in the possible interval (between the latests transactions queried and the oldest ones)
  if (
    currentData.txs.internal.newest != null &&
    currentData.txs.internal.oldest != null &&
    currentData.txs.internal.oldest < currentData.txs.internal.newest
  ) {
    //Here we can query interval transactions
    await queryNewInteractedContracts(
      network,
      address,
      currentData.txs.internal.newest,
      currentData.txs.internal.oldest,
      queryCallback,
      hasTimedOut
    );
  }

  // Then we query new transactions
  await queryNewInteractedContracts(
    network,
    address,
    null,
    currentData.txs.external.newest,
    queryCallback,
    hasTimedOut
  );

  // We then query old data if not finalized
  if (willQueryBefore) {
    await queryNewInteractedContracts(
      network,
      address,
      currentData.txs.external.oldest,
      null,
      queryCallback,
      hasTimedOut
    );
  }

  if (hasTimedOut.timeout) {
    currentData.state = UpdateState.Partial;
  } else {
    currentData.state = UpdateState.Full;
  }

  return currentData;
}

function toTokenKey(network: string, address: string) {
  return `token:${address}@${network}`;
}

function toNFTKey(network: string, address: string) {
  return `nft:${address}@${network}`;
}

const acceptedActions = [undefined, "plain_db", "update","force_update"]

function validateRequest(network: string, action: string | undefined, res: any): boolean {
  if (chains[network] == undefined) {
    res.status(404).send({ status: 'Network not found' });
    return false;
  } else if(!acceptedActions.includes(action)){
    res.status(404).send({ status: `Action not found. Choose one of undefined${acceptedActions}` });
    return false;
  }else{
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


function returnQueriedData(res: any, action: string, returnData: any){
    // If there is no update, we simply return the result
    if (action == 'update') {
      returnData.state = UpdateState.isUpdating;
    } else if (action == 'force_update') {
      returnData = defaultContractsApiStructure();
      returnData.state = UpdateState.isUpdating;
    }
    res.status(200).send(serialise(returnData));
}

async function canUpdate(db: Redis, redlock: Redlock,  dbKey: string){

    
    // First we check that the we don't update too often
    if(Date.now() <
          (await lastUpdateStartTime(db, dbKey)) + IDLE_UPDATE_INTERVAL){
      console.log("Too much requests my girl");
      return;
    }

    // Then we check that we can update the records (and someone is not doing the same thing simultaneously)
    // We do that my using a Redis Redlock. This Redlock lasts at most UPDATE_DESPITE_LOCK_TIME, to not be blocking in case of program crash
    let isLocked = false;
    let lock = await acquireUpdateLock(redlock, dbKey).catch((_error) => {
      console.log("islocked");
      isLocked = true;
    });
    if(isLocked){
      return;
    }

    await setLastUpdateStartTime(db, dbKey, Date.now());
    return lock;
}


async function main() {
  let db = await initDB();
  let redlock = await initMutex(db);

  app.get('/nfts', async (_req: any, res: any) => {
    await res.status(404).send('You got the wrong syntax, sorry mate');
  });
  //db.flushdb();

  // Query a list of known NFTS
  app.get('/nfts/query/:network', async (req: any, res: any) => {
    const network = req.params.network;
    if (!validateRequest(network, undefined, res)) {
      return;
    }
    let officialList: any = await axios.get(
      `https://assets.terra.money/cw721/contracts.json`
    );
    let localList: any = require('../nft_list.json');
    let nftList = { ...officialList.data[network], ...localList[network] };
    await res.status(200).send(nftList);
  });

  // Query the current NFT database state and trigger update if necessary
  app.get('/nfts/query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    const action = req.query.action;

    if (!validateRequest(network,action, res)) {
      return;
    }

    let dbKey = toNFTKey(network, address);
    let currentData: ContractsInteracted = await getDb(db, dbKey);

    // First we send a message back to the user
    returnQueriedData(res, action, {...currentData});

    // If we don't want to update, there is nothing to do anymore
    if (action != 'update' && action != 'force_update') {
      return;
    }

    // Here we want to update the database
    let lock = await canUpdate(db, redlock, dbKey);
    
    if (!lock){
      return;
    }
    

    // We deal with timeouts and shit
    let hasTimedOut = { timeout: false };
    console.log(QUERY_TIMEOUT);
    let timeout = setTimeout(async () => {
      hasTimedOut.timeout = true;
      console.log('has timed-out');
    }, QUERY_TIMEOUT);

    // We launch the actual update code

    // Force update restarts everything from scratch
    if (action == 'force_update') {
      currentData = defaultContractsApiStructure();
    }
    currentData = await updateAddress(
      db,
      dbKey,
      network,
      address,
      { ...currentData },
      hasTimedOut,
      updateInteractedNfts,
      parseNFTSet
    );
    clearTimeout(timeout);

    // We save the updated object to db and release the Lock on the database
    await saveToDb(db, dbKey, currentData);
    await releaseUpdateLock(lock);
    console.log('Released lock');
  });

  // Token part
  // Query the list of known CW20 tokens
  app.get('/tokens/query/:network', async (req: any, res: any) => {
    const network = req.params.network;
    if (validateRequest(network, undefined, res)) {
      let officialList: any = await axios.get(
        `https://assets.terra.money/cw20/tokens.json`
      );
      let localList: any = require('../cw20_list.json');
      let nftList = { ...officialList.data[network], ...localList[network] };
      await res.status(200).send(nftList);
    }
  });

  // Query the current NFT database state and trigger update if necessary
  app.get('/cw20/query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    const action = req.query.action;

    if (!validateRequest(network,action, res)) {
      return;
    }

    let dbKey = toTokenKey(network, address);
    let currentData: ContractsInteracted = await getDb(db, dbKey);

    // First we send a message back to the user
    returnQueriedData(res, action, {...currentData});

    // If we don't want to update, there is nothing to do anymore
    if (action != 'update' && action != 'force_update') {
      return;
    }

    // Here we want to update the database
    let lock = await canUpdate(db, redlock, dbKey);
    if (!lock){
      return;
    }

    // We deal with timeouts and shit
    let hasTimedOut = { timeout: false };
    let timeout = setTimeout(async () => {
      hasTimedOut.timeout = true;
      console.log('has timed-out');
    }, QUERY_TIMEOUT);

    // We launch the actual update code

    // Force update restarts everything from scratch
    if (action == 'force_update') {
      currentData = defaultContractsApiStructure();
    }
    currentData = await updateAddress(
      db,
      dbKey,
      network,
      address,
      { ...currentData },
      hasTimedOut,
      updateInteractedCW20s,
      parseCW20Set
    );
    clearTimeout(timeout);

    // We save the updated object to db and release the Lock on the database
    await saveToDb(db, dbKey, currentData);
    await releaseUpdateLock(lock);
    console.log('Released lock');
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

const IPFS = require('ipfs');
const OrbitDB = require('orbit-db');
import {
  queryAfterNewest,
  queryBeforeOldest,
  parseNFTSet,
  chains,
  TxInterval
} from './index.js';
import express from 'express';
import 'dotenv/config';
import https from "https";
import fs from  "fs";
const toobusy = require('toobusy-js');

type Nullable<T> = T | null

const UPDATE_INTERVAL = 200_000;
const FORCE_END_UPDATE = 120_000;
const IDLE_UPDATE_INTERVAL = 20_000;
const PORT = 8080;
const QUERY_TIMEOUT = 50_000;

const updateLock: any = {};
const lastUpdateStartTime: any = {};
const lastWalletContentUpdate: any = {};

enum NFTState {
  Full,
  Partial,
  isUpdating
}

interface NFTsInteracted {
  interacted_nfts: Set<string>;
  owned_nfts: any;
  state: NFTState;
  queried_transactions: TxInterval;
  last_update_start_time: number;
}

interface SerializableNFTsInteracted {
  interacted_nfts: string[];
  owned_nfts: any;
  state: NFTState;
  queried_transactions: TxInterval;
  last_update_start_time: number;
}

interface Timeouts {
  before: number;
  after: number;
}

const app = express();

app.listen(PORT, () => {
  console.log("Serveur à l'écoute");
});


function serialise(currentData: NFTsInteracted): SerializableNFTsInteracted{
  let serialised: any = {...currentData};
  serialised.interacted_nfts = Array.from(serialised.interacted_nfts);
  return serialised
}

function deserialise(serialisedData: SerializableNFTsInteracted): NFTsInteracted{
  let currentData: any = {...serialisedData};
  currentData.interacted_nfts = Array.from(currentData.interacted_nfts);
  return currentData
}


function saveToDb(db: any, key: string,currentData: NFTsInteracted){
  return db.put(key, serialise(currentData));
}

function getDb(db: any, key: string) : NFTsInteracted{
  let currentData = db.get(key);
  if(!currentData){
    currentData = default_api_structure();
  }
  return deserialise(currentData);
}

function default_api_structure(): NFTsInteracted {
  return {
    interacted_nfts: new Set(),
    owned_nfts: {},
    state: NFTState.Full,
    queried_transactions: {
      oldest: null,
      newest: null
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
  let ownedNfts: any = await parseNFTSet(network, newNfts, address);
  Object.keys(ownedNfts).forEach((nft, tokens) => {
    currentData.owned_nfts[nft] = ownedNfts[nft];
  });

  return currentData;
}

async function updateOwnedAndSave(
  network: string,
  address: string,
  new_nfts: Set<string>,
  currentData: NFTsInteracted,
  new_queried_transactions: TxInterval,
  hasTimedOut: boolean
) {
  if (new_nfts.size) {
    let nfts: Set<string> = new Set(currentData.interacted_nfts);

    // For new nft interactions, we update the owned nfts
    console.log('Querying NFT data from LCD');

    new_nfts.forEach((nft) => nfts.add(nft));
    currentData.interacted_nfts = nfts;
    currentData = await updateOwnedNfts(network, address, new_nfts, {
      ...currentData
    });
  }

  if (
    currentData.queried_transactions.newest == null ||
    (new_queried_transactions.newest &&
      new_queried_transactions.newest > currentData.queried_transactions.newest)
  ) {
    currentData.queried_transactions.newest = new_queried_transactions.newest;
  }
  if (
    currentData.queried_transactions.oldest == null ||
    (new_queried_transactions.oldest &&
      new_queried_transactions.oldest < currentData.queried_transactions.oldest)
  ) {
    currentData.queried_transactions.oldest = new_queried_transactions.oldest;
  }
  if (hasTimedOut) {
    currentData.state = NFTState.Partial;
  } else {
    currentData.state = NFTState.Full;
  }
  return currentData;
}

async function updateAddress(
  db: any,
  network: Nullable<string>,
  address: Nullable<string>,
  currentData: Nullable<NFTsInteracted>,
  timeout: number
) {
  if(!currentData){
    currentData = default_api_structure();
  }
  if(!network || !address){
    return currentData;
  }
  let willQueryBefore = currentData.state != NFTState.Full;
  // We update currentData to prevent multiple updates
  currentData.state = NFTState.isUpdating;
  currentData.last_update_start_time = Date.now();
  await saveToDb(db, to_key(network, address),currentData);

  let queryCallback = async (newNfts: Set<string>, txSeen: TxInterval) => {
    if(!network || !address || ! currentData){
      return;
    }
    currentData = await updateOwnedAndSave(
      network,
      address,
      newNfts,
      { ...currentData },
      txSeen,
      true
    );
    currentData.state = NFTState.isUpdating;
    await saveToDb(db, to_key(network, address),currentData);
  };

  // We start by querying new data
  let [newNfts, seenTx, hasTimedOut] = await queryAfterNewest(
    network,
    address,
    currentData.queried_transactions.newest,
    timeout,
    queryCallback
  );
  currentData = await updateOwnedAndSave(
    network,
    address,
    new Set(),
    { ...currentData },
    seenTx,
    hasTimedOut
  );

  // We then query old data if not finalized
  if (willQueryBefore) {
    currentData.state = NFTState.isUpdating;
    [newNfts, seenTx, hasTimedOut] = await queryBeforeOldest(
      network,
      address,
      currentData.queried_transactions.oldest,
      timeout,
      queryCallback
    );
    currentData = await updateOwnedAndSave(
      network,
      address,
      new Set(),
      { ...currentData },
      seenTx,
      hasTimedOut
    );
  }
  await saveToDb(db, to_key(network, address),currentData);
  let returnData = {...currentData};
  currentData = null;
  network = null;
  address = null;
  return returnData;
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
// Allow any to access this API.
app.use(function (req: any, res: any, next: any) {
  res.header('Access-Control-Allow-Origin', '*');
  res.header(
    'Access-Control-Allow-Headers',
    'Origin, X-Requested-With, Content-Type, Accept'
  );
  next();
});

app.use(function(req, res, next) {
  if (toobusy()) {
    res.status(503).send("I'm busy right now, sorry.");
  } else {
    next();
  }
});


// Handle generic errors thrown by the express application.
function expressErrorHandler(err: any) {
  if (err.code === 'EADDRINUSE')
    console.error(
      `Port ${PORT} is already in use. Is this program already running?`
    );
  else console.error(JSON.stringify(err, null, 2));

  console.error('Express could not start!');
  process.exit(0);
}

async function main() {
  // Create IPFS instance
  const ipfsOptions = {
    repo: './ipfs',
    EXPERIMENTAL: {
      pubsub: true
    }
  };
  const ipfs = await IPFS.create(ipfsOptions);

  // Create OrbitDB instance
  const orbitdb = await OrbitDB.createInstance(ipfs);

  // Create database instance
  const db = await orbitdb.keyvalue('wallet-nfts');
  await db.load();
  console.log('Created database at', db.address);

  app.get('/nfts', async (req: any, res: any) => {
    await res.status(200).send('You got the wrong syntax, sorry mate');
  });

  // Simple query, just query the current state
  app.get('/nfts/query/:network/:address', async (req: any, res: any) => {
    const address = req.params.address;
    const network = req.params.network;
    if (validate(network, res)) {
      let currentData: NFTsInteracted = await getDb(db,to_key(network, address));

      const action = req.query.action;

      // In general, we simply return the current database state

      // If we want to update, we do it in the background
      if (action == 'update' || action == 'force_update') {
        if (
          currentData &&
          ((updateLock[to_key(network, address)] &&
            Date.now() <
              lastUpdateStartTime[to_key(network, address)] +
                UPDATE_INTERVAL) ||
            Date.now() <
              lastUpdateStartTime[to_key(network, address)] +
                IDLE_UPDATE_INTERVAL)
        ) {
          console.log('Wait inbetween updates please');
          await res.status(200).send({ ...currentData });
          return;
        }

        // Force update restarts everything from scratch
        if (action == 'force_update') {
          console.log('resetData');
          currentData = default_api_structure();
        }
        let returnData = { ...currentData };
        returnData.state = NFTState.isUpdating;
        await res.status(200).send({ ...returnData });

        updateLock[to_key(network, address)] = true;
        lastUpdateStartTime[to_key(network, address)] = Date.now();
        await Promise.race([
          new Promise((res) =>
            setTimeout(async () => {
              let currentData = await getDb(db,to_key(network, address));
              currentData.state = NFTState.Partial;
              await saveToDb(db, to_key(network, address),currentData);
            }, FORCE_END_UPDATE)
          ),
          updateAddress(db, network, address, { ...currentData }, QUERY_TIMEOUT)
        ]);
        updateLock[to_key(network, address)] = false;
      } else {
        await res.status(200).send({ ...currentData });
      }
    }
  });

  if(process.env.EXECUTION=="PRODUCTION")
  {
    const options = {
      cert: fs.readFileSync('/home/ubuntu/identity/fullchain.pem'),
      key: fs.readFileSync('/home/ubuntu/identity/privkey.pem')
    };
    https.createServer(options, app).listen(8443);
  }

}
main();

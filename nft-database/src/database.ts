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

const UPDATE_INTERVAL = 80_000;
const IDLE_UPDATE_INTERVAL = 20_000;
const WALLET_CONTENT_UPDATE_INTERVAL = 20_000;
const PORT = 8080;
const QUERY_TIMEOUT = 50_000;

const updateLock: any = {}
const lastUpdateStartTime: any = {}
const lastWalletContentUpdate: any = {}

enum NFTState {
  Full,
  Partial,
  isUpdating
}

interface NFTsInteracted {
  interacted_nfts: string[];
  owned_nfts: any;
  state: NFTState;
  queried_transactions: TxInterval;
  last_update_start_time: number;
  last_wallet_content_update: number;
}

interface Timeouts {
  before: number;
  after: number;
}

const app = express();

app.listen(PORT, () => {
  console.log("Serveur à l'écoute");
});

function default_api_structure(): NFTsInteracted {
  return {
    interacted_nfts: [],
    owned_nfts: {},
    state: NFTState.Full,
    queried_transactions: {
      oldest: null,
      newest: null
    },
    last_update_start_time: 0,
    last_wallet_content_update: 0
  };
}

async function updateOwnedNfts(
  network: string,
  address: string,
  currentData: NFTsInteracted
) {
  let ownedNfts = await parseNFTSet(
    network,
    currentData.interacted_nfts,
    address
  );
  if (Object.entries(ownedNfts).length !== 0) {
    console.log('not empty');
    currentData.owned_nfts = ownedNfts;
    currentData.last_wallet_content_update = Date.now();
  }
  return currentData;
}

async function saveNewData(
  network: string,
  address: string,
  new_nfts: Set<string>,
  currentData: NFTsInteracted,
  new_queried_transactions: TxInterval,
  hasTimedOut: boolean
) {
  if (new_nfts.size) {
    let nfts: Set<string> = new Set(currentData.interacted_nfts);
    new_nfts.forEach((nft) => nfts.add(nft));

    currentData.interacted_nfts = [...nfts];
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
  network: string,
  address: string,
  currentData: NFTsInteracted | undefined = undefined,
  timeout: number
) {
  if (!currentData) {
    let currentData: NFTsInteracted = await db.get(to_key(network, address));
    // In case the address was never scanned (or never interacted with any NFT contract)
  }
  if (!currentData) {
    currentData = default_api_structure();
  }
  let willQueryBefore = currentData.state != NFTState.Full;
  // We update currentData to prevent multiple updates
  currentData.state = NFTState.isUpdating;
  currentData.last_update_start_time = Date.now();
  await db.put(to_key(network, address), currentData);

  let queryCallback = async (newNfts: Set<string>, txSeen: TxInterval) => {
    if (!currentData) {
      currentData = default_api_structure();
    }
    currentData = await saveNewData(
      network,
      address,
      newNfts,
      {...currentData},
      txSeen,
      true
    );
    currentData.state = NFTState.isUpdating;
    await db.put(to_key(network, address), currentData);
  };

  // We start by querying new data
  let [new_nfts, seenTx, hasTimedOut] = await queryAfterNewest(
    network,
    address,
    currentData.queried_transactions.newest,
    timeout,
    queryCallback
  );
  currentData = await saveNewData(
    network,
    address,
    new_nfts,
    { ...currentData },
    seenTx,
    hasTimedOut
  );

  // We then query old data if not finalized
  if (willQueryBefore) {
    currentData.state = NFTState.isUpdating;
    [new_nfts, seenTx, hasTimedOut] = await queryBeforeOldest(
      network,
      address,
      currentData.queried_transactions.oldest,
      timeout,
      queryCallback
    );
    currentData = await saveNewData(
      network,
      address,
      new_nfts,
      {...currentData},
      seenTx,
      hasTimedOut
    );
  }
  await db.put(to_key(network, address), currentData);

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
// Allow any to access this API.
app.use(function (req: any, res: any, next: any) {
  res.header('Access-Control-Allow-Origin', '*');
  res.header(
    'Access-Control-Allow-Headers',
    'Origin, X-Requested-With, Content-Type, Accept'
  );
  next();
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
      let currentData = await db.get(to_key(network, address));
      if (!currentData) {
        currentData = default_api_structure();
      }
      const action = req.query.action;

      // In case you only want adresses (without updating)
      if (action == 'plain_db') {
        await res.status(200).send(currentData);
        return;
      }

      // In general, we query the NFTs the address actually owns
      if (
        currentData &&
        (!lastWalletContentUpdate[to_key(network,address)] ||
          Date.now() >
            lastWalletContentUpdate[to_key(network,address)] +
              WALLET_CONTENT_UPDATE_INTERVAL)
      ) {
        console.log('Querying NFT data from LCD');
        lastWalletContentUpdate[to_key(network,address)] = Date.now();
        currentData = await updateOwnedNfts(network, address, {...currentData});
        await db.put(to_key(network, address), currentData);
      }
      await res.status(200).send({...currentData});

      // If we want to update, we do it in the background
      // Force update restarts everything from scratch
      if (action == 'update' || action == 'force_update') {
        if (
          currentData &&
          ((updateLock[to_key(network,address)] &&
            Date.now() <
              lastUpdateStartTime[to_key(network,address)] + UPDATE_INTERVAL) ||
            Date.now() <
              lastUpdateStartTime[to_key(network,address)] + IDLE_UPDATE_INTERVAL) 
        ) {
          console.log('Wait inbetween updates please');
          return;
        }
        if (action == 'force_update') {
          console.log("resetData");
          currentData = default_api_structure();
        }
        updateLock[to_key(network,address)] = true;
        lastUpdateStartTime[to_key(network,address)] = Date.now();
        currentData = await updateAddress(
          db,
          network,
          address,
          {...currentData},
          QUERY_TIMEOUT
        );
        updateLock[to_key(network,address)] = false;
      }
    }
  });
}
main();

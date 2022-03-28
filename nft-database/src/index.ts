import { LCDClient, MnemonicKey, Wallet } from '@terra-money/terra.js';
import axios from 'axios';
import pLimit from 'p-limit';
const limitNFT = pLimit(10);
const limitToken = pLimit(15);

export interface TxInterval {
  oldest: number | null;
  newest: number | null;
}

export const chains: any = {
  testnet: {
    URL: 'https://bombay.stakesystems.io',
    chainID: 'bombay-12'
  },
  mainnet: {
    URL: 'https://lcd.terra.dev',
    chainID: 'columbus-5'
  }
};

export let fcds: any = {
  testnet: 'https://bombay-fcd.terra.dev',
  mainnet: 'https://fcd.terra.dev'
};

function asyncAction(promise: any) {
  return Promise.resolve(promise)
    .then((data) => [null, data])
    .catch((error) => [error]);
}

interface NFTInfo {
  address: string;
  token_id: any[];
}

function addFromWasmEvents(tx: any, nftsInteracted: any) {
  if (!tx.raw_log || !tx.raw_log.includes('token_id')) {
    return;
  }
  if (tx.logs) {
    for (let log of tx.logs) {
      for (let event of log.events) {
        if (event.type == 'wasm') {
          let hasNftTransfered = false;
          let contract;
          // We check the tx transfered an NFT
          for (let attribute of event.attributes) {
            if (
              attribute.value == 'transfer_nft' ||
              attribute.value == 'mint'
            ) {
              hasNftTransfered = true;
            }
            if (attribute.key == 'contract_address') {
              contract = attribute.value;
            }
          }
          if (hasNftTransfered) {
            nftsInteracted.add(contract);
          }
        }
      }
    }
  }
}

function addFromMsg(tx: any, nftsInteracted: any) {
  for (let msg of tx.tx.value.msg) {
    if (msg.type == 'wasm/MsgExecuteContract') {
      let execute_msg = msg.value.execute_msg;
      if (
        (execute_msg.transfer_nft || execute_msg.mint) &&
        !tx.raw_log.includes('failed')
      ) {
        nftsInteracted.add(msg.value.contract);
      }
    }
  }
}

function getNftsFromTxList(tx_data: any): [Set<string>, number, number] {
  var nftsInteracted: Set<string> = new Set();
  let lastTxIdSeen = 0;
  let newestTxIdSeen = 0;
  for (let tx of tx_data.data.txs) {
    // We add NFTS interacted with
    addFromWasmEvents(tx, nftsInteracted);
    addFromMsg(tx, nftsInteracted);

    // We update the block and id info
    if (lastTxIdSeen === 0 || tx.id < lastTxIdSeen) {
      lastTxIdSeen = tx.id;
    }
    if (tx.id > newestTxIdSeen) {
      newestTxIdSeen = tx.id;
    }
  }
  return [nftsInteracted, lastTxIdSeen, newestTxIdSeen];
}

export async function queryAfterNewest(
  network: string,
  address: string,
  newestTxId: number | null,
  timeout: number,
  callback: any
) {
  return updateInteractedNfts(
    network,
    address,
    newestTxId,
    null,
    timeout,
    callback
  );
}

export async function queryBeforeOldest(
  network: string,
  address: string,
  last_id: number | null,
  timeout: number,
  callback: any
) {
  return updateInteractedNfts(
    network,
    address,
    null,
    last_id,
    timeout,
    callback
  );
}

async function updateInteractedNfts(
  network: string,
  address: string,
  newestTxIdSaved: number | null,
  lastTxIdSaved: number | null,
  timeout: number,
  callback: any
): Promise<[Set<string>, TxInterval, boolean]> {
  const terra = new LCDClient(chains[network]);

  let nftsInteracted: Set<string> = new Set();
  let query_next: boolean = true;
  let networkError = false;
  let limit = 100;
  let offset;
  if (lastTxIdSaved) {
    offset = lastTxIdSaved;
  } else {
    offset = 0;
  }
  timeout += Date.now();
  let newestTxIdSeen: number | null = null;
  let lastTxIdSeen: number | null = null;
  while (query_next && Date.now() < timeout) {
    // When timeout, stop querying
    console.log('New fcd query', offset);
    const source = axios.CancelToken.source();
    const axiosTimeout = setTimeout(() => {
      source.cancel();
    }, timeout - Date.now());
    let tx_data = await axios
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`,
        { cancelToken: source.token }
      )
      .catch((error: any) => {
        if (error.response != undefined && error.response.status == 500) {
          // No more results
        } else {
          networkError = true;
          console.log(error);
        }
        return null;
      })
      .then((response: any) => {
        clearTimeout(axiosTimeout);
        return response;
      });
    console.log('New fcd query done');
    if (tx_data == null) {
      query_next = false;
    } else {
      // We query the NFTs from the transaction result and messages
      let [new_nfts, lastTxId, newestTxId] = getNftsFromTxList(tx_data);
      offset = lastTxId;
      if (newestTxIdSeen == null || newestTxId > newestTxIdSeen) {
        newestTxIdSeen = newestTxId;
      }
      if (lastTxIdSeen == null || lastTxId < lastTxIdSeen) {
        lastTxIdSeen = lastTxId;
      }
      // Stopping tests
      if (newestTxIdSaved != null && newestTxIdSaved > lastTxIdSeen) {
        query_next = false;
      }

      new_nfts.forEach((nft) => nftsInteracted.add(nft));
      await callback(nftsInteracted, {
        newest: newestTxIdSeen,
        oldest: lastTxIdSeen
      });
    }
  }
  let hasTimedOut = Date.now() >= timeout || networkError;

  return [
    nftsInteracted,
    { newest: newestTxIdSeen, oldest: lastTxIdSeen },
    hasTimedOut
  ];
}


async function getOneTokenBatchFromNFT(lcdClient: LCDClient, address: string, nft: string, start_after: string | undefined = undefined){
  return lcdClient.wasm
  .contractQuery(nft, {
    tokens: { 
      owner: address, 
      start_after: start_after 
    }
  })
  .catch((error) => {
    if (error && error.request && error.request.response) {
      console.log(error!.request!.response!.data);
    } else {
      console.log(error);
    }
  })
  .then((tokenId: any) => {
    if (tokenId) {
      return Promise.all(
        tokenId['tokens'].map((id: string) => getOneTokenInfo(lcdClient, nft, id))
      ).catch(() => 
        tokenId['tokens'].map((token_id: any) => ({
            token_id: token_id,
            nft_info: {}
          }
        ))
      );
    }
  })
}


async function parseTokensFromOneNft(lcdClient: LCDClient, address: string, nft: string){
  let tokens: any;
  let start_after: string | undefined = undefined;
  let allTokens: any[] = [];

  do{
    tokens = await getOneTokenBatchFromNFT(lcdClient, address, nft, start_after);
    if(tokens && tokens.length > 0){
      start_after = tokens[tokens.length - 1].tokenId;
      allTokens = allTokens.concat(tokens);
    }
  }
  while(tokens && tokens.length > 0);

  return {
    [nft]: {
      contract: nft,
      tokens: allTokens
    }
  };
}  



async function getOneTokenInfo(lcdClient: LCDClient, nft: string, id: string){
  return limitToken(() => {
    return lcdClient.wasm
      .contractQuery(nft, {
        nft_info: { token_id: id }
      })
      .then((nftInfo: any) => {
        return {
          tokenId: id,
          nftInfo: nftInfo
        };
      });
  });
}


// We limit the request concurrency to 10 elements
export async function parseNFTSet(
  network: string,
  nfts: Set<string> | string[],
  address: string
) {
  const lcdClient = new LCDClient(chains[network]);

  let promiseArray = Array.from(nfts).map(async (nft) => {
    return limitNFT(() => parseTokensFromOneNft(lcdClient, address, nft));
  });

  return await Promise.all(promiseArray).then((response: any) => {
    let owned_nfts = {};
    response.forEach((response: any) => {
      owned_nfts = { ...owned_nfts, ...response };
    });
    return owned_nfts;
  });
}

async function main() {
  let mainnet = 'mainnet';
  let testnet = 'testnet';
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  let testnet_address = 'terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8';
  let [response, seenTx, hasTimedOut] = await updateInteractedNfts(
    testnet,
    testnet_address,
    null,
    null,
    10_000,
    undefined
  );

  let owned_nfts = await parseNFTSet(testnet, response, testnet_address);
  console.log(response, seenTx, hasTimedOut);
  console.log(owned_nfts);
}

//main();

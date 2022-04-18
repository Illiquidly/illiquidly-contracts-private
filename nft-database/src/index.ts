import { LCDClient, TxLog } from '@terra-money/terra.js';
import axios from 'axios';
import pLimit from 'p-limit';
const limitNFT = pLimit(10);
const limitToken = pLimit(50);
const AXIOS_TIMEOUT = 10_000;

export interface TxInterval {
  oldest: number | null;
  newest: number | null;
}

export const chains: any = {
  testnet: {
    URL: "https://bombay-lcd.terra.dev/",
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

function addFromWasmEvents(tx: any, nftsInteracted: any) {
  if (tx.logs) {
    for (let log of tx.logs) {
      let parsedLog = new TxLog(log.msg_index, log.log, log.events);
      let from_contract = parsedLog.eventsByType.from_contract;
      //console.log(from_contract)
      if (from_contract) {
        if (from_contract.action) {
          if (
            from_contract.action.includes('transfer_nft') ||
            from_contract.action.includes('mint')
          ) {
            from_contract.contract_address.forEach(
              nftsInteracted.add,
              nftsInteracted
            );
          }
        }
      }
    }
  }
  return nftsInteracted;
}

function addFromMsg(tx: any, nftsInteracted: any) {
  for (let msg of tx.tx.value.msg) {
    if (msg.type == 'wasm/MsgExecuteContract') {
      let execute_msg = msg.value.execute_msg;
      if (execute_msg.transfer_nft || execute_msg.mint) {
        nftsInteracted.add(msg.value.contract);
      }
    }
  }
  return nftsInteracted;
}

function getNftsFromTxList(tx_data: any): [Set<string>, number, number] {
  var nftsInteracted: Set<string> = new Set();
  let lastTxIdSeen = 0;
  let newestTxIdSeen = 0;
  for (let tx of tx_data.data.txs) {
    // We add NFTS interacted with
    nftsInteracted = addFromWasmEvents(tx, nftsInteracted);
    nftsInteracted = addFromMsg(tx, nftsInteracted);

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

export async function updateInteractedNfts(
  network: string,
  address: string,
  start: number | null,
  stop: number | null,
  callback: any,
  hasTimedOut: any = {timeout: false},
){
  let nftsInteracted: Set<string> = new Set();
  let query_next: boolean = true;
  let limit = 100;
  let offset;
  if (start) {
    offset = start;
  } else {
    offset = 0;
  }
  console.log(start, stop);
  let newestTxIdSeen: number | null = null;
  let lastTxIdSeen: number | null = null;
  while (query_next) {
    if(hasTimedOut.timeout){
      return;
    }
    const source = axios.CancelToken.source();
    const axiosTimeout = setTimeout(() => {
      source.cancel();
    }, AXIOS_TIMEOUT);
    let tx_data = await axios
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`,
        { cancelToken: source.token }
      )
      .catch((_error: any) => {
        return null;
      })
      .then((response: any) => {
        clearTimeout(axiosTimeout);
        return response;
      });
    console.log('New fcd query done', offset);
    if (tx_data == null) {
      query_next = false;
    } else {
      // We query the NFTs from the transaction result and messages
      let [newNfts, lastTxId, newestTxId] = getNftsFromTxList(tx_data);
      if(lastTxId != 0){
        offset = lastTxId;
      }else{
        query_next = false;
      }
      if (newestTxIdSeen == null || newestTxId > newestTxIdSeen) {
        newestTxIdSeen = newestTxId;
      }
      if (lastTxIdSeen == null || lastTxId < lastTxIdSeen) {
        lastTxIdSeen = lastTxId;
      }
      // Stopping tests
      if (stop != null && stop > lastTxIdSeen) {
        query_next = false;
      }
      if (newNfts) {
        newNfts.forEach((nft) => nftsInteracted.add(nft));
        if (callback) {
          await callback(newNfts, {
            newest: newestTxIdSeen,
            oldest: lastTxIdSeen
          });
        }
      }
    }
  }

  return;
}

async function getOneTokenBatchFromNFT(
  lcdClient: LCDClient,
  address: string,
  nft: string,
  start_after: string | undefined = undefined
) {
  return lcdClient.wasm
    .contractQuery(nft, {
      tokens: {
        owner: address,
        start_after: start_after
      }
    })
    .then((tokenId: any) => {
      if (tokenId) {
        return Promise.all(
          tokenId['tokens'].map((id: string) =>
            limitToken(() => getOneTokenInfo(lcdClient, nft, id))
          )
        ).catch(() =>
          tokenId['tokens'].map((token_id: any) => ({
            tokenId: token_id,
            nftInfo: {}
          }))
        );
      }
    })
    .catch((_error) => {
      //console.log(error);
    });
}

async function parseTokensFromOneNft(
  lcdClient: LCDClient,
  address: string,
  nft: string
) {
  let tokens: any;
  let start_after: string | undefined = undefined;
  let allTokens: any = {};

  do {
    tokens = await getOneTokenBatchFromNFT(
      lcdClient,
      address,
      nft,
      start_after
    );

    if (tokens && tokens.length > 0) {
      start_after = tokens[tokens.length - 1].tokenId;
      let tokenExport = Object.assign(
        {},
        ...tokens.map((token: any) => ({ [token.tokenId]: token }))
      );
      allTokens = { ...allTokens, ...tokenExport };
    }
  } while (tokens && tokens.length > 0);

  if (Object.keys(allTokens).length === 0) {
    return {
      [nft]: {
        contract: nft,
        tokens: {}
      }
    };
  } else {
    return {
      [nft]: {
        contract: nft,
        tokens: allTokens
      }
    };
  }
}

async function getOneTokenInfo(lcdClient: LCDClient, nft: string, id: string) {
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
      if (response) {
        owned_nfts = { ...owned_nfts, ...response };
      }
    });
    return owned_nfts;
  });
}

async function main() {
  let mainnet = 'mainnet';
  let testnet = 'testnet';
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  let testnet_address = 'terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp';
  testnet_address = 'terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp';

  let owned_nfts = await parseNFTSet(
    testnet,
    new Set(['terra1q30g8fvancxm4v5te07r2zprh2mqpuy3a0k8mj']),
    testnet_address
  );
  console.log(owned_nfts);

  let test = await updateInteractedNfts(
    testnet,
    testnet_address,
    null,
    null,
    null
  );
  console.log(test);

  owned_nfts = await parseNFTSet(
    testnet,
    new Set(['terra1q30g8fvancxm4v5te07r2zprh2mqpuy3a0k8mj']),
    testnet_address
  );
  console.log(owned_nfts);
}

//main()

import { LCDClient, MnemonicKey, Wallet, TxLog } from '@terra-money/terra.js';
import axios from 'axios';
import pLimit from 'p-limit';
const limitNFT = pLimit(10);
const limitToken = pLimit(50);

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

function getTokenIdFromTxList(tx_data: any): [Set<string>, number] {
  let lastTxIdSeen = 0;

  let tokenIds: Set<string> = new Set();
  for (let tx of tx_data.data.txs) {
    if (lastTxIdSeen === 0 || tx.id < lastTxIdSeen) {
      lastTxIdSeen = tx.id;
    }
    if (tx.logs) {
      for (let log of tx.logs) {
        let parsedLog = new TxLog(log.msg_index, log.log, log.events);
        let from_contract = parsedLog.eventsByType.from_contract;
        if (from_contract.token_id) {
          from_contract.token_id.forEach(tokenIds.add, tokenIds);
        }
      }
    }
  }
  return [tokenIds, lastTxIdSeen];
}

async function getAllTokenFromFCD(network: string, address: string) {
  let limit = 100;
  let offset = 0;
  let tokenIds: Set<string> = new Set();
  let query_next: boolean = true;
  while (query_next) {
    // When timeout, stop querying
    console.log('New fcd query', offset);
    let tx_data = await axios
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`
      )
      .catch((error: any) => {
        console.log(error);
      });
    console.log('New fcd query done');
    if (tx_data == null) {
      query_next = false;
    } else {
      // We query the NFTs from the transaction result and messages
      let [newTokenIds, lastTxId] = getTokenIdFromTxList(tx_data);
      offset = lastTxId;
      if (newTokenIds) {
        console.log(newTokenIds);
        newTokenIds.forEach((token: any) => tokenIds.add(token));
      }
    }
  }
}

async function getAllTokenFromLCD(network: string, address: string) {
  const lcdClient = new LCDClient(chains[network]);
  //We assume here the contract implemented the enumerable Standard, so that we can query the different token ids
  let start_after = undefined;
  let tokens = undefined;
  while (!tokens || tokens.length) {
    let msg: any = {
      all_tokens: {
        start_after: start_after
      }
    };
    console.log(address, msg);
    tokens = await lcdClient.wasm
      .contractQuery(address, msg)
      .then((tokens: any) => {
        return tokens.tokens;
      })
      .catch((error: any) => {
        console.log(error);
        return { tokens: [] };
      });
    if (tokens.length) {
      start_after = tokens[0];
      let test: any = await lcdClient.wasm.contractQuery(address, {
        owner_of: {
          token_id: start_after
        }
      });
      start_after = test.owner + start_after;
      console.log(test);
    }
    console.log(tokens, start_after);
  }
}

interface NFTInfo {
  address: string;
  token_id: any[];
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
    .catch((error) => {
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
    return undefined;
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

async function randomEarthCollections() {
  let page = 1;
  let collections = await axios
    .get(`https://api.luart.io/columbus-5/volume`)
    .catch((error: any) => {
      console.log(error);
    });
  console.log(collections);
}

async function main() {
  let mainnet = 'mainnet';
  let testnet = 'testnet';
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  let testnet_address = 'terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp';
  let nft = 'terra1q30g8fvancxm4v5te07r2zprh2mqpuy3a0k8mj';
  //nft = "terra103z9cnqm8psy0nyxqtugg6m7xnwvlkqdzm4s4k"

  //let owned_nfts = await getAllTokenFromFCD(mainnet, "terra1uv9w7aaq6lu2kn0asnvknlcgg2xd5ts57ss7qt");
  //console.log(owned_nfts);
  randomEarthCollections();
}

main();

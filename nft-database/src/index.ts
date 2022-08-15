import { LCDClient, TxLog } from '@terra-money/terra.js';
import axios from 'axios';
import pLimit from 'p-limit';
const fs = require("fs");
var cloudscraper = require('cloudscraper');

var _ = require('lodash');

const limitNFT = pLimit(10);
const limitToken = pLimit(50);
const AXIOS_TIMEOUT = 10_000;

import {
  chains, fcds, registered_nft_contracts
} from "./utils/blockchain/chains.js";


const local_nft_list = require("../nft_list.json");

async function registeredNFTs(network: string): Promise<string[]>{
  let nft_list_to_return: string[] = [];
  
  if(local_nft_list[network]){
        nft_list_to_return = Object.keys(local_nft_list[network])
  }
  
  let nft_list = await axios
      .get(registered_nft_contracts);
  if(nft_list?.data[network]){
    return [...nft_list_to_return, ...Object.keys(nft_list.data[network])]
  }else{
    return nft_list_to_return
  }
}

function addFromWasmEvents(tx: any, nftsInteracted: any, chain_type: string) {


  if (tx.logs) {
    for (let log of tx.logs) {
      if(chain_type == "classic"){
        let parsedLog = new TxLog(log.msg_index, log.log, log.events);
        let from_contract = parsedLog.eventsByType.from_contract;
        if (from_contract) {
          if (from_contract.action) {
            if(from_contract.contract_address.includes("terra1ycp3azjymqckrdlzpp88zfyk6x09m658c2c63d")){
              console.log("ha")
            }
            if (
              from_contract.action.includes('transfer_nft') ||
              from_contract.action.includes('send_nft') ||
              from_contract.action.includes('mint')
            ) {
              from_contract.contract_address.forEach(
                nftsInteracted.add,
                nftsInteracted
              );
            }
          }
        }
      }else{
        let parsedLog = new TxLog(log.msg_index, log.log, log.events);
        let from_contract = parsedLog.eventsByType.wasm;
        if (from_contract) {
            if (from_contract.action) {
            if (
              from_contract.action.includes('transfer_nft') ||
              from_contract.action.includes('send_nft') ||
              from_contract.action.includes('mint')
            ) {
              from_contract._contract_address.forEach(
                nftsInteracted.add,
                nftsInteracted
              );
            }
          }
        }
      }
    }
  }
  return nftsInteracted;
}

function getNftsFromTxList(tx_data: any, chain_type: string): [Set<string>, number, number] {
  var nftsInteracted: Set<string> = new Set();
  let lastTxIdSeen = 0;
  let newestTxIdSeen = 0;
  // In case we are using cloudscraper to get rid of cloudflare
  if(tx_data.data == undefined){
    tx_data = {
      data: JSON.parse(tx_data)
    }
  }
  for (let tx of tx_data.data.txs) {
    // We add NFTS interacted with
    nftsInteracted = addFromWasmEvents(tx, nftsInteracted, chain_type);

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
  hasTimedOut: any = { timeout: false }
) {
  let nftsInteracted: Set<string> = new Set();
  let query_next: boolean = true;
  let limit = 100;
  let offset;
  if (start) {
    offset = start;
  } else {
    offset = 0;
  }
  let newestTxIdSeen: number | null = null;
  let lastTxIdSeen: number | null = null;
  while (query_next) {
    if (hasTimedOut.timeout) {
      return;
    }
    const source = axios.CancelToken.source();
    const axiosTimeout = setTimeout(() => {
      source.cancel();
    }, AXIOS_TIMEOUT);
    let tx_data = await cloudscraper
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`,
        { cancelToken: source.token }
      )
      .catch((_error: any) => {
        console.log(_error);
        return null;
      })
      .then((response: any) => {
        clearTimeout(axiosTimeout);
        return response;
      });
    if (tx_data == null) {
      query_next = false;
    } else {
      // We query the NFTs from the transaction result and messages
      let [newNfts, lastTxId, newestTxId] = getNftsFromTxList(tx_data, network);

      // If it's the first time we query the fcd, we need to add recognized NFTs (because some minted NFTs don't get recognized in mint)
      if(start == null && stop == null){
        let registered = await registeredNFTs(network)
        registered.forEach((nft) => newNfts.add(nft));
      }

      if (lastTxId != 0) {
        offset = lastTxId;
      } else {
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
        start_after: start_after,
      	limit: 100,
      }
    })
    .then((tokenId: any) => {
      if (tokenId && tokenId.tokens) {
        return Promise.all(
          tokenId.tokens.map((id: string) =>
            limitToken(() => getOneTokenInfo(lcdClient, nft, id))
          )
        ).catch(() =>
          tokenId.tokens.map((token_id: any) => ({
            tokenId: token_id,
            nftInfo: {}
          }))
        );
      }
    })
    .catch((_error) => {
        //console.log(_error);
    });
}

async function parseTokensFromOneNft(
  lcdClient: LCDClient,
  address: string,
  nft: string
) {
  let tokens: any;
  let start_after: string | undefined = undefined;
  let last_tokens: any;
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
    if (_.isEqual(last_tokens, tokens) && tokens) {
      // If we have the same response twice, we stop, it's not right
      tokens = undefined;
    }
    last_tokens = tokens;
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
  let lcdClient: LCDClient;
  lcdClient = new LCDClient(chains[network]);

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

export async function main() {
  let testnet = 'testnet';
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

import { LCDClient, TxLog } from '@terra-money/terra.js';
import axios from 'axios';
import pLimit from 'p-limit';
var cloudscraper = require('cloudscraper');

const limitCW20 = pLimit(40);
const AXIOS_TIMEOUT = 10_000;

export interface TxInterval {
  oldest: number | null;
  newest: number | null;
}

import {
  chains, fcds
} from "./utils/blockchain/chains.js";

function addFromWasmEvents(tx: any, CW20Interacted: any) {
  if (tx.logs) {
    for (let log of tx.logs) {
      let parsedLog = new TxLog(log.msg_index, log.log, log.events);
      let from_contract = parsedLog.eventsByType.from_contract;
      if (from_contract) {
        if (from_contract.action) {
          if (
            from_contract.action.includes('transfer') ||
            from_contract.action.includes('transfer_from') ||
            from_contract.action.includes('send') ||
            from_contract.action.includes('send_from') ||
            from_contract.action.includes('mint') ||
            from_contract.action.includes('burn')
          ) {
            from_contract.contract_address.forEach(
              CW20Interacted.add,
              CW20Interacted
            );
          }
        }
      }
    }
  }
  return CW20Interacted;
}

function getCW20sFromTxList(tx_data: any): [Set<string>, number, number] {
  var CW20Interacted: Set<string> = new Set();
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
    CW20Interacted = addFromWasmEvents(tx, CW20Interacted);

    // We update the block and id info
    if (lastTxIdSeen === 0 || tx.id < lastTxIdSeen) {
      lastTxIdSeen = tx.id;
    }
    if (tx.id > newestTxIdSeen) {
      newestTxIdSeen = tx.id;
    }
  }
  return [CW20Interacted, lastTxIdSeen, newestTxIdSeen];
}

export async function updateInteractedCW20s(
  network: string,
  address: string,
  start: number | null,
  stop: number | null,
  callback: any,
  hasTimedOut: any = { timeout: false }
) {
  let CW20sInteracted: Set<string> = new Set();
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
    let tx_data: any = await cloudscraper
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
      let [newCW20s, lastTxId, newestTxId] = getCW20sFromTxList(tx_data);
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
      if (newCW20s) {
        newCW20s.forEach((token) => CW20sInteracted.add(token));
        if (callback) {
          await callback(newCW20s, {
            newest: newestTxIdSeen,
            oldest: lastTxIdSeen
          });
        }
      }
    }
  }

  return;
}

async function getOneCW20Balance(
  lcdClient: LCDClient,
  address: string,
  token: string
) {
  return await lcdClient.wasm
    .contractQuery(token, {
      balance: {
        address
      }
    })
    .catch((_error) => {
      //console.log(error.response.data);
    });
}

async function getOneCW20Info(lcdClient: LCDClient, token: string) {
  return await lcdClient.wasm
    .contractQuery(token, {
      token_info: {}
    })
    .catch((_error) => {
      //console.log(error?.response?.data);
    });
}

// We limit the request concurrency to 10 elements
export async function parseCW20Set(
  network: string,
  tokens: Set<string> | string[],
  address: string
) {
  const lcdClient = new LCDClient(chains[network]);

  let promiseArray = Array.from(tokens).map(async (token) => {
    return Promise.all([
      token,
      limitCW20(() => getOneCW20Balance(lcdClient, address, token)),
      limitCW20(() => getOneCW20Info(lcdClient, token))
    ]);
  });

  return await Promise.all(promiseArray).then((response: any) => {
    let owned_CW20s = {};
    response.forEach((response: any) => {
      if (response[1]) {
        let [token, balance, token_info] = response;
        owned_CW20s = {
          ...owned_CW20s,
          ...{
            [token]: {
              ...token_info,
              ...balance
            }
          }
        };
      }
    });
    return owned_CW20s;
  });
}

export async function main() {
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';

  const callback = async (CW20: Set<string>, _txs: any) => {
    let CW20Balance = await parseCW20Set('classic', CW20, address);
    console.log(CW20Balance);
  };
  await updateInteractedCW20s('classic', address, null, null, callback);
}

//main();

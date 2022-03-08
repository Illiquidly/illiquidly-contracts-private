import { Query, createWrapperProxy } from './terra_utils';
import { env } from './env_helper';
import { Wallet } from '@terra-money/terra.js';
const { LCDClient, MnemonicKey } = require('@terra-money/terra.js');
const axios = require('axios');
let fcdUrl = 'https://fcd.terra.dev';

interface NFTInfo {
  address: string;
  token_id: any[];
}

function addFromWasmEvents(tx: any, nftsInteracted: any) {
  for (let log of tx.logs) {
    for (let event of log.events) {
      if (event.type == 'wasm') {
        let nft_transfered = false;
        let contract;
        // We check the tx transfered an NFT
        for (let attribute of event.attributes) {
          if (attribute.value == 'transfer_nft' || attribute.value == 'mint') {
            nft_transfered = true;
          }
          if (attribute.key == 'contract_address') {
            contract = attribute.value;
          }
        }
        if (nft_transfered) {
          nftsInteracted.add(contract);
        }
      }
    }
  }
}

function addFromMsg(tx: any, nftsInteracted: any, min_block_height: number) {
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

function getNftsFromTxList(
  tx_data: any,
  min_block_height: number = 0
): [Set<string>, number, number] {
  var nftsInteracted: Set<string> = new Set();
  let min_block_height_seen: number = min_block_height;
  let last_tx_id_seen = 0;
  for (let tx of tx_data.data.txs) {
    if (tx.height > min_block_height) {
      // We add NFTS interacted with
      addFromWasmEvents(tx, nftsInteracted);
      addFromMsg(tx, nftsInteracted, min_block_height);
    }

    // We update the block and id info
    if (min_block_height_seen == 0 || tx.height < min_block_height_seen) {
      min_block_height_seen = tx.height;
    }
    if (last_tx_id_seen == 0 || tx.id < last_tx_id_seen) {
      last_tx_id_seen = tx.id;
    }
  }
  return [nftsInteracted, last_tx_id_seen, min_block_height_seen];
}

async function getNewInteractedNfts(
  address: string,
  last_block_height: number | undefined = undefined
) {
  const terra = new LCDClient(env['chain']);

  let nftsInteracted: Set<string> = new Set();
  let behind_last_block_height: boolean = true;
  let limit = 100;
  let offset: number | Set<unknown> = 0;
  while (behind_last_block_height) {
    console.log('New fcd query');
    let tx_data = await axios
      .get(
        `${fcdUrl}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`
      )
      .catch((error: any) => {
        if (error.response.status == 500) {
          // No more results
        } else {
          console.log(error);
        }
        return null;
      });
    if (tx_data == null) {
      behind_last_block_height = false;
    } else {
      let [new_nfts, last_tx_id_seen, min_tx_height_seen] = getNftsFromTxList(
        tx_data,
        last_block_height
      );
      if (last_block_height && min_tx_height_seen <= last_block_height) {
        behind_last_block_height = false;
      }
      offset = last_tx_id_seen;
      new_nfts.forEach((nft) => nftsInteracted.add(nft));
    }
  }
  return nftsInteracted;
}

export async function getBlockHeight() {
  const terra = new LCDClient(env['chain']);
  return await terra.tendermint
    .blockInfo()
    .then((response: any) => response.block.header.height);
}

export async function getNewDatabaseInfo(
  address: string,
  blockHeight: number | undefined = undefined
) {
  const terra = new LCDClient(env['chain']);
  return await getNewInteractedNfts(address, blockHeight);
}

export async function parseNFTSet(
  nfts: Set<string> | string[],
  address: string
) {
  let promiseArray: any[] = [];
  for (let nft of nfts) {
    let contract = createWrapperProxy(
      new Query(new LCDClient(env['chain']), undefined, nft)
    );
    promiseArray.push(
      contract
        .tokens({
          owner: address
        })
        .then((token_id: any) => {
          return {
            [nft]: token_id.tokens
          };
        })
        .catch(() => console.log('Error for', nft))
    );
  }
  return await Promise.all(promiseArray).then((response: any) => {
    response = response.filter(function (x: any) {
      return x !== undefined;
    });
    let owned_nfts = {};
    response.forEach((response: any) => {
      owned_nfts = { ...owned_nfts, ...response };
    });
    return owned_nfts;
  });
}

async function main() {
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  let response = await getNewDatabaseInfo(address);
  let owned_nfts = await parseNFTSet(response, address);
  console.log(owned_nfts);
}

//main()

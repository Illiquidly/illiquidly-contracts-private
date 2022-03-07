import { Address } from './terra_utils';
import { env } from './env_helper';
const { LCDClient, MnemonicKey } = require('@terra-money/terra.js');
const axios = require('axios');
let fcdUrl = 'https://fcd.terra.dev';

function getNftsFromTxList(
  tx_data: any,
  min_block_height: number = 0
): [Set<string>, number, number] {
  var nftsInteracted: Set<string> = new Set();
  let min_block_height_seen: number = min_block_height;
  let last_tx_id_seen = 0;
  for (let tx of tx_data.data.txs) {
    if (min_block_height_seen == 0 || tx.height < min_block_height_seen) {
      min_block_height_seen = tx.height;
    }
    if (last_tx_id_seen == 0 || tx.id < last_tx_id_seen) {
      last_tx_id_seen = tx.id;
    }
    if (tx.height > min_block_height) {
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

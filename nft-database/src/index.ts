import { Query, createWrapperProxy } from './terra_utils';
import { Wallet } from '@terra-money/terra.js';
const { LCDClient, MnemonicKey } = require('@terra-money/terra.js');
const axios = require('axios');

export const chains: any = {
  "testnet": {
            "URL": "https://bombay.stakesystems.io",
            "chainID": "bombay-12"
        },
  "mainnet": {
            "URL": "https://lcd.terra.dev",
            "chainID": "columbus-5"
        },
}

export let fcds: any = {
  "testnet":"https://bombay-fcd.terra.dev",
  "mainnet":'https://fcd.terra.dev',
}


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
  network: string, 
  address: string,
  last_block_height: number | undefined = undefined
) {
  const terra = new LCDClient(chains[network]);

  let nftsInteracted: Set<string> = new Set();
  let behind_last_block_height: boolean = true;
  let limit = 100;
  let offset: number | Set<unknown> = 0;
  while (behind_last_block_height) {
    console.log('New fcd query');
    let tx_data = await axios
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`
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

export async function getBlockHeight(network: string) {
  const terra = new LCDClient(chains[network]);
  return await terra.tendermint
    .blockInfo()
    .then((response: any) => response.block.header.height);
}

export async function getNewDatabaseInfo(
  network: string, 
  address: string,
  blockHeight: number | undefined = undefined
) {
  return await getNewInteractedNfts(network, address, blockHeight);
}

export async function parseNFTSet(
  network: string,
  nfts: Set<string> | string[],
  address: string
) {
  let promiseArray: any[] = [];
  for (let nft of nfts) {
    let contract = createWrapperProxy(
      new Query(new LCDClient(chains[network]), undefined, nft)
    );
    promiseArray.push(
      contract
        .tokens({
          owner: address
        })
        .catch(() => {
            
        })
        .then((token_id: any) => {
          if(token_id)
          {
            // We try to fetch the token_id info
            return Promise.all(  
              token_id["tokens"].map((token_id: any)=>{
                return contract.nft_info({token_id:token_id})
                  .then((nft_info:any) => {
                    return {
                      token_id: token_id,
                      nft_info: nft_info
                    }
                  })
              })
            )
            .catch(() => {
              return token_id["tokens"].map((token_id: any)=>{
                return {
                  token_id:token_id,
                  nft_info:{}
                }
              })
            })
          }
        })
        .then((response: any) =>{
          if(response != undefined){
            return {
              [nft]:{
                contract: nft,
                tokens: response
              }
            }
          }
        })
    );
  }
  return await Promise.all(promiseArray).then((response: any) => {
    let owned_nfts = {};
    response.forEach((response: any) => {
      owned_nfts = { ...owned_nfts, ...response };
    });
    return owned_nfts;
  });
}

async function main() {
  let net = "mainnet";
  let address = 'terra1pa9tyjtxv0qd5pgqyu6ugtedds0d42wt5rxk4w';
  let response = await getNewDatabaseInfo(net,address);
  let owned_nfts = await parseNFTSet(net, response, address);
  
}

main()

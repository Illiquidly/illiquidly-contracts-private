import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { TxLog } from "@terra-money/terra.js"
var cloudscraper = require('cloudscraper');
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

import {
  chains, fcds, registered_nft_contracts
} from "../utils/blockchain/chains";

interface NFT{
  nft:{
    contract_addr: string,
    token_id: string
  }
}

interface NativeToken{
  native_token:{
    denom: string,
  }
}

interface NFTPrice{
  contract_addr: string,
  price: number,
  unit: string
}

function isNFT(obj: any): obj is NFT {
    return typeof obj?.nft?.contract_addr === "string" && typeof obj?.nft?.token_id === "string";
}
function isNativeToken(obj: any): obj is NFT {
    return typeof obj?.native_token?.denom === "string";
}

function addDays(date: Date, days: number) {
  var result = new Date(date);
  result.setDate(result.getDate() + days);
  return result;
}



// Add NFT Price from wasm events
function addFromRandomEarthWasmEvents(tx: any, stop_date: Date = new Date(0)) : [NFTPrice[], Date]{
  // We get the transaction date and save the last seen date
  let last_date_encountered: Date = new Date();
  let date = new Date(tx.timestamp);
  last_date_encountered = last_date_encountered < date? last_date_encountered: date;
  let nft_prices: NFTPrice[] = [];
  // We treat the transaction result
  if (tx.logs && date >= stop_date) {
    for (let log of tx.logs) {
      let parsedLog = new TxLog(log.msg_index, log.log, log.events);
      let from_contract = parsedLog.eventsByType.from_contract;
      if (from_contract) {
        if (from_contract.action) {
          if (
            from_contract.action.includes('execute_orders') 
          ) {
            from_contract.order.forEach((order_text:string)=>{
              let order = JSON.parse(atob(order_text))
              let maker_asset = order.order.maker_asset;
              let taker_asset = order.order.taker_asset;
              let nft, fund;
              if(isNFT(maker_asset.info)){
                if(isNativeToken(taker_asset.info)){
                  nft = maker_asset;
                  fund = taker_asset;
                }else{
                  throw "Transaction not in the acceptable type"
                }
              }else{
                if(isNativeToken(maker_asset.info)){
                  if(isNFT(taker_asset.info)){
                    nft = taker_asset;
                    fund = maker_asset;
                  }else{
                    throw "Transaction not in the acceptable type"
                  }
                }
              }
              console.log({
                contract_addr: nft.info.nft.contract_addr,
                price: fund.amount,
                unit: fund.info.native_token.denom
              })
              nft_prices.push({
                contract_addr: nft.info.nft.contract_addr,
                price: fund.amount,
                unit: fund.info.native_token.denom
              })
            })            
          }
        }
      }
    }
  }
  return [nft_prices, last_date_encountered]
}
/// Here we want to upload the p2p contract and add the fee contract
async function main(){


  let randomEarthLedger = "terra1eek0ymmhyzja60830xhzm7k7jkrk99a60q2z2t";
  let network = "classic"
  // We first query the ledger transactions to fetch the price

  let offset = 0;
  let limit= 100;/*
  let txResponse = await cloudscraper
      .get(
        `${fcds[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${randomEarthLedger}`
      )
      */
  let txResponse = require("../../src/oracle/response.json")
  offset = txResponse.next;
  for(let tx of txResponse.txs){
    //addFromRandomEarthWasmEvents(JSON.parse(tx));
    let nft_price = addFromRandomEarthWasmEvents(tx);
    console.log(nft_price)
  }
 
} 

let nft_address = "terra18eezxhys9jwku67cm4w84xhnzt4xjj77w2qt62";
main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });

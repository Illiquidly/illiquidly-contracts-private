import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';
import {asyncAction} from "../utils/js/asyncAction"
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let mainnetHandler = new Address(env['mnemonics'][0], "mainnet");

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[2]]);
  let gpMainnetContract = mainnetHandler.getContract("terra16ds898j530kn4nnlc7xlj6hcxzqpcxxk4mj8gkcl3vswksu6s3zszs8kp2")

  let mario = "terra12ywvf22d3etfgh5qtguk35zwc7ayfzr2uq2fn0";
  let mint_to_address = mario;
  console.log(nft.address)
  console.log(handler.getAddress())
  // We start by querying some tokenIds from the contract

  let tokenIds = await queryTokens(gpMainnetContract, nft, 90)
 
  let mintMsgs: MsgExecuteContract[] = await Promise.all(tokenIds.map(async (tokenId: string) =>  
      createMintMsg(gpMainnetContract, handler.getAddress(),  tokenId, mint_to_address, nft.address))
  )

  let response = await handler.post(mintMsgs);
  console.log(response)
}


async function queryTokens(gpMainnetContract: any, nft: any, nb: number = 10): Promise<string[]>{

  let startAfter = undefined;
  let limit = 100;
  let allTokenIds: Set<string> = new Set();

  do{
    // First we get some tokens
    let tokens: any = (await gpMainnetContract.query.all_tokens({
      start_after: startAfter,
      limit
    }))?.tokens;

    // Then we get the next parameters
    if(tokens?.length){
      startAfter = tokens[tokens.length - 1]
    }else{
      startAfter = undefined
    }

    // We verify the tokens don't exist on the original contract
    let tokenFilter = await Promise.all(
        tokens.map(async (tokenId: string) => {
          let [err, nftInfo] = await asyncAction(nft.query.nft_info({
            token_id: tokenId
          }))
          console.log(err, nftInfo)
          if(err){
            return true;
          }else{
            return false
          }

        }
      )
    );

    console.log(tokenFilter)

    tokens.filter((_v: any, index: number) => tokenFilter[index]).forEach((item: string) => allTokenIds.add(item))
  }while(startAfter && allTokenIds.size < nb)

  return Array.from(allTokenIds).slice(0, nb)
}


async function createMintMsg(gpMainnetContract: any, user: string, tokenId: string, owner: string, contract: string): Promise<MsgExecuteContract>{

  // We start by querying the on-chain metadata

  let nftInfo = await gpMainnetContract.query.nft_info({
    token_id: tokenId
  })


   let msg = {
      mint:{
        owner: owner,
        token_id: tokenId,
        token_uri: nftInfo.token_uri,
        extension: nftInfo.extension
      }
    };
  return new MsgExecuteContract(
      user, // sender
      contract, // contract address
      { ...msg }, // handle msg,
    );
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });

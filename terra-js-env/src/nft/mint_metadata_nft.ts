import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[1]]);

  let mario = "terra12ywvf22d3etfgh5qtguk35zwc7ayfzr2uq2fn0";
  let mint_to_address = handler.getAddress();

  let mintMsgs: MsgExecuteContract[] = []

  for(var i=0;i<100;i++){
    mintMsgs.push(createMintMsg(handler.getAddress(),  new Date().toString() + Math.floor(Math.random() * 434876823), mint_to_address, nft.address))
  }
  
  let response = await handler.post(mintMsgs)
  console.log(response);

}


function createMintMsg(user: string, tokenId: string, owner: string, contract: string): MsgExecuteContract{
   let msg = {
      mint:{
        owner: owner,
        token_id: tokenId,
        extension: {
          image:"Same image for eveybody",
          image_data:"Wait this is not binary right ?"
        }
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

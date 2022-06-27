import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);

  /*
  let all_handlers: Address[] = env['mnemonics'].map(
    (mnemonic: string) => new Address(mnemonic)
  );
  let token_id = "terra1kj6vwwvsw7vy7x35mazqfxyln2gk5xy00r87qy1763";
  // Mint one new nft to a specific address
  let response = await nft.execute.mint({
    token_id,
    owner: handler.getAddress(),
    token_uri: 'testing'
  });
  console.log(response);

  */

  let to_mint = ['matio_the_best83583065'];

  let mint_to_address = handler.getAddress();

  let mintMsgs: MsgExecuteContract[] = []
  /*  
  let prefix = "mario_the_best";
  for(var i=0;i<7;i++){
    mintMsgs.push(createMintMsg(handler.getAddress(),  prefix + Math.floor(Math.random() * 434876823), mint_to_address, nft.address))
  }
  */

  to_mint.forEach((token: string)=>{
    mintMsgs.push(createMintMsg(handler.getAddress(), token, mint_to_address, nft.address))
  })
  let response = await handler.post(mintMsgs)
  console.log(response);

}


function createMintMsg(user: string, tokenId: string, owner: string, contract: string): MsgExecuteContract{
   let msg = {
      mint:{
        owner: owner,
        token_id: tokenId
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

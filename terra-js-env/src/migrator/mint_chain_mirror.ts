import { Address } from '../terra_utils';
import { env, globalEnv} from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';


/// Here we want to upload the p2p contract and add the fee contract
async function duplicateMint() {
  // Getting a handler for the current address
  let handler1 = new Address(globalEnv.classic['mnemonics'][0], "classic");
  let handler2 = new Address(env['mnemonics'][0]);

  let nft1_tokens = globalEnv.classic['cw721'];
  let nft1_token_names = Object.keys(nft1_tokens);

  let nft2_tokens = env['cw721'];
  let nft2_token_names = Object.keys(nft2_tokens);

  let nft1Minter = handler1.getContract(nft1_tokens[nft1_token_names[0]]);
  let nft2Minter = handler2.getContract(nft2_tokens[nft2_token_names[0]]);

  let tokens1 = await get_all_tokens(nft1Minter);
  let tokens2 = await get_all_tokens(nft2Minter);

  // We need to mint all tokens from contract 1 to contract 2
  let tokensToMint = [...tokens1].filter((token: string) => !tokens2.has(token))
  // We mint the tokens on chain 2
  console.log("We will mint", tokensToMint);
  let tokenMsgs = tokensToMint.map((token: string)=>{
    let msg = {
      mint:{
        owner: handler2.getAddress(),
        token_id: token
      }
    }
    return new MsgExecuteContract(
      handler2.getAddress(), // sender
      nft2Minter.address, // contract address
      { ...msg }, // handle msg,
    );
  })
  return await handler2.post(tokenMsgs);
}


async function get_all_tokens(contract: any): Promise<Set<string>>{
  let all_tokens: Set<string> = new Set();
  let start_after = undefined;
  let tokens = undefined;
  do{
      tokens = await contract.query.all_tokens({
        start_after: start_after,
        limit: 100
      });
      console.log(tokens,start_after);
      if(tokens?.tokens){
        let length: number = tokens.tokens.length;
        if(length > 0){
          start_after = tokens.tokens[length - 1];
          tokens.tokens.forEach(all_tokens.add, all_tokens);
          console.log(start_after)
        }
      }
    }while(tokens?.tokens?.length > 0)
  return all_tokens;
}


main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });

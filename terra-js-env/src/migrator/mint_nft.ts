import { Address } from '../terra_utils';
import { env, globalEnv } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

async function main() {
  // Getting a handler for the current address


  // Handler for the destination chain
  let handler2 = new Address(env['mnemonics'][0]);
  let cw721_tokens2 = env['cw721'];
  let cw721_token_names2 = Object.keys(cw721_tokens2);
  let nft2 = handler2.getContract(cw721_tokens2[cw721_token_names2[0]]);


  // Hanlders for the classic chain (origin)
  let handler1 = new Address(globalEnv.classic['mnemonics'][0], "classic");
  let cw721_tokens1 = globalEnv.classic['cw721'];
  let cw721_token_names1 = Object.keys(cw721_tokens1);
  let nft1 = handler1.getContract(cw721_tokens1[cw721_token_names1[0]]);

  let mario = "terra12ywvf22d3etfgh5qtguk35zwc7ayfzr2uq2fn0";
  let jack = "terra12wdq8y0d08sh8mg6lhfe0ncqgm4n3skfz62tyd";
  let nicoco = "terra1kj6vwwvsw7vy7x35mazqfxyln2gk5xy00r87qy";

  let mintToAddressTerra1 = nicoco;
  let mintToAddressTerra2 = handler2.getAddress();
  let prefix = "heres_some_test_token";

  let mintMsgs1 = [];
  let mintMsgs2 = [];
  for(var i=0;i<7;i++){
    let tokenId = prefix + Math.floor(Math.random() * 434876823);
    let msg1 = {
      mint:{
        owner: mintToAddressTerra1,
        token_id: tokenId
      }
    };
    mintMsgs1.push(
      new MsgExecuteContract(
        handler1.getAddress(), // sender
        nft1.address, // contract address
        { ...msg1 }, // handle msg,
      )
    );
    let msg2 = {
      mint:{
        owner: mintToAddressTerra2,
        token_id: tokenId
      }
    };
    mintMsgs2.push(
      new MsgExecuteContract(
        handler2.getAddress(), // sender
        nft2.address, // contract address
        { ...msg2 }, // handle msg,
      )
    );
  }
  console.log(nft1.address)
  let response = await handler1.post(mintMsgs1)
  console.log("Terra classic",response);
  console.log(nft1.address)
  response = await handler2.post(mintMsgs2)
  console.log("Destination", response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });

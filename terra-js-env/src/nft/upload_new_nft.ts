import { Address } from '../terra_utils';
import { env, add_uploaded_nft } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

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


async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map(
    (mnemonic: string) => new Address(mnemonic)
  );

  // Uploading the contract code
  /*
  let nft_codeId: string[] = await handler.uploadContract(
    '../artifacts/cw721_base1.0.wasm'
  );
  */
  let nft_codeId: string[];
  if(env.type == "classic"){
    /*
    nft_codeId = await handler.uploadContract(
      '../artifacts/cw721_base0.16.wasm'
    );
    */
    nft_codeId =  ['5790'];
  }else{
    nft_codeId = await handler.uploadContract(
      '../artifacts/cw721_base1.0.wasm'
    );
  }

  let codeName: string = 'NFT' + Math.ceil(Math.random() * 10000);

  // Instantiating the contract
  let NFTInitMsg = {
    name: codeName,
    symbol: 'ILIQ',
    minter: handler.getAddress()
  };
  let nft = await handler.instantiateContract(+nft_codeId[0], NFTInitMsg);
  add_uploaded_nft(codeName, nft.execute.contractAddress);
  const mint_to_address = handler.getAddress();
  let mintMsgs: MsgExecuteContract[] = []

  for(var i=0;i<100;i++){
    mintMsgs.push(createMintMsg(handler.getAddress(),  new Date().toString() + Math.floor(Math.random() * 434876823), mint_to_address, nft.address))
  }
  
  let response = await handler.post(mintMsgs)

  return [codeName, nft.execute.contractAddress];
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });

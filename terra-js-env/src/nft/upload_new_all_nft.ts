import { Address } from '../terra_utils';
import { env, add_uploaded_nft, env_name } from '../env_helper';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map(
    (mnemonic: string) => new Address(mnemonic)
  );

  // Uploading the contract code
  let nft_codeId: string[];
  if(env.type == "classic"){
    /*
    nft_codeId = await handler.uploadContract(
      '../artifacts/cw721_base0.16.wasm'
    );
    */
    nft_codeId =  ['5790'];
  }else if(env_name == "staging"){
    nft_codeId =  ['3489'];
  }else{
    nft_codeId = await handler.uploadContract(
      '../artifacts/cw721_metadata_all.wasm'
    );
  }

  let codeName: string = "Space Toadz"

  // Instantiating the contract
  let NFTInitMsg = {
      "name": codeName,
      "symbol": "TDZ",
      minter: handler.getAddress()
  };
  let nft = await handler.instantiateContract(+nft_codeId[0], NFTInitMsg);
  add_uploaded_nft(codeName, nft.execute.contractAddress);
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });

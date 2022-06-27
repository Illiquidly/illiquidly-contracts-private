import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';
import fs from "fs";

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address

  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress())
  // Uploading the contract code
  
  let escrow_codeId: string[] = ["5811"];
  /*
  escrow_codeId: string[] = await handler.uploadContract(
    '../../terra_nft_escrow/artifacts/nft_escrow_classic.wasm'
  );
  */
  
  // We upload a new migrator contract for the nft addres that doesn't have on in the nft_contract.json file

  let nftFilename = "../nft_migrator/nft_contracts.json"
  let current_migrator = require("../../" + nftFilename)
  let nfts: string[] = Object.keys(current_migrator)
    for (let nft in nfts) {
      let nft_address = nfts[nft]
      let current_nft = current_migrator[nft_address];
      if(current_nft.escrow_contract === undefined){
        console.log("Let's upload a contract for", current_nft.name)
        // Initialize p2p contract
        let escrowInitMsg = {
          name: 'NFTEscrow',
          nft_address: nft_address
        };

        let escrow = await handler.instantiateContract(+escrow_codeId[0], escrowInitMsg);
        console.log(escrow);
        current_nft.escrow_contract = escrow.address;
        current_migrator[nft_address] = current_nft;
        let data = JSON.stringify(current_migrator, undefined, 4);
        fs.writeFileSync(nftFilename, data);
        console.log('Uploaded the escrow contract');

        break;
      }
  }



  /*
  
  */
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });

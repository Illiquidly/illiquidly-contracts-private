import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  console.log(handler.getAddress());

  // Uploading the contract code
  
  let loan_codeId: string[] = await handler.uploadContract(
    '../artifacts/nft_escrow_classic.wasm'
  );
  console.log(handler.getAddress());

  let nfts = env["cw721"];
  let nfts_names = Object.keys(nfts);

  // Initialize p2p contract
  let escrowInitMsg = {
    name: 'NFTEscrow',
    nft_address: nfts[nfts_names[0]]
  };
  console.log(escrowInitMsg);

  let escrow = await handler.instantiateContract(+loan_codeId[0], escrowInitMsg);
  add_contract('escrow', escrow.address);

  console.log('Uploaded the loan contract');
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });

import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  console.log(handler.getAddress());

  // Uploading the contract code
  
  let loan_codeId: string[] = await handler.uploadContract(
    '../artifacts/nft_oracle.wasm'
  );
  console.log(handler.getAddress());

  // Initialize p2p contract
  let oracleInitMsg = {
    name: 'NFTOracle',
    timeout: 8*3600
  };

  let oracle = await handler.instantiateContract(+loan_codeId[0], oracleInitMsg);
  add_contract('nft_oracle', oracle.address);

  console.log('Uploaded the oracle contract');
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });

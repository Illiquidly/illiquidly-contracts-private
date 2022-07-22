import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let codeId: string[] = await handler.uploadContract(
    '../artifacts/randomness_verifier.wasm'
  );

  // Initialize p2p contract
  let initMsg = {
  };
  console.log(initMsg)

  let contract = await handler.instantiateContract(+codeId[0], initMsg);
  add_contract('raffle_verifier', contract.address);

  console.log('Uploaded the raffle verifier contract');
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });

import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {


  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let user = new Address(env['mnemonics'][1]);
  // Uploading the contract code
  let escrow = user.getContract(env.contracts.escrow);


  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);

  let response = await escrow.query.registered_tokens({
    limit: 30
  })
  console.log(response);


} 

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });

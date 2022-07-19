import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {


  // Getting a handler for the current address
  let user = new Address(env['mnemonics'][1]);
  // Getting the contract object
  let oracle = user.getContract(env.contracts.nft_oracle);

  let response = await oracle.query.nft_price({
    contract: oracle.address,
    unit: {
      cw20 :"uluna"  
    }
  })
  console.log(response)
} 

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });

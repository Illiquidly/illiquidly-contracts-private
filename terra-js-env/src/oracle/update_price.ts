import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function updateOraclePrice(nft_address: string, price: String){

  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Getting the contract object
  let oracle = handler.getContract(env.contracts.nft_oracle);

  let response = await oracle.execute.set_nft_price({
    contract: nft_address,
    price,
    unit: {
      coin:"uluna"
    },
    oracle_owner: "terra18eezxhys9jwku67cm4w84xhnzt4xjj77w2qt62"
  });
  console.log(response);  
} 

let nft_address = "terra18eezxhys9jwku67cm4w84xhnzt4xjj77w2qt62";
updateOraclePrice(nft_address,"817")
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
